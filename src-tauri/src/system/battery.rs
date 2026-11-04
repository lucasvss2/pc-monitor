//! Battery through the UPower DisplayDevice on the system D-Bus.

use std::io;
#[cfg(target_os = "linux")]
use std::thread;

use serde::Serialize;
#[cfg(target_os = "linux")]
use zbus::blocking::fdo::PropertiesProxy;
#[cfg(target_os = "linux")]
use zbus::blocking::Connection;
#[cfg(target_os = "linux")]
use zbus::names::InterfaceName;

/// Properties of org.freedesktop.UPower.Device used by this module.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayDevice {
    pub kind: u32,
    pub present: bool,
    pub percentage: f64,
    pub state: u32,
    /// Seconds; zero when UPower cannot estimate.
    pub until_empty: i64,
    pub until_full: i64,
}

/// Injectable UPower D-Bus port.
pub trait UPower: Send {
    fn display_device(&self) -> io::Result<DisplayDevice>;
}

/// UPower Type for batteries.
const BATTERY_TYPE: u32 = 2;
const WARNING_PERCENTAGE: f64 = 15.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percentage: f64,
    pub state: BatteryState,
    /// Time to empty or full, when reported by UPower.
    pub seconds_remaining: Option<i64>,
    /// Below 15%.
    pub warning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    Empty,
    /// Plugged in without charging, for example at a charge limit.
    PluggedIn,
    Unknown,
}

/// Read DisplayDevice; None without a battery or UPower.
pub fn read(upower: &impl UPower) -> Option<Battery> {
    let d = upower.display_device().ok()?;
    if d.kind != BATTERY_TYPE || !d.present {
        return None;
    }
    let state = match d.state {
        1 => BatteryState::Charging,
        2 | 6 => BatteryState::Discharging,
        3 => BatteryState::Empty,
        4 => BatteryState::Full,
        5 => BatteryState::PluggedIn,
        _ => BatteryState::Unknown,
    };
    let remaining = match state {
        BatteryState::Charging => d.until_full,
        BatteryState::Discharging => d.until_empty,
        _ => 0,
    };
    Some(Battery {
        percentage: d.percentage.round(),
        state,
        seconds_remaining: (remaining > 0).then_some(remaining),
        warning: d.percentage < WARNING_PERCENTAGE,
    })
}

#[cfg(target_os = "linux")]
const DESTINATION: &str = "org.freedesktop.UPower";
#[cfg(target_os = "linux")]
const PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
#[cfg(target_os = "linux")]
const INTERFACE: &str = "org.freedesktop.UPower.Device";

/// Real UPower on the system bus. Without a bus, readings fail and the battery block is hidden.
#[cfg(target_os = "linux")]
pub struct UPowerDbus(Option<Connection>);

#[cfg(target_os = "linux")]
impl UPowerDbus {
    pub fn connect() -> Self {
        UPowerDbus(Connection::system().ok())
    }
}

#[cfg(target_os = "linux")]
fn properties(connection: &Connection) -> zbus::Result<PropertiesProxy<'static>> {
    PropertiesProxy::builder(connection)
        .destination(DESTINATION)?
        .path(PATH)?
        .build()
}

#[cfg(target_os = "linux")]
impl UPower for UPowerDbus {
    fn display_device(&self) -> io::Result<DisplayDevice> {
        let connection = self
            .0
            .as_ref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))?;
        let all = properties(connection)
            .map_err(io::Error::other)?
            .get_all(InterfaceName::from_static_str_unchecked(INTERFACE))
            .map_err(io::Error::other)?;
        let field = |name: &str| {
            all.get(name)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, name.to_owned()))
        };
        let convert = |e: zbus::zvariant::Error| io::Error::new(io::ErrorKind::InvalidData, e);
        Ok(DisplayDevice {
            kind: field("Type")?.downcast_ref().map_err(convert)?,
            present: field("IsPresent")?.downcast_ref().map_err(convert)?,
            percentage: field("Percentage")?.downcast_ref().map_err(convert)?,
            state: field("State")?.downcast_ref().map_err(convert)?,
            until_empty: field("TimeToEmpty")?.downcast_ref().map_err(convert)?,
            until_full: field("TimeToFull")?.downcast_ref().map_err(convert)?,
        })
    }
}

/// Call notice on each DisplayDevice PropertiesChanged signal, on a dedicated thread.
#[cfg(target_os = "linux")]
pub fn listen_upower(notice: impl Fn() + Send + 'static) {
    let listen = move || -> zbus::Result<()> {
        let connection = Connection::system()?;
        for _ in properties(&connection)?.receive_properties_changed()? {
            notice();
        }
        Ok(())
    };
    let _ = thread::Builder::new().name("upower".into()).spawn(move || {
        if let Err(e) = listen() {
            eprintln!("system: UPower signals unavailable: {e}");
        }
    });
}

#[cfg(test)]
pub mod fake {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::ports::fakes::fixtures;

    /// Fake UPower backed by recorded busctl GetAll output in fixtures/system/upower/.
    #[derive(Clone)]
    pub struct FakeUPower {
        response: Arc<Mutex<io::Result<DisplayDevice>>>,
        pub readings: Arc<AtomicUsize>,
    }

    impl FakeUPower {
        pub fn from(recording: &str) -> Self {
            let fake = FakeUPower {
                response: Arc::new(Mutex::new(Ok(Default::default()))),
                readings: Default::default(),
            };
            fake.respond(recording);
            fake
        }

        pub fn respond(&self, recording: &str) {
            *self.response.lock().unwrap() = Ok(read_recording(recording));
        }

        pub fn disconnect(&self) {
            *self.response.lock().unwrap() = Err(io::Error::from(io::ErrorKind::ConnectionRefused));
        }
    }

    impl UPower for FakeUPower {
        fn display_device(&self) -> io::Result<DisplayDevice> {
            self.readings.fetch_add(1, Ordering::SeqCst);
            match &*self.response.lock().unwrap() {
                Ok(d) => Ok(d.clone()),
                Err(e) => Err(io::Error::from(e.kind())),
            }
        }
    }

    fn read_recording(name: &str) -> DisplayDevice {
        let path = fixtures().join(format!("system/upower/{name}.json"));
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let props = &json["data"][0];
        let v = |key: &str| &props[key]["data"];
        DisplayDevice {
            kind: v("Type").as_u64().unwrap() as u32,
            present: v("IsPresent").as_bool().unwrap(),
            percentage: v("Percentage").as_f64().unwrap(),
            state: v("State").as_u64().unwrap() as u32,
            until_empty: v("TimeToEmpty").as_i64().unwrap(),
            until_full: v("TimeToFull").as_i64().unwrap(),
        }
    }
}
