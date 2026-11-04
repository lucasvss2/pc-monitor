use super::*;
use crate::ports::fakes::FakeClock;
use crate::ports::HttpError;
use std::sync::{Arc, Mutex};

const NOW: u64 = 1793448000000;

struct NoHttp;
impl TrelloApi for NoHttp {
    fn get(&self, _: &Auth, _: &str) -> Result<Value, HttpError> {
        panic!("HTTP must not be called without credentials")
    }
    fn put(&self, _: &Auth, _: &str) -> Result<(), HttpError> {
        panic!("HTTP must not be called without credentials")
    }
}

#[test]
fn without_credentials_the_module_stays_visible_with_setup_state() {
    let mut trello = Trello::new(NoHttp, FakeClock::at(NOW));
    let snapshot = trello.step(true, false).unwrap();
    assert_eq!(snapshot.module, Module::Tasks);
    assert_eq!(snapshot.data["state"], "missingCredential");
    assert!(snapshot.data["lists"].is_null());
}

fn fixture(name: &str) -> Value {
    let path = crate::ports::fakes::fixtures().join(format!("trello/{name}.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Serves the recorded responses by route; `status` simulates a refused or failed request.
#[derive(Clone, Default)]
struct Api {
    status: Arc<Mutex<Option<u16>>>,
    lists: Arc<Mutex<Option<Value>>>,
    calls: Arc<Mutex<Vec<String>>>,
    /// Status of the next card update; Some(0) is a network failure.
    put_status: Arc<Mutex<Option<u16>>>,
    lifecycle_after_get: Arc<Mutex<Option<crate::lifecycle::Shared>>>,
    lifecycle_after_put: Arc<Mutex<Option<crate::lifecycle::Shared>>>,
}
impl Api {
    fn fail(&self, status: Option<u16>) {
        *self.status.lock().unwrap() = status;
    }
}
impl TrelloApi for Api {
    fn get(&self, _: &Auth, path: &str) -> Result<Value, HttpError> {
        self.calls.lock().unwrap().push(path.into());
        if let Some(shared) = self.lifecycle_after_get.lock().unwrap().take() {
            shared.lock().unwrap().generation += 1;
        }
        if let Some(status) = *self.status.lock().unwrap() {
            return Err(HttpError {
                status: (status != 0).then_some(status),
                ..HttpError::default()
            });
        }
        let route = path.split('?').next().unwrap();
        Ok(match route {
            "/members/me" => fixture("me"),
            "/members/me/boards" => open_boards(),
            _ if route.starts_with("/boards/") && route.ends_with("/lists") => self
                .lists
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| fixture("lists")),
            _ => panic!("unexpected route {route}"),
        })
    }
    fn put(&self, _: &Auth, path: &str) -> Result<(), HttpError> {
        self.calls.lock().unwrap().push(format!("PUT {path}"));
        if let Some(shared) = self.lifecycle_after_put.lock().unwrap().take() {
            shared.lock().unwrap().online = false;
        }
        match *self.put_status.lock().unwrap() {
            Some(status) => Err(HttpError {
                status: (status != 0).then_some(status),
                ..HttpError::default()
            }),
            None => Ok(()),
        }
    }
}

#[derive(Clone, Default)]
struct Vault(Arc<Mutex<Option<String>>>);
impl Keyring for Vault {
    fn read(&self, account: &str) -> Result<Option<String>, ()> {
        assert_eq!(account, "trello");
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, account: &str, secret: &str) -> Result<(), ()> {
        assert_eq!(account, "trello");
        *self.0.lock().unwrap() = Some(secret.into());
        Ok(())
    }
    fn delete(&self, _: &str) -> Result<(), ()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

/// The recorded account has one open board; a second one is a controlled variant
/// so tests can switch boards.
fn open_boards() -> Value {
    let mut boards = fixture("boards");
    boards
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"0000000000000000000000b2","name":"Quadro extra"}));
    boards
}

fn board_id(index: usize) -> String {
    open_boards()[index]["id"].as_str().unwrap().into()
}

#[test]
fn connecting_validates_identity_and_open_boards_before_saving_key_and_token() {
    let api = Api::default();
    let vault = Vault::default();
    let mut trello = Trello::new(api.clone(), FakeClock::at(NOW)).with_keyring(vault.clone());
    assert_eq!(
        trello.connect(" 0123abcd ", " private-token "),
        Ok("exemplo".into())
    );
    let calls = api.calls.lock().unwrap().clone();
    assert!(calls[0].starts_with("/members/me?"));
    assert!(calls[1].starts_with("/members/me/boards?filter=open"));
    let saved = vault.0.lock().unwrap().clone().unwrap();
    assert!(saved.contains("0123abcd") && saved.contains("private-token"));
    let data = trello.snapshot(false).data;
    assert_eq!(data["state"], "chooseBoard");
    assert_eq!(data["account"], json!({"login":"exemplo"}));
    assert_eq!(
        data["boards"],
        json!([{"id":board_id(0),"name":"Quadro 1"},{"id":board_id(1),"name":"Quadro extra"}])
    );
    let text = data.to_string();
    assert!(!text.contains("private-token") && !text.contains("0123abcd"));
}

