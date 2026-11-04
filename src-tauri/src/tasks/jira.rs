//! Jira Cloud: board-scoped pagination and issue-specific workflow actions.
use super::{Action, Connection, Error, Source, Transition};
use crate::{
    lifecycle,
    module::Module,
    ports::{Clock, HttpError, Keyring, Processes},
    snapshot::Snapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Serialize, Deserialize)]
struct Credential {
    site: String,
    email: String,
    token: String,
    account: String,
    login: String,
    board: Option<String>,
}
pub struct Auth<'a> {
    site: &'a str,
    email: &'a str,
    token: &'a str,
}
pub trait JiraApi: Send {
    fn get(&self, auth: &Auth, path: &str, query: &[(String, String)]) -> Result<Value, HttpError>;
    fn transition(&self, auth: &Auth, item: &str, transition: &str) -> Result<(), HttpError>;
}
pub struct JiraHttp;
impl JiraHttp {
    fn request(
        method: reqwest::Method,
        auth: &Auth,
        path: &str,
        query: &[(String, String)],
    ) -> Result<reqwest::blocking::RequestBuilder, HttpError> {
        if canonical_site(auth.site).as_deref() != Some(auth.site) || !path.starts_with("/rest/") {
            return Err(HttpError::default());
        }
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| HttpError::default())?;
        let mut url = reqwest::Url::parse(&format!("{}{path}", auth.site))
            .map_err(|_| HttpError::default())?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        }
        Ok(client
            .request(method, url)
            .basic_auth(auth.email, Some(auth.token))
            .header("Accept", "application/json"))
    }
    fn response(
        response: reqwest::blocking::Response,
    ) -> Result<reqwest::blocking::Response, HttpError> {
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(HttpError::response(
                response.status().as_u16(),
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok()),
            ))
        }
    }
}
impl JiraApi for JiraHttp {
    fn get(&self, auth: &Auth, path: &str, query: &[(String, String)]) -> Result<Value, HttpError> {
        let response = Self::request(reqwest::Method::GET, auth, path, query)?
            .send()
            .map_err(|_| HttpError::default())?;
        Self::response(response)?
            .json()
            .map_err(|_| HttpError::default())
    }
    fn transition(&self, auth: &Auth, item: &str, transition: &str) -> Result<(), HttpError> {
        if !numeric(item) || !numeric(transition) {
            return Err(HttpError::response(400, None));
        }
        let response = Self::request(
            reqwest::Method::POST,
            auth,
            &format!("/rest/api/3/issue/{item}/transitions"),
            &[],
        )?
        .json(&json!({"transition":{"id":transition}}))
        .send()
        .map_err(|_| HttpError::default())?;
        Self::response(response).map(|_| ())
    }
}

pub fn canonical_site(site: &str) -> Option<String> {
    let url = reqwest::Url::parse(site.trim()).ok()?;
    let host = url.host_str()?;
    let tenant = host.strip_suffix(".atlassian.net")?;
    (url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path() == "/"
        && !tenant.is_empty()
        && !tenant.contains('.')
        && tenant
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'))
    .then(|| format!("https://{host}"))
}
fn numeric(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|c| c.is_ascii_digit())
}
fn auth(c: &Credential) -> Auth<'_> {
    Auth {
        site: &c.site,
        email: &c.email,
        token: &c.token,
    }
}
fn error(e: HttpError) -> Error {
    match e.status {
        Some(400 | 401) => Error::InvalidCredential,
        Some(403 | 404) => Error::Refused,
        _ => Error::Offline,
    }
}
fn empty() -> Value {
    json!({"state":"missingCredential","refreshing":false,"account":null,"boards":[],"board":null,"columns":[],"issues":[]})
}

