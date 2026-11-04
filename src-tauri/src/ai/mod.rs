//! AI module: Codex usage limits from a long-lived `codex app-server`, with the
//! newest rollout snapshot as fallback, and Claude limits from the Claude Code
//! status line and, opted in, the `oauth/usage` endpoint.
mod claude;
mod operation;
mod process;
mod usage;
use crate::module::Module;
use crate::persistence::Storage;
use crate::persistence::{Event, LogRecord, Persistence, Volatile};
use crate::ports::{Clock, Files, Programs, Watch};
use crate::snapshot::Snapshot;
pub use claude::{copy_from_stdin, ClaudeError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Starts `codex app-server` (JSON-RPC over stdio).
pub trait AppServer: Send {
    /// Err when the CLI is missing or does not start.
    fn spawn(&mut self) -> Result<Box<dyn Connection>, ()>;
}
pub trait Connection: Send {
    fn send(&mut self, message: &Value) -> Result<(), ()>;
    /// Messages received since the last call, without blocking.
    fn receive(&mut self) -> Vec<Value>;
    fn alive(&mut self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    AppServer,
    Rollout,
    StatusLine,
    OauthUsage,
}

/// One usage window; percent and reset come from the source, never assumed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub used_percent: Option<f64>,
    pub duration_mins: Option<u64>,
    /// Epoch milliseconds.
    pub resets_at: Option<u64>,
    /// The window restarted after this reading; decided at publish time.
    #[serde(default)]
    pub reset: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub id: String,
    pub name: Option<String>,
    pub primary: Option<Window>,
    pub secondary: Option<Window>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub source: Source,
    /// Epoch milliseconds when the source produced the values.
    pub read_at: u64,
    pub buckets: Vec<Bucket>,
}

/// A request still waiting for its answer.
struct PendingRequest {
    id: u64,
    method: &'static str,
    sent_at: u64,
}

pub struct Codex<A, F, R> {
    operations: operation::Operations,
    server: A,
    files: F,
    clock: R,
    connection: Option<Box<dyn Connection>>,
    next_id: u64,
    pending: Vec<PendingRequest>,
    /// Last guarantee or manual query.
    query_at: Option<u64>,
    manual_at: Option<u64>,
    refresh_pending: bool,
    spawn_at: Option<u64>,
    lifecycle: Option<crate::lifecycle::Status>,
    /// Connectivity changed since the last published snapshot.
    dirty: bool,
    persistence: Box<dyn Persistence>,
    /// Consecutive failed queries, for backoff.
    failures: usize,
    failure_since: Option<u64>,
    retry_at: u64,
    plan: Option<String>,
    reading: Option<Reading>,
    /// `codex` started and `account/read` found a login; None until known.
    detected: Option<bool>,
}

impl<A: AppServer, F: Files, R: Clock> Codex<A, F, R> {
    pub fn new(server: A, files: F, clock: R) -> Self {
        Self {
            operations: operation::Operations::default(),
            server,
            files,
            clock,
            connection: None,
            next_id: 1,
            pending: Vec::new(),
            query_at: None,
            manual_at: None,
            refresh_pending: true,
            spawn_at: None,
            lifecycle: None,
            dirty: false,
            persistence: Box::new(Volatile),
            failures: 0,
            failure_since: None,
            retry_at: 0,
            plan: None,
            reading: None,
            detected: None,
        }
    }

    /// Start from the sanitized disk cache, before the server answers.
    pub fn with_persistence(mut self, persistence: impl Persistence + 'static) -> Self {
        if let Some(snapshot) = persistence.load(Module::Ai) {
            let codex = &snapshot.data["codex"];
            self.plan = codex["plan"].as_str().map(str::to_owned);
            self.reading = serde_json::from_value(codex["reading"].clone()).ok();
        }
        self.persistence = Box::new(persistence);
        self
    }

    /// A new reading: cache it and record the event.
    fn save_reading(&mut self, started_at: u64) {
        self.persistence.save(&self.snapshot());
        self.log(Event::Updated, started_at);
    }

    /// Metadata only: never account data, paths or rollout content.
    fn log(&self, event: Event, started_at: u64) {
        self.persistence.log(LogRecord {
            source: Module::Ai,
            event,
            http_status: None,
            duration_ms: self.clock.now().saturating_sub(started_at),
        });
    }

    pub fn snapshot(&self) -> Snapshot {
        let reading = restarted(self.reading.clone(), self.clock.now());
        let state = match (self.detected, &reading) {
            (Some(false), _) => "missing",
            (_, Some(_)) => "ready",
            (_, None) => "empty",
        };
        let mut data = json!({
            "refreshing": self.querying(),
            "codex": {
                "state": state,
                "plan": self.plan,
                "reading": reading,
            },
        });
        if let Some(operation) = &self.operations.current {
            data["codex"]["operation"] = json!(operation);
        }
        if self.failures >= FAILURES_SHOWN {
            data["codex"]["failureSince"] = json!(self.failure_since);
        }
        if self.paused() {
            data["paused"] = json!(true);
        }
        Snapshot {
            module: Module::Ai,
            updated_at: self.reading.as_ref().map(|r| r.read_at),
            data,
        }
    }

    /// Each new connectivity generation (login, reconnect, resume) queries once and
    /// may restart a dead server right away.
    pub fn lifecycle(&mut self, status: crate::lifecycle::Status) {
        let previous = self.lifecycle.replace(status);
        if previous.is_none_or(|p| p.generation != status.generation) {
            self.refresh_pending = true;
            self.query_at = None;
            self.manual_at = None;
            self.spawn_at = None;
            self.retry_at = 0;
        }
        if self.paused() {
            self.pending.retain(|request| request.method != RATE_LIMITS);
            self.operations.pause();
        } else if previous.is_some_and(|p| p.generation != status.generation) {
            self.pending.retain(|request| request.method != RATE_LIMITS);
            self.operations.discard();
        }
        self.dirty |=
            previous.is_none_or(|p| p.online != status.online || p.generation != status.generation);
    }

    /// Offline, or before this source's turn after login/reconnection.
    fn paused(&self) -> bool {
        self.lifecycle.is_some_and(|status| {
            !status.online || self.clock.now() < status.ready_for(SOURCE_POSITION)
        })
    }

    fn querying(&self) -> bool {
        self.pending
            .iter()
            .any(|request| request.method == RATE_LIMITS)
    }

    /// Ask for the limits when due; true when a query was sent.
    fn query(&mut self, visible: bool, manual: bool) -> bool {
        let now = self.clock.now();
        let since =
            |at: Option<u64>, interval: u64| at.is_none_or(|t| now.saturating_sub(t) >= interval);
        if self.connection.is_none() || self.querying() || self.paused() || now < self.retry_at {
            return false;
        }
        let manual = manual && since(self.manual_at, MANUAL_WAIT);
        let interval = if visible { VISIBLE_INTERVAL } else { INTERVAL };
        // After a failure the backoff decides the next attempt.
        let due = self.failures > 0 || since(self.query_at, interval);
        if !(self.refresh_pending || manual || due) {
            return false;
        }
        if manual {
            self.manual_at = Some(now);
        }
        self.refresh_pending = false;
        self.query_at = Some(now);
        self.operations.begin(manual);
        self.request(RATE_LIMITS, Value::Null);
        if !self.querying() {
            self.failed(now);
        }
        true
    }

    /// Returns a snapshot whenever something the module shows may have changed.
    pub fn step(&mut self, visible: bool, manual: bool) -> Option<Snapshot> {
        let mut changed = std::mem::take(&mut self.dirty);
        let now = self.clock.now();
        if self.connection.as_mut().is_some_and(|c| !c.alive()) {
            if self.querying() {
                self.failed(now);
            }
            self.connection = None;
            self.pending.clear();
            changed = true;
        }
        let restart = self
            .spawn_at
            .is_none_or(|at| now.saturating_sub(at) >= RESTART_WAIT);
        if self.connection.is_none() && restart {
            self.spawn_at = Some(now);
            match self.server.spawn() {
                Ok(connection) => self.connection = Some(connection),
                Err(()) => {
                    self.detected = Some(false);
                    self.operations.unavailable();
                    return Some(self.snapshot());
                }
            }
            self.refresh_pending = true;
            self.request(
                "initialize",
                json!({"clientInfo": {"name": "kvojps-monitor", "version": env!("CARGO_PKG_VERSION")}}),
            );
            self.notify("initialized");
            self.request("account/read", json!({}));
            changed = true;
        }
        // Messages may ask for a query; its answer may already be waiting.
        changed |= self.receive();
        if self.query(visible, manual) {
            changed = true;
            self.receive();
        }
        changed |= self.expire();
        changed.then(|| self.snapshot())
    }

    fn receive(&mut self) -> bool {
        let messages = self
            .connection
            .as_mut()
            .map(|c| c.receive())
            .unwrap_or_default();
        let received = !messages.is_empty();
        for message in messages {
            self.handle(message);
        }
        received
    }

    /// Requests unanswered for too long count as failed.
    fn expire(&mut self) -> bool {
        let now = self.clock.now();
        let before = self.pending.len();
        let late = |request: &PendingRequest| now.saturating_sub(request.sent_at) >= ANSWER_TIMEOUT;
        let expired = self
            .pending
            .iter()
            .find(|request| request.method == RATE_LIMITS && late(request))
            .map(|request| request.sent_at);
        if let Some(sent_at) = expired {
            self.failed(sent_at);
        }
        self.pending.retain(|request| !late(request));
        self.pending.len() != before
    }

    fn request(&mut self, method: &'static str, params: Value) {
        let id = self.next_id;
        self.next_id += 1;
        let mut message = json!({"id": id, "method": method});
        if !params.is_null() {
            message["params"] = params;
        }
        if let Some(connection) = &mut self.connection {
            if connection.send(&message).is_ok() {
                self.pending.push(PendingRequest {
                    id,
                    method,
                    sent_at: self.clock.now(),
                });
            }
        }
    }

    /// The server could not read the limits: back off and fall back to the rollouts.
    fn failed(&mut self, started_at: u64) {
        let now = self.clock.now();
        self.failure_since.get_or_insert(now);
        self.failures += 1;
        self.operations.failure();
        self.retry_at = now.saturating_add(BACKOFF[(self.failures - 1).min(BACKOFF.len() - 1)]);
        self.log(Event::Failed, started_at);
        if self.fall_back() {
            self.persistence.save(&self.snapshot());
        }
    }

    /// Use the newest rollout snapshot when it is newer than the current reading.
    fn fall_back(&mut self) -> bool {
        let Some((reading, plan)) = newest_rollout(&self.files) else {
            return false;
        };
        if self
            .reading
            .as_ref()
            .is_none_or(|current| current.read_at < reading.read_at)
        {
            if let Some(plan) = plan {
                self.plan.get_or_insert(plan);
            }
            self.reading = Some(reading);
            return true;
        }
        false
    }

    /// Server notifications. Rate limit updates are sparse: missing values keep
    /// the last known ones of an app-server reading. Without one, the full
    /// snapshot is read instead, so rollout values never look fresh.
    fn on_notification(&mut self, message: &Value) {
        let params = &message["params"];
        let plan = match message["method"].as_str() {
            Some("account/updated") => &params["planType"],
            Some("account/rateLimits/updated") => {
                let snapshot = &params["rateLimits"];
                let from_server = self
                    .reading
                    .as_ref()
                    .is_some_and(|r| r.source == Source::AppServer);
                match bucket(snapshot, &SERVER_KEYS) {
                    Some(update) if from_server => {
                        self.merge(update);
                        self.save_reading(self.clock.now());
                    }
                    // The server has news: read it now, even during backoff.
                    _ => {
                        self.refresh_pending = true;
                        self.retry_at = 0;
                    }
                }
                &snapshot["planType"]
            }
            _ => return,
        };
        if let Some(plan) = plan.as_str() {
            self.plan = Some(plan.to_owned());
        }
    }

    fn merge(&mut self, update: Bucket) {
        let Some(reading) = &mut self.reading else {
            return;
        };
        reading.read_at = self.clock.now();
        match reading.buckets.iter_mut().find(|b| b.id == update.id) {
            Some(bucket) => {
                bucket.name = update.name.or(bucket.name.take());
                bucket.primary = update.primary.or(bucket.primary.take());
                bucket.secondary = update.secondary.or(bucket.secondary.take());
            }
            None => reading.buckets.push(update),
        }
    }

    fn notify(&mut self, method: &str) {
        if let Some(connection) = &mut self.connection {
            let _ = connection.send(&json!({"method": method}));
        }
    }

    fn handle(&mut self, message: Value) {
        let Some(id) = message["id"].as_u64() else {
            return self.on_notification(&message);
        };
        let Some(index) = self.pending.iter().position(|request| request.id == id) else {
            return;
        };
        let PendingRequest {
            method, sent_at, ..
        } = self.pending.remove(index);
        let result = &message["result"];
        match method {
            "account/read" => {
                let account = &result["account"];
                self.detected = Some(account.is_object());
                if let Some(plan) = account["planType"].as_str() {
                    self.plan = Some(plan.to_owned());
                }
            }
            RATE_LIMITS if !result.is_object() => self.failed(sent_at),
            RATE_LIMITS => {
                let buckets = match result["rateLimitsByLimitId"].as_object() {
                    Some(map) => map
                        .values()
                        .filter_map(|b| bucket(b, &SERVER_KEYS))
                        .collect(),
                    None => bucket(&result["rateLimits"], &SERVER_KEYS)
                        .into_iter()
                        .collect(),
                };
                self.reading = Some(Reading {
                    source: Source::AppServer,
                    read_at: self.clock.now(),
                    buckets,
                });
                if self.detected == Some(false)
                    || self.reading.as_ref().is_none_or(|r| {
                        r.buckets
                            .iter()
                            .all(|b| b.primary.is_none() && b.secondary.is_none())
                    })
                {
                    self.operations.finish(operation::State::Unavailable);
                } else {
                    self.operations.success();
                }
                self.failures = 0;
                self.failure_since = None;
                self.retry_at = 0;
                self.save_reading(sent_at);
            }
            _ => {}
        }
    }
}

/// Windows whose reset already passed lose their percent: never assumed 0%.
/// Decided at publish time so a cached reading turns into a restart without a query.
fn restarted(reading: Option<Reading>, now: u64) -> Option<Reading> {
    reading.map(|mut reading| {
        for bucket in &mut reading.buckets {
            for window in [&mut bucket.primary, &mut bucket.secondary]
                .into_iter()
                .flatten()
            {
                if window.resets_at.is_some_and(|at| at <= now) {
                    window.reset = true;
                    window.used_percent = None;
                }
            }
        }
        reading
    })
}

/// The AI snapshot: Codex with the Claude part beside it.
fn combine<S: Storage, P: Programs, W: Watch, R: Clock>(
    snapshot: Snapshot,
    claude: &claude::Claude<S, P, W, R>,
) -> Snapshot {
    combine_data(snapshot, claude.data())
}

/// Compose both intermediate and final provider snapshots through one contract.
fn combine_data(mut snapshot: Snapshot, claude: Value) -> Snapshot {
    snapshot.updated_at = snapshot
        .updated_at
        .max(claude["reading"]["readAt"].as_u64());
    snapshot.data["claude"] = claude;
    snapshot
}

/// The disk persistence and the last entry saved through it.
type CacheEntry = (Box<dyn Persistence>, Option<Snapshot>);
/// Codex and Claude each save their own part of the one AI cache entry.
#[derive(Clone)]
pub struct SharedCache(std::sync::Arc<std::sync::Mutex<CacheEntry>>);
impl SharedCache {
    pub fn new(persistence: impl Persistence + 'static) -> Self {
        let cached = persistence.load(Module::Ai);
        Self(std::sync::Arc::new(std::sync::Mutex::new((
            Box::new(persistence),
            cached,
        ))))
    }
}
impl Persistence for SharedCache {
    fn load(&self, module: Module) -> Option<Snapshot> {
        self.0.lock().unwrap().0.load(module)
    }
    fn save(&self, snapshot: &Snapshot) {
        let mut guard = self.0.lock().unwrap();
        let (persistence, last) = &mut *guard;
        let mut merged = last.take().unwrap_or_else(|| Snapshot {
            module: Module::Ai,
            updated_at: None,
            data: json!({}),
        });
        for key in ["codex", "claude"] {
            if let Some(part) = snapshot.data.get(key) {
                merged.data[key] = part.clone();
            }
        }
        merged.updated_at = merged.updated_at.max(snapshot.updated_at);
        persistence.save(&merged);
        *last = Some(merged);
    }
    fn clear(&self, module: Module) -> std::io::Result<()> {
        let mut guard = self.0.lock().unwrap();
        guard.0.clear(module)?;
        guard.1 = None;
        Ok(())
    }
    fn log(&self, record: LogRecord) {
        self.0.lock().unwrap().0.log(record);
    }
}

/// Field names of one rate limit snapshot: the app-server uses camelCase,
/// rollouts snake_case, with the same meaning.
struct Keys {
    id: &'static str,
    name: &'static str,
    used_percent: &'static str,
    duration_mins: &'static str,
    resets_at: &'static str,
}
const SERVER_KEYS: Keys = Keys {
    id: "limitId",
    name: "limitName",
    used_percent: "usedPercent",
    duration_mins: "windowDurationMins",
    resets_at: "resetsAt",
};
const ROLLOUT_KEYS: Keys = Keys {
    id: "limit_id",
    name: "limit_name",
    used_percent: "used_percent",
    duration_mins: "window_minutes",
    resets_at: "resets_at",
};

fn bucket(snapshot: &Value, keys: &Keys) -> Option<Bucket> {
    let window = |window: &Value| {
        Some(Window {
            used_percent: Some(window[keys.used_percent].as_f64()?),
            duration_mins: window[keys.duration_mins].as_u64(),
            // Epoch seconds in both sources.
            resets_at: window[keys.resets_at]
                .as_u64()
                .map(|s| s.saturating_mul(1000)),
            reset: false,
        })
    };
    Some(Bucket {
        id: snapshot[keys.id].as_str()?.to_owned(),
        name: snapshot[keys.name].as_str().map(str::to_owned),
        primary: window(&snapshot["primary"]),
        secondary: window(&snapshot["secondary"]),
    })
}

const RATE_LIMITS: &str = "account/rateLimits/read";
/// Guarantee query with the module hidden, and while it is visible.
const INTERVAL: u64 = 300_000;
const VISIBLE_INTERVAL: u64 = 60_000;
const MANUAL_WAIT: u64 = 10_000;
const ANSWER_TIMEOUT: u64 = 30_000;
const RESTART_WAIT: u64 = 30_000;
/// Order among remote sources after login or reconnection (GitHub 0, Trello 1).
const SOURCE_POSITION: u64 = 2;
/// "Falha ao atualizar há X" after this many consecutive failures.
const FAILURES_SHOWN: usize = 3;
/// Failed queries: 30 s, then doubling up to 15 min.
const BACKOFF: [u64; 6] = [30_000, 60_000, 120_000, 240_000, 480_000, 900_000];
/// Rollout files read per fallback: enough to skip sessions without windows.
const ROLLOUT_FILES: usize = 10;

/// The newest rollout snapshot with a window among the latest sessions, and its
/// plan. Codex writes one `token_count` event with `rate_limits` per turn; a
/// session started earlier may hold the newest one.
fn newest_rollout(files: &impl Files) -> Option<(Reading, Option<String>)> {
    let newest_first = |dir: &str| {
        let mut names = files.list(dir).unwrap_or_default();
        names.sort_unstable_by(|a, b| b.cmp(a));
        names
    };
    let mut paths = Vec::new();
    'days: for year in newest_first("/sessions") {
        for month in newest_first(&format!("/sessions/{year}")) {
            for day in newest_first(&format!("/sessions/{year}/{month}")) {
                let dir = format!("/sessions/{year}/{month}/{day}");
                for name in newest_first(&dir) {
                    if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                        paths.push(format!("{dir}/{name}"));
                    }
                }
                if paths.len() >= ROLLOUT_FILES {
                    break 'days;
                }
            }
        }
    }
    paths.truncate(ROLLOUT_FILES);
    paths
        .iter()
        .filter_map(|path| {
            let content = files.read(path).ok()?;
            content.lines().rev().find_map(rollout_reading)
        })
        .max_by_key(|(reading, _)| reading.read_at)
}