#[test]
fn failed_validation_saves_nothing() {
    for (status, expected) in [
        (Some(401), CredentialError::InvalidCredential),
        (Some(400), CredentialError::InvalidCredential),
        (Some(503), CredentialError::Offline),
        (Some(0), CredentialError::Offline),
    ] {
        let api = Api::default();
        api.fail(status);
        let vault = Vault::default();
        vault.0.lock().unwrap().replace("previous".into());
        let mut trello = Trello::new(api, FakeClock::at(NOW)).with_keyring(vault.clone());
        assert_eq!(trello.connect("0123abcd", "token"), Err(expected));
        assert_eq!(vault.0.lock().unwrap().as_deref(), Some("previous"));
    }
    let mut trello = Trello::new(NoHttp, FakeClock::at(NOW)).with_keyring(Vault::default());
    assert_eq!(
        trello.connect("", "token"),
        Err(CredentialError::InvalidCredential)
    );
    assert_eq!(
        trello.connect("0123abcd", " "),
        Err(CredentialError::InvalidCredential)
    );
}

#[test]
fn without_keyring_connecting_is_refused() {
    let mut trello = Trello::new(Api::default(), FakeClock::at(NOW));
    assert_eq!(
        trello.connect("0123abcd", "token"),
        Err(CredentialError::KeyringUnavailable)
    );
    assert_eq!(trello.snapshot(false).data["state"], "missingCredential");
    assert!(trello.snapshot(false).data["account"].is_null());
}

fn connected(api: Api, clock: FakeClock) -> (Trello<Api, FakeClock>, Vault) {
    let vault = Vault::default();
    let mut trello = Trello::new(api, clock).with_keyring(vault.clone());
    trello.connect("0123abcd", "private-token").unwrap();
    trello.select_board(&board_id(0)).unwrap();
    (trello, vault)
}

/// Controlled variant of the recorded board: due dates around NOW and colored or
/// colorless labels on the first card.
fn dated_lists() -> Value {
    let mut lists = fixture("lists");
    let card = &mut lists[0]["cards"][0];
    card["labels"] = json!([
        {"id":"l1","idBoard":"b","name":"privado","color":"red"},
        {"id":"l2","idBoard":"b","name":"","color":null},
        {"id":"l3","idBoard":"b","name":"","color":"sky_light"}
    ]);
    card["due"] = json!("2026-10-31T11:59:00.000Z");
    card["dueComplete"] = json!(false);
    let mut done = card.clone();
    done["id"] = json!("000000000000000000000f01");
    done["name"] = json!("Card concluído");
    done["dueComplete"] = json!(true);
    let mut later = card.clone();
    later["id"] = json!("000000000000000000000f02");
    later["name"] = json!("Card futuro");
    later["due"] = json!("2026-10-31T13:00:00.000Z");
    let cards = lists[0]["cards"].as_array_mut().unwrap();
    cards.push(done);
    cards.push(later);
    lists
}

#[test]
fn chosen_board_shows_lists_as_columns_with_title_labels_and_due_date() {
    let api = Api::default();
    *api.lists.lock().unwrap() = Some(dated_lists());
    let (mut trello, _) = connected(api.clone(), FakeClock::at(NOW));
    let data = trello.step(true, false).unwrap().data;
    assert_eq!(data["state"], "ready");
    assert_eq!(data["board"], json!({"id":board_id(0),"name":"Quadro 1"}));
    assert!(api
        .calls
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .starts_with(&format!("/boards/{}/lists?cards=open", board_id(0))));
    let recorded = fixture("lists");
    assert_eq!(
        data["lists"].as_array().unwrap().len(),
        recorded.as_array().unwrap().len()
    );
    assert_eq!(data["lists"][0]["name"], "Lista 1");
    let cards = &data["lists"][0]["cards"];
    assert_eq!(cards[0]["name"], "Card 1.1");
    assert_eq!(cards[0]["url"], recorded[0]["cards"][0]["shortUrl"]);
    assert_eq!(cards[0]["labels"], json!(["red", "sky_light"]));
    assert_eq!(cards[0]["due"], json!(1793447940000u64));
    let overdue: Vec<_> = cards
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["overdue"].clone())
        .collect();
    assert_eq!(overdue[0], true);
    assert_eq!(overdue[overdue.len() - 2], false, "completed");
    assert_eq!(overdue[overdue.len() - 1], false, "future");
    assert!(
        !data.to_string().contains("privado"),
        "label names are not shown"
    );
}

#[test]
fn due_dates_become_overdue_as_time_passes_without_a_new_request() {
    let api = Api::default();
    *api.lists.lock().unwrap() = Some(dated_lists());
    let clock = FakeClock::at(NOW);
    let (mut trello, _) = connected(api, clock.clone());
    trello.step(true, false).unwrap();
    let later = |t: &Trello<Api, FakeClock>| {
        let cards = t.snapshot(false).data["lists"][0]["cards"].clone();
        cards[cards.as_array().unwrap().len() - 1]["overdue"].clone()
    };
    assert_eq!(later(&trello), false);
    clock.advance(3_600_000);
    assert_eq!(later(&trello), false);
    clock.advance(1);
    assert_eq!(later(&trello), true);
}

#[test]
fn board_refreshes_every_thirty_seconds_when_visible_and_five_minutes_otherwise() {
    let clock = FakeClock::at(NOW);
    let (mut trello, _) = connected(Api::default(), clock.clone());
    assert_eq!(trello.step(false, false).unwrap().updated_at, Some(NOW));
    clock.advance(29_999);
    assert!(trello.step(true, false).is_none());
    clock.advance(1);
    assert!(trello.step(false, false).is_none());
    assert!(trello.step(true, false).is_some());
    clock.advance(299_999);
    assert!(trello.step(false, false).is_none());
    clock.advance(1);
    assert!(trello.step(false, false).is_some());
}

