//! Claude usage limits from the Claude Code status line. After the user's click,
//! a wrapper chains their own status line and copies only `rate_limits` to a
//! cache file that this collector watches. Opted in, the undocumented
//! `oauth/usage` endpoint is read too, and each window shows its newest reading.
use super::usage::{self, Token, UsageApi};
use super::{restarted, Bucket, Reading, Source, Window};
use crate::module::Module;
use crate::persistence::{Event, LogRecord, Persistence, Storage, Volatile};
use crate::ports::{Clock, HttpError, Programs, Watch};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Files the collector reads or writes, as paths for its storage.
pub struct Locations {
    /// The user's `~/.claude/settings.json`.
    pub settings: String,
    pub credentials: String,
    /// App configuration that keeps the original status line.
    pub config: String,
    pub wrapper: String,
    /// Copy of `rate_limits` written by the wrapper.
    pub cache: String,
    /// Program the wrapper runs as `<executable> statusline <cache>`.
    pub executable: String,
    /// App configuration that keeps the `oauth/usage` opt-in.
    pub usage_choice: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaudeError {
    /// `settings.json` is not a JSON object; nothing was changed.
    InvalidSettings,
    Unwritable,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusLine {
    Off,
    Connected,
    /// Another tool changed `statusLine` after connecting; never rewritten here.
    Replaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Usage {
    Unsupported,
    Off,
    On,
    /// The token expired or was refused: the status line is shown until
    /// Claude Code renews it. The app never does.
    Expired,
}

/// The `oauth/usage` opt-in, kept in the app configuration.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageChoice {
    enabled: bool,
    /// Expiry of the refused token, so a restart never sends it again.
    /// Metadata only: the token itself is never stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refused_expires_at: Option<u64>,
}

/// Kept in the app configuration while connected.
#[derive(Serialize, Deserialize)]
struct Connection {
    /// `statusLine` before connecting; None when there was none.
    original: Option<Value>,
}

/// The only fields copied from the status line payload.
#[derive(Debug, Serialize, Deserialize)]
struct StoredLimits {
    /// Epoch milliseconds.
    read_at: u64,
    rate_limits: RateLimits,
}
#[derive(Debug, Serialize, Deserialize)]
struct RateLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    five_hour: Option<Limit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seven_day: Option<Limit>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Limit {
    used_percentage: f64,
    /// Epoch seconds.
    resets_at: u64,
}

/// The copy the wrapper stores for one status line payload; None without windows.
pub fn extract(payload: &str, now: u64) -> Option<String> {
    #[derive(Deserialize)]
    struct Payload {
        rate_limits: Option<RateLimits>,
    }
    let rate_limits = serde_json::from_str::<Payload>(payload).ok()?.rate_limits?;
    if rate_limits.five_hour.is_none() && rate_limits.seven_day.is_none() {
        return None;
    }
    serde_json::to_string(&StoredLimits {
        read_at: now,
        rate_limits,
    })
    .ok()
}

/// `statusline <cache>` mode of the app binary, run by the wrapper.
pub fn copy_from_stdin(cache: &std::path::Path) {
    use crate::ports::{Clock, HookInput};
    let Ok(payload) = crate::ports::TimedStdin.read(1000, 1_048_576) else {
        return;
    };
    let Some(copy) = extract(&payload, crate::ports::SystemClock.now()) else {
        return;
    };
    let temporary = cache.with_extension("json.tmp");
    let _ = cache
        .parent()
        .map(std::fs::create_dir_all)
        .unwrap_or(Ok(()))
        .and_then(|_| std::fs::write(&temporary, copy))
        .and_then(|_| std::fs::rename(&temporary, cache));
}

/// POSIX shell single quotes.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

pub struct Claude<S, P, W, R> {
    storage: S,
    programs: P,
    watch: W,
    clock: R,
    locations: Locations,
    persistence: Box<dyn Persistence>,
    /// `claude` in PATH and local credentials; None until checked.
    detected: Option<bool>,
    plan: Option<String>,
    status_line: StatusLine,
    /// Newest status line reading.
    reading: Option<Reading>,
    checked_at: Option<u64>,
    stepped_at: u64,
    api: Box<dyn UsageApi>,
    operations: super::operation::Operations,
    /// The user opted in to `oauth/usage`; read from the app configuration.
    usage_enabled: bool,
    usage_supported: bool,
    #[cfg(windows)]
    bash: Option<String>,
    usage_unavailable: bool,
    lifecycle_source: Option<Box<dyn crate::lifecycle::CurrentStatus>>,
    /// Newest `oauth/usage` reading.
    usage_reading: Option<Reading>,
    /// Identity of a token that expired or was refused; never sent again.
    rejected: Option<u64>,
    usage_pending: bool,
    usage_at: Option<u64>,
    manual_at: Option<u64>,
    failures: usize,
    failure_since: Option<u64>,
    retry_at: u64,
    lifecycle: Option<crate::lifecycle::Status>,
}

impl<S: Storage, P: Programs, W: Watch, R: Clock> Claude<S, P, W, R> {
    pub fn new(storage: S, programs: P, watch: W, clock: R, locations: Locations) -> Self {
        Self {
            storage,
            programs,
            watch,
            clock,
            locations,
            persistence: Box::new(Volatile),
            detected: None,
            plan: None,
            status_line: StatusLine::Off,
            reading: None,
            checked_at: None,
            stepped_at: 0,
            api: Box::new(usage::NoUsage),
            operations: super::operation::Operations::default(),
            usage_enabled: false,
            usage_supported: true,
            #[cfg(windows)]
            bash: None,
            usage_unavailable: false,
            lifecycle_source: None,
            usage_reading: None,
            rejected: None,
            usage_pending: true,
            usage_at: None,
            manual_at: None,
            failures: 0,
            failure_since: None,
            retry_at: 0,
            lifecycle: None,
        }
    }

    pub fn with_lifecycle(
        mut self,
        source: impl crate::lifecycle::CurrentStatus + 'static,
    ) -> Self {
        self.lifecycle_source = Some(Box::new(source));
        self
    }

    /// Native Windows uses only status line input; do not inspect OAuth storage.
    #[cfg(windows)]
    pub fn for_windows(mut self, bash: Option<String>) -> Self {
        self.usage_supported = false;
        self.bash = bash;
        self
    }

    /// The `oauth/usage` endpoint, used only after the user opts in.
    #[cfg_attr(
        windows,
        allow(dead_code, reason = "Optional OAuth remains unsupported on Windows")
    )]
    pub fn with_usage(mut self, api: impl UsageApi + 'static) -> Self {
        self.api = Box::new(api);
        let choice = self.usage_choice();
        self.usage_enabled = choice.enabled;
        // Still the refused token: Claude Code has not renewed it yet.
        if let (true, Some(refused)) = (choice.enabled, choice.refused_expires_at) {
            self.rejected = self
                .token()
                .filter(|token| token.expires_at == Some(refused))
                .map(|token| token.id());
        }
        self
    }