fn rollout_reading(line: &str) -> Option<(Reading, Option<String>)> {
    // Most lines carry conversation content; skip them before parsing.
    if !line.contains("\"token_count\"") {
        return None;
    }
    let event: Value = serde_json::from_str(line).ok()?;
    let limits = &event["payload"]["rate_limits"];
    if event["payload"]["type"] != "token_count" || !limits.is_object() {
        return None;
    }
    let bucket = bucket(limits, &ROLLOUT_KEYS)?;
    if bucket.primary.is_none() && bucket.secondary.is_none() {
        return None;
    }
    let read_at = chrono::DateTime::parse_from_rfc3339(event["timestamp"].as_str()?)
        .ok()?
        .timestamp_millis()
        .max(0) as u64;
    let plan = limits["plan_type"].as_str().map(str::to_owned);
    Some((
        Reading {
            source: Source::Rollout,
            read_at,
            buckets: vec![bucket],
        },
        plan,
    ))
}

#[cfg(test)]
mod tests;

pub enum Command {
    Refresh,
    ConnectClaude(tokio::sync::oneshot::Sender<Result<(), ClaudeError>>),
    DisconnectClaude(tokio::sync::oneshot::Sender<Result<(), ClaudeError>>),
    /// The `oauth/usage` opt-in, only on the user's click.
    SetClaudeUsage(bool, tokio::sync::oneshot::Sender<Result<(), ClaudeError>>),
}
pub struct Collector(pub std::sync::mpsc::Sender<Command>);
pub fn start(app: tauri::AppHandle) {
    use tauri::Manager;
    let (tx, rx) = std::sync::mpsc::channel();
    app.manage(Collector(tx));
    std::thread::spawn(move || {
        let home = dirs::home_dir().unwrap_or_default();
        let mut codex = Codex::new(
            process::CodexCli,
            crate::ports::RootFilesystem::at(home.join(".codex")),
            crate::ports::SystemClock,
        );
        let path = |dir: Option<std::path::PathBuf>, rest: &str| {
            dir.unwrap_or_else(|| home.clone())
                .join(rest)
                .to_string_lossy()
                .into_owned()
        };
        let locations = claude::Locations {
            settings: path(Some(home.join(".claude")), "settings.json"),
            credentials: path(Some(home.join(".claude")), ".credentials.json"),
            config: path(dirs::config_dir(), "kvojps-monitor/claude-statusline.json"),
            wrapper: path(
                Some(home.clone()),
                ".local/share/kvojps-monitor/claude-statusline",
            ),
            cache: path(dirs::cache_dir(), "kvojps-monitor/claude-statusline.json"),
            // An AppImage runs from a temporary mount: point at the AppImage file.
            executable: std::env::var_os("APPIMAGE")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::current_exe().ok())
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            usage_choice: path(dirs::config_dir(), "kvojps-monitor/claude-usage.json"),
        };
        let watch = crate::ports::FileWatch::new(std::path::Path::new(&locations.cache));
        let mut claude = claude::Claude::new(
            crate::persistence::DiskStorage("/".into()),
            crate::ports::SystemPrograms,
            watch,
            crate::ports::SystemClock,
            locations,
        )
        .with_usage(usage::AnthropicUsage)
        .with_lifecycle(app.state::<crate::lifecycle::Shared>().inner().clone());
        if let (Some(cache), Some(state)) = (dirs::cache_dir(), dirs::state_dir()) {
            use crate::persistence::{Cache, DiskStorage, LocalPersistence, Logs};
            let shared = SharedCache::new(LocalPersistence {
                cache: Cache(DiskStorage(cache.join("kvojps-monitor"))),
                logs: Logs(DiskStorage(state.join("kvojps-monitor"))),
            });
            codex = codex.with_persistence(shared.clone());
            claude = claude.with_persistence(shared);
        }
        crate::snapshot::publish(&app, combine(codex.snapshot(), &claude));
        loop {
            // Short wait: the server pushes updates between commands.
            let command = rx.recv_timeout(std::time::Duration::from_millis(500)).ok();
            let manual = matches!(command, Some(Command::Refresh));
            let mut changed = false;
            match command {
                Some(Command::ConnectClaude(reply)) => {
                    let _ = reply.send(claude.connect());
                    changed = true;
                }
                Some(Command::DisconnectClaude(reply)) => {
                    let _ = reply.send(claude.disconnect());
                    changed = true;
                }
                Some(Command::SetClaudeUsage(enabled, reply)) => {
                    let _ = reply.send(claude.set_usage(enabled));
                    changed = true;
                }
                _ => {}
            }
            let status = *app.state::<crate::lifecycle::Shared>().lock().unwrap();
            codex.lifecycle(status);
            claude.lifecycle(status);
            let visible = crate::collection::is_visible(&app, Module::Ai);
            changed |= codex.step(visible, manual).is_some();
            if changed {
                crate::snapshot::publish(&app, combine(codex.snapshot(), &claude));
            }
            changed |= claude.step_with_progress(manual, |data| {
                crate::snapshot::publish(&app, combine_data(codex.snapshot(), data));
            });
            if changed {
                crate::snapshot::publish(&app, combine(codex.snapshot(), &claude));
            }
        }
    });
}