#[test]
fn manual_refresh_waits_ten_seconds_between_clicks() {
    let clock = FakeClock::at(NOW);
    let (mut trello, _) = connected(Api::default(), clock.clone());
    trello.step(true, false).unwrap();
    assert!(trello.step(false, true).is_some());
    clock.advance(9_999);
    assert!(trello.step(false, true).is_none());
    clock.advance(1);
    assert!(trello.step(false, true).is_some());
}

#[test]
fn connecting_and_choosing_a_board_refresh_right_away() {
    let clock = FakeClock::at(NOW);
    let (mut trello, _) = connected(Api::default(), clock.clone());
    trello.step(true, false).unwrap();
    trello.select_board(&board_id(1)).unwrap();
    assert_eq!(
        trello.step(false, false).unwrap().data["board"]["id"],
        board_id(1)
    );
}

#[test]
fn missing_credential_is_checked_again_after_thirty_seconds() {
    let clock = FakeClock::at(NOW);
    let mut trello = Trello::new(NoHttp, clock.clone()).with_keyring(Vault::default());
    assert!(trello.step(false, false).is_some());
    clock.advance(29_999);
    assert!(trello.step(true, false).is_none());
    clock.advance(1);
    assert!(trello.step(true, false).is_some());
}

#[test]
fn transient_failures_back_off_and_show_error_only_after_the_third() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    let ready = trello.step(true, false).unwrap();
    api.fail(Some(503));
    clock.advance(30_000);
    for (attempt, delay) in [30_000, 60_000, 120_000, 240_000, 480_000, 900_000, 900_000]
        .into_iter()
        .enumerate()
    {
        let snapshot = trello.step(true, false).unwrap();
        let expected = if attempt >= 2 { "error" } else { "stale" };
        assert_eq!(snapshot.data["state"], expected);
        assert_eq!(snapshot.data["lists"], ready.data["lists"]);
        assert_eq!(snapshot.data["lastSuccessAt"], json!(NOW));
        assert_eq!(snapshot.data["failureSince"], json!(NOW + 30_000));
        clock.advance(delay - 1);
        assert!(
            trello.step(true, true).is_none(),
            "manual waits for backoff"
        );
        clock.advance(1);
    }
    api.fail(None);
    let recovered = trello.step(true, false).unwrap();
    assert_eq!(recovered.data["state"], "ready");
    assert!(recovered.data.get("failureSince").is_none());
    clock.advance(29_999);
    assert!(
        trello.step(true, false).is_none(),
        "normal cadence restored"
    );
}

struct RateLimited;
impl TrelloApi for RateLimited {
    fn get(&self, auth: &Auth, path: &str) -> Result<Value, HttpError> {
        if path.starts_with("/boards/") {
            return Err(HttpError::response(429, Some("180")));
        }
        Api::default().get(auth, path)
    }
    fn put(&self, auth: &Auth, path: &str) -> Result<(), HttpError> {
        Api::default().put(auth, path)
    }
}

#[test]
fn retry_after_is_a_minimum_wait() {
    let clock = FakeClock::at(NOW);
    let vault = Vault::default();
    let mut trello = Trello::new(Api::default(), clock.clone()).with_keyring(vault.clone());
    trello.connect("0123abcd", "token").unwrap();
    trello.select_board(&board_id(0)).unwrap();
    let mut trello = Trello::new(RateLimited, clock.clone()).with_keyring(vault);
    trello.step(true, false).unwrap();
    clock.advance(179_999);
    assert!(trello.step(true, true).is_none());
    clock.advance(1);
    assert!(trello.step(true, false).is_some());
}

#[test]
fn refused_credential_keeps_last_board_and_stops_until_reconnected() {
    for status in [401, 403] {
        let clock = FakeClock::at(NOW);
        let api = Api::default();
        let (mut trello, _) = connected(api.clone(), clock.clone());
        let ready = trello.step(true, false).unwrap();
        api.fail(Some(status));
        clock.advance(30_000);
        let refused = trello.step(true, false).unwrap();
        assert_eq!(refused.data["state"], "invalidCredential");
        assert_eq!(refused.data["lists"], ready.data["lists"]);
        assert_eq!(refused.data["lastSuccessAt"], json!(NOW));
        api.fail(None);
        clock.advance(3_600_000);
        assert!(trello.step(true, true).is_none());
        trello.connect("0123abcd", "new-token").unwrap();
        trello.select_board(&board_id(0)).unwrap();
        assert_eq!(trello.step(true, false).unwrap().data["state"], "ready");
    }
}

#[test]
fn offline_pauses_requests_and_keeps_last_board_with_its_age() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    let ready = trello.step(true, false).unwrap();
    trello.lifecycle(crate::lifecycle::Status::default());
    clock.advance(600_000);
    let calls = api.calls.lock().unwrap().len();
    assert!(trello.step(true, true).is_none());
    assert_eq!(api.calls.lock().unwrap().len(), calls);
    let paused = trello.snapshot(true).data;
    assert_eq!(paused["paused"], true);
    assert_eq!(paused["refreshing"], false);
    assert_eq!(paused["state"], "ready");
    assert_eq!(paused["lastSuccessAt"], json!(NOW));
    assert_eq!(paused["lists"], ready.data["lists"]);
    assert_eq!(
        trello.connect("0123abcd", "token"),
        Err(CredentialError::Offline)
    );
}

