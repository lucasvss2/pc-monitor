use super::*;
use crate::ports::fakes::{FakeClock, FakeRoot};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Shortly after the recorded app-server session.
const NOW: u64 = 1793460871000;

/// Recorded `codex app-server` replies keyed by method, in capture order.
fn recorded() -> Vec<(String, Value)> {
    let methods = ["initialize", "account/read", "account/rateLimits/read"];
    let path = crate::ports::fakes::fixtures().join("codex/app-server.jsonl");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter_map(|message| {
            let id = message["id"].as_u64()?;
            Some((
                methods[id as usize - 1].to_owned(),
                message["result"].clone(),
            ))
        })
        .collect()
}

/// Answers each request with the recorded result for its method.
#[derive(Clone, Default)]
struct Server {
    missing: Arc<Mutex<bool>>,
    dead: Arc<Mutex<bool>>,
    /// Methods answered with a JSON-RPC error.
    failing: Arc<Mutex<Vec<&'static str>>>,
    /// Methods that never get an answer.
    silent: Arc<Mutex<Vec<&'static str>>>,
    results: Arc<Mutex<Vec<(String, Value)>>>,
    inbox: Arc<Mutex<VecDeque<Value>>>,
    sent: Arc<Mutex<Vec<Value>>>,
    spawns: Arc<Mutex<usize>>,
}
impl Server {
    fn recorded() -> Self {
        let server = Server::default();
        *server.results.lock().unwrap() = recorded();
        server
    }
    fn methods(&self) -> Vec<String> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .map(|m| m["method"].as_str().unwrap().to_owned())
            .collect()
    }
    fn queries(&self) -> usize {
        self.methods()
            .iter()
            .filter(|m| *m == "account/rateLimits/read")
            .count()
    }
    fn result(&self, method: &str) -> Value {
        self.results
            .lock()
            .unwrap()
            .iter()
            .find(|(m, _)| m == method)
            .unwrap()
            .1
            .clone()
    }
    fn set_result(&self, method: &str, result: Value) {
        for (m, r) in self.results.lock().unwrap().iter_mut() {
            if m == method {
                *r = result.clone();
            }
        }
    }
    fn notify(&self, method: &str, params: Value) {
        self.inbox
            .lock()
            .unwrap()
            .push_back(json!({"method": method, "params": params}));
    }
}
impl AppServer for Server {
    fn installed(&self) -> bool {
        !*self.missing.lock().unwrap()
    }

    fn spawn(&mut self) -> Result<Box<dyn Connection>, ()> {
        if *self.missing.lock().unwrap() {
            return Err(());
        }
        *self.spawns.lock().unwrap() += 1;
        *self.dead.lock().unwrap() = false;
        Ok(Box::new(self.clone()))
    }
}
impl Connection for Server {
    fn send(&mut self, message: &Value) -> Result<(), ()> {
        self.sent.lock().unwrap().push(message.clone());
        let (Some(id), Some(method)) = (message.get("id"), message["method"].as_str()) else {
            return Ok(());
        };
        if self.silent.lock().unwrap().contains(&method) {
            return Ok(());
        }
        let reply = if self.failing.lock().unwrap().contains(&method) {
            json!({"id": id, "error": {"code": -32603, "message": "failed"}})
        } else {
            json!({"id": id, "result": self.result(method)})
        };
        self.inbox.lock().unwrap().push_back(reply);
        Ok(())
    }
    fn receive(&mut self) -> Vec<Value> {
        self.inbox.lock().unwrap().drain(..).collect()
    }
    fn alive(&mut self) -> bool {
        !*self.dead.lock().unwrap()
    }
}

fn codex(server: &Server, clock: &FakeClock) -> Codex<Server, FakeRoot, FakeClock> {
    Codex::new(server.clone(), FakeRoot::from("codex/home"), clock.clone())
}

#[test]
fn app_server_reading_shows_plan_and_both_windows() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(
        data["codex"],
        json!({
            "installed": true,
            "state": "ready",
            "operation": {"id":1,"origin":"automatic","state":"succeeded"},
            "plan": "plus",
            "reading": {
                "source": "appServer",
                "readAt": NOW,
                "buckets": [{
                    "id": "codex",
                    "name": null,
                    "primary": {"usedPercent": 91.0, "durationMins": 300, "resetsAt": 1793467355000u64, "reset": false},
                    "secondary": {"usedPercent": 48.0, "durationMins": 10080, "resetsAt": 1793814230000u64, "reset": false},
                }],
            },
        })
    );
    assert_eq!(
        server.methods(),
        [
            "initialize",
            "initialized",
            "account/read",
            "account/rateLimits/read"
        ]
    );
}

