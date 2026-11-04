//! Trello module: the chosen board's open lists and cards, read by polling.
mod api;

pub use api::{authorization_url, Auth, TrelloApi, TrelloHttp};

use crate::module::Module;
use crate::persistence::{Event, LogRecord, Persistence, Volatile};
use crate::ports::{Clock, HttpError, Keyring, Processes};
use crate::snapshot::Snapshot;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Keyring entry holding key, token and the non-secret choices made with them.
#[derive(Serialize, Deserialize)]
struct Credential {
    key: String,
    token: String,
    login: String,
    board: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Board {
    id: String,
    name: String,
}

/// Board response, reduced to what the module shows.
#[derive(Deserialize)]
struct RemoteList {
    id: String,
    name: String,
    cards: Vec<RemoteCard>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCard {
    id: String,
    name: String,
    short_url: String,
    due: Option<String>,
    #[serde(default)]
    due_complete: bool,
    labels: Vec<RemoteLabel>,
}
#[derive(Deserialize)]
struct RemoteLabel {
    color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialError {
    InvalidCredential,
    Offline,
    KeyringUnavailable,
}

/// The card stays where the last board shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MoveError;

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
    Cancelled,
}

pub struct Trello<A, R> {
    operation: Option<Operation>,
    movement: Option<Operation>,
    operation_id: u64,
    resume_pending: bool,
    operation_failed: bool,
    api: A,
    clock: R,
    keyring: Box<dyn Keyring>,
    persistence: Box<dyn Persistence>,
    data: Value,
    /// Last successful read.
    updated_at: Option<u64>,
    attempt_at: Option<u64>,
    manual_at: Option<u64>,
    refresh_pending: bool,
    /// Re-list open boards on the next read (manual refresh).
    boards_pending: bool,
    failures: usize,
    failure_since: Option<u64>,
    retry_at: u64,
    /// Identity of a credential refused with 401/403; no request until it changes.
    rejected: Option<u64>,
    lifecycle: Option<crate::lifecycle::Status>,
    lifecycle_source: Option<Box<dyn crate::lifecycle::CurrentStatus>>,
    /// Key typed before "Gerar token". The browser closes the panel, so the next
    /// panel reads it back. Memory only; dropped on connect or disconnect.
    pending_key: Option<String>,
}
impl<A: TrelloApi, R: Clock> Trello<A, R> {
    pub fn new(api: A, clock: R) -> Self {
        Self {
            operation: None,
            movement: None,
            operation_id: 0,
            resume_pending: false,
            operation_failed: false,
            api,
            clock,
            keyring: Box::new(crate::ports::UnavailableKeyring),
            persistence: Box::new(Volatile),
            data: disconnected(),
            updated_at: None,
            attempt_at: None,
            manual_at: None,
            refresh_pending: false,
            boards_pending: false,
            failures: 0,
            failure_since: None,
            retry_at: 0,
            rejected: None,
            lifecycle: None,
            lifecycle_source: None,
            pending_key: None,
        }
    }
    pub fn with_lifecycle(
        mut self,
        source: impl crate::lifecycle::CurrentStatus + 'static,
    ) -> Self {
        self.lifecycle = Some(source.current());
        self.lifecycle_source = Some(Box::new(source));
        self
    }
    fn changed_lifecycle(&self) -> Option<crate::lifecycle::Status> {
        let latest = self.lifecycle_source.as_ref()?.current();
        (!latest.online
            || self
                .lifecycle
                .is_some_and(|started| started.generation != latest.generation))
        .then_some(latest)
    }
    pub fn with_keyring(mut self, keyring: impl Keyring + 'static) -> Self {
        self.keyring = Box::new(keyring);
        self
    }
    /// Start from the sanitized disk cache, before any request.
    pub fn with_persistence(mut self, persistence: impl Persistence + 'static) -> Self {
        if let Some(snapshot) = persistence.load(Module::Trello) {
            self.updated_at = snapshot.updated_at;
            self.data = snapshot.data;
        }
        self.persistence = Box::new(persistence);
        self
    }
    /// Validate with the account and its open boards; only then save.
    pub fn connect(&mut self, key: &str, token: &str) -> Result<String, CredentialError> {
        let (key, token) = (key.trim(), token.trim());
        if authorization_url(key).is_none() || token.is_empty() {
            return Err(CredentialError::InvalidCredential);
        }
        if self.lifecycle.is_some_and(|status| !status.online) {
            return Err(CredentialError::Offline);
        }
        let auth = Auth::new(key, token);
        let me = self
            .api
            .get(&auth, "/members/me?fields=username")
            .map_err(validation_error)?;
        let login = me["username"]
            .as_str()
            .filter(|login| !login.is_empty())
            .ok_or(CredentialError::InvalidCredential)?
            .to_owned();
        let boards = self.boards(&auth).map_err(validation_error)?;
        // Reconnecting the same account keeps its board while it is still open.
        let previous = self
            .credential()
            .ok()
            .flatten()
            .filter(|previous| previous.login == login);
        let board = previous
            .and_then(|previous| previous.board)
            .and_then(|id| boards.iter().find(|board| board.id == id).cloned());
        let credential = Credential {
            key: key.into(),
            token: token.into(),
            login: login.clone(),
            board: board.as_ref().map(|board| board.id.clone()),
        };
        self.keyring
            .save("trello", &serde_json::to_string(&credential).unwrap())
            .map_err(|_| CredentialError::KeyringUnavailable)?;
        if board.is_none() {
            self.persistence.clear(Module::Trello);
            self.data = disconnected();
            self.data["state"] = json!("chooseBoard");
            self.updated_at = None;
        } else if self.data["state"] == "invalidCredential" {
            self.data["state"] = json!("stale");
        }
        self.data["account"] = json!({"login": login});
        self.data["boards"] = json!(boards);
        self.pending_key = None;
        self.refresh_pending = true;
        self.rejected = None;
        self.failures = 0;
        self.failure_since = None;
        self.retry_at = 0;
        Ok(login)
    }
    fn boards(&self, auth: &Auth) -> Result<Vec<Board>, HttpError> {
        let boards = self
            .api
            .get(auth, "/members/me/boards?filter=open&fields=name")?;
        serde_json::from_value(boards).map_err(|_| HttpError::default())
    }
    /// Err while the keyring is unavailable (e.g. still locked after login).
    fn credential(&self) -> Result<Option<Credential>, ()> {
        let secret = self.keyring.read("trello")?;
        Ok(secret.and_then(|secret| serde_json::from_str(&secret).ok()))
    }
    /// Choose among the open boards listed with the saved credential.
    pub fn select_board(&mut self, id: &str) -> Result<(), CredentialError> {
        let mut credential = self
            .credential()
            .map_err(|_| CredentialError::KeyringUnavailable)?
            .ok_or(CredentialError::InvalidCredential)?;
        let boards: Vec<Board> =
            serde_json::from_value(self.data["boards"].clone()).unwrap_or_default();
        let board = boards
            .into_iter()
            .find(|board| board.id == id)
            .ok_or(CredentialError::InvalidCredential)?;
        credential.board = Some(board.id.clone());
        self.keyring
            .save("trello", &serde_json::to_string(&credential).unwrap())
            .map_err(|_| CredentialError::KeyringUnavailable)?;
        if self.data["board"]["id"] != json!(board.id) {
            self.persistence.clear(Module::Trello);
            self.data["lists"] = Value::Null;
            self.updated_at = None;
        }
        self.data["board"] = json!(board);
        self.refresh_pending = true;
        Ok(())
    }
    /// Open the token page for the typed key and remember the key for the next panel.
    pub fn authorize(
        &mut self,
        processes: &impl Processes,
        key: &str,
    ) -> Result<(), CredentialError> {
        let url = authorization_url(key).ok_or(CredentialError::InvalidCredential)?;
        self.pending_key = Some(key.trim().to_owned());
        processes
            .execute("xdg-open", &[&url])
            .map(|_| ())
            .map_err(|_| CredentialError::Offline)
    }
    /// Erase key and token and forget the cached board.
    pub fn disconnect(&mut self) -> Result<(), CredentialError> {
        self.keyring
            .delete("trello")
            .map_err(|_| CredentialError::KeyringUnavailable)?;
        self.persistence.clear(Module::Trello);
        self.data = disconnected();
        self.pending_key = None;
        self.updated_at = None;
        self.rejected = None;
        self.failures = 0;
        self.failure_since = None;
        self.retry_at = 0;
        Ok(())
    }
    /// Move a card to another list of the shown board. On success the board is read
    /// again, which restarts the 30 s clock; on failure nothing changes, so the
    /// panel's optimistic move falls back to the last board.
    fn begin_move(&mut self) -> Snapshot {
        self.operation_id += 1;
        self.movement = Some(Operation {
            id: self.operation_id,
            origin: OperationOrigin::User,
            state: OperationState::Running,
        });
        self.snapshot(false)
    }
    pub fn move_card(
        &mut self,
        card: &str,
        list: &str,
        mut progress: impl FnMut(Snapshot),
    ) -> Result<(), MoveError> {
        progress(self.begin_move());
        let result = self.perform_move(card, list);
        if let Some(movement) = &mut self.movement {
            if movement.state == OperationState::Cancelled {
                progress(self.snapshot(false));
                return Err(MoveError);
            }
            movement.state = if result.is_ok() {
                OperationState::Succeeded
            } else {
                OperationState::Failed
            };
        }
        progress(self.snapshot(false));
        if result.is_ok() {
            self.attempt_at = Some(self.clock.now());
            self.new_operation(OperationOrigin::Automatic, OperationState::Running);
            progress(self.snapshot(true));
            if let Some(snapshot) = self.refresh() {
                progress(snapshot);
            }
        }
        result
    }
    fn perform_move(&mut self, card: &str, list: &str) -> Result<(), MoveError> {
        // Ids come from the panel: only cards and lists of the shown board reach the URL.
        let lists = self.data["lists"].as_array().ok_or(MoveError)?;
        let position = |id: &str, items: &[Value]| items.iter().position(|item| item["id"] == id);
        let to = position(list, lists).ok_or(MoveError)?;
        let (from, index) = lists
            .iter()
            .enumerate()
            .find_map(|(from, source)| {
                position(card, source["cards"].as_array()?).map(|index| (from, index))
            })
            .ok_or(MoveError)?;
        // Same pauses as polling: offline, backoff, refused credential.
        if from == to
            || self.paused()
            || self.rejected.is_some()
            || self.clock.now() < self.retry_at
        {
            return Err(MoveError);
        }
        let credential = self.credential().ok().flatten().ok_or(MoveError)?;
        let auth = Auth::new(&credential.key, &credential.token);
        let started_at = self.clock.now();
        let response = self
            .api
            .put(&auth, &format!("/cards/{card}?idList={list}&pos=bottom"));
        if let Some(latest) = self.changed_lifecycle() {
            self.lifecycle(latest);
            if let Some(movement) = &mut self.movement {
                movement.state = OperationState::Cancelled;
            }
            return Err(MoveError);
        }
        if let Err(error) = response {
            if matches!(error.status, Some(401 | 403)) {
                self.rejected = Some(credential_id(&credential));
                self.data["state"] = json!("invalidCredential");
                self.log(Event::InvalidCredential, error.status, started_at);
            } else {
                self.log(Event::Failed, error.status, started_at);
            }
            return Err(MoveError);
        }
        // Keep the move even if the read below fails.
        let moved = self.data["lists"][from]["cards"]
            .as_array_mut()
            .unwrap()
            .remove(index);
        self.data["lists"][to]["cards"]
            .as_array_mut()
            .unwrap()
            .push(moved);
        Ok(())
    }
    /// Each new connectivity generation (login, reconnect, resume) refreshes once.
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
            if let Some(movement) = &mut self.movement {
                if movement.state == OperationState::Running {
                    movement.state = OperationState::Cancelled;
                }
            }
        }
        if previous.is_some_and(|old| old.generation != status.generation) || was_paused {
            self.resume_pending = true;
        }
    }
    fn paused(&self) -> bool {
        self.lifecycle.is_some_and(|status| {
            !status.online || self.clock.now() < status.ready_for(SOURCE_POSITION)
        })
    }
    pub fn snapshot(&self, refreshing: bool) -> Snapshot {
        let mut data = self.data.clone();
        if let Some(operation) = &self.operation {
            data["operation"] = json!(operation);
        }
        if let Some(movement) = &self.movement {
            data["movement"] = json!(movement);
        }
        let paused = self.paused();
        data["refreshing"] = json!(refreshing && !paused);
        if let Some(key) = &self.pending_key {
            data["pendingKey"] = json!(key);
        }
        if paused {
            data["paused"] = json!(true);
            data["lastSuccessAt"] = json!(self.updated_at);
        }
        if !matches!(
            data["state"].as_str(),
            Some("ready" | "missingCredential" | "chooseBoard")
        ) {
            data["lastSuccessAt"] = json!(self.updated_at);
            data["failureSince"] = json!(self.failure_since);
        }
        let now = self.clock.now();
        // Decided at publish time so a cached board turns overdue without a request.
        for list in data["lists"].as_array_mut().into_iter().flatten() {
            for card in list["cards"].as_array_mut().into_iter().flatten() {
                card["overdue"] = json!(
                    card["due"].as_u64().is_some_and(|due| due < now)
                        && card["dueComplete"] != true
                );
            }
        }
        Snapshot {
            module: Module::Trello,
            updated_at: self.updated_at,
            data,
        }
    }
    pub fn should_collect(&self, visible: bool, manual: bool) -> bool {
        if self.paused() {
            return false;
        }
        let now = self.clock.now();
        let since =
            |at: Option<u64>, interval: u64| at.is_none_or(|t| now.saturating_sub(t) >= interval);
        if let Some(rejected) = self.rejected {
            return self
                .credential()
                .ok()
                .flatten()
                .is_some_and(|credential| credential_id(&credential) != rejected);
        }
        if now < self.retry_at {
            return false;
        }
        let manual = manual && since(self.manual_at, 10_000);
        let due = since(self.updated_at, if visible { 30_000 } else { 300_000 });
        // Without data, retry at most every 30 s (e.g. keyring still locked).
        self.refresh_pending
            || manual
            || ((self.failures > 0 || due) && since(self.attempt_at, 30_000))
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
    /// Use the injected clock for every collection decision.
    pub fn step(&mut self, visible: bool, manual: bool) -> Option<Snapshot> {
        self.begin(visible, manual)?;
        let now = self.clock.now();
        if manual
            && self
                .manual_at
                .is_none_or(|t| now.saturating_sub(t) >= 10_000)
        {
            self.manual_at = Some(now);
            self.boards_pending = true;
        }
        self.refresh_pending = false;
        self.attempt_at = Some(now);
        self.refresh()
    }
    fn refresh(&mut self) -> Option<Snapshot> {
        if !self
            .operation
            .as_ref()
            .is_some_and(|op| op.state == OperationState::Running)
        {
            self.new_operation(OperationOrigin::Automatic, OperationState::Running);
        }
        let now = self.clock.now();
        let Ok(credential) = self.credential() else {
            self.finish_operation(OperationState::Unavailable);
            self.operation_failed = true;
            // Keep the restored board; the keyring is retried with the normal cadence.
            self.log(Event::Failed, None, now);
            return Some(self.snapshot(false));
        };
        let Some(credential) = credential else {
            self.updated_at = None;
            self.data = disconnected();
            self.finish_operation(OperationState::Unavailable);
            self.operation_failed = true;
            self.log(Event::MissingCredential, None, now);
            return Some(self.snapshot(false));
        };
        let credential_id = credential_id(&credential);
        if self.rejected.take().is_some_and(|id| id != credential_id) {
            self.failures = 0;
            self.failure_since = None;
        }
        let previous_data = self.data.clone();
        let boards_pending = self.boards_pending;
        let result = self.read(&credential);
        if let Some(latest) = self.changed_lifecycle() {
            self.data = previous_data;
            self.boards_pending = boards_pending;
            let cancelled = self.operation.clone();
            self.lifecycle(latest);
            self.operation = cancelled;
            self.finish_operation(OperationState::Cancelled);
            self.resume_pending = true;
            return Some(self.snapshot(false));
        }
        let status = result.as_ref().err().and_then(|error| error.status);
        match result {
            Ok(()) => {
                self.updated_at = Some(now);
                self.failures = 0;
                self.failure_since = None;
                self.retry_at = 0;
            }
            Err(error) if matches!(error.status, Some(401 | 403)) => {
                self.rejected = Some(credential_id);
                self.data["state"] = json!("invalidCredential");
            }
            Err(error) => {
                // Wait the full delay after a slow request finishes.
                let failed_at = self.clock.now();
                self.failure_since.get_or_insert(failed_at);
                self.failures += 1;
                self.retry_at = failed_at
                    .saturating_add(
                        BACKOFF[(self.failures - 1).min(BACKOFF.len() - 1)]
                            .max(error.retry_after_ms),
                    )
                    .max(error.retry_after_at.unwrap_or(0));
                self.data["state"] = json!(if self.failures >= 3 { "error" } else { "stale" });
            }
        }
        let success = self.data["state"] == "ready";
        self.finish_operation(if success {
            if self.operation_failed {
                OperationState::Recovered
            } else {
                OperationState::Succeeded
            }
        } else if self.data["state"] == "chooseBoard" {
            OperationState::Unavailable
        } else {
            OperationState::Failed
        });
        self.operation_failed = !success;
        let snapshot = self.snapshot(false);
        let event = match self.data["state"].as_str() {
            Some("ready") => {
                self.persistence.save(&snapshot);
                Event::Updated
            }
            Some("chooseBoard") => Event::Updated,
            Some("invalidCredential") => Event::InvalidCredential,
            _ => Event::Failed,
        };
        let status = status.or(matches!(event, Event::Updated).then_some(200));
        self.log(event, status, now);
        Some(snapshot)
    }
    /// Metadata only: never paths, board content or credentials.
    fn log(&self, event: Event, http_status: Option<u16>, started_at: u64) {
        self.persistence.log(LogRecord {
            source: Module::Trello,
            event,
            http_status,
            duration_ms: self.clock.now().saturating_sub(started_at),
        });
    }
    /// The chosen board's lists with open cards; open boards only when unknown,
    /// without a chosen board, or on manual refresh.
    fn read(&mut self, credential: &Credential) -> Result<(), HttpError> {
        let auth = Auth::new(&credential.key, &credential.token);
        let known: Vec<Board> =
            serde_json::from_value(self.data["boards"].clone()).unwrap_or_default();
        let boards = if self.boards_pending || known.is_empty() || credential.board.is_none() {
            let boards = self.boards(&auth)?;
            self.boards_pending = false;
            self.data["boards"] = json!(boards);
            boards
        } else {
            known
        };
        self.data["account"] = json!({"login": credential.login});
        let Some(board) = boards
            .into_iter()
            .find(|board| Some(&board.id) == credential.board.as_ref())
        else {
            self.data["state"] = json!("chooseBoard");
            self.data["board"] = Value::Null;
            self.data["lists"] = Value::Null;
            return Ok(());
        };
        let lists = self.api.get(&auth, &format!("/boards/{}/lists?cards=open&card_fields=name,due,dueComplete,labels,shortUrl&fields=name", board.id))?;
        let lists: Vec<RemoteList> =
            serde_json::from_value(lists).map_err(|_| HttpError::default())?;
        self.data["board"] = json!(board);
        self.data["lists"] = lists.into_iter().map(|list| json!({
            "id": list.id,
            "name": list.name,
            "cards": list.cards.into_iter().map(|card| json!({
                "id": card.id,
                "name": card.name,
                "url": card.short_url,
                "labels": card.labels.into_iter().filter_map(|label| label.color).collect::<Vec<_>>(),
                "due": card.due.and_then(|due| chrono::DateTime::parse_from_rfc3339(&due).ok()).map(|due| due.timestamp_millis().max(0)),
                "dueComplete": card.due_complete,
            })).collect::<Vec<_>>(),
        })).collect();
        self.data["state"] = json!("ready");
        Ok(())
    }
}
/// Order among remote sources after login or reconnection (GitHub is 0).
const SOURCE_POSITION: u64 = 1;
/// Transient failures: 30 s, then doubling up to 15 min.
const BACKOFF: [u64; 6] = [30_000, 60_000, 120_000, 240_000, 480_000, 900_000];

fn credential_id(credential: &Credential) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (&credential.key, &credential.token).hash(&mut hash);
    hash.finish()
}
fn validation_error(error: HttpError) -> CredentialError {
    if matches!(error.status, Some(400 | 401 | 403)) {
        CredentialError::InvalidCredential
    } else {
        CredentialError::Offline
    }
}
/// No account: the module stays visible with its setup state.
fn disconnected() -> Value {
    json!({"state":"missingCredential", "refreshing":false, "account":null, "boards":[], "board":null, "lists":null})
}

