use crate::module::Module;
use crate::persistence::{Event, LogRecord, Persistence, Volatile};
use crate::ports::{Clock, Http, HttpError, Processes};
use crate::snapshot::Snapshot;
use chrono::{DateTime, Datelike, Days, NaiveDate, Utc};
use serde::Serialize;
use serde_json::{json, Value};

/// Transient operational metadata, serialized only into live snapshots.
#[derive(Clone, Serialize)]
struct Operation {
    id: u64,
    origin: OperationOrigin,
    state: OperationState,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum OperationOrigin {
    User,
    Automatic,
    Resume,
}
#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum OperationState {
    Running,
    Succeeded,
    Failed,
    Recovered,
    Paused,
    Unavailable,
}

pub struct Github<P, H, R> {
    keyring: Box<dyn crate::ports::Keyring>,
    processes: P,
    http: H,
    clock: R,
    calendar_at: Option<u64>,
    repo_at: Option<u64>,
    manual_at: Option<u64>,
    attempt_at: Option<u64>,
    data: Value,
    failures: usize,
    retry_at: u64,
    rejected: Option<u64>,
    active_credential: Option<u64>,
    persistence: Box<dyn Persistence>,
    failure_since: Option<u64>,
    lifecycle: Option<crate::lifecycle::Status>,
    refresh_pending: bool,
    operation: Option<Operation>,
    operation_id: u64,
    resume_pending: bool,
    operation_failed: bool,
    /// Contribution days of earlier years, fetched only when the streak reaches them; immutable.
    past_days: Vec<(NaiveDate, u64)>,
    past_fetches: usize,
}
impl<P: Processes, H: Http, R: Clock> Github<P, H, R> {
    pub fn new(processes: P, http: H, clock: R) -> Self {
        Self {
            operation: None,
            operation_id: 0,
            resume_pending: false,
            operation_failed: false,
            lifecycle: None,
            refresh_pending: false,
            past_days: Vec::new(),
            past_fetches: 0,
            failures: 0,
            retry_at: 0,
            rejected: None,
            active_credential: None,
            persistence: Box::new(Volatile),
            failure_since: None,
            keyring: Box::new(crate::ports::UnavailableKeyring),
            processes,
            http,
            clock,
            calendar_at: None,
            repo_at: None,
            manual_at: None,
            attempt_at: None,
            data: json!({"state":"missingGh", "calendar":null, "featured":null, "refreshing":false}),
        }
    }
    pub fn with_keyring(mut self, keyring: impl crate::ports::Keyring + 'static) -> Self {
        self.keyring = Box::new(keyring);
        self
    }
    fn credential(&self) -> Option<(String, &'static str)> {
        if let Ok(token) = self.processes.execute("gh", &["auth", "token"]) {
            if !token.trim().is_empty() {
                return Some((token, "gh"));
            }
        }
        self.keyring
            .read("github")
            .ok()
            .flatten()
            .filter(|token| !token.trim().is_empty())
            .map(|token| (token, "token"))
    }
    pub fn save_token(&mut self, token: &str) -> Result<String, CredentialError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(CredentialError::InvalidToken);
        }
        if self.lifecycle.is_some_and(|status| !status.online) {
            return Err(CredentialError::Offline);
        }
        let response = self.http.github_validation(token).map_err(|error| {
            if matches!(error.status, Some(401 | 403)) {
                CredentialError::InvalidToken
            } else {
                CredentialError::Offline
            }
        })?;
        if response.body.get("errors").is_some() {
            return Err(CredentialError::InvalidToken);
        }
        let login = response.body["data"]["viewer"]["login"]
            .as_str()
            .filter(|login| !login.is_empty())
            .ok_or(CredentialError::InvalidToken)?
            .to_owned();
        if !response
            .scopes
            .as_deref()
            .is_some_and(|scopes| scopes.split(',').any(|scope| scope.trim() == "repo"))
        {
            return Err(CredentialError::MissingRepo);
        }
        self.keyring
            .save("github", token)
            .map_err(|_| CredentialError::KeyringUnavailable)?;
        self.rejected = None;
        self.retry_at = 0;
        self.failures = 0;
        self.failure_since = None;
        self.refresh_pending = true;
        // gh remains the preferred source when authenticated.
        if self
            .credential()
            .is_some_and(|(_, source)| source == "token")
        {
            self.data["credential"] = json!({"source":"token", "login":login});
        }
        Ok(login)
    }
    pub fn with_persistence(
        processes: P,
        http: H,
        clock: R,
        persistence: impl Persistence + 'static,
    ) -> Self {
        let mut collector = Self::new(processes, http, clock);
        if let Some(snapshot) = persistence.load(Module::Github) {
            collector.repo_at = snapshot.updated_at;
            collector.data = snapshot.data;
        }
        collector.persistence = Box::new(persistence);
        collector
    }
    pub fn lifecycle(&mut self, status: crate::lifecycle::Status) {
        if self
            .lifecycle
            .is_none_or(|previous| previous.generation != status.generation)
        {
            self.refresh_pending = true;
            self.attempt_at = None;
            self.manual_at = None;
            self.retry_at = 0;
        }
        let was_paused = self.paused();
        let previous = self.lifecycle;
        self.lifecycle = Some(status);
        if self.paused() && !was_paused {
            self.new_operation(OperationOrigin::Automatic, OperationState::Paused);
        }
        if previous.is_some_and(|old| old.generation != status.generation) || was_paused {
            self.resume_pending = true;
        }
    }
    fn paused(&self) -> bool {
        self.lifecycle
            .is_some_and(|s| !s.online || self.clock.now() < s.ready_for(0))
    }
    pub fn snapshot(&self, refreshing: bool) -> Snapshot {
        let mut data = self.data.clone();
        if let Some(operation) = &self.operation {
            data["operation"] = json!(operation);
        }
        data["refreshing"] = json!(refreshing && !self.paused());
        if self.paused() {
            data["paused"] = json!(true);
        }
        if self.paused() {
            data["lastSuccessAt"] = json!(self.repo_at);
        }
        if self.data["state"] == "ready" && !self.paused() {
            data.as_object_mut().unwrap().remove("lastSuccessAt");
            data.as_object_mut().unwrap().remove("failureSince");
        }
        if self.data["state"] != "ready" && self.data["state"] != "missingGh" {
            data["lastSuccessAt"] = json!(self.repo_at);
            data["failureSince"] = json!(self.failure_since);
        }
        Snapshot {
            module: Module::Github,
            updated_at: self.repo_at,
            data,
        }
    }
    pub fn due(&self, visible: bool) -> bool {
        let now = self.clock.now();
        self.calendar_at
            .is_none_or(|t| now.saturating_sub(t) >= 3_600_000 || year(t) != year(now))
            || self
                .repo_at
                .is_none_or(|t| now.saturating_sub(t) >= if visible { 300_000 } else { 900_000 })
    }
    pub fn manual(&mut self) -> bool {
        let now = self.clock.now();
        if self
            .manual_at
            .is_some_and(|t| now.saturating_sub(t) < 10_000)
        {
            return false;
        }
        self.manual_at = Some(now);
        true
    }
    pub fn should_collect(&self, visible: bool, manual: bool) -> bool {
        if self.paused() {
            return false;
        }
        let now = self.clock.now();
        if let Some(rejected) = self.rejected {
            return self
                .credential()
                .is_some_and(|(token, _)| credential_id(token.trim()) != rejected);
        }
        let manual_allowed = manual
            && self
                .manual_at
                .is_none_or(|t| now.saturating_sub(t) >= 10_000);
        if now < self.retry_at {
            return false;
        }
        self.refresh_pending
            || manual_allowed
            || ((self.failures > 0 || self.due(visible))
                && (self.data["state"] == "ready"
                    || self
                        .attempt_at
                        .is_none_or(|t| now.saturating_sub(t) >= 30_000)))
    }
    fn new_operation(&mut self, origin: OperationOrigin, state: OperationState) {
        self.operation_id += 1;
        self.operation = Some(Operation {
            id: self.operation_id,
            origin,
            state,
        });
    }
    fn finish_operation(&mut self, state: OperationState) {
        if let Some(operation) = &mut self.operation {
            operation.state = state;
        }
    }
    /// Publish the real start before the synchronous HTTP work; identity is independent of time.
    pub fn begin(&mut self, visible: bool, manual: bool) -> Option<Snapshot> {
        if !self.should_collect(visible, manual) {
            return None;
        }
        if !self
            .operation
            .as_ref()
            .is_some_and(|op| op.state == OperationState::Running)
        {
            let origin = if manual {
                OperationOrigin::User
            } else if self.resume_pending {
                OperationOrigin::Resume
            } else {
                OperationOrigin::Automatic
            };
            self.new_operation(origin, OperationState::Running);
            self.resume_pending = false;
        }
        Some(self.snapshot(true))
    }
    /// Use the same clock for collection decisions, windows and calendar updates.
    pub fn step(&mut self, visible: bool, manual: bool) -> Option<Snapshot> {
        self.begin(visible, manual)?;
        let force = manual && self.manual();
        self.refresh(force || self.refresh_pending)
    }
    pub fn refresh(&mut self, force: bool) -> Option<Snapshot> {
        if self.paused() {
            return None;
        }
        let now = self.clock.now();
        if now < self.retry_at {
            return None;
        }
        if !self
            .operation
            .as_ref()
            .is_some_and(|op| op.state == OperationState::Running)
        {
            self.new_operation(OperationOrigin::Automatic, OperationState::Running);
        }
        self.refresh_pending = false;
        self.attempt_at = Some(now);
        let mut calendar = force
            || self
                .calendar_at
                .is_none_or(|t| now.saturating_sub(t) >= 3_600_000 || year(t) != year(now));
        // Never store the token in the struct or snapshot.
        let Some((token, source)) = self.credential() else {
            self.data["state"] = json!("missingGh");
            self.persistence.log(LogRecord {
                source: Module::Github,
                event: Event::MissingCredential,
                http_status: None,
                duration_ms: self.clock.now().saturating_sub(now),
            });
            self.finish_operation(OperationState::Unavailable);
            self.operation_failed = true;
            return Some(self.snapshot(false));
        };
        let credential = credential_id(token.trim());
        if self.rejected == Some(credential) {
            return None;
        }
        if self.rejected.take().is_some() {
            self.failures = 0;
            self.failure_since = None;
        }
        if self.active_credential != Some(credential) {
            calendar = true;
        }
        if self.active_credential != Some(credential) {
            self.past_days.clear();
            self.past_fetches = 0;
        }
        let result = self
            .query(token.trim(), source, now, calendar)
            .and_then(|()| self.update_streak(token.trim(), now));
        if result.is_ok() {
            self.active_credential = Some(credential);
        }
        let status = result.as_ref().err().and_then(|error| error.status);
        match result {
            Ok(()) => {
                self.failure_since = None;
                self.failures = 0;
                self.retry_at = 0;
                self.repo_at = Some(now);
                self.data["state"] = json!("ready");
            }
            Err(error) if matches!(error.status, Some(401 | 403)) => {
                self.rejected = Some(credential);
                self.retry_at = 0;
                self.data["state"] = json!("invalidCredential");
            }
            Err(error) => {
                let failed_at = self.clock.now();
                self.failure_since.get_or_insert(failed_at);
                self.failures += 1;
                let delays = [30_000, 60_000, 120_000, 240_000, 480_000, 900_000];
                self.retry_at = failed_at
                    .saturating_add(delays[(self.failures - 1).min(5)].max(error.retry_after_ms))
                    .max(error.retry_after_at.unwrap_or(0));
                self.data["state"] = json!(if self.failures >= 3 { "error" } else { "stale" });
            }
        }
        let success = self.data["state"] == "ready";
        let outcome = if success {
            if self.operation_failed {
                OperationState::Recovered
            } else {
                OperationState::Succeeded
            }
        } else {
            OperationState::Failed
        };
        self.finish_operation(outcome);
        self.operation_failed = !success;
        let snapshot = self.snapshot(false);
        let event = match self.data["state"].as_str() {
            Some("ready") => {
                self.persistence.save(&snapshot);
                Event::Updated
            }
            Some("invalidCredential") => Event::InvalidCredential,
            _ => Event::Failed,
        };
        self.persistence.log(LogRecord {
            source: Module::Github,
            event,
            http_status: status.or(if matches!(event, Event::Updated) {
                Some(200)
            } else {
                None
            }),
            duration_ms: self.clock.now().saturating_sub(now),
        });
        Some(snapshot)
    }
    fn query(
        &mut self,
        token: &str,
        source: &str,
        now: u64,
        calendar: bool,
    ) -> Result<(), HttpError> {
        let year = year(now);
        let field = if calendar {
            "contributionsCollection(from:$from,to:$to) { restrictedContributionsCount contributionCalendar { totalContributions weeks { contributionDays { date contributionCount color } } } }"
        } else {
            ""
        };
        let declaration = if calendar {
            "($from:DateTime!,$to:DateTime!)"
        } else {
            ""
        };
        let query = format!("query{declaration} {{ viewer {{ id login {field} repositories(first:20, affiliations:[OWNER,COLLABORATOR,ORGANIZATION_MEMBER], orderBy:{{field:PUSHED_AT,direction:DESC}}) {{ nodes {{ nameWithOwner }} }} }} }}");
        let response = self.http.graphql(token, &json!({"query":query,"variables":{"from":format!("{year}-01-01T00:00:00Z"),"to":format!("{year}-12-31T23:59:59Z")}}))?;
        if response.get("errors").is_some() {
            return Err(().into());
        }
        let viewer = &response["data"]["viewer"];
        let id = viewer["id"].as_str().ok_or(())?;
        self.data["credential"] = json!({"source":source,"login":viewer["login"]});
        let mut featured: Option<Value> = None;
        let since = timestamp(now.saturating_sub(7 * 24 * 3_600_000));
        let until = timestamp(now);
        for repo in viewer["repositories"]["nodes"].as_array().ok_or(())? {
            let full_name = repo["nameWithOwner"].as_str().ok_or(())?;
            let (owner, name) = full_name.split_once('/').ok_or(())?;
            let response = self.http.graphql(token, &json!({"query": "query($owner:String!,$name:String!,$id:ID!,$since:GitTimestamp!,$until:GitTimestamp!) { repository(owner:$owner,name:$name) { defaultBranchRef { target { ... on Commit { window:history(first:1,author:{id:$id},since:$since,until:$until) { totalCount nodes { committedDate messageHeadline } } total:history(author:{id:$id}) { totalCount } } } } } }", "variables":{"owner":owner,"name":name,"id":id,"since":since,"until":until}}))?;
            if response.get("errors").is_some() {
                return Err(().into());
            }
            let commit = &response["data"]["repository"]["defaultBranchRef"]["target"];
            if commit.is_null() {
                continue;
            }
            let n = commit["window"]["totalCount"].as_u64().ok_or(())?;
            if n == 0 {
                continue;
            }
            let last = &commit["window"]["nodes"][0];
            let data = last["committedDate"].as_str().ok_or(())?;
            let ms = DateTime::parse_from_rfc3339(data)
                .map_err(|_| ())?
                .timestamp_millis();
            let candidate = json!({"name":full_name,"commits":n,"total":commit["total"]["totalCount"],"title":last["messageHeadline"],"when":ms});
            if featured.as_ref().is_none_or(|d| {
                (n, ms)
                    > (
                        d["commits"].as_u64().unwrap_or(0),
                        d["when"].as_i64().unwrap_or(0),
                    )
            }) {
                featured = Some(candidate);
            }
        }
        if calendar {
            let collection = &viewer["contributionsCollection"];
            if !collection["contributionCalendar"].is_object() {
                return Err(().into());
            }
            // totalContributions includes private contributions; restricted is the anonymous subset, so do not add it again.
            self.data["calendar"] = collection.clone();
            self.calendar_at = Some(now);
        }
        self.data["login"] = viewer["login"].clone();
        self.data["featured"] = json!(featured);
        Ok(())
    }
    /// Recomputes the contribution streak, fetching earlier years only while it reaches their end.
    fn update_streak(&mut self, token: &str, now: u64) -> Result<(), HttpError> {
        let today = date(now, self.clock.utc_offset_ms(now));
        loop {
            let current = calendar_days(&self.data["calendar"]);
            let Some(&(start, _)) = current.first() else {
                self.data["streak"] = Value::Null;
                return Ok(());
            };
            let mut days: Vec<_> = self
                .past_days
                .iter()
                .copied()
                .filter(|(day, _)| *day < start)
                .collect();
            days.extend(current);
            let (streak, reached_start) = contribution_streak(&days, today);
            let before = days[0].0 - Days::new(1);
            if !reached_start || self.past_fetches >= MAX_PAST_FETCHES {
                self.data["streak"] = json!(streak);
                return Ok(());
            }
            let query = "query($from:DateTime!,$to:DateTime!) { viewer { contributionsCollection(from:$from,to:$to) { contributionCalendar { weeks { contributionDays { date contributionCount } } } } } }";
            let response = self.http.graphql(token, &json!({"query":query,"variables":{"from":format!("{}-01-01T00:00:00Z", before.year()),"to":format!("{before}T23:59:59Z")}}))?;
            if response.get("errors").is_some() {
                return Err(().into());
            }
            self.past_fetches += 1;
            let fetched: Vec<_> =
                calendar_days(&response["data"]["viewer"]["contributionsCollection"])
                    .into_iter()
                    .filter(|(day, _)| *day <= before)
                    .collect();
            if fetched.is_empty() {
                self.data["streak"] = json!(streak);
                return Ok(());
            }
            self.past_days.extend(fetched);
            self.past_days.sort();
            self.past_days.dedup_by_key(|(day, _)| *day);
        }
    }
}
/// How many earlier years a streak may reach back into.
const MAX_PAST_FETCHES: usize = 5;
/// Contribution days of a contributionsCollection, oldest first.
fn calendar_days(collection: &Value) -> Vec<(NaiveDate, u64)> {
    let mut days: Vec<_> = collection["contributionCalendar"]["weeks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|week| week["contributionDays"].as_array().into_iter().flatten())
        .filter_map(|day| {
            Some((
                day["date"].as_str()?.parse().ok()?,
                day["contributionCount"].as_u64()?,
            ))
        })
        .collect();
    days.sort();
    days
}
/// Consecutive days with at least one contribution ending today, or yesterday while
/// today has none yet. Also tells whether the count ran past the oldest known day.
fn contribution_streak(days: &[(NaiveDate, u64)], today: NaiveDate) -> (u32, bool) {
    let counts: std::collections::HashMap<_, _> = days.iter().copied().collect();
    let Some(&(start, _)) = days.first() else {
        return (0, false);
    };
    let mut day = today;
    if counts.get(&day).copied().unwrap_or(0) == 0 {
        day = day - Days::new(1);
    }
    let mut streak = 0;
    loop {
        if day < start {
            return (streak, true);
        }
        if counts.get(&day).copied().unwrap_or(0) == 0 {
            return (streak, false);
        }
        streak += 1;
        day = day - Days::new(1);
    }
}
/// Local calendar day of an instant. GitHub files each commit under the day of its
/// own time zone, so a streak judged by the UTC day would break every evening.
fn date(ms: u64, offset_ms: i64) -> NaiveDate {
    DateTime::<Utc>::from_timestamp_millis(ms as i64 + offset_ms)
        .unwrap()
        .date_naive()
}
fn credential_id(token: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hash);
    hash.finish()
}
fn year(ms: u64) -> i32 {
    DateTime::<Utc>::from_timestamp_millis(ms as i64)
        .unwrap()
        .year()
}
fn timestamp(ms: u64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms as i64)
        .unwrap()
        .to_rfc3339()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialError {
    InvalidToken,
    MissingRepo,
    Offline,
    KeyringUnavailable,
}

