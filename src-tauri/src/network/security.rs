use super::{Connection, NetworkData, NetworkManager};
use crate::ports::{Notification, Notifier};
use serde::{Deserialize, Serialize};

const PRIVACY: u32 = 0x1;
const PAIR_TKIP: u32 = 0x4;
const GROUP_TKIP: u32 = 0x40;
const OWE: u32 = 0x800;
const OWE_TRANSITION: u32 = 0x1000;

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Wifi {
    pub flags: u32,
    pub wpa: u32,
    pub rsn: u32,
    pub key_mgmt: String,
    pub pairwise: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Security {
    NoAlerts,
    Warning,
    Risk,
}
impl NetworkData {
    pub fn risk_badge(&self) -> bool {
        self.connected && self.security == Some(Security::Risk)
    }
}
pub trait Check: Send {
    /// Some(true): a nonexistent name resolved; None: inconclusive lookup.
    fn dns(&self) -> Option<bool>;
    fn mac(&self, connection: &Connection) -> Option<String>;
}
pub struct NoopCheck;
impl Check for NoopCheck {
    fn dns(&self) -> Option<bool> {
        None
    }
    fn mac(&self, _: &Connection) -> Option<String> {
        None
    }
}
pub struct NoopNotifier;
impl Notifier for NoopNotifier {
    fn notify(&self, _: Notification) {}
}
#[derive(Default)]
pub struct Assessment {
    next_check: u64,
    dns: Option<bool>,
    portal: bool,
    connectivity: Option<u32>,
    mac_revision: Option<u64>,
    mac: Option<String>,
    mac_changed: bool,
    notified: bool,
}
impl Assessment {
    pub fn resume(&mut self) {
        self.next_check = 0;
        self.mac_revision = None;
    }

    pub fn assess(
        &mut self,
        c: &Connection,
        now: u64,
        v: &impl Check,
        nm: &impl NetworkManager,
        n: &impl Notifier,
    ) -> (Option<Security>, Vec<String>) {
        if c.id.is_empty() {
            return (None, vec![]);
        }
        if now >= self.next_check {
            self.next_check = now.saturating_add(600_000);
            self.dns = v.dns();
            self.portal = nm.check_portal().unwrap_or(c.connectivity == 2);
        }
        if self
            .connectivity
            .is_some_and(|previous| previous != c.connectivity)
        {
            self.portal = c.connectivity == 2;
        }
        self.connectivity = Some(c.connectivity);
        if self.mac_revision != Some(c.revision) {
            self.mac_revision = Some(c.revision);
            if let Some(mac) = v.mac(c) {
                if self.mac.as_ref().is_some_and(|previous| previous != &mac) {
                    self.mac_changed = true;
                }
                self.mac = Some(mac);
            }
        }
        let mut risks = vec![];
        if c.kind == "802-11-wireless" {
            if let Some(w) = &c.wifi {
                let owe = w.rsn & (OWE | OWE_TRANSITION) != 0 || w.key_mgmt == "owe";
                if !owe && w.wpa == 0 && w.rsn == 0 {
                    risks.push(
                        if w.flags & PRIVACY == 0 {
                            "Rede aberta sem OWE"
                        } else {
                            "Criptografia WEP"
                        }
                        .to_string(),
                    );
                } else if (w.wpa | w.rsn) & (PAIR_TKIP | GROUP_TKIP) != 0
                    || w.pairwise.iter().any(|p| p == "tkip")
                {
                    risks.push("WPA com TKIP".into());
                }
            }
        }
        if self.dns == Some(true) {
            risks.push("DNS respondeu a um nome inexistente".into());
        }
        let risk = !risks.is_empty();
        if self.portal {
            risks.push("Portal cativo detectado".into());
        }
        if self.mac_changed {
            risks.push("MAC do gateway mudou nesta conexão".into());
        }
        let state = if risk {
            Security::Risk
        } else if !risks.is_empty() {
            Security::Warning
        } else {
            Security::NoAlerts
        };
        if risk && !self.notified {
            self.notified = true;
            n.notify(Notification::NetworkAtRisk);
        }
        (Some(state), risks)
    }
}

/// D-Bus response without the queried name or content from other services.
#[derive(Clone, Deserialize)]
pub struct DnsResponse {
    pub answered: bool,
    pub error: Option<String>,
}
pub trait SecurityReadings: Send {
    fn resolver(&self) -> Option<DnsResponse>;
    fn neighbors(&self, c: &Connection) -> Option<String>;
}
pub struct SystemReadings;
pub struct SystemCheck<P = SystemReadings>(pub P);
impl<P: SecurityReadings> Check for SystemCheck<P> {
    fn dns(&self) -> Option<bool> {
        let r = self.0.resolver()?;
        match r.error.as_deref() {
            None => Some(r.answered),
            Some("org.freedesktop.resolve1.DnsError.NXDOMAIN") => Some(false),
            _ => None,
        }
    }
    fn mac(&self, c: &Connection) -> Option<String> {
        let neighbors: serde_json::Value = serde_json::from_str(&self.0.neighbors(c)?).ok()?;
        let gateway = c.gateway?.to_string();
        neighbors
            .as_array()?
            .iter()
            .filter(|v| v.get("dst").and_then(|v| v.as_str()) == Some(gateway.as_str()))
            .find_map(|v| {
                v.get("lladdr")?
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            })
    }
}
#[cfg(target_os = "linux")]
type ResolveHostnameResponse = (Vec<(i32, i32, Vec<u8>)>, String, u64);

#[cfg(target_os = "linux")]
impl SecurityReadings for SystemReadings {
    fn resolver(&self) -> Option<DnsResponse> {
        // Use a fresh name for each query and the current connection resolver, including VPN.
        let random_id = std::fs::read_to_string("/proc/sys/kernel/random/uuid").ok()?;
        let name = format!("{}.com", random_id.trim());
        let bus = zbus::blocking::Connection::system().ok()?;
        let proxy = zbus::blocking::Proxy::new(
            &bus,
            "org.freedesktop.resolve1",
            "/org/freedesktop/resolve1",
            "org.freedesktop.resolve1.Manager",
        )
        .ok()?;
        let response: zbus::Result<ResolveHostnameResponse> =
            proxy.call("ResolveHostname", &(0i32, name, 0i32, 0u64));
        match response {
            Ok((enderecos, _, _)) => Some(DnsResponse {
                answered: !enderecos.is_empty(),
                error: None,
            }),
            Err(zbus::Error::MethodError(name, _, _)) => Some(DnsResponse {
                answered: false,
                error: Some(name.to_string()),
            }),
            _ => None,
        }
    }
    fn neighbors(&self, c: &Connection) -> Option<String> {
        let gateway = c.gateway?;
        let output = std::process::Command::new("ip")
            .args([
                "-j",
                "neigh",
                "show",
                "to",
                &gateway.to_string(),
                "dev",
                &c.interface,
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout).ok()
    }
}