#[test]
fn reconnecting_refreshes_once_three_seconds_after_the_first_remote_source() {
    let clock = FakeClock::at(NOW);
    let (mut trello, _) = connected(Api::default(), clock.clone());
    trello.step(true, false).unwrap();
    let online = crate::lifecycle::Status {
        online: true,
        generation: 1,
        ready_at: NOW + 10_000,
    };
    trello.lifecycle(online);
    clock.advance(12_999);
    assert!(trello.step(false, false).is_none());
    clock.advance(1);
    assert!(trello.step(false, false).is_some());
    assert!(trello.step(false, false).is_none());
    trello.lifecycle(online);
    assert!(trello.step(false, false).is_none(), "same generation");
    trello.lifecycle(crate::lifecycle::Status {
        generation: 2,
        ready_at: clock.now(),
        ..online
    });
    clock.advance(3_000);
    assert!(trello.step(false, false).is_some());
}

#[test]
fn reconnecting_keeps_a_refused_credential_blocked() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    api.fail(Some(401));
    trello.step(true, false).unwrap();
    trello.lifecycle(crate::lifecycle::Status {
        online: true,
        generation: 1,
        ready_at: NOW,
    });
    clock.advance(3_000);
    assert!(trello.step(true, false).is_none());
}

fn disk(
    dir: &tempfile::TempDir,
) -> crate::persistence::LocalPersistence<
    crate::persistence::DiskStorage,
    crate::persistence::DiskStorage,
> {
    use crate::persistence::{Cache, DiskStorage, LocalPersistence, Logs};
    LocalPersistence {
        cache: Cache(DiskStorage(dir.path().join("cache"))),
        logs: Logs(DiskStorage(dir.path().join("state"))),
    }
}

#[test]
fn disconnecting_erases_key_token_and_cached_board() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(NOW);
    let vault = Vault::default();
    let mut trello = Trello::new(Api::default(), clock.clone())
        .with_keyring(vault.clone())
        .with_persistence(disk(&dir));
    trello.connect("0123abcd", "private-token").unwrap();
    trello.select_board(&board_id(0)).unwrap();
    trello.step(true, false).unwrap();
    assert!(dir.path().join("cache/tasks.json").exists());
    assert_eq!(trello.disconnect(), Ok(()));
    assert_eq!(*vault.0.lock().unwrap(), None);
    assert!(!dir.path().join("cache/tasks.json").exists());
    let data = trello.snapshot(false).data;
    assert_eq!(data["state"], "missingCredential");
    assert!(data["account"].is_null() && data["lists"].is_null());
    let restarted = Trello::new(NoHttp, clock).with_persistence(disk(&dir));
    assert_eq!(restarted.snapshot(false).data["state"], "missingCredential");
}

#[test]
fn disconnect_without_keyring_is_refused() {
    let mut trello = Trello::new(NoHttp, FakeClock::at(NOW));
    assert_eq!(
        trello.disconnect(),
        Err(CredentialError::KeyringUnavailable)
    );
}

#[test]
fn restart_restores_last_board_without_card_titles_or_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    *api.lists.lock().unwrap() = Some(dated_lists());
    let vault = Vault::default();
    let mut trello = Trello::new(api, clock.clone())
        .with_keyring(vault.clone())
        .with_persistence(disk(&dir));
    trello.connect("0123abcd", "private-token").unwrap();
    trello.select_board(&board_id(0)).unwrap();
    let fresh = trello.step(true, false).unwrap();
    let restored = Trello::new(NoHttp, clock)
        .with_keyring(vault)
        .with_persistence(disk(&dir))
        .snapshot(false);
    assert_eq!(restored.updated_at, Some(NOW));
    assert_eq!(restored.data["state"], "stale");
    assert_eq!(restored.data["board"], fresh.data["board"]);
    let card = &restored.data["lists"][0]["cards"][0];
    assert_eq!(card["name"], "");
    assert_eq!(card["labels"], json!(["red", "sky_light"]));
    assert_eq!(card["url"], fresh.data["lists"][0]["cards"][0]["url"]);
    assert_eq!(card["overdue"], true);
    let cache = std::fs::read_to_string(dir.path().join("cache/tasks.json")).unwrap();
    let log = std::fs::read_to_string(dir.path().join("state/info.log")).unwrap();
    for text in [&cache, &log] {
        for secret in ["private-token", "0123abcd", "Card 1.1", "privado"] {
            assert!(!text.contains(secret), "{secret}");
        }
    }
    assert_eq!(
        serde_json::from_str::<Value>(log.lines().last().unwrap()).unwrap(),
        json!({"source":"tasks","event":"updated","httpStatus":200,"durationMs":0})
    );
}