#[test]
fn a_restarted_window_never_assumes_zero_percent() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    // Past the recorded 5 h reset, with no newer reading.
    clock.advance(1793467355000 - NOW + 1);
    let data = codex.snapshot().data;
    let bucket = &data["codex"]["reading"]["buckets"][0];
    assert_eq!(
        bucket["primary"],
        json!({"usedPercent": null, "durationMins": 300, "resetsAt": 1793467355000u64, "reset": true})
    );
    assert_eq!(bucket["secondary"]["usedPercent"], 48.0);
    assert_eq!(bucket["secondary"]["reset"], false);
    assert_eq!(data["codex"]["reading"]["readAt"], NOW);
}

#[test]
fn without_the_cli_the_module_explains_it_is_missing() {
    let server = Server::recorded();
    *server.missing.lock().unwrap() = true;
    let mut codex = codex(&server, &FakeClock::at(NOW));
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(data["codex"]["state"], "missing");
    assert!(data["codex"]["reading"].is_null());
}

#[test]
fn an_account_without_login_counts_as_missing() {
    let server = Server::recorded();
    server.set_result(
        "account/read",
        json!({"account": null, "requiresOpenaiAuth": true}),
    );
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    let mut codex = Codex::new(
        server.clone(),
        FakeRoot::from("session"),
        FakeClock::at(NOW),
    );
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(data["codex"]["state"], "missing");
}

/// Timestamp of the newest recorded rollout snapshot (2026-10-31T14:33:58.475Z).
const ROLLOUT_AT: u64 = 1793457238475;

#[test]
fn failed_query_falls_back_to_the_newest_rollout_snapshot() {
    let server = Server::recorded();
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    let mut codex = codex(&server, &FakeClock::at(NOW));
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(data["codex"]["state"], "ready");
    assert_eq!(data["codex"]["plan"], "plus");
    assert_eq!(
        data["codex"]["reading"],
        json!({
            "source": "rollout",
            "readAt": ROLLOUT_AT,
            "buckets": [{
                "id": "codex",
                "name": null,
                "primary": {"usedPercent": 91.0, "durationMins": 300, "resetsAt": 1793467355000u64, "reset": false},
                "secondary": {"usedPercent": 48.0, "durationMins": 10080, "resetsAt": 1793814230000u64, "reset": false},
            }],
        })
    );
}

#[test]
fn rollout_events_without_windows_are_skipped() {
    let server = Server::recorded();
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    let root = FakeRoot::from("codex/home");
    // A newer session whose only snapshot is a bucket without windows.
    let premium = std::fs::read_to_string(root.dir.path().join(
        "sessions/2026/10/20/rollout-2026-10-20T14-17-25-22222222-2222-4222-8222-000000000005.jsonl",
    ))
    .unwrap();
    root.write(
        "/sessions/2026/11/01/rollout-2026-11-01T08-00-00-01a1aaaa-0000-7000-8000-000000000000.jsonl",
        &premium,
    );
    let mut codex = Codex::new(server, root, FakeClock::at(NOW));
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(data["codex"]["reading"]["readAt"], ROLLOUT_AT);
}

#[test]
fn an_older_rollout_never_replaces_a_newer_reading() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    clock.advance(300_000);
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(data["codex"]["reading"]["source"], "appServer");
    assert_eq!(data["codex"]["reading"]["readAt"], NOW);
}

#[test]
fn guarantee_query_runs_every_five_minutes_or_every_minute_while_visible() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    assert_eq!(server.queries(), 1);
    clock.advance(299_000);
    codex.step(false, false);
    assert_eq!(server.queries(), 1);
    clock.advance(1_000);
    codex.step(false, false);
    assert_eq!(server.queries(), 2);
    clock.advance(59_000);
    codex.step(true, false);
    assert_eq!(server.queries(), 2);
    clock.advance(1_000);
    codex.step(true, false);
    assert_eq!(server.queries(), 3);
}

