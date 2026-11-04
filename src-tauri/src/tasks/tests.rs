use super::*;
use crate::persistence::{Cache, LocalPersistence, Logs};
use crate::ports::{Clock, HttpError, Keyring};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};

fn fixture(provider: &str, name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            crate::ports::fakes::fixtures().join(format!("{provider}/{name}.json")),
        )
        .unwrap(),
    )
    .unwrap()
}
#[derive(Clone)]
struct Time {
    wall: Arc<Mutex<u64>>,
    mono: Arc<Mutex<u64>>,
}
impl Default for Time {
    fn default() -> Self {
        Self {
            wall: Arc::new(Mutex::new(1_793_448_000_000)),
            mono: Arc::new(Mutex::new(0)),
        }
    }
}
impl Clock for Time {
    fn now(&self) -> u64 {
        *self.wall.lock().unwrap()
    }
    fn monotonic_ms(&self) -> u64 {
        *self.mono.lock().unwrap()
    }
}
impl Time {
    fn advance(&self, ms: u64) {
        *self.mono.lock().unwrap() += ms;
        *self.wall.lock().unwrap() += ms;
    }
}

#[derive(Clone, Default)]
struct Disk {
    files: Arc<Mutex<HashMap<String, String>>>,
    write_fails: Arc<Mutex<bool>>,
    remove_fails: Arc<Mutex<bool>>,
    rename_fails: Arc<Mutex<bool>>,
    rename_calls: Arc<Mutex<usize>>,
    fail_rename_at: Arc<Mutex<Option<usize>>>,
}
impl Storage for Disk {
    fn read(&self, p: &str) -> io::Result<String> {
        self.files
            .lock()
            .unwrap()
            .get(p)
            .cloned()
            .ok_or(io::ErrorKind::NotFound.into())
    }
    fn write(&self, p: &str, data: &str) -> io::Result<()> {
        if *self.write_fails.lock().unwrap() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.files.lock().unwrap().insert(p.into(), data.into());
        Ok(())
    }
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let mut calls = self.rename_calls.lock().unwrap();
        *calls += 1;
        if *self.rename_fails.lock().unwrap()
            || *self.fail_rename_at.lock().unwrap() == Some(*calls)
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let mut files = self.files.lock().unwrap();
        let data = files.remove(from).ok_or(io::ErrorKind::NotFound)?;
        files.insert(to.into(), data);
        Ok(())
    }
    fn remove(&self, p: &str) -> io::Result<()> {
        if *self.remove_fails.lock().unwrap() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.files.lock().unwrap().remove(p);
        Ok(())
    }
}
#[derive(Clone, Default)]
struct Vault {
    values: Arc<Mutex<HashMap<String, String>>>,
    unavailable: Arc<Mutex<bool>>,
    deletion_fails: Arc<Mutex<bool>>,
}
impl Keyring for Vault {
    fn read(&self, p: &str) -> Result<Option<String>, ()> {
        if *self.unavailable.lock().unwrap() {
            Err(())
        } else {
            Ok(self.values.lock().unwrap().get(p).cloned())
        }
    }
    fn save(&self, p: &str, secret: &str) -> Result<(), ()> {
        if *self.unavailable.lock().unwrap() {
            return Err(());
        }
        self.values.lock().unwrap().insert(p.into(), secret.into());
        Ok(())
    }
    fn delete(&self, p: &str) -> Result<(), ()> {
        if *self.deletion_fails.lock().unwrap() {
            return Err(());
        }
        self.values.lock().unwrap().remove(p);
        Ok(())
    }
}
#[derive(Clone, Default)]
struct TrelloHttp(Arc<Mutex<Vec<String>>>);
impl crate::trello::TrelloApi for TrelloHttp {
    fn get(&self, _: &crate::trello::Auth, path: &str) -> Result<Value, HttpError> {
        self.0.lock().unwrap().push(path.into());
        Ok(if path.starts_with("/members/me/boards") {
            fixture("trello", "boards")
        } else if path.starts_with("/members/me?") {
            fixture("trello", "me")
        } else {
            let mut data = fixture("trello", "lists");
            if let Some(card) = data[0]["cards"][0].as_object_mut() {
                card.insert("name".into(), json!("PRIVATE_CARD_CONTENT"));
            }
            data
        })
    }
    fn put(&self, _: &crate::trello::Auth, path: &str) -> Result<(), HttpError> {
        self.0.lock().unwrap().push(path.into());
        Ok(())
    }
}
#[derive(Clone, Default)]
struct JiraHttp {
    requests: Arc<Mutex<Vec<String>>>,
    read_failure: Arc<Mutex<Option<HttpError>>>,
    write_failure: Arc<Mutex<Option<HttpError>>>,
    required: Arc<Mutex<bool>>,
    cyclic: Arc<Mutex<bool>>,
    write_gate: Arc<Mutex<Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>>>,
}
impl jira::JiraApi for JiraHttp {
    fn get(
        &self,
        _: &jira::Auth,
        path: &str,
        query: &[(String, String)],
    ) -> Result<Value, HttpError> {
        self.requests.lock().unwrap().push(path.into());
        if let Some(error) = *self.read_failure.lock().unwrap() {
            return Err(error);
        }
        Ok(if path.ends_with("/myself") {
            fixture("jira", "myself")
        } else if path.ends_with("/configuration") {
            fixture("jira", "board-configuration")
        } else if path.ends_with("/transitions") {
            if *self.required.lock().unwrap() {
                fixture("jira", "required-transition")
            } else {
                fixture("jira", "transitions")
            }
        } else if path.ends_with("/issue") {
            assert!(query
                .iter()
                .any(|(k, v)| k == "fields"
                    && v == "summary,status,issuetype,priority,assignee,duedate"));
            if query.iter().any(|(k, _)| k == "nextPageToken") && !*self.cyclic.lock().unwrap() {
                fixture("jira", "issues-last-page")
            } else {
                fixture("jira", "board-issues")
            }
        } else {
            fixture("jira", "boards")
        })
    }
    fn transition(&self, _: &jira::Auth, item: &str, transition: &str) -> Result<(), HttpError> {
        self.requests
            .lock()
            .unwrap()
            .push(format!("POST {item}/{transition}"));
        if let Some((entered, release)) = self.write_gate.lock().unwrap().take() {
            entered.send(()).unwrap();
            release
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap();
        }
        match *self.write_failure.lock().unwrap() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
#[derive(Clone, Default)]
struct Environment {
    time: Time,
    cache: Disk,
    settings: Disk,
    logs: Disk,
    vault: Vault,
    trello: TrelloHttp,
    jira: JiraHttp,
}
impl Environment {
    fn seeded() -> Self {
        let env = Self::default();
        let board = fixture("trello", "boards")[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        env.vault.save("trello",&json!({"key":"synthetickey","token":"PRIVATE_TRELLO_TOKEN","login":"exemplo","board":board}).to_string()).unwrap();
        env.vault.save("jira",&json!({"site":"https://example.atlassian.net","email":"person@example.com","token":"PRIVATE_JIRA_TOKEN","account":"synthetic-account","login":"Pessoa de exemplo","board":"42"}).to_string()).unwrap();
        env
    }
    fn tasks(&self) -> Tasks {
        Tasks::new(
            crate::trello::Trello::new(self.trello.clone(), self.time.clone())
                .with_keyring(self.vault.clone()),
            jira::Jira::new(self.jira.clone(), self.time.clone(), self.vault.clone()),
            LocalPersistence {
                cache: Cache(self.cache.clone()),
                logs: Logs(self.logs.clone()),
            },
            self.settings.clone(),
        )
    }
    fn jira_tasks(&self) -> Tasks {
        let mut tasks = self.tasks();
        tasks.switch(1, Provider::Jira, &mut |_| {}).unwrap();
        tasks.poll(true, false, &mut |_| {});
        tasks
    }
}
fn generation(tasks: &Tasks) -> u64 {
    tasks.snapshot().data["contextGeneration"].as_u64().unwrap()
}

#[test]
fn legacy_trello_connection_and_board_work_without_reconnecting() {
    let env = Environment::seeded();
    let before = env.vault.read("trello").unwrap();
    let mut tasks = env.tasks();
    tasks.poll(true, false, &mut |_| {});
    let snapshot = tasks.snapshot();
    assert_eq!(snapshot.module, Module::Tasks);
    assert_eq!(snapshot.data["provider"], "trello");
    assert_eq!(snapshot.data["state"], "ready");
    assert_eq!(snapshot.data["canWrite"], true);
    assert_eq!(env.vault.read("trello").unwrap(), before);
    assert!(env.jira.requests.lock().unwrap().is_empty());
    assert!(!env.cache.read("tasks.json").unwrap().contains("PRIVATE_"));
}

#[test]
fn failed_cleanup_hides_origin_and_blocks_both_providers_across_restart() {
    let env = Environment::seeded();
    let mut tasks = env.tasks();
    tasks.poll(true, false, &mut |_| {});
    *env.cache.remove_fails.lock().unwrap() = true;
    assert_eq!(
        tasks.switch(1, Provider::Jira, &mut |_| {}),
        Err(Error::CleanupFailed)
    );
    assert_eq!(tasks.snapshot().data["state"], "cleanupPending");
    assert!(tasks.snapshot().data["lists"].is_null());
    assert_eq!(tasks.snapshot().updated_at, None);
    let calls = env.trello.0.lock().unwrap().len();
    tasks.poll(true, true, &mut |_| {});
    assert_eq!(env.trello.0.lock().unwrap().len(), calls);
    assert!(env.jira.requests.lock().unwrap().is_empty());
    let mut restarted = env.tasks();
    assert_eq!(restarted.snapshot().data["state"], "cleanupPending");
    restarted.poll(true, true, &mut |_| {});
    assert!(env.jira.requests.lock().unwrap().is_empty());
    *env.cache.remove_fails.lock().unwrap() = false;
    restarted.retry_cleanup(generation(&restarted)).unwrap();
    restarted.poll(true, false, &mut |_| {});
    assert_eq!(restarted.snapshot().data["provider"], "jira");
    assert_eq!(restarted.snapshot().data["state"], "ready");
    assert_eq!(env.trello.0.lock().unwrap().len(), calls);
    assert!(env.vault.read("trello").unwrap().is_some());
    assert!(env.cache.files.lock().unwrap().is_empty());
}

#[test]
fn failed_intent_write_does_not_start_or_claim_a_provider_switch() {
    let env = Environment::seeded();
    let mut tasks = env.tasks();
    tasks.poll(true, false, &mut |_| {});
    *env.settings.write_fails.lock().unwrap() = true;
    assert_eq!(
        tasks.switch(1, Provider::Jira, &mut |_| {}),
        Err(Error::SettingsUnavailable)
    );
    assert_eq!(tasks.snapshot().data["provider"], "trello");
    assert_eq!(generation(&tasks), 1);
    assert_eq!(tasks.snapshot().data["state"], "ready");
}

#[test]
fn failed_final_commit_is_recoverable_without_reinserting_credentials() {
    let env = Environment::seeded();
    let mut tasks = env.tasks();
    *env.settings.fail_rename_at.lock().unwrap() = Some(3);
    let result = tasks.switch(1, Provider::Jira, &mut |_| {});
    assert_eq!(result, Err(Error::CleanupFailed));
    assert_eq!(tasks.snapshot().data["state"], "cleanupPending");
    *env.settings.rename_fails.lock().unwrap() = false;
    let mut restarted = env.tasks();
    assert_eq!(restarted.snapshot().data["provider"], "jira");
    restarted.poll(true, false, &mut |_| {});
    assert_eq!(restarted.snapshot().data["state"], "ready");
}

#[test]
fn disconnect_waits_for_vault_removal_and_is_idempotent_on_restart() {
    let env = Environment::seeded();
    let mut tasks = env.jira_tasks();
    *env.vault.deletion_fails.lock().unwrap() = true;
    assert_eq!(
        tasks.disconnect(generation(&tasks), &mut |_| {}),
        Err(Error::KeyringUnavailable)
    );
    assert_eq!(tasks.snapshot().data["state"], "cleanupPending");
    *env.vault.deletion_fails.lock().unwrap() = false;
    let mut restarted = env.tasks();
    assert_eq!(restarted.snapshot().data["provider"], "jira");
    restarted.poll(true, false, &mut |_| {});
    assert_eq!(restarted.snapshot().data["state"], "missingCredential");
    assert!(env.vault.read("jira").unwrap().is_none());
    assert!(env.vault.read("trello").unwrap().is_some());
}

#[test]
fn jira_reads_all_pages_and_keeps_issue_fields_out_of_every_disk_projection() {
    let env = Environment::seeded();
    let tasks = env.jira_tasks();
    let snapshot = tasks.snapshot();
    assert_eq!(snapshot.data["state"], "ready");
    assert_eq!(snapshot.data["issues"].as_array().unwrap().len(), 101);
    assert_eq!(snapshot.data["columns"].as_array().unwrap().len(), 5);
    let last = &snapshot.data["issues"][100];
    assert_eq!(last["key"], "EXM-101");
    assert_eq!(last["assignee"], "Pessoa de exemplo");
    assert_eq!(last["due"], "2030-01-15");
    assert!(env.cache.files.lock().unwrap().is_empty());
    for disk in [&env.settings, &env.logs] {
        for text in disk.files.lock().unwrap().values() {
            assert!(
                !text.contains("PRIVATE_")
                    && !text.contains("EXM-")
                    && !text.contains("example.atlassian.net")
            );
        }
    }
    let mut restarted = env.tasks();
    assert!(restarted.snapshot().data["issues"]
        .as_array()
        .unwrap()
        .is_empty());
    *env.jira.read_failure.lock().unwrap() = Some(HttpError::default());
    restarted.poll(true, false, &mut |_| {});
    assert!(restarted.snapshot().data["issues"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(restarted.snapshot().data["canWrite"], false);
}

#[test]
fn cyclic_pagination_is_an_error_instead_of_a_partial_or_empty_board() {
    let env = Environment::seeded();
    *env.jira.cyclic.lock().unwrap() = true;
    let tasks = env.jira_tasks();
    assert_eq!(tasks.snapshot().data["state"], "error");
    assert_eq!(tasks.snapshot().data["reason"], "unsupportedBoard");
    assert_eq!(tasks.snapshot().data["canWrite"], false);
}

#[test]
fn required_fields_and_unlisted_transitions_never_send_a_write() {
    let env = Environment::seeded();
    let mut tasks = env.jira_tasks();
    *env.jira.required.lock().unwrap() = true;
    assert_eq!(
        tasks.perform(
            generation(&tasks),
            Action::Transition {
                item: "20000".into(),
                transition: "99".into()
            },
            &mut |_| {}
        ),
        Err(Error::RequiredFields)
    );
    assert!(!env
        .jira
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|p| p.starts_with("POST")));
    tasks.poll(true, true, &mut |_| {});
    assert_eq!(
        tasks.perform(
            generation(&tasks),
            Action::Transition {
                item: "20000".into(),
                transition: "arbitrary".into()
            },
            &mut |_| {}
        ),
        Err(Error::Refused)
    );
    assert!(!env
        .jira
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|p| p.starts_with("POST")));
}

#[test]
fn unconfirmed_transition_requires_a_valid_read_and_is_never_replayed() {
    let env = Environment::seeded();
    let mut tasks = env.jira_tasks();
    let choice = tasks
        .transitions(generation(&tasks), "20000")
        .unwrap()
        .into_iter()
        .find(|t| !t.requires_fields)
        .unwrap();
    *env.jira.write_failure.lock().unwrap() = Some(HttpError::default());
    let action = Action::Transition {
        item: "20000".into(),
        transition: choice.id,
    };
    assert_eq!(
        tasks.perform(generation(&tasks), action.clone(), &mut |_| {}),
        Err(Error::Unconfirmed)
    );
    assert_eq!(tasks.snapshot().data["movement"]["state"], "unconfirmed");
    assert_eq!(
        tasks.perform(generation(&tasks), action.clone(), &mut |_| {}),
        Err(Error::ReadRequired)
    );
    *env.jira.read_failure.lock().unwrap() = Some(HttpError::default());
    tasks.poll(true, false, &mut |_| {});
    assert_eq!(
        tasks.perform(generation(&tasks), action.clone(), &mut |_| {}),
        Err(Error::ReadRequired)
    );
    assert_eq!(
        env.jira
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.starts_with("POST"))
            .count(),
        1
    );
    *env.jira.read_failure.lock().unwrap() = None;
    env.time.advance(30_000);
    tasks.poll(true, true, &mut |_| {});
    assert_eq!(tasks.snapshot().data["canWrite"], true);
    assert_eq!(
        env.jira
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.starts_with("POST"))
            .count(),
        1
    );
}

#[test]
fn stale_context_commands_cannot_change_or_open_the_new_source() {
    let env = Environment::seeded();
    let mut tasks = env.tasks();
    tasks.poll(true, false, &mut |_| {});
    tasks.switch(1, Provider::Jira, &mut |_| {}).unwrap();
    assert_eq!(
        tasks.select_board(1, "42", &mut |_| {}),
        Err(Error::StaleContext)
    );
    assert_eq!(tasks.disconnect(1, &mut |_| {}), Err(Error::StaleContext));
    assert!(env.vault.read("jira").unwrap().is_some());
    assert!(env.jira.requests.lock().unwrap().is_empty());
}

#[test]
fn jira_refuses_unavailable_vault_without_writing_credentials_to_files() {
    let env = Environment::seeded();
    let mut tasks = env.jira_tasks();
    *env.vault.unavailable.lock().unwrap() = true;
    assert_eq!(
        tasks.connect(
            generation(&tasks),
            Connection::Jira {
                site: "https://example.atlassian.net".into(),
                email: "person@example.com".into(),
                token: "PRIVATE_NEW_TOKEN".into()
            },
            &mut |_| {}
        ),
        Err(Error::KeyringUnavailable)
    );
    for text in env.settings.files.lock().unwrap().values() {
        assert!(!text.contains("PRIVATE_") && !text.contains("example.com"));
    }
}

#[test]
fn rate_limits_and_polling_follow_monotonic_time_when_wall_clock_moves_back() {
    let env = Environment::seeded();
    let mut tasks = env.jira_tasks();
    let calls = env.jira.requests.lock().unwrap().len();
    *env.time.wall.lock().unwrap() -= 100_000;
    tasks.poll(true, false, &mut |_| {});
    assert_eq!(env.jira.requests.lock().unwrap().len(), calls);
    env.time.advance(30_000);
    tasks.poll(true, false, &mut |_| {});
    assert!(env.jira.requests.lock().unwrap().len() > calls);
    *env.jira.read_failure.lock().unwrap() = Some(HttpError::response(429, Some("120")));
    env.time.advance(10_000);
    tasks.poll(true, true, &mut |_| {});
    let calls = env.jira.requests.lock().unwrap().len();
    env.time.advance(60_000);
    tasks.poll(true, true, &mut |_| {});
    assert_eq!(env.jira.requests.lock().unwrap().len(), calls);
    env.time.advance(60_000);
    tasks.poll(true, true, &mut |_| {});
    assert!(env.jira.requests.lock().unwrap().len() > calls);
}

#[test]
fn future_task_schema_is_preserved_and_does_not_collect_or_change_credentials() {
    let env = Environment::seeded();
    let future = r#"{"schemaVersion":999,"provider":"jira","cleanup":null,"futureChoice":true}"#;
    env.settings.write(SETTINGS, future).unwrap();
    let mut tasks = env.tasks();
    tasks.poll(true, true, &mut |_| {});
    assert_eq!(tasks.snapshot().data["state"], "settingsUnavailable");
    assert_eq!(
        tasks.switch(1, Provider::Trello, &mut |_| {}),
        Err(Error::SettingsUnavailable)
    );
    assert_eq!(env.settings.read(SETTINGS).unwrap(), future);
    assert!(
        env.trello.0.lock().unwrap().is_empty() && env.jira.requests.lock().unwrap().is_empty()
    );
}