    fn usage_choice(&self) -> UsageChoice {
        self.storage
            .read(&self.locations.usage_choice)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Read for one request or comparison, never kept.
    fn token(&self) -> Option<Token> {
        Token::read(&self.storage.read(&self.locations.credentials).ok()?)
    }

    /// Start from the sanitized disk cache.
    pub fn with_persistence(mut self, persistence: impl Persistence + 'static) -> Self {
        if let Some(snapshot) = persistence.load(Module::Ai) {
            let claude = &snapshot.data["claude"];
            self.plan = claude["plan"].as_str().map(str::to_owned);
            let reading: Option<Reading> = serde_json::from_value(claude["reading"].clone()).ok();
            match reading {
                Some(reading) if reading.source == Source::OauthUsage && self.usage_supported => {
                    self.usage_reading = Some(reading)
                }
                Some(reading) if reading.source == Source::OauthUsage => {}
                // Weekly limits per model come only from `oauth/usage`.
                Some(mut reading) => {
                    reading.buckets.retain(|bucket| bucket.id == "claude");
                    self.reading = Some(reading);
                }
                None => {}
            }
        }
        self.persistence = Box::new(persistence);
        self
    }

    fn command(&self) -> String {
        #[cfg(windows)]
        if !self.usage_supported {
            return super::windows::command(&self.locations.wrapper);
        }
        format!("sh {}", quote(&self.locations.wrapper))
    }

    /// Point `statusLine` at the wrapper, chaining the current status line.
    pub fn connect(&mut self) -> Result<(), ClaudeError> {
        let mut settings = self.settings()?;
        let ours = self.ours(&settings);
        let current = settings.get("statusLine").cloned();
        // Reconnecting while still ours keeps the original from the first connection.
        let original = match (ours, self.connection()) {
            (true, Some(connection)) => connection.original,
            (true, None) => None,
            (false, _) => current,
        };
        let connection = Connection { original };
        self.write(
            &self.locations.config,
            &serde_json::to_string_pretty(&connection).map_err(|_| ClaudeError::Unwritable)?,
        )?;
        let result = self
            .write(&self.locations.wrapper, &self.wrapper(&connection))
            .and_then(|_| {
                let mut line = match &connection.original {
                    Some(Value::Object(original)) => original.clone(),
                    _ => serde_json::Map::new(),
                };
                line.insert("type".into(), json!("command"));
                line.insert("command".into(), json!(self.command()));
                settings.insert("statusLine".into(), Value::Object(line));
                self.save_settings(&settings)
            });
        if result.is_err() {
            let _ = self.storage.remove(&self.locations.wrapper);
            let _ = self.storage.remove(&self.locations.config);
        }
        result?;
        self.status_line = StatusLine::Connected;
        Ok(())
    }

    /// Restore the original status line, or remove the key if there was none.
    /// A status line replaced by another tool stays as it is.
    pub fn disconnect(&mut self) -> Result<(), ClaudeError> {
        let Some(connection) = self.connection() else {
            self.status_line = StatusLine::Off;
            return Ok(());
        };
        let mut settings = self.settings()?;
        if self.ours(&settings) {
            match connection.original {
                Some(original) => settings.insert("statusLine".into(), original),
                None => settings.remove("statusLine"),
            };
            self.save_settings(&settings)?;
        }
        let _ = self.storage.remove(&self.locations.wrapper);
        let _ = self.storage.remove(&self.locations.config);
        self.status_line = StatusLine::Off;
        Ok(())
    }

    fn wrapper(&self, connection: &Connection) -> String {
        #[cfg(windows)]
        if !self.usage_supported {
            return super::windows::wrapper(
                &self.locations.executable,
                &self.locations.cache,
                connection
                    .original
                    .as_ref()
                    .and_then(|line| line["command"].as_str()),
                self.bash.as_deref(),
            );
        }
        let original = match connection
            .original
            .as_ref()
            .and_then(|line| line["command"].as_str())
        {
            Some(command) => format!("printf '%s' \"$payload\" | sh -c {}\n", quote(command)),
            None => "exit 0\n".to_owned(),
        };
        format!(
            "#!/bin/sh\n\
             # kvojps-monitor: Claude Code status line. Runs the original status line\n\
             # and copies only rate_limits for the app. Disconnect in the app to restore it.\n\
             payload=$(cat; printf x)\n\
             payload=${{payload%x}}\n\
             printf '%s' \"$payload\" | {} statusline {} >/dev/null 2>&1 &\n\
             {original}",
            quote(&self.locations.executable),
            quote(&self.locations.cache),
        )
    }

    /// `statusLine` points at the wrapper.
    fn ours(&self, settings: &serde_json::Map<String, Value>) -> bool {
        settings
            .get("statusLine")
            .is_some_and(|line| line["command"] == self.command())
    }

    fn connection(&self) -> Option<Connection> {
        serde_json::from_str(&self.storage.read(&self.locations.config).ok()?).ok()
    }

    /// A missing file counts as empty settings.
    fn settings(&self) -> Result<serde_json::Map<String, Value>, ClaudeError> {
        match self.storage.read(&self.locations.settings) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(Value::Object(settings)) => Ok(settings),
                _ => Err(ClaudeError::InvalidSettings),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
            Err(_) => Err(ClaudeError::Unwritable),
        }
    }