#[test]
fn manual_refresh_queries_now_and_waits_ten_seconds_between_clicks() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    clock.advance(1_000);
    codex.step(false, true);
    assert_eq!(server.queries(), 2);
    clock.advance(9_000);
    codex.step(false, true);
    assert_eq!(server.queries(), 2);
    clock.advance(1_000);
    codex.step(false, true);
    assert_eq!(server.queries(), 3);
}

#[test]
fn a_query_in_flight_shows_refreshing_until_answered() {
    let server = Server::recorded();
    server
        .silent
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    assert_eq!(codex.step(false, false).unwrap().data["refreshing"], true);
    // Unanswered for 30 s: give up, fall back to rollouts and ask again later.
    clock.advance(30_000);
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(data["refreshing"], false);
    assert_eq!(data["codex"]["reading"]["source"], "rollout");
    assert_eq!(server.queries(), 1);
}

#[test]
fn pushed_updates_merge_into_the_last_reading_without_clearing_known_values() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    clock.advance(20_000);
    // Sparse update shaped like the recorded RateLimitSnapshot.
    let mut update = server.result("account/rateLimits/read")["rateLimits"].clone();
    update["primary"]["usedPercent"] = json!(93);
    update["secondary"] = Value::Null;
    update["planType"] = Value::Null;
    server.notify("account/rateLimits/updated", json!({"rateLimits": update}));
    let data = codex.step(false, false).unwrap().data;
    let reading = &data["codex"]["reading"];
    assert_eq!(reading["readAt"], NOW + 20_000);
    assert_eq!(reading["buckets"][0]["primary"]["usedPercent"], 93.0);
    assert_eq!(reading["buckets"][0]["secondary"]["usedPercent"], 48.0);
    assert_eq!(data["codex"]["plan"], "plus");
    server.notify(
        "account/updated",
        json!({"authMode": "chatgpt", "planType": "pro"}),
    );
    assert_eq!(
        codex.step(false, false).unwrap().data["codex"]["plan"],
        "pro"
    );
}

#[test]
fn without_new_messages_nothing_is_published() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    clock.advance(1_000);
    assert!(codex.step(false, false).is_none());
}

#[test]
fn a_dead_app_server_is_restarted_at_most_every_thirty_seconds() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    clock.advance(1_000);
    *server.dead.lock().unwrap() = true;
    codex.step(false, false);
    assert_eq!(*server.spawns.lock().unwrap(), 1);
    clock.advance(29_000);
    codex.step(false, false);
    assert_eq!(*server.spawns.lock().unwrap(), 2);
    // The new process is initialized again and asked for the limits.
    assert_eq!(
        server.methods()[4..],
        [
            "initialize",
            "initialized",
            "account/read",
            "account/rateLimits/read"
        ]
    );
    assert_eq!(
        codex.snapshot().data["codex"]["reading"]["readAt"],
        NOW + 30_000
    );
}

fn status(online: bool, generation: u64, ready_at: u64) -> crate::lifecycle::Status {
    crate::lifecycle::Status {
        online,
        generation,
        ready_at,
    }
}

#[test]
fn offline_pauses_queries_but_keeps_pushed_updates_and_data_age() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.lifecycle(status(true, 1, NOW - 10_000));
    codex.step(false, false);
    assert_eq!(server.queries(), 1);
    codex.lifecycle(status(false, 1, NOW - 10_000));
    clock.advance(600_000);
    let data = codex.step(true, true).unwrap().data;
    assert_eq!(server.queries(), 1);
    assert_eq!(data["paused"], true);
    assert_eq!(data["codex"]["reading"]["readAt"], NOW);
    let update = server.result("account/rateLimits/read")["rateLimits"].clone();
    server.notify("account/rateLimits/updated", json!({"rateLimits": update}));
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(data["codex"]["reading"]["readAt"], NOW + 600_000);
}

