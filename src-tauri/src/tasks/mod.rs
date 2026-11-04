//! One active task provider, durable cleanup, and context-bound commands.
mod jira;
mod source;
mod worker;
pub use source::{Action, Connection, Error, Provider, Source, Transition};
pub use worker::{start, Collector, Command};

use crate::{
    module::Module,
    persistence::{Persistence, Storage},
    snapshot::Snapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

const SETTINGS: &str = "tasks.json";

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum CleanupAction {
    Switch,
    Disconnect,
    Reset,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Stage {
    Cache,
    Credential,
    Commit,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
struct Intent {
    action: CleanupAction,
    source: Provider,
    destination: Provider,
    stage: Stage,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    schema_version: u64,
    provider: Provider,
    cleanup: Option<Intent>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            provider: Provider::Trello,
            cleanup: None,
        }
    }
}

pub struct Tasks {
    trello: Box<dyn Source>,
    jira: Box<dyn Source>,
    persistence: Box<dyn Persistence>,
    storage: Box<dyn Storage>,
    settings: Settings,
    generation: u64,
    read_valid: bool,
    settings_unavailable: bool,
    write_busy: bool,
    cleanup_failed: bool,
    lifecycle_generation: Option<u64>,
}

impl Tasks {
    pub fn new(
        trello: impl Source + 'static,
        jira: impl Source + 'static,
        persistence: impl Persistence + 'static,
        storage: impl Storage + 'static,
    ) -> Self {
        let loaded = match storage.read(SETTINGS) {
            Ok(text) => serde_json::from_str::<Settings>(&text)
                .ok()
                .filter(|s| s.schema_version == 1),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(Settings::default()),
            Err(_) => None,
        };
        let mut tasks = Self {
            trello: Box::new(trello),
            jira: Box::new(jira),
            persistence: Box::new(persistence),
            storage: Box::new(storage),
            settings_unavailable: loaded.is_none(),
            settings: loaded.unwrap_or_default(),
            generation: 1,
            read_valid: false,
            write_busy: false,
            cleanup_failed: false,
            lifecycle_generation: None,
        };
        // A pending intent always wins over cache restoration and polling.
        if tasks.settings.cleanup.is_some() {
            let _ = tasks.retry_cleanup(1);
        }
        if !tasks.settings_unavailable
            && tasks.settings.cleanup.is_none()
            && tasks.settings.provider == Provider::Trello
        {
            if let Some(snapshot) = tasks.persistence.load(Module::Tasks) {
                tasks.trello.restore(snapshot);
            }
        }
        tasks
    }
    fn source(&self) -> &dyn Source {
        match self.settings.provider {
            Provider::Trello => &*self.trello,
            Provider::Jira => &*self.jira,
        }
    }
    fn source_mut(&mut self) -> &mut dyn Source {
        match self.settings.provider {
            Provider::Trello => &mut *self.trello,
            Provider::Jira => &mut *self.jira,
        }
    }
    fn check(&self, expected: u64) -> Result<(), Error> {
        if expected != self.generation {
            return Err(Error::StaleContext);
        }
        if self.settings_unavailable {
            return Err(Error::SettingsUnavailable);
        }
        if self.settings.cleanup.is_some() {
            return Err(Error::CleanupFailed);
        }
        Ok(())
    }
    fn save(&self, settings: &Settings) -> Result<(), Error> {
        let text = serde_json::to_string(settings).map_err(|_| Error::SettingsUnavailable)?;
        self.storage
            .write("tasks.json.tmp", &text)
            .map_err(|_| Error::SettingsUnavailable)?;
        self.storage
            .rename("tasks.json.tmp", SETTINGS)
            .map_err(|_| Error::SettingsUnavailable)
    }
    fn begin_change(&mut self, action: CleanupAction, destination: Provider) -> Result<(), Error> {
        let next = Settings {
            cleanup: Some(Intent {
                action,
                source: self.settings.provider,
                destination,
                stage: Stage::Cache,
            }),
            ..self.settings.clone()
        };
        self.save(&next)?;
        self.settings = next;
        self.generation += 1;
        self.read_valid = false;
        self.cleanup_failed = false;
        Ok(())
    }
    fn clean(&mut self) -> Result<(), Error> {
        let result = self.clean_inner();
        if result.is_err() {
            self.cleanup_failed = true;
            self.trello.reset();
            self.jira.reset();
        }
        result
    }
    fn clean_inner(&mut self) -> Result<(), Error> {
        let mut intent = self.settings.cleanup.ok_or(Error::CleanupFailed)?;
        if matches!(intent.stage, Stage::Cache) {
            self.persistence
                .clear(Module::Tasks)
                .map_err(|_| Error::CleanupFailed)?;
            intent.stage = if matches!(intent.action, CleanupAction::Disconnect) {
                Stage::Credential
            } else {
                Stage::Commit
            };
            let next = Settings {
                cleanup: Some(intent),
                ..self.settings.clone()
            };
            self.save(&next).map_err(|_| Error::CleanupFailed)?;
            self.settings = next;
        }
        if matches!(intent.stage, Stage::Credential) {
            self.source_mut().disconnect()?;
            intent.stage = Stage::Commit;
            let next = Settings {
                cleanup: Some(intent),
                ..self.settings.clone()
            };
            self.save(&next).map_err(|_| Error::CleanupFailed)?;
            self.settings = next;
        }
        Ok(())
    }
    fn finish_change(&mut self) -> Result<(), Error> {
        let intent = self.settings.cleanup.ok_or(Error::CleanupFailed)?;
        let next = Settings {
            provider: intent.destination,
            cleanup: None,
            ..self.settings.clone()
        };
        if self.save(&next).is_err() {
            self.cleanup_failed = true;
            self.trello.reset();
            self.jira.reset();
            return Err(Error::CleanupFailed);
        }
        self.settings = next;
        self.cleanup_failed = false;
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        if self.settings_unavailable || self.settings.cleanup.is_some() {
            return Snapshot {
                module: Module::Tasks,
                updated_at: None,
                data: json!({"provider":self.settings.provider,"contextGeneration":self.generation,"state":if self.settings_unavailable {"settingsUnavailable"} else {"cleanupPending"},"cleanupFailed":self.cleanup_failed,"cleanupPending":self.settings.cleanup.is_some(),"refreshing":false,"canWrite":false,"board":null,"boards":[],"lists":null}),
            };
        }
        self.decorate(self.source().snapshot(false))
    }
    fn decorate(&self, snapshot: Snapshot) -> Snapshot {
        decorate(
            snapshot,
            self.settings.provider,
            self.generation,
            self.read_valid,
            self.write_busy,
        )
    }
    pub fn switch(
        &mut self,
        expected: u64,
        provider: Provider,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Result<(), Error> {
        self.check(expected)?;
        if provider == self.settings.provider {
            return Ok(());
        }
        self.begin_change(CleanupAction::Switch, provider)?;
        self.trello.reset();
        self.jira.reset();
        progress(self.snapshot());
        self.clean()?;
        self.finish_change()
    }
    pub fn disconnect(
        &mut self,
        expected: u64,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Result<(), Error> {
        self.check(expected)?;
        self.begin_change(CleanupAction::Disconnect, self.settings.provider)?;
        self.trello.reset();
        self.jira.reset();
        progress(self.snapshot());
        self.clean()?;
        self.finish_change()
    }
    pub fn retry_cleanup(&mut self, expected: u64) -> Result<(), Error> {
        if expected != self.generation {
            return Err(Error::StaleContext);
        }
        if self.settings_unavailable {
            return Err(Error::SettingsUnavailable);
        }
        if self.settings.cleanup.is_none() {
            return Ok(());
        }
        self.trello.reset();
        self.jira.reset();
        self.clean()?;
        self.finish_change()
    }
    pub fn connect(
        &mut self,
        expected: u64,
        connection: Connection,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Result<String, Error> {
        self.check(expected)?;
        let matches = matches!(
            (&connection, self.settings.provider),
            (Connection::Trello { .. }, Provider::Trello)
                | (Connection::Jira { .. }, Provider::Jira)
        );
        if !matches {
            return Err(Error::StaleContext);
        }
        self.begin_change(CleanupAction::Reset, self.settings.provider)?;
        progress(self.snapshot());
        self.clean()?;
        let result = self.source_mut().connect(connection);
        if result.is_err() {
            self.source_mut().reset();
        }
        self.finish_change()?;
        result
    }
    pub fn select_board(
        &mut self,
        expected: u64,
        id: &str,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Result<(), Error> {
        self.check(expected)?;
        if !self.source().snapshot(false).data["boards"]
            .as_array()
            .is_some_and(|b| b.iter().any(|b| b["id"] == id))
        {
            return Err(Error::Refused);
        }
        self.begin_change(CleanupAction::Reset, self.settings.provider)?;
        progress(self.snapshot());
        self.clean()?;
        let result = self.source_mut().select_board(id);
        if result.is_err() {
            self.source_mut().reset();
        }
        self.finish_change()?;
        result
    }
    pub fn lifecycle(&mut self, status: crate::lifecycle::Status) {
        if !status.online || self.lifecycle_generation != Some(status.generation) {
            self.read_valid = false;
        }
        self.lifecycle_generation = Some(status.generation);
        self.source_mut().lifecycle(status);
    }
    pub fn poll(&mut self, visible: bool, manual: bool, progress: &mut dyn FnMut(Snapshot)) {
        if self.settings_unavailable || self.settings.cleanup.is_some() {
            return;
        }
        let (provider, generation) = (self.settings.provider, self.generation);
        let result = self.source_mut().poll(visible, manual, &mut |s| {
            progress(decorate(s, provider, generation, false, false))
        });
        if let Some(snapshot) = result {
            self.read_valid = fresh(&snapshot);
            if provider == Provider::Trello && self.read_valid {
                self.persistence.save(&snapshot);
            }
            progress(self.decorate(snapshot));
        }
    }
    pub fn transitions(&self, expected: u64, item: &str) -> Result<Vec<Transition>, Error> {
        self.check(expected)?;
        if !self.read_valid {
            return Err(Error::ReadRequired);
        }
        self.source().transitions(item)
    }
    pub fn perform(
        &mut self,
        expected: u64,
        action: Action,
        progress: &mut dyn FnMut(Snapshot),
    ) -> Result<(), Error> {
        self.check(expected)?;
        if !self.read_valid {
            return Err(Error::ReadRequired);
        }
        self.read_valid = false;
        self.write_busy = true;
        let (provider, generation) = (self.settings.provider, self.generation);
        let result = self.source_mut().perform(action, &mut |s| {
            progress(decorate(s, provider, generation, false, true))
        });
        self.write_busy = false;
        let snapshot = self.source().snapshot(false);
        self.read_valid = result.is_ok() && fresh(&snapshot);
        if provider == Provider::Trello && result.is_ok() {
            self.persistence.save(&snapshot);
        }
        progress(self.decorate(snapshot));
        result
    }
    pub fn authorize(
        &mut self,
        expected: u64,
        key: &str,
        processes: &dyn crate::ports::Processes,
    ) -> Result<(), Error> {
        self.check(expected)?;
        self.source_mut().authorize(key, processes)
    }
    pub fn open(
        &self,
        expected: u64,
        url: &str,
        processes: &dyn crate::ports::Processes,
    ) -> Result<(), Error> {
        self.check(expected)?;
        let data = self.source().snapshot(false).data;
        let known = data["lists"].as_array().is_some_and(|lists| {
            lists.iter().any(|list| {
                list["cards"]
                    .as_array()
                    .is_some_and(|cards| cards.iter().any(|card| card["url"] == url))
            })
        }) || data["issues"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue["url"] == url));
        if !known {
            return Err(Error::Refused);
        }
        if self.settings.provider == Provider::Trello {
            return crate::trello::open_card(processes, url)
                .then_some(())
                .ok_or(Error::Refused);
        }
        processes
            .execute("xdg-open", &[url])
            .map(|_| ())
            .map_err(|_| Error::Refused)
    }
}
fn fresh(snapshot: &Snapshot) -> bool {
    snapshot.data["state"] == "ready"
        && snapshot.data["paused"] != true
        && snapshot.data["requiresRead"] != true
        && matches!(
            snapshot.data["operation"]["state"].as_str(),
            Some("succeeded" | "recovered")
        )
}
fn decorate(
    mut snapshot: Snapshot,
    provider: Provider,
    generation: u64,
    can_write: bool,
    busy: bool,
) -> Snapshot {
    snapshot.module = Module::Tasks;
    snapshot.data["provider"] = json!(provider);
    snapshot.data["contextGeneration"] = json!(generation);
    snapshot.data["canWrite"] =
        json!(can_write && snapshot.data["paused"] != true && snapshot.data["rateLimited"] != true);
    snapshot.data["writeBusy"] = json!(busy);
    snapshot.data["initializing"] = json!(
        snapshot.data["paused"] != true
            && snapshot.data["state"] == "missingCredential"
            && (snapshot.data["operation"].is_null()
                || snapshot.data["operation"]["state"] == "running")
    );
    for key in ["operation", "movement"] {
        if snapshot.data[key].is_object() {
            snapshot.data[key]["contextGeneration"] = json!(generation);
        }
    }
    snapshot
}

#[cfg(test)]
mod tests;
