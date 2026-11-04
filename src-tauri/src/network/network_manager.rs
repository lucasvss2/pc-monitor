use super::{Connection, Wifi};
use serde::Deserialize;
use std::io;
use zbus::blocking::{Connection as DbusConnection, Proxy};
use zbus::zvariant::OwnedObjectPath;

use super::NetworkManager;

/// Properties read from D-Bus before selecting a physical link.
#[derive(Clone, Deserialize)]
pub struct NetworkManagerReading {
    pub primary: String,
    pub connectivity: u32,
    pub active: Vec<ActiveConnection>,
    #[serde(default)]
    pub revision: u64,
}
#[derive(Clone, Deserialize)]
pub struct ActiveConnection {
    #[serde(default)]
    pub addresses: Vec<std::net::IpAddr>,
    pub path: String,
    #[serde(default)]
    pub network_id: String,
    #[serde(default)]
    pub metered: u32,
    pub state: u32,
    pub kind: String,
    pub vpn: bool,
    pub interface: String,
    pub gateway4: String,
    pub gateway6: String,
    #[serde(default)]
    pub wifi: Option<Wifi>,
}

impl ActiveConnection {
    fn profile_id(&self) -> &str {
        if self.network_id.is_empty() {
            &self.path
        } else {
            &self.network_id
        }
    }
}