#[test]
fn reconnection_waits_for_its_turn_then_refreshes_once_and_restarts_a_dead_server() {
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.lifecycle(status(true, 1, NOW - 10_000));
    codex.step(false, false);
    clock.advance(60_000);
    *server.dead.lock().unwrap() = true;
    // Back online: remote sources start one every 3 s, Codex in the third slot.
    codex.lifecycle(status(true, 2, NOW + 60_000));
    codex.step(false, false);
    assert_eq!(*server.spawns.lock().unwrap(), 2);
    assert_eq!(server.queries(), 1);
    clock.advance(6_000);
    codex.step(false, false);
    assert_eq!(server.queries(), 2);
    clock.advance(1_000);
    codex.step(false, false);
    assert_eq!(server.queries(), 2);
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
fn restart_shows_the_cached_reading_before_any_query() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock).with_persistence(disk(&dir));
    let fresh = codex.step(false, false).unwrap();
    let cached = std::fs::read_to_string(dir.path().join("cache/ai.json")).unwrap();
    // The recorded replies carry the account e-mail and ids; none reach the disk.
    for forbidden in [
        "exemplo@example.com",
        "conta-exemplo",
        "00000000-0000",
        "id-exemplo",
    ] {
        assert!(!cached.contains(forbidden), "{forbidden}");
    }
    clock.advance(3_600_000);
    let restored = Codex::new(Server::default(), FakeRoot::from("codex/home"), clock)
        .with_persistence(disk(&dir))
        .snapshot();
    assert_eq!(restored.updated_at, Some(NOW));
    let mut expected = fresh.data["codex"].clone();
    expected["installed"] = Value::Null;
    expected.as_object_mut().unwrap().remove("operation");
    assert_eq!(restored.data["codex"], expected);
}

#[test]
fn info_log_records_only_source_event_and_duration() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::recorded();
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock).with_persistence(disk(&dir));
    codex.step(false, false);
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    clock.advance(300_000);
    codex.step(false, false);
    let log = std::fs::read_to_string(dir.path().join("state/info.log")).unwrap();
    let records: Vec<Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        records,
        [
            json!({"source":"ai","event":"updated","httpStatus":null,"durationMs":0}),
            json!({"source":"ai","event":"failed","httpStatus":null,"durationMs":0}),
        ]
    );
}