    fn save_settings(&self, settings: &serde_json::Map<String, Value>) -> Result<(), ClaudeError> {
        let text = serde_json::to_string_pretty(settings).map_err(|_| ClaudeError::Unwritable)?;
        self.write(&self.locations.settings, &(text + "\n"))
    }

    /// Replace by renaming, so Claude Code never reads half a file.
    fn write(&self, path: &str, content: &str) -> Result<(), ClaudeError> {
        let temporary = format!("{path}.kvojps-monitor.tmp");
        self.storage
            .write(&temporary, content)
            .and_then(|_| self.storage.rename(&temporary, path))
            .map_err(|_| ClaudeError::Unwritable)
    }

    /// Turn the `oauth/usage` opt-in on or off; off goes back to the status line at once.
    pub fn set_usage(&mut self, enabled: bool) -> Result<(), ClaudeError> {
        if !self.usage_supported {
            return Err(ClaudeError::Unsupported);
        }
        let choice = UsageChoice {
            enabled,
            ..self.usage_choice()
        };
        self.write(
            &self.locations.usage_choice,
            &serde_json::to_string(&choice).unwrap(),
        )?;
        self.operations.clear();
        self.usage_unavailable = false;
        self.usage_enabled = enabled;
        self.usage_pending = true;
        self.usage_reading = None;
        self.failures = 0;
        self.failure_since = None;
        self.retry_at = 0;
        self.persistence.save(&self.snapshot());
        Ok(())
    }

