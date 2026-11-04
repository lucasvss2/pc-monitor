//! Freedesktop notification transport. GNOME owns delivery and Do Not Disturb.
use super::notifications::Sink;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

type Targets = Arc<Mutex<HashMap<u32, String>>>;
pub struct Desktop {
    bus: Option<zbus::blocking::Connection>,
    targets: Targets,
}
impl Desktop {
    pub fn new(app: tauri::AppHandle) -> Self {
        let bus = zbus::blocking::Connection::session().ok();
        let targets = Targets::default();
        if let Some(connection) = bus.clone() {
            let targets = targets.clone();
            let (ready, wait) = std::sync::mpsc::sync_channel(0);
            std::thread::spawn(move || {
                let Ok(proxy) = proxy(&connection) else {
                    return;
                };
                let Ok(signals) = proxy.receive_all_signals() else {
                    return;
                };
                let _ = ready.send(());
                for signal in signals {
                    if signal
                        .header()
                        .member()
                        .is_some_and(|member| member.as_str() == "NotificationClosed")
                    {
                        if let Ok((id, _reason)) = signal.body().deserialize::<(u32, u32)>() {
                            targets.lock().unwrap().remove(&id);
                        }
                        continue;
                    }
                    if signal
                        .header()
                        .member()
                        .is_none_or(|member| member.as_str() != "ActionInvoked")
                    {
                        continue;
                    }
                    let Ok((id, action)) = signal.body().deserialize::<(u32, String)>() else {
                        continue;
                    };
                    if action != "default" {
                        continue;
                    }
                    let target = targets.lock().unwrap().get(&id).cloned();
                    if let Some(session) = target {
                        let app = app.clone();
                        let handle = app.clone();
                        let _ = handle
                            .run_on_main_thread(move || crate::panel::open_agent(&app, session));
                    }
                }
            });
            let _ = wait.recv();
        }
        Self { bus, targets }
    }
}
fn proxy(bus: &zbus::blocking::Connection) -> zbus::Result<zbus::blocking::Proxy<'_>> {
    zbus::blocking::Proxy::new(
        bus,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
}
impl Sink for Desktop {
    fn available(&self) -> bool {
        self.bus.is_some()
    }
    fn show(&mut self, replaces: u32, title: &str, body: &str, session: &str) -> u32 {
        let Some(bus) = &self.bus else {
            return 0;
        };
        let Ok(proxy) = proxy(bus) else {
            return 0;
        };
        let mut targets = self.targets.lock().unwrap();
        let result: zbus::Result<u32> = proxy.call(
            "Notify",
            &(
                "kvojps-monitor",
                replaces,
                "",
                title,
                body,
                vec![
                    "default",
                    crate::strings::STRINGS.indicator.open_panel.as_str(),
                ],
                HashMap::<String, zbus::zvariant::Value>::new(),
                -1i32,
            ),
        );
        let Ok(id) = result else {
            return 0;
        };
        if replaces != 0 {
            targets.remove(&replaces);
        }
        targets.insert(id, session.into());
        id
    }
    fn close(&mut self, id: u32) {
        self.targets.lock().unwrap().remove(&id);
        if let Some(bus) = &self.bus {
            if let Ok(proxy) = proxy(bus) {
                let _: zbus::Result<()> = proxy.call("CloseNotification", &(id,));
            }
        }
    }
}
