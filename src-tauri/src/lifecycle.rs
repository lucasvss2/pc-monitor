//! Connectivity and session lifecycle, shared by remote collectors.
use crate::ports::Clock;

pub trait Session: Send {
    /// Latest PrepareForSleep state; None when no signal has arrived.
    fn preparing_for_sleep(&self) -> Option<bool>;
}

#[derive(Clone, Copy, Default)]
pub struct Status {
    pub online: bool,
    pub generation: u64,
    pub ready_at: u64,
}

impl Status {
    /// Remote sources reserve stable positions in the login/recovery sequence.
    pub fn ready_for(&self, source_position: u64) -> u64 {
        self.ready_at
            .saturating_add(source_position.saturating_mul(3_000))
    }
}

pub struct Lifecycle<N, S, R> {
    network: N,
    session: S,
    clock: R,
    status: Status,
    sleeping: bool,
    login_at: u64,
}
impl<N: crate::network::NetworkManager, S: Session, R: Clock> Lifecycle<N, S, R> {
    pub fn new(network: N, session: S, clock: R) -> Self {
        let login_at = clock.now().saturating_add(10_000);
        Self {
            network,
            session,
            clock,
            status: Status::default(),
            sleeping: false,
            login_at,
        }
    }
    pub fn step(&mut self) -> Status {
        let sleeping = self.session.preparing_for_sleep().unwrap_or(self.sleeping);
        let resumed = self.sleeping && !sleeping;
        let online = !sleeping && self.network.connection().is_ok_and(|c| c.connectivity == 4);
        if online && (resumed || !self.status.online) {
            self.status.generation += 1;
            self.status.ready_at = self.clock.now().max(self.login_at);
        }
        self.sleeping = sleeping;
        self.status.online = online;
        self.status
    }
}

#[cfg(target_os = "linux")]
pub struct Logind(std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<bool>>>);
#[cfg(target_os = "linux")]
impl Logind {
    pub fn connect() -> Self {
        let state = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new()));
        let shared = state.clone();
        std::thread::spawn(move || loop {
            let _ = (|| -> zbus::Result<()> {
                let bus = zbus::blocking::Connection::system()?;
                let proxy = zbus::blocking::Proxy::new(
                    &bus,
                    "org.freedesktop.login1",
                    "/org/freedesktop/login1",
                    "org.freedesktop.login1.Manager",
                )?;
                let signals = proxy.receive_signal("PrepareForSleep")?;
                if let Ok(sleeping) = proxy.get_property::<bool>("PreparingForSleep") {
                    shared.lock().unwrap().push_back(sleeping);
                }
                for message in signals {
                    shared.lock().unwrap().push_back(sleep_state(&message)?);
                }
                Ok(())
            })();
            std::thread::sleep(std::time::Duration::from_secs(5));
        });
        Self(state)
    }
}
#[cfg(target_os = "linux")]
fn sleep_state(message: &zbus::Message) -> zbus::Result<bool> {
    let (sleeping,): (bool,) = message.body().deserialize()?;
    Ok(sleeping)
}