#[test]
fn ui_fixtures_match_published_snapshots() {
    let clock = FakeClock::at(NOW);
    let (mut full, _) = connected(Api::default(), clock.clone());
    let dated = Api::default();
    *dated.lists.lock().unwrap() = Some(dated_lists());
    let (mut overdue, _) = connected(dated, clock.clone());
    let mut empty = Trello::new(NoHttp, clock.clone()).with_keyring(Vault::default());
    let mut choose = Trello::new(Api::default(), clock.clone()).with_keyring(Vault::default());
    choose.connect("0123abcd", "private-token").unwrap();
    let refused = Api::default();
    let (mut invalid, _) = connected(refused.clone(), clock.clone());
    invalid.step(true, false).unwrap();
    let failing = Api::default();
    let (mut error, _) = connected(failing.clone(), clock.clone());
    error.step(true, false).unwrap();
    failing.fail(Some(503));
    refused.fail(Some(401));
    clock.advance(30_000);
    let invalid = invalid.step(true, false).unwrap();
    error.step(true, false).unwrap();
    clock.advance(30_000);
    error.step(true, false).unwrap();
    let (mut offline, _) = connected(Api::default(), clock.clone());
    offline.step(true, false).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut cached = Trello::new(Api::default(), clock.clone())
        .with_keyring(Vault::default())
        .with_persistence(disk(&dir));
    cached.connect("0123abcd", "private-token").unwrap();
    cached.select_board(&board_id(0)).unwrap();
    cached.step(true, false).unwrap();
    let stale = Trello::new(NoHttp, clock.clone())
        .with_persistence(disk(&dir))
        .snapshot(false);
    clock.advance(60_000);
    for (name, snapshot) in [
        ("stale", stale),
        ("offline", {
            offline.lifecycle(crate::lifecycle::Status::default());
            offline.snapshot(true)
        }),
        ("full", full.step(true, false).unwrap()),
        ("overdue", overdue.step(true, false).unwrap()),
        ("empty", empty.step(true, false).unwrap()),
        ("choose-board", choose.step(true, false).unwrap()),
        ("invalid-credential", invalid),
        ("error", error.step(true, false).unwrap()),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../src/lib/trello/fixtures/{name}.json"));
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::write(
                &path,
                serde_json::to_string_pretty(&snapshot.data).unwrap() + "\n",
            )
            .unwrap();
        }
        let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(snapshot.data, fixture, "{name}");
    }
}

#[test]
fn only_trello_card_links_open_in_the_browser() {
    let url = &fixture("lists")[0]["cards"][0]["shortUrl"];
    assert!(card_url(url.as_str().unwrap()).is_some());
    for rejected in [
        "https://trello.com/c/",
        "https://trello.com.evil.example/c/abc",
        "http://trello.com/c/abc",
        "https://trello.com/c/abc/../../x",
        "file:///etc/passwd",
    ] {
        assert_eq!(card_url(rejected), None, "{rejected}");
    }
}

/// Keyring present but locked at login: reads fail until unlocked.
#[derive(Clone)]
struct Locked(Vault, Arc<Mutex<bool>>);
impl Keyring for Locked {
    fn read(&self, account: &str) -> Result<Option<String>, ()> {
        if *self.1.lock().unwrap() {
            return Err(());
        }
        self.0.read(account)
    }
    fn save(&self, account: &str, secret: &str) -> Result<(), ()> {
        self.0.save(account, secret)
    }
    fn delete(&self, account: &str) -> Result<(), ()> {
        self.0.delete(account)
    }
}

#[test]
fn locked_keyring_keeps_the_restored_board_until_it_unlocks() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(NOW);
    let vault = Vault::default();
    let mut trello = Trello::new(Api::default(), clock.clone())
        .with_keyring(vault.clone())
        .with_persistence(disk(&dir));
    trello.connect("0123abcd", "private-token").unwrap();
    trello.select_board(&board_id(0)).unwrap();
    trello.step(true, false).unwrap();
    let locked = Arc::new(Mutex::new(true));
    let mut restarted = Trello::new(Api::default(), clock.clone())
        .with_keyring(Locked(vault, locked.clone()))
        .with_persistence(disk(&dir));
    restarted.lifecycle(crate::lifecycle::Status {
        online: true,
        generation: 1,
        ready_at: NOW,
    });
    clock.advance(3_000);
    let snapshot = restarted.step(true, false).unwrap();
    assert_eq!(snapshot.data["state"], "stale");
    assert_eq!(snapshot.data["board"]["id"], board_id(0));
    assert!(dir.path().join("cache/tasks.json").exists());
    *locked.lock().unwrap() = false;
    clock.advance(30_000);
    assert_eq!(restarted.step(true, false).unwrap().data["state"], "ready");
}

#[test]
fn reconnecting_keeps_the_chosen_board_while_it_is_still_open() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    api.fail(Some(401));
    trello.step(true, false).unwrap();
    api.fail(None);
    trello.connect("0123abcd", "new-token").unwrap();
    let snapshot = trello.step(true, false).unwrap();
    assert_eq!(snapshot.data["state"], "ready");
    assert_eq!(snapshot.data["board"]["id"], board_id(0));
}

#[test]
fn polling_reads_only_the_board_lists_once_boards_are_known() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    trello.step(true, false).unwrap();
    api.calls.lock().unwrap().clear();
    clock.advance(30_000);
    trello.step(true, false).unwrap();
    let calls = api.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].starts_with(&format!("/boards/{}/lists?cards=open", board_id(0))));
}

#[derive(Clone, Default)]
struct Launcher(Arc<Mutex<Vec<Vec<String>>>>);
impl crate::ports::Processes for Launcher {
    fn execute(&self, program: &str, args: &[&str]) -> Result<String, ()> {
        let mut call = vec![program.to_owned()];
        call.extend(args.iter().map(|arg| arg.to_string()));
        self.0.lock().unwrap().push(call);
        Ok(String::new())
    }
}