pub struct Jira<A, R> {
    api: A,
    clock: R,
    keyring: Box<dyn Keyring>,
    lifecycle: Option<lifecycle::Status>,
    current: Option<Box<dyn lifecycle::CurrentStatus>>,
    data: Value,
    updated_at: Option<u64>,
    attempted: Option<u64>,
    retry_at: u64,
    failures: u32,
    pending: bool,
    rejected: bool,
    operation: u64,
    requires_read: bool,
    ready_at_monotonic: u64,
    rate_limit: std::sync::Mutex<u64>,
    manual_at: Option<u64>,
    resume_pending: bool,
}
impl<A: JiraApi, R: Clock> Jira<A, R> {
    pub fn new(api: A, clock: R, keyring: impl Keyring + 'static) -> Self {
        Self {
            api,
            clock,
            keyring: Box::new(keyring),
            lifecycle: None,
            current: None,
            data: empty(),
            updated_at: None,
            attempted: None,
            retry_at: 0,
            failures: 0,
            pending: true,
            rejected: false,
            operation: 0,
            requires_read: true,
            ready_at_monotonic: 0,
            rate_limit: std::sync::Mutex::new(0),
            manual_at: None,
            resume_pending: false,
        }
    }
    pub fn with_lifecycle(mut self, current: impl lifecycle::CurrentStatus + 'static) -> Self {
        self.lifecycle(current.current());
        self.current = Some(Box::new(current));
        self
    }
    fn note_error(&self, e: HttpError) {
        let delay = e
            .retry_after_ms
            .max(
                e.retry_after_at
                    .unwrap_or(0)
                    .saturating_sub(self.clock.now()),
            )
            .max(if e.status == Some(429) { 30_000 } else { 0 });
        if delay > 0 {
            *self.rate_limit.lock().unwrap() = self.clock.monotonic_ms().saturating_add(delay);
        }
    }
    fn get(&self, auth: &Auth, path: &str, query: &[(String, String)]) -> Result<Value, HttpError> {
        let until = *self.rate_limit.lock().unwrap();
        if self.clock.monotonic_ms() < until {
            return Err(HttpError::response(429, None));
        }
        self.api
            .get(auth, path, query)
            .inspect_err(|e| self.note_error(*e))
    }
    fn credential(&self) -> Result<Option<Credential>, Error> {
        self.keyring
            .read("jira")
            .map_err(|_| Error::KeyringUnavailable)?
            .map(|s| {
                serde_json::from_str::<Credential>(&s)
                    .map_err(|_| Error::InvalidCredential)
                    .and_then(|c| {
                        if canonical_site(&c.site).as_deref() != Some(c.site.as_str()) {
                            Err(Error::InvalidSite)
                        } else {
                            Ok(c)
                        }
                    })
            })
            .transpose()
    }
    fn save(&self, c: &Credential) -> Result<(), Error> {
        self.keyring
            .save(
                "jira",
                &serde_json::to_string(c).map_err(|_| Error::KeyringUnavailable)?,
            )
            .map_err(|_| Error::KeyringUnavailable)
    }
    fn paused(&self) -> bool {
        self.lifecycle.is_some_and(|s| !s.online)
            || self.clock.monotonic_ms() < self.ready_at_monotonic
    }
    fn changed(&self) -> bool {
        self.current.as_ref().is_some_and(|c| {
            let now = c.current();
            !now.online
                || self
                    .lifecycle
                    .is_some_and(|old| old.generation != now.generation)
        })
    }
    fn boards(&self, c: &Credential) -> Result<Vec<Value>, Error> {
        let mut result = vec![];
        let mut start = 0;
        let started = self.clock.monotonic_ms();
        for _ in 0..1000 {
            if self.clock.monotonic_ms().saturating_sub(started) >= 60_000 {
                return Err(Error::Offline);
            }
            let page = self
                .get(
                    &auth(c),
                    "/rest/agile/1.0/board",
                    &[
                        ("startAt".into(), start.to_string()),
                        ("maxResults".into(), "100".into()),
                    ],
                )
                .map_err(error)?;
            let values = page["values"].as_array().ok_or(Error::UnsupportedBoard)?;
            for board in values {
                let id = board["id"]
                    .as_u64()
                    .ok_or(Error::UnsupportedBoard)?
                    .to_string();
                let name = board["name"].as_str().ok_or(Error::UnsupportedBoard)?;
                result.push(json!({"id":id,"name":name}));
            }
            if page["isLast"] == true
                || page["total"]
                    .as_u64()
                    .is_some_and(|t| start + values.len() as u64 >= t)
            {
                return Ok(result);
            }
            if values.is_empty() {
                return Err(Error::UnsupportedBoard);
            }
            start += values.len() as u64;
        }
        Err(Error::UnsupportedBoard)
    }
    fn read(&self, c: &Credential) -> Result<Value, Error> {
        let boards = self.boards(c)?;
        let account = json!({"login":c.login});
        let Some(board) = boards
            .iter()
            .find(|b| b["id"].as_str() == c.board.as_deref())
            .cloned()
        else {
            return Ok(
                json!({"state":"chooseBoard","account":account,"boards":boards,"board":null,"columns":[],"issues":[]}),
            );
        };
        let id = c.board.as_deref().ok_or(Error::UnsupportedBoard)?;
        if !numeric(id) {
            return Err(Error::UnsupportedBoard);
        }
        let config = self
            .get(
                &auth(c),
                &format!("/rest/agile/1.0/board/{id}/configuration"),
                &[],
            )
            .map_err(error)?;
        let remote_columns = config["columnConfig"]["columns"]
            .as_array()
            .filter(|c| !c.is_empty())
            .ok_or(Error::UnsupportedBoard)?;
        let mut known_statuses = HashSet::new();
        let mut columns = vec![];
        for (index, column) in remote_columns.iter().enumerate() {
            let statuses = column["statuses"]
                .as_array()
                .ok_or(Error::UnsupportedBoard)?
                .iter()
                .map(|s| {
                    s["id"]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or(Error::UnsupportedBoard)
                })
                .collect::<Result<Vec<_>, _>>()?;
            known_statuses.extend(statuses.clone());
            columns.push(json!({"id":index.to_string(),"name":column["name"].as_str().ok_or(Error::UnsupportedBoard)?,"statuses":statuses}));
        }
        let mut issues = vec![];
        let mut seen_ids = HashSet::new();
        let mut tokens = HashSet::new();
        let mut token: Option<String> = None;
        let started = self.clock.monotonic_ms();
        for _ in 0..1000 {
            if self.clock.monotonic_ms().saturating_sub(started) >= 60_000 {
                return Err(Error::Offline);
            }
            let mut query = vec![
                ("maxResults".into(), "100".into()),
                (
                    "fields".into(),
                    "summary,status,issuetype,priority,assignee,duedate".into(),
                ),
            ];
            if let Some(token) = &token {
                query.push(("nextPageToken".into(), token.clone()));
            }
            let page = self
                .get(
                    &auth(c),
                    &format!("/rest/software/1.0/board/{id}/issue"),
                    &query,
                )
                .map_err(error)?;
            for issue in page["issues"].as_array().ok_or(Error::UnsupportedBoard)? {
                let id = issue["id"]
                    .as_str()
                    .filter(|id| numeric(id))
                    .ok_or(Error::UnsupportedBoard)?;
                if !seen_ids.insert(id.to_owned()) {
                    continue;
                }
                let key = issue["key"]
                    .as_str()
                    .filter(|key| {
                        !key.is_empty()
                            && key
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    })
                    .ok_or(Error::UnsupportedBoard)?;
                let fields = &issue["fields"];
                let status = fields["status"]["id"]
                    .as_str()
                    .ok_or(Error::UnsupportedBoard)?;
                if !known_statuses.contains(status) {
                    return Err(Error::UnsupportedBoard);
                }
                let optional =
                    |field: &str, key: &str| fields[field][key].as_str().map(str::to_owned);
                issues.push(json!({"id":id,"key":key,"title":fields["summary"].as_str().ok_or(Error::UnsupportedBoard)?,"status":{"id":status,"name":fields["status"]["name"].as_str().ok_or(Error::UnsupportedBoard)?},"type":optional("issuetype","name"),"priority":optional("priority","name"),"assignee":optional("assignee","displayName"),"due":fields["duedate"].as_str(),"url":format!("{}/browse/{key}",c.site)}));
            }
            if page["isLast"] == true {
                return Ok(
                    json!({"state":"ready","account":account,"boards":boards,"board":board,"columns":columns,"issues":issues}),
                );
            }
            let next = page["nextPageToken"]
                .as_str()
                .filter(|t| !t.is_empty() && t.len() <= 4096)
                .ok_or(Error::UnsupportedBoard)?;
            if !tokens.insert(next.to_owned()) {
                return Err(Error::UnsupportedBoard);
            }
            token = Some(next.to_owned());
        }
        Err(Error::UnsupportedBoard)
    }
    fn known_item(&self, item: &str) -> bool {
        numeric(item)
            && self.data["issues"]
                .as_array()
                .is_some_and(|v| v.iter().any(|i| i["id"] == item))
    }
}
impl<A: JiraApi, R: Clock> Source for Jira<A, R> {
    fn snapshot(&self, refreshing: bool) -> Snapshot {
        let mut data = self.data.clone();
        data["paused"] = json!(self.paused());
        data["refreshing"] = json!(refreshing && !self.paused());
        data["requiresRead"] = json!(self.requires_read);
        data["lastSuccessAt"] = json!(self.updated_at);
        data["rateLimited"] = json!(self.clock.monotonic_ms() < *self.rate_limit.lock().unwrap());
        Snapshot {
            module: Module::Tasks,
            updated_at: self.updated_at,
            data,
        }
    }
    fn poll(
        &mut self,
        visible: bool,
        manual: bool,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Option<Snapshot> {
        let now = self.clock.monotonic_ms();
        let manual = manual
            && self
                .manual_at
                .is_none_or(|at| now.saturating_sub(at) >= 10_000);
        let interval = if visible { 30_000 } else { 300_000 };
        if self.paused()
            || self.rejected
            || now < self.retry_at
            || now < *self.rate_limit.lock().unwrap()
            || (!self.pending
                && !manual
                && self
                    .attempted
                    .is_some_and(|at| now.saturating_sub(at) < interval))
        {
            return None;
        }
        if manual {
            self.manual_at = Some(now);
        }
        let origin = if manual {
            "user"
        } else if self.resume_pending {
            "resume"
        } else {
            "automatic"
        };
        self.resume_pending = false;
        self.operation += 1;
        self.data["operation"] = json!({"id":self.operation,"origin":origin,"state":"running"});
        progress(self.snapshot(true));
        self.attempted = Some(now);
        self.pending = false;
        let result = self.credential().and_then(|c| match c {
            Some(c) => self.read(&c),
            None => Ok(empty()),
        });
        if self.changed() {
            self.requires_read = true;
            self.data["operation"]["state"] = json!("cancelled");
            return Some(self.snapshot(false));
        }
        match result {
            Ok(mut data) => {
                data["operation"] =
                    json!({"id":self.operation,"origin":origin,"state":"succeeded"});
                self.requires_read = data["state"] != "ready";
                if data["state"] == "ready" {
                    self.updated_at = Some(self.clock.now());
                }
                self.data = data;
                self.failures = 0;
                self.retry_at = 0;
            }
            Err(reason) => {
                self.requires_read = true;
                self.failures = self.failures.saturating_add(1);
                self.retry_at = self.clock.monotonic_ms().saturating_add(
                    (30_000u64.saturating_mul(1 << self.failures.min(6).saturating_sub(1)))
                        .min(900_000),
                );
                self.rejected = reason == Error::InvalidCredential;
                self.data["state"] = json!(if self.rejected {
                    "invalidCredential"
                } else {
                    "error"
                });
                self.data["reason"] = json!(reason);
                self.data["operation"]["state"] = json!("failed");
            }
        }
        Some(self.snapshot(false))
    }
    fn connect(&mut self, connection: Connection) -> Result<String, Error> {
        let Connection::Jira { site, email, token } = connection else {
            return Err(Error::InvalidCredential);
        };
        let site = canonical_site(&site).ok_or(Error::InvalidSite)?;
        let (email, token) = (email.trim(), token.trim());
        if !email.contains('@')
            || email.contains(['\r', '\n', ':'])
            || token.is_empty()
            || token.contains(['\r', '\n'])
        {
            return Err(Error::InvalidCredential);
        }
        if self.paused() {
            return Err(Error::Offline);
        }
        let mut c = Credential {
            site,
            email: email.into(),
            token: token.into(),
            account: String::new(),
            login: String::new(),
            board: None,
        };
        let me = self
            .get(&auth(&c), "/rest/api/3/myself", &[])
            .map_err(error)?;
        c.account = me["accountId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(Error::InvalidCredential)?
            .into();
        if me["active"] == false {
            return Err(Error::InvalidCredential);
        }
        c.login = me["displayName"].as_str().unwrap_or("Conta Jira").into();
        let boards = self.boards(&c)?;
        let previous = match self.credential() {
            Err(Error::InvalidCredential | Error::InvalidSite) => None,
            result => result?,
        };
        c.board = previous
            .filter(|old| old.account == c.account && old.site == c.site)
            .and_then(|old| old.board)
            .filter(|id| boards.iter().any(|b| b["id"] == *id));
        if self.changed() {
            return Err(Error::Offline);
        }
        self.save(&c)?;
        self.reset();
        self.data["state"] = json!("chooseBoard");
        self.data["boards"] = json!(boards);
        self.data["account"] = json!({"login":c.login});
        Ok(c.login)
    }
    fn select_board(&mut self, id: &str) -> Result<(), Error> {
        if !numeric(id)
            || !self.data["boards"]
                .as_array()
                .is_some_and(|b| b.iter().any(|b| b["id"] == id))
        {
            return Err(Error::Refused);
        }
        let mut c = self.credential()?.ok_or(Error::InvalidCredential)?;
        c.board = Some(id.into());
        self.save(&c)?;
        self.reset();
        Ok(())
    }
    fn disconnect(&mut self) -> Result<(), Error> {
        self.keyring
            .delete("jira")
            .map_err(|_| Error::KeyringUnavailable)?;
        self.reset();
        Ok(())
    }
    fn reset(&mut self) {
        self.data = empty();
        self.updated_at = None;
        self.attempted = None;
        self.manual_at = None;
        self.retry_at = 0;
        self.failures = 0;
        self.rejected = false;
        self.pending = true;
        self.requires_read = true;
    }
    fn restore(&mut self, _: Snapshot) {}
    fn transitions(&self, item: &str) -> Result<Vec<Transition>, Error> {
        if self.paused() || self.requires_read || !self.known_item(item) {
            return Err(Error::ReadRequired);
        }
        let c = self.credential()?.ok_or(Error::InvalidCredential)?;
        let response = self
            .get(
                &auth(&c),
                &format!("/rest/api/3/issue/{item}/transitions"),
                &[("expand".into(), "transitions.fields".into())],
            )
            .map_err(error)?;
        response["transitions"]
            .as_array()
            .ok_or(Error::Refused)?
            .iter()
            .filter(|t| t["isAvailable"] != false)
            .map(|t| {
                Ok(Transition {
                    id: t["id"]
                        .as_str()
                        .filter(|id| numeric(id))
                        .ok_or(Error::Refused)?
                        .into(),
                    name: t["name"].as_str().ok_or(Error::Refused)?.into(),
                    status: t["to"]["name"].as_str().ok_or(Error::Refused)?.into(),
                    requires_fields: t["fields"]
                        .as_object()
                        .map_or(t["hasScreen"] == true, |fields| {
                            fields.values().any(|f| f["required"] == true)
                        }),
                })
            })
            .collect()
    }
    fn perform(&mut self, action: Action, progress: &mut dyn FnMut(Snapshot)) -> Result<(), Error> {
        let Action::Transition { item, transition } = action else {
            return Err(Error::Refused);
        };
        let choices = self.transitions(&item)?;
        let chosen = choices
            .iter()
            .find(|t| t.id == transition)
            .ok_or(Error::Refused)?;
        if chosen.requires_fields {
            return Err(Error::RequiredFields);
        }
        let c = self.credential()?.ok_or(Error::InvalidCredential)?;
        self.operation += 1;
        self.data["movement"] = json!({"id":self.operation,"origin":"user","state":"running"});
        self.requires_read = true;
        progress(self.snapshot(false));
        let result = self
            .api
            .transition(&auth(&c), &item, &transition)
            .inspect_err(|e| self.note_error(*e));
        let result = match result {
            Ok(()) => Ok(()),
            Err(e) if e.status.is_none_or(|s| s >= 500 || s == 408) => Err(Error::Unconfirmed),
            Err(_) => Err(Error::Refused),
        };
        self.data["movement"]["state"] = json!(match result {
            Ok(()) => "succeeded",
            Err(Error::Unconfirmed) => "unconfirmed",
            Err(_) => "failed",
        });
        self.pending = true;
        result
    }
    fn authorize(&mut self, _: &str, _: &dyn Processes) -> Result<(), Error> {
        Err(Error::Refused)
    }
    fn lifecycle(&mut self, status: lifecycle::Status) {
        if self
            .lifecycle
            .is_none_or(|s| s.generation != status.generation || s.ready_at != status.ready_at)
        {
            self.ready_at_monotonic = self
                .clock
                .monotonic_ms()
                .saturating_add(status.ready_for(1).saturating_sub(self.clock.now()));
        }
        if self
            .lifecycle
            .is_none_or(|s| s.generation != status.generation)
        {
            self.resume_pending = self.lifecycle.is_some();
            self.pending = true;
            self.retry_at = 0;
            self.requires_read = true;
        }
        self.lifecycle = Some(status);
    }
}