    /// Each new connectivity generation (login, reconnect, resume) reads once.
    pub fn lifecycle(&mut self, status: crate::lifecycle::Status) {
        let resumed = self
            .lifecycle
            .is_some_and(|previous| previous.generation != status.generation);
        if self
            .lifecycle
            .is_none_or(|previous| previous.generation != status.generation)
        {
            self.usage_pending = true;
            self.usage_at = None;
            self.manual_at = None;
            self.retry_at = 0;
        }
        self.lifecycle = Some(status);
        if self.usage() == Usage::On {
            if self.paused() {
                self.operations.pause();
            } else if resumed {
                self.operations.discard();
            }
        }
    }

    /// Offline, or before this source's turn after login/reconnection.
    fn paused(&self) -> bool {
        self.lifecycle.is_some_and(|status| {
            !status.online || self.clock.now() < status.ready_for(SOURCE_POSITION)
        })
    }

    fn usage(&self) -> Usage {
        if !self.usage_supported {
            return Usage::Unsupported;
        }
        match (self.usage_enabled, self.rejected) {
            (false, _) => Usage::Off,
            (true, None) => Usage::On,
            (true, Some(_)) => Usage::Expired,
        }
    }

    /// The token is read for this request only. One that expired or was
    /// refused is never sent: Claude Code has to renew it.
    fn read_usage(&mut self, manual: bool, progress: &mut impl FnMut(Value)) -> bool {
        let now = self.clock.now();
        let since =
            |at: Option<u64>, interval: u64| at.is_none_or(|t| now.saturating_sub(t) >= interval);
        if self.usage() != Usage::On || self.paused() || now < self.retry_at {
            return false;
        }
        let manual = manual && since(self.manual_at, MANUAL_WAIT);
        // After a failure the backoff decides the next attempt.
        let due = self.failures > 0 || since(self.usage_at, USAGE_INTERVAL);
        if !(self.usage_pending || manual || due) {
            return false;
        }
        if manual {
            self.manual_at = Some(now);
        }
        self.usage_pending = false;
        self.usage_at = Some(now);
        let Some(token) = self.token() else {
            self.usage_unavailable = true;
            self.operations.unavailable();
            return true;
        };
        self.usage_unavailable = false;
        self.operations.begin(manual);
        if token.expires_at.is_some_and(|at| at <= now) {
            self.refuse(&token);
            self.log(Event::InvalidCredential, None, now);
            return true;
        }
        progress(self.data());
        let response = self.api.usage(&token.access_token);
        if let Some(source) = &self.lifecycle_source {
            let latest = source.current();
            if !latest.online
                || self
                    .lifecycle
                    .is_some_and(|started| started.generation != latest.generation)
            {
                self.lifecycle(latest);
                return true;
            }
        }
        match response {
            Ok(response) => match usage::buckets(&response) {
                Some(buckets) => {
                    self.usage_reading = Some(Reading {
                        source: Source::OauthUsage,
                        read_at: self.clock.now(),
                        buckets,
                    });
                    self.operations.success();
                    self.failures = 0;
                    self.failure_since = None;
                    self.persistence.save(&self.snapshot());
                    self.log(Event::Updated, Some(200), now);
                }
                // An unknown format: logged as a failed 200 and retried with backoff.
                None => self.failed(
                    HttpError {
                        status: Some(200),
                        ..HttpError::default()
                    },
                    now,
                ),
            },
            Err(error) if error.status == Some(401) => {
                self.refuse(&token);
                self.log(Event::InvalidCredential, error.status, now);
            }
            Err(error) => self.failed(error, now),
        }
        true
    }

    /// Never send this token again, even after a restart.
    fn refuse(&mut self, token: &Token) {
        self.operations.finish(super::operation::State::Unavailable);
        self.rejected = Some(token.id());
        let choice = UsageChoice {
            refused_expires_at: token.expires_at,
            ..self.usage_choice()
        };
        let _ = self.write(
            &self.locations.usage_choice,
            &serde_json::to_string(&choice).unwrap(),
        );
    }