#[cfg(target_os = "linux")]
impl Session for Logind {
    fn preparing_for_sleep(&self) -> Option<bool> {
        self.0.lock().unwrap().pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::fakes::FakeClock;
    use std::sync::{Arc, Mutex};
    struct Nm(Arc<Mutex<u32>>);
    impl crate::network::NetworkManager for Nm {
        fn connection(&self) -> std::io::Result<crate::network::Connection> {
            Ok(crate::network::Connection {
                connectivity: *self.0.lock().unwrap(),
                ..Default::default()
            })
        }
    }
    struct Login(Arc<Mutex<bool>>);
    impl Session for Login {
        fn preparing_for_sleep(&self) -> Option<bool> {
            Some(*self.0.lock().unwrap())
        }
    }
    #[test]
    fn login_waits_for_network_and_resume_starts_a_new_refresh() {
        let clock = FakeClock::at(1000);
        let online = Arc::new(Mutex::new(1));
        let sleep = Arc::new(Mutex::new(false));
        let mut lifecycle = Lifecycle::new(Nm(online.clone()), Login(sleep.clone()), clock.clone());
        assert!(!lifecycle.step().online);
        clock.advance(20_000);
        *online.lock().unwrap() = 4;
        let connected = lifecycle.step();
        assert!(connected.online);
        assert_eq!(connected.ready_at, 21_000);
        assert_eq!(lifecycle.step().generation, 1);
        *sleep.lock().unwrap() = true;
        assert!(!lifecycle.step().online);
        *online.lock().unwrap() = 1;
        *sleep.lock().unwrap() = false;
        assert!(!lifecycle.step().online);
        *online.lock().unwrap() = 4;
        assert_eq!(lifecycle.step().generation, 2);
    }
}

pub type Shared = std::sync::Arc<std::sync::Mutex<Status>>;

/// Current connectivity generation, injected into blocking collectors so obsolete
/// responses can be discarded before changing readings or persistence.
pub trait CurrentStatus: Send {
    fn current(&self) -> Status;
}
impl CurrentStatus for Shared {
    fn current(&self) -> Status {
        *self.lock().unwrap()
    }
}

pub fn start(app: &tauri::AppHandle) {
    use tauri::Manager;
    let shared = Shared::default();
    app.manage(shared.clone());
    let app = app.clone();
    std::thread::spawn(move || {
        let mut lifecycle = Lifecycle::new(
            crate::platform::connectivity(),
            crate::platform::session(),
            crate::ports::SystemClock,
        );
        loop {
            let was_sleeping = lifecycle.sleeping;
            let status = lifecycle.step();
            *shared.lock().unwrap() = status;
            if lifecycle.sleeping != was_sleeping {
                if let Some(collector) = app.try_state::<crate::system::Collector>() {
                    collector.sleep(lifecycle.sleeping);
                }
                if let Some(collector) = app.try_state::<crate::network::Collector>() {
                    let _ = collector
                        .0
                        .send(crate::network::Command::Sleep(lifecycle.sleeping));
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

#[cfg(test)]
mod scheduling_tests {
    use super::*;
    use crate::ports::fakes::FakeClock;
    struct Online;
    impl crate::network::NetworkManager for Online {
        fn connection(&self) -> std::io::Result<crate::network::Connection> {
            Ok(crate::network::Connection {
                connectivity: 4,
                ..Default::default()
            })
        }
    }
    struct Awake;
    impl Session for Awake {
        fn preparing_for_sleep(&self) -> Option<bool> {
            Some(false)
        }
    }
    #[test]
    fn login_delays_first_source_ten_seconds_and_spaces_sources_three_seconds() {
        let clock = FakeClock::at(5000);
        let mut lifecycle = Lifecycle::new(Online, Awake, clock);
        let status = lifecycle.step();
        assert_eq!(status.ready_for(0), 15_000);
        assert_eq!(status.ready_for(1), 18_000);
        assert_eq!(status.ready_for(2), 21_000);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod adapter_tests {
    use super::*;
    #[test]
    fn logind_decodes_boolean_bodies_and_preserves_sleep_resume_order() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/session/logind.json")).unwrap();
        let queue = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new()));
        queue
            .lock()
            .unwrap()
            .push_back(fixture["preparingForSleep"].as_bool().unwrap());
        for sleeping in fixture["sleepResumeSequence"].as_array().unwrap() {
            let message = zbus::Message::signal(
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
                "PrepareForSleep",
            )
            .unwrap()
            .build(&(sleeping.as_bool().unwrap(),))
            .unwrap();
            queue
                .lock()
                .unwrap()
                .push_back(sleep_state(&message).unwrap());
        }
        let logind = Logind(queue);
        assert_eq!(logind.preparing_for_sleep(), Some(false));
        assert_eq!(logind.preparing_for_sleep(), Some(true));
        assert_eq!(logind.preparing_for_sleep(), Some(false));
        assert_eq!(logind.preparing_for_sleep(), None);
    }
}
