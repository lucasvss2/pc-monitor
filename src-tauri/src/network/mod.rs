//! Continuous network collection with a sanitized cache of the last speed measurement.
#[cfg(target_os = "linux")]
mod network_manager;
mod probe;
mod process;
mod security;
mod speed;
pub use security::*;
pub use speed::SpeedResult;
#[cfg(all(test, target_os = "linux"))]
mod tests;

use crate::ports::{Clock, Files};
pub trait NetworkManager: Send {
    fn connection(&self) -> std::io::Result<Connection>;
    fn counters(&self, files: &dyn Files, _: &Connection) -> std::io::Result<Counters> {
        let text = files.read("/proc/net/dev")?;
        Ok(text
            .lines()
            .filter_map(|line| {
                let (name, numbers) = line.split_once(':')?;
                let mut values = numbers.split_whitespace();
                let rx = values.next()?.parse().ok()?;
                let tx = values.nth(7)?.parse().ok()?;
                Some((name.trim().to_string(), (rx, tx)))
            })
            .collect())
    }
    fn interface_index(&self, files: &dyn Files, interface: &str) -> u32 {
        files
            .read(&format!("/sys/class/net/{interface}/ifindex"))
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0)
    }
    fn advanced_available(&self) -> bool {
        true
    }
    fn check_portal(&self) -> Option<bool> {
        None
    }
}