    /// Transient failure: back off, keeping the last reading.
    fn failed(&mut self, error: HttpError, started_at: u64) {
        // Wait the full delay after a slow request finishes.
        let now = self.clock.now();
        self.failure_since.get_or_insert(now);
        self.failures += 1;
        self.operations.failure();
        self.retry_at = now
            .saturating_add(
                BACKOFF[(self.failures - 1).min(BACKOFF.len() - 1)].max(error.retry_after_ms),
            )
            .max(error.retry_after_at.unwrap_or(0));
        self.log(Event::Failed, error.status, started_at);
    }

    /// Metadata only: never the token, paths or response content.
    fn log(&self, event: Event, http_status: Option<u16>, started_at: u64) {
        self.persistence.log(LogRecord {
            source: Module::Ai,
            event,
            http_status,
            duration_ms: self.clock.now().saturating_sub(started_at),
        });
    }

    /// The main windows from the newest reading, the older one filling a missing
    /// window; weekly limits per model from `oauth/usage`, only while on. Source
    /// and age are those of the newest reading.
    fn merged(&self) -> Option<Reading> {
        let usage = self
            .usage_reading
            .as_ref()
            .filter(|_| self.usage() == Usage::On);
        let (Some(status), Some(usage)) = (&self.reading, usage) else {
            return self.reading.as_ref().or(usage).cloned();
        };
        let (newer, older) = if usage.read_at >= status.read_at {
            (usage, status)
        } else {
            (status, usage)
        };
        let mut merged = newer.clone();
        if let (Some(main), Some(fallback)) = (merged.buckets.first_mut(), older.buckets.first()) {
            main.primary = main.primary.take().or(fallback.primary.clone());
            main.secondary = main.secondary.take().or(fallback.secondary.clone());
        }
        if newer.source == Source::StatusLine {
            merged.buckets.extend(usage.buckets.iter().skip(1).cloned());
        }
        Some(merged)
    }