#[test]
fn browser_opens_only_vetted_trello_pages() {
    let launcher = Launcher::default();
    let card = fixture("lists")[0]["cards"][0]["shortUrl"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(open_card(&launcher, &card));
    assert!(!open_card(&launcher, "https://evil.example/c/abc"));
    let mut trello = Trello::new(NoHttp, FakeClock::at(NOW));
    assert_eq!(trello.authorize(&launcher, "0123abcd"), Ok(()));
    assert_eq!(
        trello.authorize(&launcher, "abc&return_url=x"),
        Err(CredentialError::InvalidCredential)
    );
    open_admin(&launcher);
    let calls = launcher.0.lock().unwrap().clone();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0], ["xdg-open", card.as_str()]);
    assert_eq!(calls[1][1], authorization_url("0123abcd").unwrap());
    assert_eq!(calls[2][1], "https://trello.com/apps/admin");
}

#[test]
fn generating_a_token_keeps_the_key_in_memory_while_the_panel_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(NOW);
    let mut trello = Trello::new(Api::default(), clock.clone())
        .with_keyring(Vault::default())
        .with_persistence(disk(&dir));
    trello
        .authorize(&Launcher::default(), " 0123abcd ")
        .unwrap();
    // The browser takes focus and the panel webview is destroyed; the next one
    // reads the key back from the published snapshot.
    let data = trello.step(true, false).unwrap().data;
    assert_eq!(data["state"], "missingCredential");
    assert_eq!(data["pendingKey"], "0123abcd");
    let restarted = Trello::new(NoHttp, clock).with_persistence(disk(&dir));
    assert!(restarted.snapshot(false).data.get("pendingKey").is_none());
    trello.connect("0123abcd", "private-token").unwrap();
    assert!(trello.snapshot(false).data.get("pendingKey").is_none());
    trello.authorize(&Launcher::default(), "0123abcd").unwrap();
    trello.disconnect().unwrap();
    assert!(trello.snapshot(false).data.get("pendingKey").is_none());
    for file in ["cache/tasks.json", "state/info.log"] {
        let text = std::fs::read_to_string(dir.path().join(file)).unwrap_or_default();
        assert!(!text.contains("0123abcd"), "{file}");
    }
}

const CARD: &str = "000000000000000000000004";
const FIRST_LIST: &str = "000000000000000000000003";
const EMPTY_LIST: &str = "00000000000000000000000a";

fn card_ids(data: &Value, list: usize) -> Vec<String> {
    data["lists"][list]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["id"].as_str().unwrap().to_owned())
        .collect()
}

/// The recorded board after the first card moved to the empty second list.
fn moved_lists() -> Value {
    let mut lists = fixture("lists");
    let card = lists[0]["cards"].as_array_mut().unwrap().remove(0);
    lists[1]["cards"].as_array_mut().unwrap().push(card);
    lists
}

#[test]
fn moving_a_card_updates_its_list_then_rereads_the_board_and_restarts_the_clock() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    trello.step(true, false).unwrap();
    clock.advance(20_000);
    api.calls.lock().unwrap().clear();
    *api.lists.lock().unwrap() = Some(moved_lists());
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Ok(()));
    let calls = api.calls.lock().unwrap().clone();
    assert_eq!(
        calls[0],
        format!("PUT /cards/{CARD}?idList={EMPTY_LIST}&pos=bottom")
    );
    assert!(calls[1].starts_with(&format!("/boards/{}/lists?", board_id(0))));
    let snapshot = trello.snapshot(false);
    assert_eq!(snapshot.updated_at, Some(NOW + 20_000));
    assert!(!card_ids(&snapshot.data, 0).contains(&CARD.to_owned()));
    assert_eq!(card_ids(&snapshot.data, 1), [CARD]);
    clock.advance(29_999);
    assert!(trello.step(true, false).is_none());
    clock.advance(1);
    assert!(trello.step(true, false).is_some());
}

#[test]
fn unconfirmed_move_keeps_the_last_board_and_its_clock_and_logs_only_metadata() {
    for status in [Some(500), Some(0)] {
        let dir = tempfile::tempdir().unwrap();
        let clock = FakeClock::at(NOW);
        let api = Api::default();
        let vault = Vault::default();
        let mut trello = Trello::new(api.clone(), clock.clone())
            .with_keyring(vault.clone())
            .with_persistence(disk(&dir));
        trello.connect("0123abcd", "private-token").unwrap();
        trello.select_board(&board_id(0)).unwrap();
        let before = trello.step(true, false).unwrap();
        clock.advance(20_000);
        api.calls.lock().unwrap().clear();
        *api.put_status.lock().unwrap() = status;
        assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Err(MoveError));
        assert_eq!(api.calls.lock().unwrap().len(), 1, "no re-read");
        let after = trello.snapshot(false);
        let mut board = after.data.clone();
        assert_eq!(
            board.as_object_mut().unwrap().remove("movement").unwrap()["state"],
            "unconfirmed"
        );
        assert_eq!(board, before.data);
        assert_eq!(after.updated_at, Some(NOW));
        let log = std::fs::read_to_string(dir.path().join("state/info.log")).unwrap();
        assert!(log.lines().last().unwrap().contains("failed"));
        for secret in [CARD, EMPTY_LIST, "private-token", "0123abcd", "Card 1.1"] {
            assert!(!log.contains(secret), "{secret}");
        }
        clock.advance(10_000);
        assert!(trello.step(true, false).is_some(), "regular 30 s cadence");
    }
}