#[cfg(test)]
mod tests;

/// Only card links on trello.com open from the panel.
pub fn card_url(url: &str) -> Option<&str> {
    let id = url.strip_prefix("https://trello.com/c/")?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some(url)
}

/// Open a card page in the default browser; false for anything but a card link.
pub fn open_card(processes: &impl Processes, url: &str) -> bool {
    card_url(url).is_some_and(|url| processes.execute("xdg-open", &[url]).is_ok())
}

/// Where the user creates the app and API key.
pub fn open_admin(processes: &impl Processes) {
    let _ = processes.execute("xdg-open", &["https://trello.com/apps/admin"]);
}

type Reply<T> = tokio::sync::oneshot::Sender<Result<T, CredentialError>>;
pub enum Command {
    Refresh,
    Connect(String, String, Reply<String>),
    SelectBoard(String, Reply<()>),
    Disconnect(Reply<()>),
    Authorize(String, Reply<()>),
    /// Card and destination list.
    Move(
        String,
        String,
        tokio::sync::oneshot::Sender<Result<(), MoveError>>,
    ),
}
pub struct Collector(pub std::sync::mpsc::Sender<Command>);
pub fn start(app: tauri::AppHandle) {
    use tauri::Manager;
    let (tx, rx) = std::sync::mpsc::channel();
    app.manage(Collector(tx));
    std::thread::spawn(move || {
        let mut trello = Trello::new(TrelloHttp, crate::ports::SystemClock)
            .with_keyring(crate::ports::NativeKeyring)
            .with_lifecycle(app.state::<crate::lifecycle::Shared>().inner().clone());
        if let (Some(cache), Some(state)) = (dirs::cache_dir(), dirs::state_dir()) {
            use crate::persistence::{Cache, DiskStorage, LocalPersistence, Logs};
            trello = trello.with_persistence(LocalPersistence {
                cache: Cache(DiskStorage(cache.join("kvojps-monitor"))),
                logs: Logs(DiskStorage(state.join("kvojps-monitor"))),
            });
        }
        crate::snapshot::publish(&app, trello.snapshot(false));
        let mut previous_status = (false, 0, 0);
        loop {
            let command = rx.recv_timeout(std::time::Duration::from_secs(1)).ok();
            let manual = matches!(command, Some(Command::Refresh));
            let status = *app.state::<crate::lifecycle::Shared>().lock().unwrap();
            trello.lifecycle(status);
            match command {
                Some(Command::Connect(key, token, reply)) => {
                    let result = trello.connect(&key, &token);
                    crate::snapshot::publish(&app, trello.snapshot(false));
                    let _ = reply.send(result);
                }
                Some(Command::SelectBoard(id, reply)) => {
                    let result = trello.select_board(&id);
                    crate::snapshot::publish(&app, trello.snapshot(false));
                    let _ = reply.send(result);
                }
                Some(Command::Authorize(key, reply)) => {
                    let result = trello.authorize(&crate::ports::SystemProcesses, &key);
                    crate::snapshot::publish(&app, trello.snapshot(false));
                    let _ = reply.send(result);
                }
                Some(Command::Move(card, list, reply)) => {
                    let result = trello.move_card(&card, &list, |snapshot| {
                        crate::snapshot::publish(&app, snapshot)
                    });
                    let _ = reply.send(result);
                }
                Some(Command::Disconnect(reply)) => {
                    let result = trello.disconnect();
                    crate::snapshot::publish(&app, trello.snapshot(false));
                    let _ = reply.send(result);
                }
                _ => {}
            }
            let current_status = (status.online, status.generation, status.ready_at);
            if current_status != previous_status {
                previous_status = current_status;
                crate::snapshot::publish(&app, trello.snapshot(false));
            }
            let visible = crate::collection::is_visible(&app, Module::Trello);
            // Publish previous data before blocking on HTTP.
            if let Some(snapshot) = trello.begin(visible, manual) {
                crate::snapshot::publish(&app, snapshot);
            }
            if let Some(snapshot) = trello.step(visible, manual) {
                crate::snapshot::publish(&app, snapshot);
            }
        }
    });
}