    /// Detection, plan and status line, rechecked on a timer.
    fn check(&mut self) {
        let first = self.checked_at.is_none();
        self.checked_at = Some(self.clock.now());
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Credentials {
            claude_ai_oauth: Option<Oauth>,
        }
        /// Only the plan: the token itself is never kept.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Oauth {
            subscription_type: Option<String>,
        }
        let credentials = self
            .usage_supported
            .then(|| self.storage.read(&self.locations.credentials).ok())
            .flatten();
        let oauth = credentials
            .as_deref()
            .and_then(|text| serde_json::from_str::<Credentials>(text).ok())
            .and_then(|credentials| credentials.claude_ai_oauth);
        // Claude Code renewed the refused token: read again right away. The
        // token is read only while one is refused, which needs the opt-in.
        if self.rejected.is_some_and(|rejected| {
            credentials
                .as_deref()
                .and_then(Token::read)
                .is_some_and(|token| token.id() != rejected)
        }) {
            self.rejected = None;
            self.usage_pending = true;
        }
        self.detected =
            Some(self.programs.installed("claude") && (!self.usage_supported || oauth.is_some()));
        if let Some(plan) = oauth.and_then(|oauth| oauth.subscription_type) {
            self.plan = Some(plan);
        }
        let connection = self.connection();
        self.status_line = match &connection {
            None => StatusLine::Off,
            Some(_) => {
                if self.settings().is_ok_and(|settings| self.ours(&settings)) {
                    StatusLine::Connected
                } else {
                    StatusLine::Replaced
                }
            }
        };
        // The app binary may have moved since connecting, e.g. an AppImage.
        if let (true, StatusLine::Connected, Some(connection)) =
            (first, self.status_line, &connection)
        {
            let _ = self.write(&self.locations.wrapper, &self.wrapper(connection));
        }
    }

    /// Read the newest copy; true when it brought a newer reading.
    fn read_copy(&mut self) -> bool {
        let started_at = self.clock.now();
        let Some(copy) = self
            .storage
            .read(&self.locations.cache)
            .ok()
            .and_then(|text| serde_json::from_str::<StoredLimits>(&text).ok())
        else {
            return false;
        };
        if self
            .reading
            .as_ref()
            .is_some_and(|reading| reading.read_at >= copy.read_at)
        {
            return false;
        }
        let window = |limit: Option<Limit>, duration_mins| {
            limit.map(|limit| Window {
                used_percent: Some(limit.used_percentage),
                duration_mins: Some(duration_mins),
                resets_at: Some(limit.resets_at.saturating_mul(1000)),
                reset: false,
            })
        };
        // Claude Code drops a window once its reset passes: keep the old one,
        // which then shows as restarted instead of disappearing.
        let previous = |pick: fn(&Bucket) -> &Option<Window>| {
            self.reading
                .as_ref()
                .and_then(|reading| reading.buckets.first())
                .and_then(|bucket| pick(bucket).clone())
                .filter(|window| window.resets_at.is_some_and(|at| at <= copy.read_at))
        };
        let primary = window(copy.rate_limits.five_hour, 300).or_else(|| previous(|b| &b.primary));
        let secondary =
            window(copy.rate_limits.seven_day, 10080).or_else(|| previous(|b| &b.secondary));
        self.reading = Some(Reading {
            source: Source::StatusLine,
            read_at: copy.read_at,
            buckets: vec![Bucket {
                id: "claude".into(),
                name: None,
                primary,
                secondary,
            }],
        });
        self.persistence.save(&self.snapshot());
        self.log(Event::Updated, None, started_at);
        true
    }

    /// True whenever something the module shows may have changed.
    #[cfg(test)]
    pub fn step(&mut self, manual: bool) -> bool {
        self.step_with_progress(manual, |_| {})
    }

    /// Publish real request start before the blocking HTTP port is called.
    pub fn step_with_progress(&mut self, manual: bool, mut progress: impl FnMut(Value)) -> bool {
        let now = self.clock.now();
        let first = self.checked_at.is_none();
        let mut changed = false;
        if manual
            || self
                .checked_at
                .is_none_or(|at| now.saturating_sub(at) >= CHECK_INTERVAL)
        {
            let shown = |claude: &Self| {
                let status = (claude.detected, claude.plan.clone(), claude.status_line);
                (status, claude.usage())
            };
            let before = shown(self);
            self.check();
            changed |= before != shown(self);
        }
        if self.watch.changed() || first || manual {
            changed |= self.read_copy();
        }
        changed |= self.read_usage(manual, &mut progress);
        // A window restarting turns into "Reiniciado às" without a new reading.
        let since = std::mem::replace(&mut self.stepped_at, now);
        changed |= self.merged().iter().flat_map(|r| &r.buckets).any(|bucket| {
            [&bucket.primary, &bucket.secondary]
                .into_iter()
                .flatten()
                .any(|w| w.resets_at.is_some_and(|at| since < at && at <= now))
        });
        changed
    }

    pub fn data(&self) -> Value {
        let reading = restarted(self.merged(), self.clock.now());
        let state = match (self.detected, &reading) {
            (Some(false), _) => "missing",
            (_, Some(_)) => "ready",
            (_, None) => "empty",
        };
        let mut data = json!({
            "state": state,
            "plan": self.plan,
            "statusLine": self.status_line,
            "usage": self.usage(),
            "reading": reading,
        });
        if self.usage_unavailable {
            data["usageUnavailable"] = json!(true);
        }
        if let Some(operation) = &self.operations.current {
            data["operation"] = json!(operation);
        }
        if self.usage() == Usage::On && self.failures >= FAILURES_SHOWN {
            data["failureSince"] = json!(self.failure_since);
        }
        if self.usage() == Usage::On && self.paused() {
            data["paused"] = json!(true);
        }
        data
    }

    pub fn updated_at(&self) -> Option<u64> {
        self.merged().map(|reading| reading.read_at)
    }

    /// Only the Claude part; the shared cache keeps the Codex part.
    fn snapshot(&self) -> crate::snapshot::Snapshot {
        crate::snapshot::Snapshot {
            module: Module::Ai,
            updated_at: self.updated_at(),
            data: json!({"claude": self.data()}),
        }
    }
}

/// Recheck detection, plan and the status line this often.
pub const CHECK_INTERVAL: u64 = 5_000;
/// `oauth/usage` while opted in, whether or not the module is visible.
const USAGE_INTERVAL: u64 = 300_000;
const MANUAL_WAIT: u64 = 10_000;
/// Order among remote sources after login or reconnection (GitHub 0, Trello 1, Codex 2).
const SOURCE_POSITION: u64 = 3;
/// "Falha ao atualizar há X" after this many consecutive failures.
const FAILURES_SHOWN: usize = 3;
/// Transient failures: 30 s, then doubling up to 15 min.
const BACKOFF: [u64; 6] = [30_000, 60_000, 120_000, 240_000, 480_000, 900_000];

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