#[test]
fn refused_move_marks_the_credential_invalid_and_stops_requests() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    trello.step(true, false).unwrap();
    *api.put_status.lock().unwrap() = Some(401);
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Err(MoveError));
    assert_eq!(trello.snapshot(false).data["state"], "invalidCredential");
    api.calls.lock().unwrap().clear();
    clock.advance(600_000);
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Err(MoveError));
    assert!(trello.step(true, true).is_none());
    assert!(api.calls.lock().unwrap().is_empty());
}

#[test]
fn moves_wait_for_the_backoff_after_a_failed_read() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    trello.step(true, false).unwrap();
    clock.advance(30_000);
    api.fail(Some(503));
    trello.step(true, false).unwrap();
    api.fail(None);
    api.calls.lock().unwrap().clear();
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Err(MoveError));
    assert!(api.calls.lock().unwrap().is_empty());
    clock.advance(30_000);
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Ok(()));
}

#[test]
fn moves_only_cards_and_lists_of_the_shown_board_while_online() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    assert_eq!(
        trello.move_card(CARD, EMPTY_LIST, |_| {}),
        Err(MoveError),
        "board not read yet"
    );
    trello.step(true, false).unwrap();
    api.calls.lock().unwrap().clear();
    for (card, list) in [
        ("000000000000000000000999", EMPTY_LIST),
        (CARD, "000000000000000000000999"),
        (CARD, "a&idBoard=x"),
        (CARD, FIRST_LIST),
    ] {
        assert_eq!(
            trello.move_card(card, list, |_| {}),
            Err(MoveError),
            "{card} {list}"
        );
    }
    trello.lifecycle(crate::lifecycle::Status::default());
    assert_eq!(
        trello.move_card(CARD, EMPTY_LIST, |_| {}),
        Err(MoveError),
        "offline"
    );
    assert!(api.calls.lock().unwrap().is_empty());
}

#[test]
fn a_move_stays_visible_when_the_reread_fails() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    trello.step(true, false).unwrap();
    api.fail(Some(503));
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Ok(()));
    let data = trello.snapshot(false).data;
    assert!(!card_ids(&data, 0).contains(&CARD.to_owned()));
    assert_eq!(card_ids(&data, 1), [CARD]);
}

#[test]
fn refresh_operations_have_real_identity_origin_and_outcome() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    assert_eq!(
        trello.begin(true, true).unwrap().data["operation"],
        json!({"id":1,"origin":"user","state":"running"})
    );
    let result = trello.step(true, true).unwrap();
    assert_eq!(
        result.data["operation"],
        json!({"id":1,"origin":"user","state":"succeeded"})
    );
    clock.advance(30_000);
    api.fail(Some(500));
    assert_eq!(
        trello.step(true, false).unwrap().data["operation"]["state"],
        "failed"
    );
    clock.advance(30_000);
    api.fail(None);
    assert_eq!(
        trello.step(true, false).unwrap().data["operation"]["state"],
        "recovered"
    );
}

#[test]
fn movement_result_is_independent_of_refresh_and_never_contains_card_content() {
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), FakeClock::at(NOW));
    trello.step(true, false).unwrap();
    api.fail(Some(500));
    let mut published = Vec::new();
    assert_eq!(
        trello.move_card(CARD, EMPTY_LIST, |snapshot| published.push(snapshot)),
        Ok(())
    );
    assert_eq!(
        published[0].data["movement"],
        json!({"id":2,"origin":"user","state":"running"})
    );
    let result = trello.snapshot(false);
    assert_eq!(
        result.data["movement"],
        json!({"id":2,"origin":"user","state":"succeeded"})
    );
    assert_eq!(result.data["operation"]["state"], "failed");
    for secret in [CARD, EMPTY_LIST, "private-token", "0123abcd", "Card 1.1"] {
        assert!(!result.data["movement"].to_string().contains(secret));
    }
}

#[test]
fn paused_refresh_resumes_once_and_reports_recovery_without_replaying_snapshots() {
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), clock.clone());
    trello.step(true, false).unwrap();
    clock.advance(30_000);
    api.fail(Some(500));
    trello.step(true, false).unwrap();
    trello.lifecycle(crate::lifecycle::Status::default());
    let paused = trello.snapshot(false);
    assert_eq!(paused.data["operation"]["state"], "paused");
    assert!(trello.begin(true, true).is_none());
    trello.lifecycle(crate::lifecycle::Status::default());
    assert_eq!(
        trello.snapshot(false).data["operation"],
        paused.data["operation"]
    );
    trello.lifecycle(crate::lifecycle::Status {
        online: true,
        generation: 1,
        ready_at: clock.now(),
    });
    assert!(trello.begin(true, false).is_none());
    clock.advance(3_000);
    api.fail(None);
    let start = trello.begin(true, false).unwrap();
    assert_eq!(start.data["operation"]["origin"], "resume");
    assert_eq!(start.data["operation"]["state"], "running");
    let end = trello.step(true, false).unwrap();
    assert_eq!(end.data["operation"]["id"], start.data["operation"]["id"]);
    assert_eq!(end.data["operation"]["state"], "recovered");
    assert_eq!(
        trello.snapshot(false).data["operation"],
        end.data["operation"]
    );
}

