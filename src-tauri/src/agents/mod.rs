//! Local Claude and Codex session polling. Session metadata stays in memory only.
#[cfg_attr(
    windows,
    allow(dead_code, reason = "Linux rollout discovery is not used on Windows")
)]
mod codex;
#[cfg(target_os = "linux")]
pub(crate) mod desktop;
pub mod hooks;
pub mod notifications;
use crate::ports::{Clock, Processes};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionState {
    Working,
    Waiting,
    Done,
    Failed,
    Closed,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Provider {
    Claude,
    Codex,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    #[serde(skip)]
    interrupted: bool,
    #[serde(skip)]
    turn_duration: Option<u64>,
    id: String,
    provider: Provider,
    branch: Option<String>,
    updated_at: Option<u64>,
    project: String,
    name: Option<String>,
    state: SessionState,
    waiting_for: Option<String>,
    started_at: u64,
    closed_at: Option<u64>,
    last_state: Option<SessionState>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    discovery_partial: bool,
    hooks_enabled: bool,
    sessions: Vec<Session>,
    closed: Vec<Session>,
    notification_preferences: notifications::Preferences,
    notifications_available: bool,
    accent_dot: bool,
    failed: bool,
    observed_at: u64,
    /// Time of the last successful poll (hook events do not count); sessions shown
    /// after a failure are this old.
    last_success_at: Option<u64>,
    codex_waiting_unavailable: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    id: Option<String>,
    session_id: Option<String>,
    pid: Option<u64>,
    cwd: String,
    name: Option<String>,
    started_at: u64,
    status: Option<String>,
    state: Option<String>,
    waiting_for: Option<String>,
}
impl Record {
    fn session(self) -> Result<(Session, bool), ()> {
        let state = match self.status.as_deref().or(self.state.as_deref()) {
            Some("busy" | "working") => SessionState::Working,
            Some("waiting" | "blocked") => SessionState::Waiting,
            Some("idle" | "done" | "stopped") => SessionState::Done,
            Some("failed") => SessionState::Failed,
            _ => return Err(()),
        };
        let terminal = self.pid.is_none()
            && matches!(self.state.as_deref(), Some("done" | "failed" | "stopped"));
        let id = self
            .session_id
            .or(self.id)
            .or_else(|| self.pid.map(|pid| format!("{pid}:{}", self.started_at)))
            .ok_or(())?;
        Ok((
            Session {
                interrupted: matches!(
                    self.status.as_deref().or(self.state.as_deref()),
                    Some("stopped" | "interrupted")
                ),
                turn_duration: None,
                id,
                provider: Provider::Claude,
                branch: None,
                updated_at: None,
                project: std::path::Path::new(&self.cwd)
                    .file_name()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or(self.cwd),
                name: self.name,
                state,
                waiting_for: self.waiting_for,
                started_at: self.started_at,
                closed_at: None,
                last_state: None,
            },
            !terminal,
        ))
    }
}
struct Observation {
    started_at: u64,
    claude: Result<Vec<(Session, bool)>, ()>,
    codex: Option<Result<Vec<(Session, bool)>, ()>>,
}

pub struct Monitor<P, C> {
    discovery_partial: bool,
    hooks_enabled: bool,
    hooked_at: std::collections::HashMap<String, u64>,
    notifications: notifications::Policy,
    codex: Option<Box<dyn codex::Source>>,
    process: P,
    clock: C,
    next: Option<u64>,
    sessions: Vec<Session>,
    closed: Vec<Session>,
    last_success_at: Option<u64>,
}
impl<P: Processes, C: Clock> Monitor<P, C> {
    pub fn new(process: P, clock: C) -> Self {
        Self {
            discovery_partial: false,
            hooks_enabled: false,
            hooked_at: Default::default(),
            notifications: Default::default(),
            codex: None,
            process,
            clock,
            next: None,
            sessions: vec![],
            closed: vec![],
            last_success_at: None,
        }
    }
    pub fn with_notifications(mut self, sink: impl notifications::Sink + 'static) -> Self {
        self.notifications = notifications::Policy::new(sink);
        self
    }
    #[cfg(target_os = "linux")]
    fn with_codex(mut self, source: impl codex::Source + 'static) -> Self {
        self.codex = Some(Box::new(source));
        self
    }
    pub fn set_notification_preferences(&mut self, preferences: notifications::Preferences) {
        self.notifications.preferences = preferences;
    }
    fn hook(&mut self, event: hooks::Metadata) -> Data {
        let now = self.clock.now();
        let id = if event.provider == Provider::Codex {
            format!("codex:{}", event.id)
        } else {
            event.id
        };
        self.hooked_at.insert(id.clone(), now);
        let previous = self.sessions.clone();
        let index = self
            .sessions
            .iter()
            .position(|s| s.id == id && s.provider == event.provider);
        if event.state == SessionState::Closed {
            if let Some(index) = index {
                let mut session = self.sessions.remove(index);
                session.last_state = Some(session.state);
                session.state = SessionState::Closed;
                session.closed_at = Some(now);
                session.waiting_for = None;
                self.closed.push(session);
            }
        } else {
            let mut session = index
                .map(|i| self.sessions[i].clone())
                .unwrap_or_else(|| Session {
                    id: id.clone(),
                    provider: event.provider,
                    state: event.state,
                    project: std::path::Path::new(&event.cwd)
                        .file_name()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or(event.cwd),
                    name: None,
                    branch: None,
                    started_at: now,
                    updated_at: None,
                    closed_at: None,
                    last_state: None,
                    waiting_for: None,
                    interrupted: false,
                    turn_duration: None,
                });
            session.state = event.state;
            session.waiting_for = if event.state == SessionState::Waiting {
                event.tool
            } else {
                None
            };
            session.interrupted = event.interrupted;
            session.turn_duration = None;
            if let Some(index) = index {
                self.sessions[index] = session;
            } else {
                self.sessions.push(session);
            }
            self.closed.retain(|s| s.id != id);
        }
        self.notifications.observe(&previous, &self.sessions, now);
        self.data(false)
    }
    #[cfg(test)]
    pub fn step(&mut self) -> Option<Data> {
        let observation = self.poll()?;
        Some(self.observe(observation))
    }
    fn poll(&mut self) -> Option<Observation> {
        let now = self.clock.now();
        if self.next.is_some_and(|next| now < next) {
            return None;
        }
        self.next = Some(now.saturating_add(2000));
        Some(self.collect())
    }
    fn collect(&self) -> Observation {
        let started_at = self.clock.now();
        let result = self
            .process
            .execute("claude", &["agents", "--json", "--all"])
            .and_then(|json| serde_json::from_str::<Vec<Record>>(&json).map_err(|_| ()))
            .and_then(|records| {
                records
                    .into_iter()
                    .map(Record::session)
                    .collect::<Result<Vec<_>, _>>()
            });
        let codex = self.codex.as_ref().map(|source| {
            source.sessions(
                &self
                    .sessions
                    .iter()
                    .filter(|s| s.provider == Provider::Codex)
                    .map(|s| s.id.clone())
                    .collect::<Vec<_>>(),
            )
        });
        Observation {
            started_at,
            claude: result,
            codex,
        }
    }
    fn observe(&mut self, observation: Observation) -> Data {
        let now = self.clock.now();
        let result = observation.claude;
        let mut failed = result.is_err();
        let mut sessions = result.unwrap_or_else(|_| {
            self.sessions
                .iter()
                .filter(|s| s.provider == Provider::Claude)
                .cloned()
                .map(|s| (s, true))
                .collect()
        });
        if let Some(result) = observation.codex {
            match result {
                Ok(records) => sessions.extend(records),
                Err(()) => {
                    failed = true;
                    sessions.extend(
                        self.sessions
                            .iter()
                            .filter(|s| s.provider == Provider::Codex)
                            .cloned()
                            .map(|s| (s, true)),
                    );
                }
            }
        }
        // A hook received while I/O was in flight is newer than this sample.
        self.hooked_at.retain(|_, at| *at >= observation.started_at);
        sessions.retain(|(session, _)| !self.hooked_at.contains_key(&session.id));
        sessions.extend(
            self.sessions
                .iter()
                .filter(|s| self.hooked_at.contains_key(&s.id))
                .cloned()
                .map(|s| (s, true)),
        );
        // Codex polling has no waiting state: retain a hook's approval state while
        // the rollout still reports an active turn. Terminal outcomes win.
        if self.hooks_enabled {
            for (session, live) in &mut sessions {
                if *live
                    && session.provider == Provider::Codex
                    && session.state == SessionState::Working
                {
                    if let Some(previous) = self
                        .sessions
                        .iter()
                        .find(|s| s.id == session.id && s.state == SessionState::Waiting)
                    {
                        session.state = SessionState::Waiting;
                        session.waiting_for = previous.waiting_for.clone();
                    }
                }
            }
        }
        {
            let outcomes: std::collections::HashMap<_, _> = sessions
                .iter()
                .map(|(s, _)| (s.id.clone(), s.state))
                .collect();
            // Terminal background jobs without a live process are historical.
            let mut live = vec![];
            for (session, live_process) in sessions.drain(..) {
                if live_process {
                    live.push(session);
                }
            }
            for previous in &self.sessions {
                if !live.iter().any(|s| s.id == previous.id) {
                    let mut closed = previous.clone();
                    closed.last_state = Some(
                        outcomes
                            .get(&previous.id)
                            .copied()
                            .unwrap_or(previous.state),
                    );
                    closed.state = SessionState::Closed;
                    closed.closed_at = Some(now);
                    closed.waiting_for = None;
                    self.closed.push(closed);
                }
            }
            self.closed
                .retain(|s| !live.iter().any(|active| active.id == s.id));
            self.notifications.observe(&self.sessions, &live, now);
            self.sessions = live;
        }
        if !failed {
            self.last_success_at = Some(now);
        }
        self.data(failed)
    }
    fn data(&mut self, failed: bool) -> Data {
        let now = self.clock.now();
        self.closed
            .retain(|s| now.saturating_sub(s.closed_at.unwrap()) < 7_200_000);
        self.closed.sort_by_key(|s| std::cmp::Reverse(s.closed_at));
        self.closed.truncate(5);
        self.sessions
            .sort_by_key(|s| (s.state != SessionState::Waiting, s.started_at, s.id.clone()));
        Data {
            discovery_partial: self.discovery_partial,
            notifications_available: self.notifications.available(),
            notification_preferences: self.notifications.preferences,
            accent_dot: self
                .sessions
                .iter()
                .any(|s| s.state == SessionState::Waiting),
            sessions: self.sessions.clone(),
            closed: self.closed.clone(),
            failed,
            observed_at: now,
            last_success_at: self.last_success_at,
            hooks_enabled: self.hooks_enabled,
            codex_waiting_unavailable: !self.hooks_enabled,
        }
    }
}
pub struct Collector(
    pub std::sync::mpsc::Sender<(bool, tokio::sync::oneshot::Sender<Result<(), String>>)>,
);

pub fn start(app: tauri::AppHandle) {
    use crate::ports::HookSocket;
    use tauri::Manager;
    let (sender, receiver) =
        std::sync::mpsc::channel::<(bool, tokio::sync::oneshot::Sender<Result<(), String>>)>();
    app.manage(Collector(sender));
    std::thread::spawn(move || {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let executable = std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut configuration =
            hooks::Configuration::new(crate::persistence::DiskStorage(home.clone()), executable);
        let mut socket =
            hooks::socket_path().and_then(|path| crate::ports::NativeHookSocket::bind(&path).ok());
        let (observations, tracked) = polling(home.join(".codex"));
        let mut monitor = Monitor::new(crate::ports::SystemProcesses, crate::ports::SystemClock)
            .with_notifications(crate::platform::agent_notifications(app.clone()));
        #[cfg(windows)]
        {
            monitor.discovery_partial = true;
        }
        monitor.hooks_enabled = configuration.enabled();
        loop {
            for (enabled, reply) in receiver.try_iter() {
                let result = if enabled && socket.is_none() {
                    Err("socketUnavailable".to_string())
                } else {
                    configuration
                        .set_enabled(enabled)
                        .map_err(|_| "configurationFailed".to_string())
                };
                monitor.hooks_enabled = configuration.enabled();
                publish(&app, monitor.data(false));
                let _ = reply.send(result);
            }
            let preferences = app
                .state::<std::sync::Mutex<crate::state::AppState>>()
                .lock()
                .unwrap()
                .agent_notifications();
            monitor.set_notification_preferences(preferences);
            for observation in observations.try_iter() {
                publish(&app, monitor.observe(observation));
            }
            if let Some(socket) = &mut socket {
                for _ in 0..64 {
                    let Some(bytes) = socket.receive() else {
                        break;
                    };
                    if monitor.hooks_enabled {
                        if let Ok(event) = serde_json::from_slice::<hooks::Metadata>(&bytes) {
                            publish(&app, monitor.hook(event));
                        }
                    }
                }
            }
            *tracked.lock().unwrap() = monitor.sessions.clone();
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    });
}
/// Slow subprocess/SQLite reads never hold the event loop that handles hooks.
fn polling(
    root: std::path::PathBuf,
) -> (
    std::sync::mpsc::Receiver<Observation>,
    std::sync::Arc<std::sync::Mutex<Vec<Session>>>,
) {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let tracked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let worker_tracked = tracked.clone();
    std::thread::spawn(move || {
        #[cfg(target_os = "linux")]
        let mut poller =
            Monitor::new(crate::ports::SystemProcesses, crate::ports::SystemClock).with_codex(
                codex::Adapter::new(root, crate::ports::RootFilesystem::system()),
            );
        #[cfg(windows)]
        let mut poller = Monitor::new(crate::ports::SystemProcesses, crate::ports::SystemClock);
        #[cfg(windows)]
        let _ = root;
        loop {
            poller.sessions = worker_tracked.lock().unwrap().clone();
            if let Some(observation) = poller.poll() {
                if sender.send(observation).is_err() {
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });
    (receiver, tracked)
}
fn publish(app: &tauri::AppHandle, data: Data) {
    #[cfg(target_os = "linux")]
    if let Some(handle) =
        tauri::Manager::try_state::<ksni::Handle<crate::indicator::Indicator>>(app)
    {
        let dot = data.accent_dot;
        tauri::async_runtime::block_on(handle.update(move |i| i.state.accent_dot = dot));
    }
    crate::snapshot::publish(
        app,
        crate::snapshot::Snapshot {
            module: crate::module::Module::Agents,
            updated_at: Some(data.observed_at),
            data: serde_json::to_value(data).unwrap(),
        },
    );
}
#[cfg(test)]
mod tests;