pub enum Command {
    Refresh,
    SaveToken(
        String,
        tokio::sync::oneshot::Sender<Result<String, CredentialError>>,
    ),
}
pub struct Collector(pub std::sync::mpsc::Sender<Command>);
pub fn start(app: tauri::AppHandle) {
    use tauri::Manager;
    let (tx, rx) = std::sync::mpsc::channel();
    app.manage(Collector(tx));
    std::thread::spawn(move || {
        let mut github = match (dirs::cache_dir(), dirs::state_dir()) {
            (Some(cache), Some(state)) => Github::with_persistence(
                crate::ports::SystemProcesses,
                crate::ports::GithubHttp,
                crate::ports::SystemClock,
                crate::persistence::LocalPersistence {
                    cache: crate::persistence::Cache(crate::persistence::DiskStorage(
                        cache.join("kvojps-monitor"),
                    )),
                    logs: crate::persistence::Logs(crate::persistence::DiskStorage(
                        state.join("kvojps-monitor"),
                    )),
                },
            ),
            _ => Github::new(
                crate::ports::SystemProcesses,
                crate::ports::GithubHttp,
                crate::ports::SystemClock,
            ),
        }
        .with_keyring(crate::ports::NativeKeyring);
        crate::snapshot::publish(&app, github.snapshot(false));
        let mut previous_status = (false, 0, 0);
        loop {
            let command = rx.recv_timeout(std::time::Duration::from_secs(1)).ok();
            let manual = matches!(command, Some(Command::Refresh));
            let status = *app.state::<crate::lifecycle::Shared>().lock().unwrap();
            github.lifecycle(status);
            if let Some(Command::SaveToken(token, reply)) = command {
                let result = github.save_token(&token);
                crate::snapshot::publish(&app, github.snapshot(false));
                let _ = reply.send(result);
            }
            let current_status = (status.online, status.generation, status.ready_at);
            if current_status != previous_status {
                previous_status = current_status;
                crate::snapshot::publish(&app, github.snapshot(false));
            }
            let visible = crate::collection::is_visible(&app, Module::Github);
            // Publish previous data before blocking on HTTP.
            if let Some(snapshot) = github.begin(visible, manual) {
                crate::snapshot::publish(&app, snapshot);
            }
            if let Some(snapshot) = github.step(visible, manual) {
                crate::snapshot::publish(&app, snapshot);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::fakes::FakeClock;
    pub(super) struct Gh;
    impl crate::ports::Processes for Gh {
        fn execute(&self, _: &str, _: &[&str]) -> Result<String, ()> {
            Err(())
        }
    }
    pub(super) struct Http;
    impl crate::ports::Http for Http {
        fn graphql(
            &self,
            _: &str,
            _: &serde_json::Value,
        ) -> Result<serde_json::Value, crate::ports::HttpError> {
            panic!("HTTP must not be called without credentials")
        }
    }
    #[test]
    fn missing_gh_publishes_setup_without_credentials() {
        let mut github = Github::new(Gh, Http, FakeClock::at(1793448000000));
        let s = github.refresh(true).unwrap();
        assert_eq!(s.data["state"], "missingGh");
        assert!(!serde_json::to_string(&s).unwrap().contains("token"));
    }
}

#[cfg(test)]
mod scenarios {
    use super::*;
    use crate::ports::fakes::FakeClock;
    struct Gh;
    impl Processes for Gh {
        fn execute(&self, _: &str, _: &[&str]) -> Result<String, ()> {
            Ok("test-secret".into())
        }
    }
    pub(super) struct Api {
        pub(super) empty: bool,
    }
    impl Http for Api {
        fn graphql(&self, _: &str, body: &Value) -> Result<Value, HttpError> {
            if body["query"].as_str().unwrap().contains("viewer {") {
                Ok(
                    serde_json::from_str(include_str!("../../fixtures/github/viewer.json"))
                        .unwrap(),
                )
            } else {
                let mut v: Value =
                    serde_json::from_str(include_str!("../../fixtures/github/history.json"))
                        .unwrap();
                let c = &mut v["data"]["repository"]["defaultBranchRef"]["target"];
                if self.empty {
                    c["window"]["totalCount"] = json!(0);
                }
                // Controlled capture variant: another repository ties but has a more recent commit.
                if body["variables"]["name"] == "repo-1" {
                    c["window"]["nodes"][0]["committedDate"] = json!("2026-10-31T12:00:00Z");
                }
                Ok(v)
            }
        }
    }
    #[test]
    fn reconnect_refreshes_fresh_cache_once_and_preserves_it_while_offline() {
        let clock = FakeClock::at(1793448000000);
        let mut g = Github::new(Gh, Api { empty: false }, clock.clone());
        let ready = g.step(false, false).unwrap();
        let online = crate::lifecycle::Status {
            online: true,
            generation: 1,
            ready_at: clock.now() + 10_000,
        };
        g.lifecycle(online);
        assert!(g.step(true, true).is_none());
        clock.advance(10_000);
        assert!(g.step(false, false).is_some());
        assert!(g.step(false, false).is_none());
        g.lifecycle(crate::lifecycle::Status {
            online: false,
            ..online
        });
        let paused = g.snapshot(false);
        assert_eq!(paused.data["featured"], ready.data["featured"]);
        assert_eq!(paused.data["lastSuccessAt"], json!(clock.now()));
        assert!(g.step(true, true).is_none());
        g.lifecycle(crate::lifecycle::Status {
            online: true,
            generation: 2,
            ready_at: clock.now(),
        });
        assert!(g.step(false, false).is_some());
        assert!(g.step(false, false).is_none());
    }

    #[test]
    fn transient_failures_back_off_without_showing_error_until_third_failure() {
        let clock = FakeClock::at(1793448000000);
        let mut g = Github::new(Gh, ApiError, clock.clone());
        for delay in [30_000, 60_000, 120_000, 240_000, 480_000, 900_000, 900_000] {
            let snapshot = g.step(false, false).unwrap();
            assert_eq!(snapshot.data["state"] == "error", delay >= 120_000);
            clock.advance(delay - 1);
            assert!(g.step(false, false).is_none());
            clock.advance(1);
        }
    }
    #[test]
    fn ties_choose_latest_commit_without_exposing_token() {
        let mut g = Github::new(Gh, Api { empty: false }, FakeClock::at(1793448000000));
        let s = g.refresh(true).unwrap();
        assert_eq!(s.data["featured"]["name"], "exemplo/repo-1");
        assert_eq!(s.data["featured"]["commits"], 27);
        assert!(!serde_json::to_string(&s).unwrap().contains("test-secret"));
    }
    #[test]
    fn empty_window_does_not_replace_featured_repository() {
        let mut g = Github::new(Gh, Api { empty: true }, FakeClock::at(1793448000000));
        assert!(g.refresh(true).unwrap().data["featured"].is_null());
    }
    #[test]
    fn visible_and_manual_cadence_use_injected_clock() {
        let r = FakeClock::at(1793448000000);
        let mut g = Github::new(Gh, Api { empty: false }, r.clone());
        g.refresh(true);
        assert!(!g.due(true));
        r.advance(300_000);
        assert!(g.due(true));
        assert!(!g.due(false));
        assert!(g.manual());
        assert!(!g.manual());
        r.advance(10_000);
        assert!(g.manual());
    }
    struct CalendarYear(FakeClock);
    impl Http for CalendarYear {
        fn graphql(&self, token: &str, body: &Value) -> Result<Value, HttpError> {
            if body["query"]
                .as_str()
                .unwrap()
                .contains("contributionsCollection")
            {
                assert_eq!(
                    body["variables"]["from"],
                    format!("{}-01-01T00:00:00Z", year(self.0.now()))
                );
                assert_eq!(
                    body["variables"]["to"],
                    format!("{}-12-31T23:59:59Z", year(self.0.now()))
                );
            }
            Api { empty: false }.graphql(token, body)
        }
    }
    #[test]
    fn year_rollover_refreshes_calendar_before_one_hour() {
        let r = FakeClock::at(1798761599000); // 2026-12-31 23:59:59 UTC
        let mut g = Github::new(Gh, CalendarYear(r.clone()), r.clone());
        g.refresh(true);
        r.advance(2000);
        assert!(g.due(false));
        assert_eq!(g.step(false, false).unwrap().data["state"], "ready");
    }
    fn day(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }
    fn days(from: &str, counts: &[u64]) -> Vec<(NaiveDate, u64)> {
        (0..counts.len())
            .map(|i| (day(from) + Days::new(i as u64), counts[i]))
            .collect()
    }
    #[test]
    fn streak_counts_through_today_when_it_has_contributions() {
        let d = days("2026-10-25", &[0, 3, 1, 2]);
        assert_eq!(contribution_streak(&d, day("2026-10-28")), (3, false));
    }
    #[test]
    fn streak_stays_alive_until_yesterday_while_today_is_empty() {
        let d = days("2026-10-25", &[0, 3, 1, 0]);
        assert_eq!(contribution_streak(&d, day("2026-10-28")), (2, false));
        // Today not in the calendar yet counts as empty too.
        assert_eq!(contribution_streak(&d[..3], day("2026-10-28")), (2, false));
    }
    #[test]
    fn streak_breaks_after_a_whole_empty_day() {
        let d = days("2026-10-25", &[3, 1, 0, 0]);
        assert_eq!(contribution_streak(&d, day("2026-10-28")), (0, false));
    }
    #[test]
    fn streak_reports_reaching_the_oldest_known_day() {
        let d = days("2026-01-01", &[2, 1]);
        assert_eq!(contribution_streak(&d, day("2026-01-02")), (2, true));
        assert_eq!(contribution_streak(&d[..0], day("2026-01-02")), (0, false));
    }
    /// The current year has contributed every day since 1 Jan; the previous year ends with a run of 3.
    struct YearBoundary(std::sync::Arc<std::sync::Mutex<Vec<Value>>>);
    impl Http for YearBoundary {
        fn graphql(&self, token: &str, body: &Value) -> Result<Value, HttpError> {
            let query = body["query"].as_str().unwrap();
            let calendar = |from: &str, counts: &[u64]| {
                let days: Vec<_> = days(from, counts)
                    .into_iter()
                    .map(|(d, n)| json!({"date": d.to_string(), "contributionCount": n, "color": ""}))
                    .collect();
                json!({"restrictedContributionsCount": 0, "contributionCalendar": {"totalContributions": 0, "weeks": [{"contributionDays": days}]}})
            };
            if !query.contains("repositories") && query.contains("contributionsCollection") {
                self.0.lock().unwrap().push(body["variables"].clone());
                let mut previous = vec![0; 365];
                previous[361..].fill(4);
                return Ok(
                    json!({"data": {"viewer": {"contributionsCollection": calendar("2025-01-01", &previous)}}}),
                );
            }
            let mut response = Api { empty: false }.graphql(token, body)?;
            if query.contains("contributionsCollection") {
                response["data"]["viewer"]["contributionsCollection"] =
                    calendar("2026-01-01", &[1, 2, 5]);
            }
            Ok(response)
        }
    }
    #[test]
    fn streak_reaching_first_of_january_fetches_previous_year_once() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let clock = FakeClock::at(1767355200000); // 2026-01-02 12:00 UTC
        let mut g = Github::new(Gh, YearBoundary(calls.clone()), clock.clone());
        assert_eq!(g.refresh(true).unwrap().data["streak"], 6);
        assert_eq!(
            *calls.lock().unwrap(),
            vec![json!({"from": "2025-01-01T00:00:00Z", "to": "2025-12-31T23:59:59Z"})]
        );
        clock.advance(3_600_000);
        assert_eq!(g.refresh(true).unwrap().data["streak"], 6);
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
    /// A clock in São Paulo (UTC-3).
    #[derive(Clone)]
    struct SaoPaulo(FakeClock);
    impl Clock for SaoPaulo {
        fn now(&self) -> u64 {
            self.0.now()
        }
        fn utc_offset_ms(&self, _: u64) -> i64 {
            -3 * 3_600_000
        }
    }
    /// Contributions up to 30 Oct, none yet on 31 Oct local.
    struct EveningCalendar;
    impl Http for EveningCalendar {
        fn graphql(&self, token: &str, body: &Value) -> Result<Value, HttpError> {
            let mut response = Api { empty: false }.graphql(token, body)?;
            if body["query"]
                .as_str()
                .unwrap()
                .contains("contributionsCollection")
            {
                let days: Vec<_> = days("2026-10-27", &[1, 2, 3, 4, 0, 0])
                    .into_iter()
                    .map(|(d, n)| json!({"date": d.to_string(), "contributionCount": n, "color": ""}))
                    .collect();
                response["data"]["viewer"]["contributionsCollection"]["contributionCalendar"]
                    ["weeks"] = json!([{ "contributionDays": days }]);
            }
            Ok(response)
        }
    }
    #[test]
    fn streak_judges_today_by_the_local_day_in_the_evening() {
        // 31 Oct 22:00 in São Paulo is already 1 Nov in UTC.
        let clock = FakeClock::at(1793494800000);
        let mut g = Github::new(Gh, EveningCalendar, SaoPaulo(clock));
        assert_eq!(g.refresh(true).unwrap().data["streak"], 4);
    }
    #[test]
    fn streak_is_null_without_calendar() {
        let mut g = Github::new(
            super::tests::Gh,
            super::tests::Http,
            FakeClock::at(1793448000000),
        );
        assert!(g.refresh(true).unwrap().data["streak"].is_null());
    }
    struct ApiError;
    impl Http for ApiError {
        fn graphql(&self, _: &str, _: &Value) -> Result<Value, HttpError> {
            Err(().into())
        }
    }
    #[test]
    fn ui_snapshots_match_core_output() {
        let clock = FakeClock::at(1793448000000);
        let mut full = Github::new(Gh, Api { empty: false }, clock.clone());
        let mut empty = Github::new(Gh, Api { empty: true }, clock.clone());
        let mut error = Github::new(Gh, ApiError, clock.clone());
        let mut without = Github::new(super::tests::Gh, super::tests::Http, clock.clone());
        error.step(false, false);
        clock.advance(30_000);
        error.step(false, false);
        clock.advance(60_000);
        for (name, snapshot) in [
            ("full", full.refresh(true).unwrap()),
            ("empty", empty.refresh(true).unwrap()),
            ("missing-gh", without.refresh(true).unwrap()),
            ("error", error.refresh(true).unwrap()),
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../src/lib/github/fixtures/{name}.json"));
            if std::env::var_os("UPDATE_FIXTURES").is_some() {
                std::fs::write(
                    &path,
                    serde_json::to_string_pretty(&snapshot.data).unwrap() + "\n",
                )
                .unwrap();
            }
            let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(snapshot.data, fixture);
        }
    }

    #[test]
    fn missing_gh_backs_off_and_manual_refresh_waits_ten_seconds() {
        let r = FakeClock::at(1793448000000);
        let mut g = Github::new(super::tests::Gh, super::tests::Http, r.clone());
        assert!(g.step(false, false).is_some());
        assert!(g.step(false, false).is_none());
        assert!(g.step(false, true).is_some());
        r.advance(9999);
        assert!(g.step(false, true).is_none());
        r.advance(1);
        assert!(g.step(false, true).is_some());
        r.advance(30000);
        assert!(g.step(false, false).is_some());
    }
    struct CadenciaCalendar {
        clock: FakeClock,
    }
    impl Http for CadenciaCalendar {
        fn graphql(&self, token: &str, body: &Value) -> Result<Value, HttpError> {
            if body["query"].as_str().unwrap().contains("viewer {") {
                let mut response = Api { empty: false }.graphql(token, body)?;
                let calendar = body["query"]
                    .as_str()
                    .unwrap()
                    .contains("contributionsCollection");
                let mut total = 1;
                if self.clock.now() >= 1793451600000 {
                    total = 99;
                }
                if calendar {
                    response["data"]["viewer"]["contributionsCollection"]["contributionCalendar"]
                        ["totalContributions"] = json!(total);
                } else {
                    response["data"]["viewer"]
                        .as_object_mut()
                        .unwrap()
                        .remove("contributionsCollection");
                }
                Ok(response)
            } else {
                Api { empty: false }.graphql(token, body)
            }
        }
    }
    #[test]
    fn calendar_waits_one_hour_while_featured_refreshes_every_five_minutes() {
        let r = FakeClock::at(1793448000000);
        let mut g = Github::new(Gh, CadenciaCalendar { clock: r.clone() }, r.clone());
        let initial = g.step(true, false).unwrap();
        assert_eq!(
            initial.data["calendar"]["contributionCalendar"]["totalContributions"],
            1
        );
        r.advance(300000);
        let seguinte = g.step(true, false).unwrap();
        assert_eq!(seguinte.updated_at, Some(1793448300000));
        assert_eq!(seguinte.data["calendar"], initial.data["calendar"]);
        r.advance(3300000);
        let time = g.step(false, false).unwrap();
        assert_eq!(
            time.data["calendar"]["contributionCalendar"]["totalContributions"],
            99
        );
    }
}

#[cfg(test)]
mod resilience_tests {
    use super::*;
    use crate::ports::fakes::FakeClock;
    use std::sync::{Arc, Mutex};
    struct Credential(Arc<Mutex<String>>);
    impl Processes for Credential {
        fn execute(&self, _: &str, _: &[&str]) -> Result<String, ()> {
            Ok(self.0.lock().unwrap().clone())
        }
    }
    struct Refused(u16);
    impl Http for Refused {
        fn graphql(&self, _: &str, _: &Value) -> Result<Value, HttpError> {
            Err(HttpError {
                status: Some(self.0),
                retry_after_ms: 180_000,
                ..HttpError::default()
            })
        }
    }
    struct SwitchApi(Arc<Mutex<Option<u16>>>);
    impl Http for SwitchApi {
        fn graphql(&self, token: &str, body: &Value) -> Result<Value, HttpError> {
            if let Some(status) = *self.0.lock().unwrap() {
                return Refused(status).graphql(token, body);
            }
            super::scenarios::Api { empty: false }.graphql(token, body)
        }
    }
    #[test]
    fn refused_credentials_keep_last_data_and_pause_even_manual_until_changed() {
        for status in [401, 403] {
            let clock = FakeClock::at(1793448000000);
            let token = Arc::new(Mutex::new("secret-one".into()));
            let failure = Arc::new(Mutex::new(None));
            let mut g = Github::new(
                Credential(token.clone()),
                SwitchApi(failure.clone()),
                clock.clone(),
            );
            let old = g.step(true, false).unwrap();
            *failure.lock().unwrap() = Some(status);
            clock.advance(300_000);
            let s = g.step(true, false).unwrap();
            assert_eq!(s.data["state"], "invalidCredential");
            assert_eq!(s.data["featured"], old.data["featured"]);
            assert_eq!(s.data["lastSuccessAt"], 1793448000000u64);
            clock.advance(3_600_000);
            *failure.lock().unwrap() = None;
            assert!(g.step(true, true).is_none());
            *token.lock().unwrap() = "secret-two".into();
            assert_eq!(g.step(true, false).unwrap().data["state"], "ready");
        }
    }
    #[test]
    fn resilience_ui_fixtures_match_published_snapshots() {
        let clock = FakeClock::at(1793448000000);
        let failure = Arc::new(Mutex::new(None));
        let mut g = Github::new(
            Credential(Arc::new(Mutex::new("secret".into()))),
            SwitchApi(failure.clone()),
            clock.clone(),
        );
        g.step(true, false).unwrap();
        clock.advance(300_000);
        *failure.lock().unwrap() = Some(503);
        let stale = g.step(true, false).unwrap();
        clock.advance(180_000);
        g.step(true, false).unwrap();
        clock.advance(180_000);
        let failed = g.step(true, false).unwrap();
        g.lifecycle(crate::lifecycle::Status::default());
        let offline = g.snapshot(false);
        g.lifecycle(crate::lifecycle::Status {
            online: true,
            generation: 1,
            ready_at: clock.now(),
        });
        clock.advance(240_000);
        *failure.lock().unwrap() = Some(401);
        let invalid = g.step(true, false).unwrap();
        for (name, snapshot) in [
            ("stale", stale),
            ("failure", failed),
            ("offline", offline),
            ("invalid-credential", invalid),
        ] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../src/lib/github/fixtures/{name}.json"));
            if std::env::var_os("UPDATE_FIXTURES").is_some() {
                std::fs::write(
                    &path,
                    serde_json::to_string_pretty(&snapshot.data).unwrap() + "\n",
                )
                .unwrap();
            }
            let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(snapshot.data, fixture);
        }
    }
    #[test]
    fn successful_retry_resets_failures_and_restores_normal_cadence() {
        let clock = FakeClock::at(1793448000000);
        let failure = Arc::new(Mutex::new(None));
        let mut g = Github::new(
            Credential(Arc::new(Mutex::new("secret".into()))),
            SwitchApi(failure.clone()),
            clock.clone(),
        );
        g.step(true, false).unwrap();
        // A manual refresh can fail before the normal five-minute deadline.
        *failure.lock().unwrap() = Some(503);
        clock.advance(10_000);
        g.step(true, true).unwrap();
        *failure.lock().unwrap() = None;
        clock.advance(180_000);
        assert_eq!(g.step(true, false).unwrap().data["state"], "ready");
        assert!(g.step(true, false).is_none());
        clock.advance(300_000);
        *failure.lock().unwrap() = Some(500);
        assert_eq!(g.step(true, false).unwrap().data["state"], "stale");
    }
    struct SlowTimeout(FakeClock);
    impl Http for SlowTimeout {
        fn graphql(&self, _: &str, _: &Value) -> Result<Value, HttpError> {
            self.0.advance(30_000);
            Err(HttpError::default())
        }
    }
    #[test]
    fn timeout_waits_the_full_backoff_after_request_finishes() {
        let clock = FakeClock::at(1793448000000);
        let mut g = Github::new(
            Credential(Arc::new(Mutex::new("secret".into()))),
            SlowTimeout(clock.clone()),
            clock.clone(),
        );
        g.step(true, false).unwrap();
        assert!(g.step(true, false).is_none());
        clock.advance(29_999);
        assert!(g.step(true, true).is_none());
        clock.advance(1);
        assert!(g.step(true, false).is_some());
    }
    struct SlowResponse {
        clock: FakeClock,
        header: &'static str,
    }
    impl Http for SlowResponse {
        fn graphql(&self, _: &str, _: &Value) -> Result<Value, HttpError> {
            self.clock.advance(20_000);
            Err(HttpError::response(429, Some(self.header)))
        }
    }
    #[test]
    fn retry_after_seconds_and_http_dates_use_response_time_and_injected_clock() {
        // Controlled response metadata variants; the body is deliberately discarded.
        for (header, wait) in [
            ("180", 180_000),
            ("Sat, 31 Oct 2026 12:03:00 GMT", 160_000),
            ("invalid", 30_000),
        ] {
            let clock = FakeClock::at(1793448000000);
            let mut g = Github::new(
                Credential(Arc::new(Mutex::new("secret".into()))),
                SlowResponse {
                    clock: clock.clone(),
                    header,
                },
                clock.clone(),
            );
            g.step(true, false).unwrap();
            clock.advance(wait - 1);
            assert!(g.step(true, true).is_none(), "{header}");
            clock.advance(1);
            assert!(g.step(true, false).is_some(), "{header}");
        }
    }
    #[test]
    fn retry_after_is_a_minimum_even_for_manual_refresh() {
        let clock = FakeClock::at(1793448000000);
        let mut g = Github::new(
            Credential(Arc::new(Mutex::new("secret".into()))),
            Refused(429),
            clock.clone(),
        );
        g.step(true, false).unwrap();
        clock.advance(179_999);
        assert!(g.step(true, true).is_none());
        clock.advance(1);
        assert!(g.step(true, false).is_some());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::{
        persistence::{Cache, DiskStorage, LocalPersistence, Logs},
        ports::fakes::FakeClock,
    };
    struct Gh;
    impl Processes for Gh {
        fn execute(&self, _: &str, _: &[&str]) -> Result<String, ()> {
            Ok("private-token".into())
        }
    }
    #[test]
    fn restart_publishes_sanitized_last_snapshot_before_any_network_request() {
        let cache = tempfile::tempdir().unwrap();
        let logs = tempfile::tempdir().unwrap();
        let persistence = || LocalPersistence {
            cache: Cache(DiskStorage(cache.path().into())),
            logs: Logs(DiskStorage(logs.path().into())),
        };
        let clock = FakeClock::at(1793448000000);
        let mut g = Github::with_persistence(
            Gh,
            super::scenarios::Api { empty: false },
            clock.clone(),
            persistence(),
        );
        let fresh = g.step(false, false).unwrap();
        let restored =
            Github::with_persistence(Gh, super::tests::Http, clock, persistence()).snapshot(false);
        assert_eq!(restored.updated_at, fresh.updated_at);
        assert_eq!(restored.data["featured"]["commits"], 27);
        assert_eq!(restored.data["featured"]["title"], "");
        assert_eq!(restored.data["state"], "stale");
        let cache_text = std::fs::read_to_string(cache.path().join("github.json")).unwrap();
        let log_text = std::fs::read_to_string(logs.path().join("info.log")).unwrap();
        for text in [&cache_text, &log_text] {
            assert!(!text.contains("operation"));
            assert!(!text.contains("private-token"));
            assert!(!text.contains("Atualiza documentação"));
        }
        assert_eq!(
            serde_json::from_str::<Value>(log_text.trim()).unwrap(),
            json!({"source":"github","event":"updated","httpStatus":200,"durationMs":0})
        );
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::ports::fakes::FakeClock;
    struct NoProcesses;
    impl Processes for NoProcesses {
        fn execute(&self, _: &str, _: &[&str]) -> Result<String, ()> {
            panic!("paused collector must not execute processes")
        }
    }
    struct NoHttp;
    impl Http for NoHttp {
        fn graphql(&self, _: &str, _: &Value) -> Result<Value, HttpError> {
            panic!("paused collector must not send HTTP")
        }
    }
    #[test]
    fn offline_blocks_manual_and_automatic_collection_without_error_banner() {
        let mut github = Github::new(NoProcesses, NoHttp, FakeClock::at(1000));
        github.lifecycle(crate::lifecycle::Status::default());
        assert!(github.step(true, true).is_none());
        assert!(!github.should_collect(true, false));
        let snapshot = github.snapshot(false);
        assert_eq!(snapshot.data["paused"], true);
        assert_ne!(snapshot.data["state"], "error");
    }
}

#[cfg(test)]
mod credentials_tests;

#[cfg(test)]
mod operation_tests {
    use super::*;
    use crate::ports::fakes::FakeClock;

    #[test]
    fn pause_and_recovery_have_real_results_and_new_identities_without_advancing_time() {
        use std::sync::{Arc, Mutex};
        struct Credential;
        impl Processes for Credential {
            fn execute(&self, _: &str, _: &[&str]) -> Result<String, ()> {
                Ok("SENTINEL_TOKEN".into())
            }
        }
        struct Api(Arc<Mutex<bool>>);
        impl Http for Api {
            fn graphql(&self, token: &str, body: &Value) -> Result<Value, HttpError> {
                if *self.0.lock().unwrap() {
                    Err(HttpError::default())
                } else {
                    super::scenarios::Api { empty: false }.graphql(token, body)
                }
            }
        }
        let clock = FakeClock::at(1793448000000);
        let fail = Arc::new(Mutex::new(true));
        let mut github = Github::new(Credential, Api(fail.clone()), clock.clone());
        let failure = github.step(true, false).unwrap();
        assert_eq!(
            failure.data["operation"],
            json!({"id":1,"origin":"automatic","state":"failed"})
        );
        github.lifecycle(crate::lifecycle::Status::default());
        assert_eq!(
            github.snapshot(false).data["operation"],
            json!({"id":2,"origin":"automatic","state":"paused"})
        );
        assert!(github.begin(true, true).is_none());
        assert!(github.step(true, true).is_none());
        github.lifecycle(crate::lifecycle::Status {
            online: true,
            generation: 1,
            ready_at: clock.now(),
        });
        *fail.lock().unwrap() = false;
        let start = github.begin(true, false).unwrap();
        assert_eq!(
            start.data["operation"],
            json!({"id":3,"origin":"resume","state":"running"})
        );
        let result = github.step(true, false).unwrap();
        assert_eq!(
            result.data["operation"],
            json!({"id":3,"origin":"resume","state":"recovered"})
        );
        assert!(!result.data["operation"].to_string().contains("SENTINEL"));
        assert!(github.step(true, false).is_none());
    }

    #[test]
    fn requested_refresh_publishes_correlated_start_and_real_result() {
        let mut github = Github::new(super::tests::Gh, super::tests::Http, FakeClock::at(1000));
        let start = github.begin(true, true).unwrap();
        assert_eq!(
            start.data["operation"],
            json!({"id":1,"origin":"user","state":"running"})
        );
        let result = github.step(true, true).unwrap();
        assert_eq!(
            result.data["operation"],
            json!({"id":1,"origin":"user","state":"unavailable"})
        );
        assert_eq!(
            github.snapshot(false).data["operation"],
            result.data["operation"]
        );
    }
}