#[test]
fn movement_and_refresh_metadata_are_never_cached_or_logged() {
    let dir = tempfile::tempdir().unwrap();
    let api = Api::default();
    let (trello, _) = connected(api.clone(), FakeClock::at(NOW));
    let mut trello = trello.with_persistence(disk(&dir));
    let mut lists = fixture("lists");
    lists[0]["cards"][0]["name"] = json!("CARD_CONTENT_SENTINEL");
    *api.lists.lock().unwrap() = Some(lists);
    trello.step(true, false).unwrap();
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Ok(()));
    for file in ["cache/tasks.json", "state/info.log"] {
        let text = std::fs::read_to_string(dir.path().join(file)).unwrap();
        for secret in [
            "operation",
            "movement",
            "CARD_CONTENT_SENTINEL",
            "private-token",
            "0123abcd",
        ] {
            assert!(!text.contains(secret), "{file}: {secret}");
        }
    }
}

#[test]
fn movement_result_is_published_before_the_board_reread_starts() {
    let api = Api::default();
    let (mut trello, _) = connected(api.clone(), FakeClock::at(NOW));
    trello.step(true, false).unwrap();
    api.calls.lock().unwrap().clear();
    let mut published = Vec::new();
    trello
        .move_card(CARD, EMPTY_LIST, |snapshot| {
            if snapshot.data["movement"]["state"] == "succeeded" && published.len() == 1 {
                assert_eq!(api.calls.lock().unwrap().len(), 1, "only PUT completed");
            }
            published.push(snapshot);
        })
        .unwrap();
    assert_eq!(published[0].data["movement"]["state"], "running");
    assert_eq!(published[1].data["movement"]["state"], "succeeded");
    assert_eq!(published[2].data["operation"]["state"], "running");
    assert_eq!(published[3].data["operation"]["state"], "succeeded");
}

#[test]
fn suspension_during_put_leaves_the_result_unconfirmed_and_keeps_the_last_reading() {
    let api = Api::default();
    let shared = Arc::new(Mutex::new(crate::lifecycle::Status {
        online: true,
        generation: 1,
        ready_at: 0,
    }));
    let (trello, _) = connected(api.clone(), FakeClock::at(NOW));
    let mut trello = trello.with_lifecycle(shared.clone());
    let before = trello.step(true, false).unwrap();
    *api.lifecycle_after_put.lock().unwrap() = Some(shared);
    let mut published = Vec::new();
    assert_eq!(
        trello.move_card(CARD, EMPTY_LIST, |snapshot| published.push(snapshot)),
        Err(MoveError)
    );
    let end = published.last().unwrap();
    assert_eq!(end.data["movement"]["state"], "unconfirmed");
    assert_eq!(end.data["lists"], before.data["lists"]);
    assert_eq!(end.updated_at, before.updated_at);
    assert!(end.data["paused"].as_bool().unwrap());
    assert!(!published
        .iter()
        .any(|snapshot| snapshot.data["movement"]["state"] == "succeeded"));
}

#[test]
fn connection_generation_changed_during_get_discards_the_read_without_success_or_cache() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::at(NOW);
    let api = Api::default();
    let shared = Arc::new(Mutex::new(crate::lifecycle::Status {
        online: true,
        generation: 1,
        ready_at: 0,
    }));
    let (trello, _) = connected(api.clone(), clock.clone());
    let mut trello = trello
        .with_lifecycle(shared.clone())
        .with_persistence(disk(&dir));
    let before = trello.step(true, false).unwrap();
    let cache = std::fs::read_to_string(dir.path().join("cache/tasks.json")).unwrap();
    clock.advance(30_000);
    *api.lists.lock().unwrap() = Some(moved_lists());
    *api.lifecycle_after_get.lock().unwrap() = Some(shared);
    let end = trello.step(true, false).unwrap();
    assert_eq!(end.data["operation"]["state"], "cancelled");
    assert_eq!(end.data["lists"], before.data["lists"]);
    assert_eq!(end.updated_at, before.updated_at);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("cache/tasks.json")).unwrap(),
        cache
    );
    assert_eq!(
        trello.begin(true, false).unwrap().data["operation"]["origin"],
        "resume"
    );
}

#[test]
fn a_new_move_after_cancel_and_resume_has_a_new_identity_and_can_succeed() {
    let api = Api::default();
    let shared = Arc::new(Mutex::new(crate::lifecycle::Status {
        online: true,
        generation: 1,
        ready_at: 0,
    }));
    let (trello, _) = connected(api.clone(), FakeClock::at(NOW));
    let mut trello = trello.with_lifecycle(shared.clone());
    trello.step(true, false).unwrap();
    *api.lifecycle_after_put.lock().unwrap() = Some(shared.clone());
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Err(MoveError));
    let cancelled = trello.snapshot(false).data["movement"]["id"].clone();
    *shared.lock().unwrap() = crate::lifecycle::Status {
        online: true,
        generation: 2,
        ready_at: 0,
    };
    trello.lifecycle(*shared.lock().unwrap());
    *api.lists.lock().unwrap() = Some(moved_lists());
    assert_eq!(trello.move_card(CARD, EMPTY_LIST, |_| {}), Ok(()));
    let end = trello.snapshot(false);
    assert_ne!(end.data["movement"]["id"], cancelled);
    assert_eq!(end.data["movement"]["state"], "succeeded");
}