#[cfg(target_os = "linux")]
pub use network_manager::NetworkManagerDbus;
#[cfg(target_os = "linux")]
pub use probe::PingSocket;
pub use probe::Probe;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr, SocketAddrV6};

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Connection {
    #[serde(default)]
    pub addresses: Vec<IpAddr>,
    pub id: String,
    pub kind: String,
    pub connectivity: u32,
    pub vpn: bool,
    pub gateway: Option<IpAddr>,
    pub interface: String,
    pub wifi: Option<Wifi>,
    pub revision: u64,
    #[serde(default)]
    pub metered: bool,
    #[serde(default)]
    pub network_id: String,
    #[serde(default)]
    pub route_id: String,
}
impl Connection {
    fn network_identity(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            if self.network_id.is_empty() {
                &self.id
            } else {
                &self.network_id
            },
            self.interface,
            self.vpn,
            self.route_id
        )
    }
    fn connected(&self) -> bool {
        !self.id.is_empty()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    Good,
    Regular,
    Poor,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Measurements {
    pub latency: Option<f64>,
    pub jitter: Option<f64>,
    pub loss: Option<f64>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Traffic {
    pub download: f64,
    pub upload: f64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkData {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub advanced_unavailable: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub connection_unavailable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_kind: Option<LinkKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_revision: Option<u64>,
    pub process_traffic: Option<process::ProcessTraffic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_operation: Option<speed::Operation>,
    pub speed: Option<SpeedResult>,
    pub speed_available: bool,
    pub speed_running: bool,
    pub speed_failed: bool,
    pub automatic_speed: bool,
    pub connected: bool,
    pub wired: bool,
    pub vpn: bool,
    pub quality: Option<Quality>,
    pub security: Option<Security>,
    pub reasons: Vec<String>,
    pub gateway: Measurements,
    pub internet: Measurements,
    pub traffic: Option<Traffic>,
    pub history: Vec<Traffic>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkKind {
    Wired,
    Wireless,
    Other,
}

pub type Counters = HashMap<String, (u64, u64)>;
pub(crate) const INTERNET_V4: std::net::Ipv4Addr = std::net::Ipv4Addr::new(1, 1, 1, 1);
pub(crate) const INTERNET_V6: std::net::Ipv6Addr =
    std::net::Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111);

pub struct Network<A, N, S, R, V = NoopCheck, T = NoopNotifier> {
    processes: Option<process::Attribution>,
    speed: Option<speed::SpeedTests>,
    check: V,
    notifier: T,
    assessment: Assessment,
    files: A,
    nm: N,
    probe: S,
    clock: R,
    next: u64,
    next_ping: u64,
    connection: Connection,
    connection_revision: u64,
    samples: VecDeque<(u64, Option<f64>, Option<f64>)>,
    previous: Option<(u64, Counters)>,
    history: VecDeque<Traffic>,
}
impl<A: Files, N: NetworkManager, S: Probe, R: Clock> Network<A, N, S, R> {
    pub fn new(files: A, nm: N, probe: S, clock: R) -> Self {
        Self {
            processes: None,
            speed: None,
            check: NoopCheck,
            notifier: NoopNotifier,
            assessment: Assessment::default(),
            files,
            nm,
            probe,
            clock,
            next: 0,
            next_ping: 0,
            connection: Connection::default(),
            connection_revision: 0,
            samples: VecDeque::new(),
            previous: None,
            history: VecDeque::new(),
        }
    }
    pub fn with_security<V: Check, T: crate::ports::Notifier>(
        self,
        check: V,
        notifier: T,
    ) -> Network<A, N, S, R, V, T> {
        Network {
            processes: self.processes,
            speed: self.speed,
            files: self.files,
            nm: self.nm,
            probe: self.probe,
            clock: self.clock,
            next: self.next,
            next_ping: self.next_ping,
            connection: self.connection,
            connection_revision: self.connection_revision,
            samples: self.samples,
            previous: self.previous,
            history: self.history,
            check,
            notifier,
            assessment: self.assessment,
        }
    }
}
impl<A: Files, N: NetworkManager, S: Probe, R: Clock, V: Check, T: crate::ports::Notifier>
    Network<A, N, S, R, V, T>
{
    pub fn with_processes(mut self, diagnostics: impl process::Diagnostics + 'static) -> Self {
        self.processes = Some(process::Attribution::new(diagnostics));
        self
    }
    pub fn process_visible(&mut self, visible: bool) {
        if let Some(processes) = &mut self.processes {
            processes.visible(visible);
        }
    }
    pub fn with_speed(
        mut self,
        transport: impl speed::Transport + 'static,
        persistence: impl crate::persistence::Persistence + 'static,
    ) -> Self {
        self.speed = Some(speed::SpeedTests::new(transport, persistence));
        self
    }
    pub fn with_preferences(mut self, storage: impl crate::persistence::Storage + 'static) -> Self {
        if let Some(speed) = &mut self.speed {
            speed.with_preferences(storage);
        }
        self
    }
    pub fn request_speed(&mut self) {
        if let Some(speed) = &mut self.speed {
            speed.request();
        }
    }
    pub fn automatic_speed(&mut self, enabled: bool) {
        if let Some(speed) = &mut self.speed {
            speed.set_automatic(enabled);
        }
    }
    pub fn resume(&mut self) {
        self.next = 0;
        self.next_ping = 0;
        self.previous = None;
        self.history.clear();
        self.samples.clear();
        self.assessment.resume();
        if let Some(processes) = &mut self.processes {
            processes.reset();
        }
        if let Some(speed) = &mut self.speed {
            speed.resume();
        }
    }
    pub fn step(&mut self) -> Option<(u64, NetworkData)> {
        let now = self.clock.now();
        if now < self.next {
            return None;
        }
        self.next = now + 1000;
        // Query the platform even while the panel is closed. Errors never reuse an old link.
        let connection = self.nm.connection();
        let connection_unavailable = connection.is_err();
        let connection = connection.unwrap_or_default();
        let changed = self.connection.network_identity() != connection.network_identity();
        if changed
            || self.connection.gateway != connection.gateway
            || self.connection.addresses != connection.addresses
        {
            if let Some(processes) = &mut self.processes {
                processes.reset();
            }
            self.samples.clear();
            self.previous = None;
            self.history.clear();
            self.next_ping = 0;
        }
        // Publish an opaque revision only when a known connected identity really changes.
        if self.connection.connected() && connection.connected() && changed {
            self.connection_revision += 1;
        }
        let next = self.connection.id != connection.id
            || self.connection.interface != connection.interface;
        if next {
            self.assessment = Assessment::default();
        }
        self.connection = connection;
        let advanced_unavailable = !self.nm.advanced_available();
        let (security, reasons) = if advanced_unavailable {
            (None, vec![])
        } else {
            self.assessment
                .assess(&self.connection, now, &self.check, &self.nm, &self.notifier)
        };
        if let Some(speed) = &mut self.speed {
            speed.step(&self.connection, now);
        }
        let connected = self.connection.connected();
        let traffic = self.traffic(self.clock.monotonic_ms());
        if let Some(processes) = &mut self.processes {
            if connected {
                processes.step(
                    now,
                    &self.files,
                    &self.connection.interface,
                    &self.connection.addresses,
                    self.previous.as_ref().map(|(_, counters)| counters),
                );
            } else {
                processes.reset();
            }
        }
        let has_probe_route = !advanced_unavailable || !self.connection.interface.is_empty();
        if connected && has_probe_route && now >= self.next_ping {
            self.next_ping = now + 5000;
            let gateway = self.connection.gateway.map(|ip| match ip {
                IpAddr::V6(ip) if ip.is_unicast_link_local() => {
                    let index = self
                        .nm
                        .interface_index(&self.files, &self.connection.interface);
                    SocketAddr::V6(SocketAddrV6::new(ip, 0, 0, index))
                }
                _ => SocketAddr::new(ip, 0),
            });
            let internet = if gateway.is_some_and(|g| g.is_ipv6())
                || (gateway.is_none()
                    && self
                        .connection
                        .addresses
                        .first()
                        .is_some_and(IpAddr::is_ipv6))
            {
                SocketAddr::new(INTERNET_V6.into(), 0)
            } else {
                SocketAddr::new(INTERNET_V4.into(), 0)
            };
            let (gateway, internet) = std::thread::scope(|scope| {
                let gateway = scope.spawn(|| gateway.and_then(|ip| self.probe.measure(ip)));
                let internet = self.probe.measure(internet);
                (gateway.join().unwrap_or(None), internet)
            });
            self.samples.push_back((now, gateway, internet));
        }
        while self
            .samples
            .front()
            .is_some_and(|a| now.saturating_sub(a.0) >= 120_000)
        {
            self.samples.pop_front();
        }
        let gateway = if self.connection.gateway.is_some() {
            measurements(self.samples.iter().map(|a| a.1))
        } else {
            Measurements::default()
        };
        let internet = measurements(self.samples.iter().map(|a| a.2));
        let quality = connected.then(|| {
            let quality = classify(&gateway, &internet, self.connection.connectivity);
            self.speed.as_ref().map_or(quality, |speed| {
                speed.quality(&self.connection, now, quality)
            })
        });
        if let Some(t) = &traffic {
            self.history.push_back(t.clone());
            while self.history.len() > 60 {
                self.history.pop_front();
            }
        }
        Some((
            now,
            NetworkData {
                advanced_unavailable,
                connection_unavailable,
                link_kind: (connected && advanced_unavailable).then_some(
                    match self.connection.kind.as_str() {
                        "802-3-ethernet" => LinkKind::Wired,
                        "802-11-wireless" => LinkKind::Wireless,
                        _ => LinkKind::Other,
                    },
                ),
                connection_revision: (self.connection_revision > 0)
                    .then_some(self.connection_revision),
                process_traffic: self.processes.as_ref().and_then(|p| p.data.clone()),
                speed_operation: self.speed.as_ref().and_then(|s| s.operation.clone()),
                speed: self.speed.as_ref().and_then(|s| s.last.clone()),
                speed_available: connected
                    && self.connection.connectivity == 4
                    && !self.connection.metered,
                speed_running: self.speed.as_ref().is_some_and(|s| s.running),
                speed_failed: self.speed.as_ref().is_some_and(|s| s.failed),
                automatic_speed: self.speed.as_ref().is_none_or(|s| s.automatic),
                connected,
                wired: self.connection.kind == "802-3-ethernet",
                vpn: self.connection.vpn,
                quality,
                security,
                reasons,
                gateway,
                internet,
                traffic,
                history: self.history.iter().cloned().collect(),
            },
        ))
    }
    fn traffic(&mut self, now: u64) -> Option<Traffic> {
        let current_counters = match self.nm.counters(&self.files, &self.connection) {
            Ok(counters) => counters,
            Err(_) => {
                self.previous = None;
                return None;
            }
        };
        let previous = self.previous.replace((now, current_counters.clone()));
        let (when, previous) = previous?;
        if !self.connection.connected() {
            return None;
        }
        // Count each physical link once; exclude tunnels, VPN and loopback to avoid duplication.
        let name = &self.connection.interface;
        let (rx, tx) = current_counters.get(name)?;
        let (previous_rx, previous_tx) = previous.get(name)?;
        let seconds = now.checked_sub(when)? as f64 / 1000.0;
        if seconds <= 0.0 || rx < previous_rx || tx < previous_tx {
            return None;
        }
        Some(Traffic {
            download: (rx - previous_rx) as f64 / seconds,
            upload: (tx - previous_tx) as f64 / seconds,
        })
    }
}
fn measurements(samples: impl Iterator<Item = Option<f64>>) -> Measurements {
    let samples: Vec<_> = samples.collect();
    if samples.is_empty() {
        return Measurements::default();
    }
    let mut values: Vec<f64> = samples.iter().flatten().copied().collect();
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let latency = if n == 0 {
        None
    } else if n.is_multiple_of(2) {
        Some((values[n / 2 - 1] + values[n / 2]) / 2.0)
    } else {
        Some(values[n / 2])
    };
    let variations: Vec<_> = samples
        .windows(2)
        .filter_map(|p| Some((p[1]? - p[0]?).abs()))
        .collect();
    Measurements {
        latency,
        jitter: (!variations.is_empty())
            .then(|| variations.iter().sum::<f64>() / variations.len() as f64),
        loss: Some((samples.len() - n) as f64 * 100.0 / samples.len() as f64),
    }
}
fn classify(g: &Measurements, i: &Measurements, connectivity: u32) -> Quality {
    let above = |v: Option<f64>, threshold| v.is_some_and(|v| v > threshold);
    if above(g.loss, 5.0)
        || above(i.loss, 5.0)
        || above(i.latency, 300.0)
        || (g.latency.is_some() && (i.latency.is_none() || matches!(connectivity, 1 | 3)))
    {
        Quality::Poor
    } else if above(g.loss, 1.0)
        || above(i.loss, 1.0)
        || above(i.latency, 100.0)
        || above(g.latency, 30.0)
        || above(g.jitter, 30.0)
        || above(i.jitter, 30.0)
    {
        Quality::Regular
    } else {
        Quality::Good
    }
}
pub enum Command {
    Visible(bool),
    Sleep(bool),
    FullSpeed,
    AutomaticSpeed(bool),
}
pub struct Collector(pub std::sync::mpsc::Sender<Command>);
pub fn start(app: tauri::AppHandle) {
    use tauri::Manager;
    let (tx, rx) = std::sync::mpsc::channel();
    app.manage(Collector(tx));
    std::thread::spawn(move || {
        #[cfg(target_os = "linux")]
        let mut network = Network::new(
            crate::ports::RootFilesystem::system(),
            NetworkManagerDbus::connect(),
            PingSocket,
            crate::ports::SystemClock,
        )
        .with_security(SystemCheck(SystemReadings), crate::ports::DesktopNotifier)
        .with_processes(process::KernelDiagnostics);
        #[cfg(windows)]
        let mut network = Network::new(
            crate::ports::RootFilesystem::system(),
            crate::platform::WindowsNetwork,
            crate::platform::WindowsPing,
            crate::ports::SystemClock,
        );
        network = match (dirs::cache_dir(), dirs::state_dir()) {
            (Some(cache), Some(state)) => network.with_speed(
                speed::Worker::new(speed::Cloudflare),
                crate::persistence::LocalPersistence {
                    cache: crate::persistence::Cache(crate::persistence::DiskStorage(
                        cache.join("kvojps-monitor"),
                    )),
                    logs: crate::persistence::Logs(crate::persistence::DiskStorage(
                        state.join("kvojps-monitor"),
                    )),
                },
            ),
            _ => network.with_speed(
                speed::Worker::new(speed::Cloudflare),
                crate::persistence::Volatile,
            ),
        };
        if let Some(config) = dirs::config_dir() {
            network = network.with_preferences(crate::persistence::DiskStorage(
                config.join("kvojps-monitor"),
            ));
        }
        // Publish the cached speed before the first live probe can block.
        let initial = NetworkData {
            advanced_unavailable: !network.nm.advanced_available(),
            connection_unavailable: false,
            link_kind: None,
            connection_revision: None,
            speed_operation: None,
            process_traffic: None,
            connected: false,
            wired: false,
            vpn: false,
            quality: None,
            security: None,
            reasons: vec![],
            gateway: Measurements::default(),
            internet: Measurements::default(),
            traffic: None,
            history: vec![],
            speed: network.speed.as_ref().and_then(|s| s.last.clone()),
            speed_available: false,
            speed_running: false,
            speed_failed: false,
            automatic_speed: network.speed.as_ref().is_none_or(|s| s.automatic),
        };
        crate::snapshot::publish(&app, snapshot(network.clock.now(), &initial));
        let mut sleeping = false;
        loop {
            for command in rx.try_iter() {
                match command {
                    Command::Sleep(value) => {
                        sleeping = value;
                        if sleeping {
                            network.resume();
                        }
                    }
                    Command::Visible(visible) => network.process_visible(visible),
                    Command::FullSpeed => network.request_speed(),
                    Command::AutomaticSpeed(enabled) => network.automatic_speed(enabled),
                }
            }
            if let Some((when, data)) = (!sleeping).then(|| network.step()).flatten() {
                #[cfg(target_os = "linux")]
                if let Some(handle) =
                    tauri::Manager::try_state::<ksni::Handle<crate::indicator::Indicator>>(&app)
                {
                    let risk = data.risk_badge();
                    tauri::async_runtime::block_on(
                        handle.update(move |i| i.state.risk_badge = risk),
                    );
                }
                crate::snapshot::publish(&app, snapshot(when, &data));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}
fn snapshot(when: u64, data: &NetworkData) -> crate::snapshot::Snapshot {
    crate::snapshot::Snapshot {
        module: crate::module::Module::Network,
        updated_at: Some(when),
        data: serde_json::to_value(data).unwrap(),
    }
}