/// UI fixtures in src/lib/ai/fixtures/ are snapshots published by the core.
#[test]
fn ui_fixtures_match_published_snapshots() {
    let full = {
        let server = Server::recorded();
        codex(&server, &FakeClock::at(NOW))
            .step(false, false)
            .unwrap()
    };
    let stale = {
        let server = Server::recorded();
        server
            .failing
            .lock()
            .unwrap()
            .push("account/rateLimits/read");
        codex(&server, &FakeClock::at(NOW))
            .step(false, false)
            .unwrap()
    };
    let reset = {
        let server = Server::recorded();
        let clock = FakeClock::at(NOW);
        let mut codex = codex(&server, &clock);
        codex.step(false, false);
        clock.advance(1793467355000 - NOW + 60_000);
        codex.snapshot()
    };
    let missing = {
        let server = Server::recorded();
        *server.missing.lock().unwrap() = true;
        codex(&server, &FakeClock::at(NOW))
            .step(false, false)
            .unwrap()
    };
    let empty = {
        let server = Server::recorded();
        server
            .failing
            .lock()
            .unwrap()
            .push("account/rateLimits/read");
        Codex::new(server, FakeRoot::from("session"), FakeClock::at(NOW))
            .step(false, false)
            .unwrap()
    };
    let failure = {
        let server = Server::recorded();
        server
            .failing
            .lock()
            .unwrap()
            .push("account/rateLimits/read");
        let clock = FakeClock::at(NOW);
        let mut codex = codex(&server, &clock);
        codex.step(false, false);
        clock.advance(30_000);
        codex.step(false, false);
        clock.advance(60_000);
        codex.step(false, false).unwrap()
    };
    for (name, snapshot) in [
        ("full", full),
        ("failure", failure),
        ("stale", stale),
        ("reset", reset),
        ("missing", missing),
        ("empty", empty),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../src/lib/ai/fixtures/{name}.json"));
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
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
fn a_push_never_makes_a_rollout_reading_look_fresh() {
    let server = Server::recorded();
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    server.failing.lock().unwrap().clear();
    clock.advance(1_000);
    let mut update = server.result("account/rateLimits/read")["rateLimits"].clone();
    update["secondary"] = Value::Null;
    server.notify("account/rateLimits/updated", json!({"rateLimits": update}));
    let data = codex.step(false, false).unwrap().data;
    // The full snapshot is read again instead of mixing sources.
    assert_eq!(server.queries(), 2);
    assert_eq!(data["codex"]["reading"]["source"], "appServer");
    assert_eq!(
        data["codex"]["reading"]["buckets"][0]["secondary"]["usedPercent"],
        48.0
    );
}

#[test]
fn the_newest_rollout_event_wins_even_in_a_session_started_earlier() {
    let server = Server::recorded();
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    let root = FakeRoot::from("codex/home");
    let older = "/sessions/2026/10/30/rollout-2026-10-30T22-52-32-22222222-2222-4222-8222-000000000006.jsonl";
    let content = root
        .read(older)
        .unwrap()
        .replace("2026-10-31T01:52:40.000Z", "2026-10-31T15:00:00.000Z");
    root.write(older, &content);
    let data = Codex::new(server, root, FakeClock::at(NOW))
        .step(false, false)
        .unwrap()
        .data;
    assert_eq!(
        data["codex"]["reading"]["buckets"][0]["secondary"]["usedPercent"],
        29.0
    );
}

#[test]
fn failed_queries_back_off_and_show_the_failure_after_three_in_a_row() {
    let server = Server::recorded();
    server
        .failing
        .lock()
        .unwrap()
        .push("account/rateLimits/read");
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    codex.step(false, false);
    clock.advance(29_000);
    codex.step(false, true);
    assert_eq!(server.queries(), 1);
    clock.advance(1_000);
    codex.step(false, false);
    assert_eq!(server.queries(), 2);
    assert!(codex.snapshot().data["codex"]["failureSince"].is_null());
    clock.advance(60_000);
    let data = codex.step(false, false).unwrap().data;
    assert_eq!(server.queries(), 3);
    assert_eq!(data["codex"]["failureSince"], NOW);
    server.failing.lock().unwrap().clear();
    clock.advance(120_000);
    let data = codex.step(false, false).unwrap().data;
    assert!(data["codex"]["failureSince"].is_null());
    assert_eq!(data["codex"]["reading"]["source"], "appServer");
}

#[test]
fn requested_query_has_its_own_real_result() {
    let clock = FakeClock::at(NOW);
    let mut codex = Codex::new(Server::recorded(), FakeRoot::from("codex/home"), clock);
    let snapshot = codex.step(true, true).unwrap();
    assert_eq!(
        snapshot.data["codex"]["operation"],
        json!({"id":1,"origin":"user","state":"succeeded"})
    );
}

#[test]
fn discarded_queries_cannot_complete_after_a_pause_and_resume_has_a_new_identity() {
    use crate::lifecycle::Status;
    let server = Server::recorded();
    server.silent.lock().unwrap().push(RATE_LIMITS);
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    let start = codex.step(true, true).unwrap();
    assert_eq!(start.data["codex"]["operation"]["state"], "running");
    let request = server
        .sent
        .lock()
        .unwrap()
        .iter()
        .find(|m| m["method"] == RATE_LIMITS)
        .unwrap()
        .clone();
    codex.lifecycle(Status {
        online: false,
        generation: 1,
        ready_at: 0,
    });
    server
        .inbox
        .lock()
        .unwrap()
        .push_back(json!({"id":request["id"],"result":server.result(RATE_LIMITS)}));
    let paused = codex.step(true, false).unwrap();
    assert_eq!(
        paused.data["codex"]["operation"],
        json!({"id":2,"origin":"automatic","state":"paused"})
    );
    assert!(paused.data["codex"]["reading"].is_null());
    server.silent.lock().unwrap().clear();
    codex.lifecycle(Status {
        online: true,
        generation: 2,
        ready_at: 0,
    });
    let resumed = codex.step(true, false).unwrap();
    assert_eq!(
        resumed.data["codex"]["operation"],
        json!({"id":3,"origin":"resume","state":"succeeded"})
    );
}

#[test]
fn failures_and_recovery_are_real_results_not_rollout_fallback_success() {
    let server = Server::recorded();
    server.failing.lock().unwrap().push(RATE_LIMITS);
    let clock = FakeClock::at(NOW);
    let mut codex = codex(&server, &clock);
    let failed = codex.step(true, true).unwrap();
    assert_eq!(failed.data["codex"]["operation"]["state"], "failed");
    assert_eq!(failed.data["codex"]["reading"]["source"], "rollout");
    server.failing.lock().unwrap().clear();
    clock.advance(30_000);
    let recovered = codex.step(true, false).unwrap();
    assert_eq!(
        recovered.data["codex"]["operation"],
        json!({"id":2,"origin":"automatic","state":"recovered"})
    );
}