/// NetworkManager properties port; tests inject recorded D-Bus output.
pub trait NetworkManagerProperties: Send {
    fn read(&self) -> io::Result<NetworkManagerReading>;
    fn portal(&self) -> Option<bool> {
        None
    }
}
pub struct NetworkManagerDbus<P = DbusSystem>(pub P);
pub struct DbusSystem(std::sync::Arc<std::sync::Mutex<Option<NetworkManagerReading>>>);
const DESTINATION: &str = "org.freedesktop.NetworkManager";
impl NetworkManagerDbus {
    pub fn connect() -> Self {
        let cache = std::sync::Arc::new(std::sync::Mutex::new(None));
        let reading = cache.clone();
        std::thread::spawn(move || loop {
            // Subscribe before the initial read to avoid missing AP or profile changes.
            let result = (|| -> zbus::Result<()> {
                let bus = DbusConnection::system()?;
                let rule = zbus::MatchRule::builder()
                    .msg_type(zbus::message::Type::Signal)
                    .sender(DESTINATION)?
                    .path_namespace("/org/freedesktop/NetworkManager")?
                    .build();
                let messages =
                    zbus::blocking::MessageIterator::for_match_rule(rule, &bus, Some(256))?;
                *reading.lock().unwrap() = Some(read_dbus()?);
                let mut revision = 0;
                for message in messages {
                    message?;
                    revision += 1;
                    let mut current = read_dbus()?;
                    current.revision = revision;
                    *reading.lock().unwrap() = Some(current);
                }
                Ok(())
            })();
            let _ = result;
            *reading.lock().unwrap() = None;
            std::thread::sleep(std::time::Duration::from_secs(5));
        });
        Self(DbusSystem(cache))
    }
}
impl<P: NetworkManagerProperties> NetworkManager for NetworkManagerDbus<P> {
    fn check_portal(&self) -> Option<bool> {
        self.0.portal()
    }
    fn connection(&self) -> io::Result<Connection> {
        let reading = self.0.read()?;
        let mut physical: Option<Connection> = None;
        let mut vpn = false;
        let mut routes = Vec::new();
        let mut vpn_addresses = Vec::new();
        let mut primary_route = String::new();
        for active in reading.active.into_iter().filter(|a| a.state == 2) {
            let is_vpn = active.vpn || active.kind == "wireguard";
            vpn |= is_vpn;
            if active.path == reading.primary && is_vpn {
                primary_route = active.profile_id().to_string();
            }
            if is_vpn {
                routes.push(active.profile_id().to_string());
                vpn_addresses.extend(active.addresses.iter().copied());
            }
            if is_vpn || active.kind == "loopback" {
                continue;
            }
            // The primary connection may be a VPN; retain the active physical link in that case.
            if physical.is_some() && active.path != reading.primary {
                continue;
            }
            physical = Some(Connection {
                addresses: active.addresses,
                id: active.path,
                network_id: active.network_id,
                route_id: String::new(),
                metered: !matches!(active.metered, 2 | 4),
                kind: active.kind,
                connectivity: reading.connectivity,
                vpn: false,
                gateway: active
                    .gateway4
                    .parse()
                    .ok()
                    .or_else(|| active.gateway6.parse().ok()),
                interface: active.interface,
                wifi: active.wifi,
                revision: reading.revision,
            });
        }
        let mut connection = physical.unwrap_or_default();
        connection.vpn = vpn;
        connection.addresses.extend(vpn_addresses);
        connection.addresses.sort();
        connection.addresses.dedup();
        routes.sort();
        connection.route_id = if routes.is_empty() {
            String::new()
        } else {
            format!("{primary_route}:{}", routes.join(","))
        };
        Ok(connection)
    }
}
impl NetworkManagerProperties for DbusSystem {
    fn portal(&self) -> Option<bool> {
        let bus = DbusConnection::system().ok()?;
        let nm = Proxy::new(
            &bus,
            DESTINATION,
            "/org/freedesktop/NetworkManager",
            DESTINATION,
        )
        .ok()?;
        let state: u32 = nm.call("CheckConnectivity", &()).ok()?;
        Some(state == 2)
    }
    fn read(&self) -> io::Result<NetworkManagerReading> {
        self.0
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| io::Error::other("NetworkManager unavailable"))
    }
}
fn read_dbus() -> zbus::Result<NetworkManagerReading> {
    let bus = DbusConnection::system()?;
    let nm = Proxy::new(
        &bus,
        DESTINATION,
        "/org/freedesktop/NetworkManager",
        DESTINATION,
    )?;
    let primary: OwnedObjectPath = nm.get_property("PrimaryConnection")?;
    let paths: Vec<OwnedObjectPath> = nm.get_property("ActiveConnections")?;
    let mut active_connections = Vec::new();
    for path in paths {
        let active = Proxy::new(
            &bus,
            DESTINATION,
            path.clone(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )?;
        let state: u32 = active.get_property("State")?;
        let kind: String = active.get_property("Type")?;
        let vpn: bool = active.get_property("Vpn")?;
        let mut reading = ActiveConnection {
            addresses: Vec::new(),
            path: path.to_string(),
            network_id: active.get_property("Uuid")?,
            metered: 0,
            state,
            kind,
            vpn,
            interface: String::new(),
            gateway4: String::new(),
            gateway6: String::new(),
            wifi: None,
        };
        if state == 2 && !vpn && !matches!(reading.kind.as_str(), "loopback" | "wireguard") {
            let dispositivos: Vec<OwnedObjectPath> = active.get_property("Devices")?;
            if let Some(d) = dispositivos.first() {
                let device = Proxy::new(
                    &bus,
                    DESTINATION,
                    d.clone(),
                    "org.freedesktop.NetworkManager.Device",
                )?;
                reading.interface = device.get_property("Interface")?;
                reading.metered = device.get_property("Metered").unwrap_or(0);
            }
            if reading.kind == "802-11-wireless" {
                if let Some(d) = dispositivos.first() {
                    let wifi = Proxy::new(
                        &bus,
                        DESTINATION,
                        d.clone(),
                        "org.freedesktop.NetworkManager.Device.Wireless",
                    )?;
                    let ap: OwnedObjectPath = wifi.get_property("ActiveAccessPoint")?;
                    if ap.as_str() != "/" {
                        let ap = Proxy::new(
                            &bus,
                            DESTINATION,
                            ap,
                            "org.freedesktop.NetworkManager.AccessPoint",
                        )?;
                        let profile: OwnedObjectPath = active.get_property("Connection")?;
                        let profile = Proxy::new(
                            &bus,
                            DESTINATION,
                            profile,
                            "org.freedesktop.NetworkManager.Settings.Connection",
                        )?;
                        // GetSettings omits secrets; never call GetSecrets.
                        let settings: std::collections::HashMap<
                            String,
                            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
                        > = profile.call("GetSettings", &())?;
                        let security = settings.get("802-11-wireless-security");
                        let key_mgmt = security
                            .and_then(|s| s.get("key-mgmt"))
                            .and_then(|v| <&str>::try_from(v).ok())
                            .unwrap_or("")
                            .to_string();
                        let pairwise = security
                            .and_then(|s| s.get("pairwise"))
                            .and_then(|v| v.try_clone().ok())
                            .and_then(|v| Vec::<String>::try_from(v).ok())
                            .unwrap_or_default();
                        reading.wifi = Some(Wifi {
                            flags: ap.get_property("Flags")?,
                            wpa: ap.get_property("WpaFlags")?,
                            rsn: ap.get_property("RsnFlags")?,
                            key_mgmt,
                            pairwise,
                        });
                    }
                }
            }
        }
        if state == 2 && reading.kind != "loopback" {
            for (field, contract, gateway) in [
                (
                    "Ip4Config",
                    "org.freedesktop.NetworkManager.IP4Config",
                    &mut reading.gateway4,
                ),
                (
                    "Ip6Config",
                    "org.freedesktop.NetworkManager.IP6Config",
                    &mut reading.gateway6,
                ),
            ] {
                let config: OwnedObjectPath = active.get_property(field)?;
                if config.as_str() != "/" {
                    let config = Proxy::new(&bus, DESTINATION, config, contract)?;
                    *gateway = config.get_property("Gateway")?;
                    let addresses: Vec<
                        std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
                    > = config.get_property("AddressData")?;
                    reading
                        .addresses
                        .extend(addresses.iter().filter_map(|entry| {
                            <&str>::try_from(entry.get("address")?)
                                .ok()?
                                .parse::<std::net::IpAddr>()
                                .ok()
                        }));
                }
            }
        }
        active_connections.push(reading);
    }
    Ok(NetworkManagerReading {
        revision: 0,
        primary: primary.to_string(),
        connectivity: nm.get_property("Connectivity")?,
        active: active_connections,
    })
}
