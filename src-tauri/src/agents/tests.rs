use super::*;
use crate::ports::fakes::FakeClock;
use std::sync::{Arc, Mutex};
#[derive(Clone)]
struct Process(Arc<Mutex<Result<String, ()>>>);
impl Processes for Process {
    fn execute(&self, program: &str, args: &[&str]) -> Result<String, ()> {
        assert_eq!(program, "claude");
        assert_eq!(args, ["agents", "--json", "--all"]);
        self.0.lock().unwrap().clone()
    }
}
#[test]
fn recorded_blocked_session_sets_accent_and_polling_runs_every_two_seconds() {
    let process = Process(Arc::new(Mutex::new(Ok(include_str!(
        "../../fixtures/agents/recorded.json"
    )
    .into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process, clock.clone());
    let data = monitor.step().unwrap();
    assert!(data.accent_dot);
    assert_eq!(data.sessions[0].project, "example-project");
    assert_eq!(data.sessions[0].state, SessionState::Waiting);
    assert!(monitor.step().is_none());
    clock.advance(2000);
    assert!(monitor.step().is_some());
}

#[test]
fn only_observed_live_sessions_are_retained_and_expire_after_two_hours() {
    let process = Process(Arc::new(Mutex::new(Ok(include_str!(
        "../../fixtures/agents/recorded.json"
    )
    .into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone());
    assert!(monitor.step().unwrap().closed.is_empty());
    *process.0.lock().unwrap() = Err(());
    clock.advance(2000);
    assert_eq!(monitor.step().unwrap().sessions.len(), 1);
    *process.0.lock().unwrap() = Ok("[]".into());
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.sessions.is_empty());
    assert!(!data.accent_dot);
    assert_eq!(data.closed.len(), 1);
    clock.advance(7_200_000);
    assert!(monitor.step().unwrap().closed.is_empty());
}

#[test]
fn documented_states_and_terminal_history_are_reconciled_without_content_leaks() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone());
    for (field, value, expected) in [
        ("status", "busy", SessionState::Working),
        ("status", "waiting", SessionState::Waiting),
        ("status", "idle", SessionState::Done),
        ("state", "working", SessionState::Working),
        ("state", "blocked", SessionState::Waiting),
        ("state", "done", SessionState::Done),
        ("state", "failed", SessionState::Failed),
        ("state", "stopped", SessionState::Done),
    ] {
        // Variants of the recorded schema for states not currently running locally.
        let mut row = serde_json::json!({"pid":123,"sessionId":"test","cwd":"/tmp/project","startedAt":1000,
            "prompt":"PRIVATE_PROMPT", "response":"PRIVATE_RESPONSE", "token":"PRIVATE_TOKEN"});
        row[field] = value.into();
        *process.0.lock().unwrap() = Ok(serde_json::json!([row]).to_string());
        clock.advance(2000);
        let data = monitor.step().unwrap();
        assert_eq!(data.sessions[0].state, expected);
        assert!(!serde_json::to_string(&data).unwrap().contains("PRIVATE_"));
    }
    *process.0.lock().unwrap() =
        Ok(r#"[{"id":"old","cwd":"/tmp/old","startedAt":1,"state":"done"}]"#.into());
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.sessions.is_empty());
    assert_eq!(data.closed.len(), 1);
    assert_eq!(data.closed[0].id, "test");
}

#[test]
fn closed_history_is_bounded_and_terminal_jobs_seen_at_startup_are_ignored() {
    let process = Process(Arc::new(Mutex::new(Ok(
        r#"[{"id":"old","cwd":"/tmp","startedAt":1,"state":"failed"}]"#.into(),
    ))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone());
    let data = monitor.step().unwrap();
    assert!(data.sessions.is_empty());
    assert!(data.closed.is_empty());
    let rows: Vec<_> = (0..7).map(|id| serde_json::json!({"id":id.to_string(),"cwd":"/tmp","startedAt":1,"status":"busy","pid":id+100})).collect();
    *process.0.lock().unwrap() = Ok(serde_json::to_string(&rows).unwrap());
    clock.advance(2000);
    assert_eq!(monitor.step().unwrap().sessions.len(), 7);
    *process.0.lock().unwrap() = Ok("not json".into());
    clock.advance(2000);
    assert_eq!(monitor.step().unwrap().sessions.len(), 7);
    *process.0.lock().unwrap() = Ok("[]".into());
    clock.advance(2000);
    assert_eq!(monitor.step().unwrap().closed.len(), 5);
    let mut restarted = Monitor::new(process, clock);
    assert!(restarted.step().unwrap().closed.is_empty());
}

#[test]
fn background_failure_keeps_its_outcome_in_closed_history() {
    let process = Process(Arc::new(Mutex::new(Ok(include_str!(
        "../../fixtures/agents/recorded.json"
    )
    .into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone());
    monitor.step().unwrap();
    let mut records: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/agents/recorded.json")).unwrap();
    records[0]["state"] = "failed".into();
    *process.0.lock().unwrap() = Ok(records.to_string());
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.sessions.is_empty());
    assert_eq!(data.closed[0].state, SessionState::Closed);
    assert_eq!(data.closed[0].last_state, Some(SessionState::Failed));
}

#[cfg(target_os = "linux")]
#[test]
fn codex_sessions_share_history_and_keep_provider_identity() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let source = codex::tests::source();
    let mut monitor = Monitor::new(process, clock.clone()).with_codex(source);
    let data = monitor.step().unwrap();
    assert_eq!(data.sessions.len(), 1);
    let json = serde_json::to_value(&data).unwrap();
    assert_eq!(json["sessions"][0]["provider"], "codex");
    assert_eq!(json["sessions"][0]["state"], "working");
    assert_eq!(json["codexWaitingUnavailable"], true);
}

#[cfg(target_os = "linux")]
#[test]
fn codex_rollout_outcomes_process_exit_and_read_failures_are_reconciled() {
    let source = codex::tests::source();
    let root = source.test_root();
    let process = Process(Arc::new(Mutex::new(Ok(include_str!(
        "../../fixtures/agents/recorded.json"
    )
    .into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone()).with_codex(source);
    assert_eq!(monitor.step().unwrap().sessions.len(), 2);
    for (event, expected) in [
        ("task_complete", SessionState::Done),
        ("task_started", SessionState::Working),
        ("error", SessionState::Failed),
        ("turn_aborted", SessionState::Done),
    ] {
        // Recorded successful/interrupted turns; error is an explicit schema variant.
        let recorded = match event {
            "task_complete" => include_str!("../../fixtures/agents/codex/task_complete.jsonl"),
            "turn_aborted" => include_str!("../../fixtures/agents/codex/turn_aborted.jsonl"),
            "task_started" => include_str!("../../fixtures/agents/codex/rollout.jsonl"),
            _ => {
                r#"{"type":"event_msg","payload":{"type":"error","message":"PRIVATE_RESPONSE","token":"PRIVATE_TOKEN"}}"#
            }
        };
        std::fs::write(root.join("rollout.jsonl"), recorded).unwrap();
        clock.advance(2000);
        let data = monitor.step().unwrap();
        let session = data
            .sessions
            .iter()
            .find(|s| s.provider == Provider::Codex)
            .unwrap();
        assert_eq!(session.state, expected);
        assert!(data.accent_dot);
        assert!(!serde_json::to_string(&data).unwrap().contains("PRIVATE_"));
    }
    std::fs::write(root.join("state_5.sqlite"), "invalid sqlite PRIVATE_TOKEN").unwrap();
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.failed);
    assert_eq!(data.sessions.len(), 2);
}

#[cfg(target_os = "linux")]
#[test]
fn codex_closed_history_only_contains_previously_live_sessions() {
    let source = codex::tests::source();
    let root = source.test_root();
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone()).with_codex(source);
    monitor.step().unwrap();
    std::fs::remove_file(root.join("proc/123/fd/4")).unwrap();
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.sessions.is_empty());
    assert_eq!(data.closed.len(), 1);
    assert_eq!(data.closed[0].last_state, Some(SessionState::Working));
    clock.advance(7_200_000);
    assert!(monitor.step().unwrap().closed.is_empty());
    let mut restarted = Monitor::new(process, clock).with_codex(codex::Adapter::new(
        root,
        crate::ports::RootFilesystem::at("/nonexistent"),
    ));
    assert!(restarted.step().unwrap().closed.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn unavailable_codex_rollout_retains_last_reading_and_database_is_read_only() {
    let source = codex::tests::source();
    let root = source.test_root();
    let before = std::fs::read(root.join("state_5.sqlite")).unwrap();
    let process = Process(Arc::new(Mutex::new(Err(()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process, clock.clone()).with_codex(source);
    let data = monitor.step().unwrap();
    assert!(data.failed); // Claude failure must not hide Codex.
    assert_eq!(data.sessions[0].state, SessionState::Working);
    assert_eq!(std::fs::read(root.join("state_5.sqlite")).unwrap(), before);
    assert!(!root.join("state_5.sqlite-journal").exists());
    std::fs::remove_file(root.join("rollout.jsonl")).unwrap();
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.failed);
    assert_eq!(data.sessions[0].state, SessionState::Working);
}

#[cfg(target_os = "linux")]
#[test]
fn injected_sqlite_and_proc_failures_never_close_a_live_codex_session() {
    use crate::ports::{Files, Space};
    use std::{io, path::PathBuf};
    struct UnavailableFiles {
        inner: crate::ports::fakes::FakeRoot,
        unavailable: Arc<Mutex<Option<&'static str>>>,
    }
    impl Files for UnavailableFiles {
        fn read(&self, path: &str) -> io::Result<String> {
            self.inner.read(path)
        }
        fn list(&self, path: &str) -> io::Result<Vec<String>> {
            if *self.unavailable.lock().unwrap() == Some(path) {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            self.inner.list(path)
        }
        fn space(&self, path: &str) -> io::Result<Space> {
            self.inner.space(path)
        }
        fn read_link(&self, path: &str) -> io::Result<PathBuf> {
            if *self.unavailable.lock().unwrap() == Some(path) {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            self.inner.read_link(path)
        }
        fn read_sqlite(&self, path: &str, query: &str) -> io::Result<Vec<serde_json::Value>> {
            if *self.unavailable.lock().unwrap() == Some(path) {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            self.inner.read_sqlite(path, query)
        }
    }
    let source = codex::tests::source();
    let files = source.test_files();
    files.write("/proc/456/status", "Uid:\t1000\t1000\t1000\t1000\n");
    files.write("/proc/self/status", "Uid:\t1000\t1000\t1000\t1000\n");
    files.write("/proc/456/comm", "systemd\n");
    let unavailable = Arc::new(Mutex::new(None));
    let source = codex::Adapter::new(
        "/".into(),
        UnavailableFiles {
            inner: files,
            unavailable: unavailable.clone(),
        },
    );
    let clock = FakeClock::at(1800000000000);
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let mut monitor = Monitor::new(process, clock.clone()).with_codex(source);
    assert_eq!(
        monitor.step().unwrap().sessions[0].state,
        SessionState::Working
    );
    *unavailable.lock().unwrap() = Some("/proc/456/exe");
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(
        !data.failed,
        "An unrelated inaccessible process must not hide Codex sessions"
    );
    assert_eq!(data.sessions[0].state, SessionState::Working);
    for path in ["/state_5.sqlite", "/proc/123/fd", "/proc/123/exe"] {
        *unavailable.lock().unwrap() = Some(path);
        clock.advance(2000);
        let data = monitor.step().unwrap();
        assert!(data.failed);
        assert_eq!(data.sessions[0].state, SessionState::Working);
        assert!(data.closed.is_empty());
    }
}

#[derive(Clone, Debug)]
struct NotificationCall {
    replaces: u32,
    title: String,
    body: String,
    session: String,
}
#[derive(Clone, Default)]
struct Notifications(Arc<Mutex<Vec<NotificationCall>>>, Arc<Mutex<Vec<u32>>>);
impl notifications::Sink for Notifications {
    fn show(&mut self, replaces: u32, title: &str, body: &str, session: &str) -> u32 {
        let mut calls = self.0.lock().unwrap();
        calls.push(NotificationCall {
            replaces,
            title: title.into(),
            body: body.into(),
            session: session.into(),
        });
        calls.len() as u32
    }
    fn close(&mut self, id: u32) {
        self.1.lock().unwrap().push(id);
    }
}
#[test]
fn waiting_notification_uses_only_session_metadata_and_replaces_previous() {
    let process = Process(Arc::new(Mutex::new(Ok(include_str!(
        "../../fixtures/agents/recorded.json"
    )
    .into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor =
        Monitor::new(process.clone(), clock.clone()).with_notifications(notifications.clone());
    monitor.step();
    let calls = notifications.0.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].session, "11111111-1111-4111-8111-111111111111");
    assert_eq!(calls[0].title, "Claude Code");
    assert_eq!(calls[0].body, "Aguardando sua resposta");
    let mut rows: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/agents/recorded.json")).unwrap();
    rows[0]["state"] = "working".into();
    *process.0.lock().unwrap() = Ok(rows.to_string());
    clock.advance(2000);
    monitor.step();
    rows[0]["state"] = "blocked".into();
    rows[0]["prompt"] = "PRIVATE_PROMPT".into();
    rows[0]["response"] = "PRIVATE_RESPONSE".into();
    rows[0]["waitingFor"] = "PRIVATE_PROMPT".into();
    *process.0.lock().unwrap() = Ok(rows.to_string());
    clock.advance(2000);
    monitor.step();
    let calls = notifications.0.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].replaces, 1);
    assert!(!format!("{calls:?}").contains("PRIVATE_"));
}

#[test]
fn only_long_completed_turns_and_failures_notify_never_interruptions_or_closed() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor =
        Monitor::new(process.clone(), clock.clone()).with_notifications(notifications.clone());
    let mut row = serde_json::json!({"pid":123,"sessionId":"test","cwd":"/tmp/project","startedAt":1,"status":"busy", "prompt":"PRIVATE_PROMPT","response":"PRIVATE_RESPONSE"});
    for (duration, outcome, count) in [
        (29_999, "idle", 0),
        (30_000, "idle", 1),
        (252_000, "idle", 2),
        (40_000, "stopped", 2),
        (2000, "failed", 3),
    ] {
        row["status"] = "busy".into();
        *process.0.lock().unwrap() = Ok(serde_json::json!([row.clone()]).to_string());
        clock.advance(2000);
        monitor.step();
        clock.advance(duration);
        row["status"] = outcome.into();
        *process.0.lock().unwrap() = Ok(serde_json::json!([row.clone()]).to_string());
        monitor.step();
        assert_eq!(notifications.0.lock().unwrap().len(), count);
    }
    *process.0.lock().unwrap() = Ok("[]".into());
    clock.advance(2000);
    monitor.step();
    let calls = notifications.0.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].body, "Turno concluído em 30 s");
    assert_eq!(calls[1].body, "Turno concluído em 4 min 12 s");
    assert_eq!(calls[2].body, "Turno terminou com erro");
    assert!(!format!("{calls:?}").contains("PRIVATE_"));
}

#[test]
fn three_distinct_sessions_within_ten_seconds_collapse_into_one_group() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor =
        Monitor::new(process.clone(), clock.clone()).with_notifications(notifications.clone());
    let mut rows = Vec::new();
    for id in 0..4 {
        rows.push(serde_json::json!({"id":id.to_string(), "pid":123+id, "cwd":"/tmp/project","startedAt":1,"status":"waiting"}));
        *process.0.lock().unwrap() = Ok(serde_json::to_string(&rows).unwrap());
        monitor.step();
        clock.advance(2000);
    }
    let calls = notifications.0.lock().unwrap();
    let mut closed = notifications.1.lock().unwrap().clone();
    closed.sort_unstable();
    assert_eq!(closed, vec![1, 2]);
    assert_eq!(calls[2].title, "Agentes");
    assert_eq!(calls[2].body, "3 sessões aguardando você");
    assert_eq!(calls[3].replaces, 3);
    assert_eq!(calls[3].body, "4 sessões aguardando você");
    drop(calls);
    clock.advance(10_001);
    rows.push(serde_json::json!({"id":"late", "pid":999,"cwd":"/tmp/late","startedAt":1,"status":"waiting"}));
    *process.0.lock().unwrap() = Ok(serde_json::to_string(&rows).unwrap());
    monitor.step();
    assert_eq!(
        notifications.0.lock().unwrap().last().unwrap().title,
        "Claude Code"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn notification_switches_apply_to_both_providers_and_codex_interruptions_stay_silent() {
    let source = codex::tests::source();
    let files = source.test_files();
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor = Monitor::new(process, clock.clone())
        .with_codex(source)
        .with_notifications(notifications.clone());
    monitor.step();
    files.write(
        "/rollout.jsonl",
        include_str!("../../fixtures/agents/codex/task_complete.jsonl"),
    );
    clock.advance(2000);
    monitor.step();
    assert_eq!(
        notifications.0.lock().unwrap()[0].body,
        "Turno concluído em 40 s"
    );
    files.write(
        "/rollout.jsonl",
        include_str!("../../fixtures/agents/codex/rollout.jsonl"),
    );
    clock.advance(2000);
    monitor.step();
    clock.advance(40_000);
    files.write(
        "/rollout.jsonl",
        include_str!("../../fixtures/agents/codex/turn_aborted.jsonl"),
    );
    monitor.step();
    assert_eq!(notifications.0.lock().unwrap().len(), 1);
    monitor.set_notification_preferences(notifications::Preferences {
        waiting: false,
        done: false,
        failed: false,
    });
    files.write(
        "/rollout.jsonl",
        r#"{"type":"event_msg","payload":{"type":"error","message":"PRIVATE_RESPONSE"}}"#,
    );
    clock.advance(2000);
    monitor.step();
    assert_eq!(notifications.0.lock().unwrap().len(), 1);
}

#[test]
fn approval_wait_preserves_the_start_of_the_current_turn() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor =
        Monitor::new(process.clone(), clock.clone()).with_notifications(notifications.clone());
    let mut row = serde_json::json!({"pid":123,"id":"approval","cwd":"/tmp/project","startedAt":1,"status":"busy"});
    for (advance, status) in [
        (0, "busy"),
        (40_000, "waiting"),
        (2000, "busy"),
        (5000, "idle"),
    ] {
        row["status"] = status.into();
        *process.0.lock().unwrap() = Ok(serde_json::json!([row.clone()]).to_string());
        clock.advance(advance);
        monitor.step();
    }
    assert_eq!(
        notifications.0.lock().unwrap().last().unwrap().body,
        "Turno concluído em 47 s"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn startup_does_not_announce_a_codex_turn_that_already_finished() {
    let source = codex::tests::source();
    source.test_files().write(
        "/rollout.jsonl",
        include_str!("../../fixtures/agents/codex/task_complete.jsonl"),
    );
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let notifications = Notifications::default();
    let mut monitor = Monitor::new(process, FakeClock::at(1800000000000))
        .with_codex(source)
        .with_notifications(notifications.clone());
    monitor.step();
    assert!(notifications.0.lock().unwrap().is_empty());
}

#[test]
fn claude_switches_suppress_each_event_without_replaying_it_when_reenabled() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor =
        Monitor::new(process.clone(), clock.clone()).with_notifications(notifications.clone());
    monitor.set_notification_preferences(notifications::Preferences {
        waiting: false,
        done: false,
        failed: false,
    });
    let mut row = serde_json::json!({"pid":123,"id":"test","cwd":"/tmp/project","startedAt":1,"status":"busy"});
    for status in ["busy", "waiting", "busy", "idle", "failed"] {
        row["status"] = status.into();
        *process.0.lock().unwrap() = Ok(serde_json::json!([row.clone()]).to_string());
        clock.advance(40_000);
        monitor.step();
    }
    assert!(notifications.0.lock().unwrap().is_empty());
    monitor.set_notification_preferences(notifications::Preferences::default());
    clock.advance(2000);
    monitor.step();
    assert!(notifications.0.lock().unwrap().is_empty());
    row["status"] = "waiting".into();
    row["waitingFor"] = "Bash".into();
    *process.0.lock().unwrap() = Ok(serde_json::json!([row]).to_string());
    clock.advance(2000);
    monitor.step();
    assert_eq!(
        notifications.0.lock().unwrap()[0].body,
        "Aguardando aprovação: Bash"
    );
}

#[test]
fn hooks_publish_waiting_immediately_and_claude_polling_still_reconciles() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone());
    monitor.hooks_enabled = true;
    monitor.step().unwrap();
    let event = |provider, id: &str, state| hooks::Metadata {
        provider,
        id: id.into(),
        state,
        tool: Some("Bash".into()),
        cwd: "/tmp/project".into(),
        interrupted: false,
    };
    let data = monitor.hook(event(Provider::Codex, "abc", SessionState::Waiting));
    assert!(data.accent_dot);
    assert!(!data.codex_waiting_unavailable);
    assert_eq!(data.sessions[0].id, "codex:abc");
    assert!(monitor.step().is_none());
    let data = monitor.hook(event(Provider::Codex, "abc", SessionState::Closed));
    assert!(data.sessions.is_empty());
    assert_eq!(data.closed[0].last_state, Some(SessionState::Waiting));
    monitor.hook(event(
        Provider::Claude,
        "claude-test",
        SessionState::Working,
    ));
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.sessions.is_empty());
    assert_eq!(data.closed.len(), 2);
}

#[test]
fn codex_hook_without_polling_is_not_closed_by_an_unrelated_claude_read() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor =
        Monitor::new(process, clock.clone()).with_notifications(notifications.clone());
    monitor.hooks_enabled = true;
    monitor.discovery_partial = true;
    let baseline = monitor.step().unwrap().last_success_at;
    let event = |provider, id: &str, state| hooks::Metadata {
        provider,
        id: id.into(),
        state,
        tool: None,
        cwd: "/tmp/example".into(),
        interrupted: false,
    };
    monitor.hook(event(Provider::Codex, "synthetic-waiting", SessionState::Waiting));
    monitor.hook(event(Provider::Claude, "synthetic-working", SessionState::Working));
    for _ in 0..2 {
        clock.advance(2000);
        let data = monitor.step().unwrap();
        assert_eq!(data.sessions.len(), 1);
        assert_eq!(data.sessions[0].id, "codex:synthetic-waiting");
        assert_eq!(data.sessions[0].state, SessionState::Waiting);
        assert!(data.failed);
        assert_eq!(data.last_success_at, baseline);
        assert!(data.closed.iter().any(|s| s.id == "synthetic-working"));
        assert!(notifications.1.lock().unwrap().is_empty());
    }
    let data = monitor.hook(event(Provider::Codex, "synthetic-waiting", SessionState::Closed));
    assert!(data.sessions.is_empty());
    assert!(data.closed.iter().any(|s| s.id == "codex:synthetic-waiting"));
    assert_eq!(*notifications.1.lock().unwrap(), vec![1]);
}

#[cfg(target_os = "linux")]
#[test]
fn codex_waiting_survives_polling_until_a_hook_or_rollout_finishes_the_turn() {
    let source = codex::tests::source();
    let files = source.test_files();
    let clock = FakeClock::at(1800000000000);
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let mut monitor = Monitor::new(process, clock.clone()).with_codex(source);
    monitor.hooks_enabled = true;
    let data = monitor.step().unwrap();
    let id = data.sessions[0]
        .id
        .strip_prefix("codex:")
        .unwrap()
        .to_string();
    monitor.hook(hooks::Metadata {
        provider: Provider::Codex,
        id,
        state: SessionState::Waiting,
        tool: Some("Bash".into()),
        cwd: "/tmp/project".into(),
        interrupted: false,
    });
    clock.advance(2000);
    assert_eq!(
        monitor.step().unwrap().sessions[0].state,
        SessionState::Waiting
    );
    files.write(
        "/rollout.jsonl",
        include_str!("../../fixtures/agents/codex/task_complete.jsonl"),
    );
    clock.advance(2000);
    assert_eq!(
        monitor.step().unwrap().sessions[0].state,
        SessionState::Done
    );
}

#[test]
fn a_slow_poll_cannot_overwrite_a_newer_hook_event() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process, clock.clone());
    let old_poll = monitor.collect();
    clock.advance(100);
    monitor.hook(hooks::Metadata {
        provider: Provider::Claude,
        id: "abc".into(),
        state: SessionState::Waiting,
        tool: Some("Bash".into()),
        cwd: "/tmp/project".into(),
        interrupted: false,
    });
    let data = monitor.observe(old_poll);
    assert_eq!(data.sessions.len(), 1);
    assert_eq!(data.sessions[0].state, SessionState::Waiting);
}

#[test]
fn a_failed_read_keeps_usable_sessions_with_the_time_of_the_last_good_read() {
    let process = Process(Arc::new(Mutex::new(Ok(include_str!(
        "../../fixtures/agents/recorded.json"
    )
    .into()))));
    let clock = FakeClock::at(1800000000000);
    let mut monitor = Monitor::new(process.clone(), clock.clone());
    let data = monitor.step().unwrap();
    assert_eq!(data.last_success_at, Some(1800000000000));
    *process.0.lock().unwrap() = Err(());
    clock.advance(2000);
    let data = monitor.step().unwrap();
    assert!(data.failed);
    assert_eq!(data.sessions[0].project, "example-project");
    assert_eq!(data.last_success_at, Some(1800000000000));
    let json = serde_json::to_value(&data).unwrap();
    assert_eq!(json["lastSuccessAt"], 1800000000000u64);
}

#[test]
fn hook_content_never_reaches_notifications_or_the_published_snapshot() {
    #[derive(Default)]
    struct Socket(Vec<Vec<u8>>);
    impl crate::ports::HookSocket for Socket {
        fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
            self.0.push(bytes.to_vec());
            Ok(())
        }
        fn receive(&mut self) -> Option<Vec<u8>> {
            self.0.pop()
        }
    }
    let mut socket = Socket::default();
    hooks::forward(
        "codex",
        "PermissionRequest",
        include_str!("../../fixtures/agents/hooks/codex-permission.json"),
        &mut socket,
    );
    let event: hooks::Metadata =
        serde_json::from_slice(&crate::ports::HookSocket::receive(&mut socket).unwrap()).unwrap();
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let notifications = Notifications::default();
    let mut monitor = Monitor::new(process, FakeClock::at(1800000000000))
        .with_notifications(notifications.clone());
    monitor.hooks_enabled = true;
    monitor.step().unwrap();
    let data = monitor.hook(event);
    assert_eq!(data.sessions[0].state, SessionState::Waiting);
    // A hook event is not a read of the session source.
    assert_eq!(data.last_success_at, Some(1800000000000));
    let calls = notifications.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].session, "codex:hook-codex");
    let published = serde_json::to_string(&data).unwrap();
    assert!(!format!("{calls:?}{published}").contains("PRIVATE_"));
}

#[test]
fn notifications_never_include_project_or_session_names() {
    let process = Process(Arc::new(Mutex::new(Ok("[]".into()))));
    let clock = FakeClock::at(1800000000000);
    let notifications = Notifications::default();
    let mut monitor =
        Monitor::new(process.clone(), clock.clone()).with_notifications(notifications.clone());
    monitor.step();
    *process.0.lock().unwrap() = Ok(serde_json::json!([{
        "pid":123,"sessionId":"synthetic-session","cwd":"/PRIVATE_PROJECT",
        "name":"PRIVATE_SESSION_NAME","startedAt":1000,"status":"waiting"
    }])
    .to_string());
    clock.advance(2000);
    monitor.step();
    let calls = notifications.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(!calls[0].title.contains("PRIVATE_") && !calls[0].body.contains("PRIVATE_"));
}
