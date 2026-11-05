use super::super::{combine, SharedCache};
use super::*;
use crate::persistence::DiskStorage;
use crate::ports::fakes::{fixtures, FakeClock};
use crate::snapshot::Snapshot;
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Shortly after the documented status line payload: both windows still valid.
const NOW: u64 = 1738420000000;
const FIVE_HOUR_RESET: u64 = 1738425600000;

#[derive(Clone)]
struct Installed(Arc<Mutex<bool>>);
impl Programs for Installed {
    fn installed(&self, name: &str) -> bool {
        name == "claude" && *self.0.lock().unwrap()
    }
}

#[derive(Clone, Default)]
struct Changes(Arc<Mutex<bool>>);
impl Changes {
    fn touch(&self) {
        *self.0.lock().unwrap() = true;
    }
}
impl Watch for Changes {
    fn changed(&mut self) -> bool {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// A home with Claude Code installed, logged in, and another app's status line.
struct Home {
    dir: tempfile::TempDir,
    installed: Installed,
    changes: Changes,
    clock: FakeClock,
}
impl Home {
    fn new() -> Self {
        let home = Home {
            dir: tempfile::tempdir().unwrap(),
            installed: Installed(Arc::new(Mutex::new(true))),
            changes: Changes::default(),
            clock: FakeClock::at(NOW),
        };
        home.copy("claude/settings.json", ".claude/settings.json");
        home.copy("claude/credentials.json", ".claude/.credentials.json");
        home
    }
    fn copy(&self, fixture: &str, path: &str) {
        self.write(
            path,
            &std::fs::read_to_string(fixtures().join(fixture)).unwrap(),
        );
    }
    fn path(&self, path: &str) -> std::path::PathBuf {
        self.dir.path().join(path)
    }
    fn write(&self, path: &str, content: &str) {
        std::fs::create_dir_all(self.path(path).parent().unwrap()).unwrap();
        std::fs::write(self.path(path), content).unwrap();
    }
    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.path(path)).unwrap()
    }
    fn settings(&self) -> Value {
        serde_json::from_str(&self.read(".claude/settings.json")).unwrap()
    }
    fn locations(&self) -> Locations {
        Locations {
            settings: ".claude/settings.json".into(),
            credentials: ".claude/.credentials.json".into(),
            config: ".config/kvojps-monitor/claude-statusline.json".into(),
            wrapper: ".local/share/kvojps-monitor/claude-statusline".into(),
            cache: ".cache/kvojps-monitor/claude-statusline.json".into(),
            executable: "/usr/bin/kvojps-monitor".into(),
            usage_choice: ".config/kvojps-monitor/claude-usage.json".into(),
        }
    }
    fn claude(&self) -> Claude<DiskStorage, Installed, Changes, FakeClock> {
        Claude::new(
            DiskStorage(self.dir.path().into()),
            self.installed.clone(),
            self.changes.clone(),
            self.clock.clone(),
            self.locations(),
        )
    }
    /// What the wrapper's copy step writes after Claude Code runs it.
    fn status_line_runs(&self, payload: &str) {
        let cache = extract(payload, self.clock.now()).unwrap();
        self.write(".cache/kvojps-monitor/claude-statusline.json", &cache);
        self.changes.touch();
    }
}

fn payload() -> String {
    std::fs::read_to_string(fixtures().join("claude/statusline-payload.json")).unwrap()
}

const WRAPPER_COMMAND: &str = "sh '.local/share/kvojps-monitor/claude-statusline'";

#[cfg(windows)]
#[test]
fn windows_detects_cli_and_status_line_without_reading_credentials_or_opt_in() {
    struct NoCredentials(DiskStorage);
    impl Storage for NoCredentials {
        fn read(&self, path: &str) -> std::io::Result<String> {
            assert!(
                !path.ends_with(".credentials.json"),
                "Windows must not read OAuth credentials"
            );
            assert!(
                !path.ends_with("claude-usage.json"),
                "Windows must not restore an unsupported opt-in"
            );
            self.0.read(path)
        }
        fn write(&self, path: &str, text: &str) -> std::io::Result<()> {
            self.0.write(path, text)
        }
        fn rename(&self, from: &str, to: &str) -> std::io::Result<()> {
            self.0.rename(from, to)
        }
        fn remove(&self, path: &str) -> std::io::Result<()> {
            self.0.remove(path)
        }
    }
    let home = Home::new();
    let before = home.read(".claude/settings.json");
    let mut claude = Claude::new(
        NoCredentials(DiskStorage(home.dir.path().into())),
        home.installed.clone(),
        home.changes.clone(),
        home.clock.clone(),
        home.locations(),
    )
    .for_windows(None);
    claude.step(false);
    assert_eq!(claude.data()["state"], "empty");
    assert_eq!(claude.data()["usage"], "unsupported");
    assert_eq!(claude.set_usage(true), Err(ClaudeError::Unsupported));
    assert_eq!(home.read(".claude/settings.json"), before);
    claude.connect().unwrap();
    home.status_line_runs(&payload());
    claude.step(false);
    assert_eq!(claude.data()["state"], "ready");
    assert_eq!(claude.data()["reading"]["source"], "statusLine");
    claude.disconnect().unwrap();
    assert_eq!(home.read(".claude/settings.json"), before);
}

#[test]
fn settings_never_change_without_the_click() {
    let home = Home::new();
    let before = home.read(".claude/settings.json");
    let mut claude = home.claude();
    claude.step(false);
    home.clock.advance(600_000);
    claude.step(true);
    assert_eq!(home.read(".claude/settings.json"), before);
    assert_eq!(claude.data()["statusLine"], "off");
}

#[test]
fn connecting_chains_the_existing_status_line_and_disconnecting_restores_it_exactly() {
    let home = Home::new();
    let before = home.read(".claude/settings.json");
    let original = home.settings();
    let mut claude = home.claude();
    claude.connect().unwrap();

    let settings = home.settings();
    assert_eq!(
        settings["statusLine"],
        json!({"type": "command", "command": WRAPPER_COMMAND, "padding": 1})
    );
    // Everything else, including another app's hooks, stays as it was.
    for key in ["hooks", "enabledPlugins", "theme", "autoMode"] {
        assert_eq!(settings[key], original[key], "{key}");
    }
    let wrapper = home.read(".local/share/kvojps-monitor/claude-statusline");
    assert!(
        wrapper.contains(r#"sh -c '/bin/sh "${HOME-}/.orca/agent-hooks/claude-statusline.sh"'"#)
    );
    claude.step(false);
    assert_eq!(claude.data()["statusLine"], "connected");

    claude.disconnect().unwrap();
    assert_eq!(home.read(".claude/settings.json"), before);
    assert!(!home
        .path(".local/share/kvojps-monitor/claude-statusline")
        .exists());
    assert!(!home
        .path(".config/kvojps-monitor/claude-statusline.json")
        .exists());
    claude.step(false);
    assert_eq!(claude.data()["statusLine"], "off");
}

#[test]
fn without_a_previous_status_line_disconnecting_removes_the_key() {
    let home = Home::new();
    let mut settings = home.settings();
    settings.as_object_mut().unwrap().remove("statusLine");
    home.write(
        ".claude/settings.json",
        &(serde_json::to_string_pretty(&settings).unwrap() + "\n"),
    );
    let before = home.read(".claude/settings.json");
    let mut claude = home.claude();
    claude.connect().unwrap();
    assert_eq!(
        home.settings()["statusLine"],
        json!({"type": "command", "command": WRAPPER_COMMAND})
    );
    assert!(home
        .read(".local/share/kvojps-monitor/claude-statusline")
        .ends_with("exit 0\n"));
    claude.disconnect().unwrap();
    assert_eq!(home.read(".claude/settings.json"), before);
}

#[test]
fn connecting_twice_keeps_the_original_status_line() {
    let home = Home::new();
    let before = home.read(".claude/settings.json");
    let mut claude = home.claude();
    claude.connect().unwrap();
    claude.connect().unwrap();
    claude.disconnect().unwrap();
    assert_eq!(home.read(".claude/settings.json"), before);
}

#[test]
fn unreadable_settings_are_never_overwritten() {
    let home = Home::new();
    home.write(".claude/settings.json", "{ invalid");
    let mut claude = home.claude();
    assert_eq!(claude.connect(), Err(ClaudeError::InvalidSettings));
    assert_eq!(home.read(".claude/settings.json"), "{ invalid");
    assert!(!home
        .path(".config/kvojps-monitor/claude-statusline.json")
        .exists());
}

#[test]
fn a_missing_settings_file_gets_only_the_status_line() {
    let home = Home::new();
    std::fs::remove_file(home.path(".claude/settings.json")).unwrap();
    let mut claude = home.claude();
    claude.connect().unwrap();
    assert_eq!(
        home.settings(),
        json!({"statusLine": {"type": "command", "command": WRAPPER_COMMAND}})
    );
}

#[test]
fn a_status_line_replaced_by_another_tool_warns_and_is_never_rewritten() {
    let home = Home::new();
    let mut claude = home.claude();
    claude.connect().unwrap();
    let mut settings = home.settings();
    settings["statusLine"] = json!({"type": "command", "command": "other-tool statusline"});
    let replaced = serde_json::to_string_pretty(&settings).unwrap() + "\n";
    home.write(".claude/settings.json", &replaced);

    home.clock.advance(CHECK_INTERVAL);
    assert!(claude.step(false));
    assert_eq!(claude.data()["statusLine"], "replaced");
    home.clock.advance(600_000);
    claude.step(true);
    assert_eq!(home.read(".claude/settings.json"), replaced);

    // Reconnecting wraps the new status line; disconnecting restores it.
    claude.connect().unwrap();
    assert!(home
        .read(".local/share/kvojps-monitor/claude-statusline")
        .contains("sh -c 'other-tool statusline'"));
    claude.disconnect().unwrap();
    assert_eq!(home.read(".claude/settings.json"), replaced);
}

#[test]
fn disconnecting_after_a_replacement_leaves_the_other_tool_in_place() {
    let home = Home::new();
    let mut claude = home.claude();
    claude.connect().unwrap();
    let mut settings = home.settings();
    settings["statusLine"]["command"] = json!("other-tool statusline");
    let replaced = serde_json::to_string_pretty(&settings).unwrap() + "\n";
    home.write(".claude/settings.json", &replaced);
    claude.disconnect().unwrap();
    assert_eq!(home.read(".claude/settings.json"), replaced);
    claude.step(true);
    assert_eq!(claude.data()["statusLine"], "off");
}

#[test]
fn the_copy_keeps_only_the_five_hour_and_weekly_windows() {
    let cache: Value = serde_json::from_str(&extract(&payload(), NOW).unwrap()).unwrap();
    assert_eq!(
        cache,
        json!({
            "read_at": NOW,
            "rate_limits": {
                "five_hour": {"used_percentage": 23.5, "resets_at": 1738425600},
                "seven_day": {"used_percentage": 41.2, "resets_at": 1738857600},
            },
        })
    );
    // Before the first answer of a session, or without Pro/Max, nothing is copied.
    let mut without: Value = serde_json::from_str(&payload()).unwrap();
    without.as_object_mut().unwrap().remove("rate_limits");
    assert_eq!(extract(&without.to_string(), NOW), None);
    assert_eq!(extract("not json", NOW), None);
}

/// The generated wrapper, run by a real shell as Claude Code would.
#[cfg(target_os = "linux")]
#[test]
fn the_wrapper_passes_stdin_to_the_original_command_and_returns_its_output() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let home = Home::new();
    // Stands in for the app binary: records its arguments and stdin.
    let copier = home.path("copier");
    std::fs::write(
        &copier,
        "#!/bin/sh\nprintf '%s' \"$1 $2\" > \"$0.args\"\ncat > \"$0.stdin\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&copier, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut settings = home.settings();
    settings["statusLine"]["command"] = json!("read line; printf 'original: %s' \"$line\"; exit 3");
    home.write(
        ".claude/settings.json",
        &serde_json::to_string_pretty(&settings).unwrap(),
    );
    let mut locations = home.locations();
    locations.executable = copier.to_string_lossy().into_owned();
    let mut claude = Claude::new(
        DiskStorage(home.dir.path().into()),
        home.installed.clone(),
        home.changes.clone(),
        home.clock.clone(),
        locations,
    );
    claude.connect().unwrap();

    let mut child = std::process::Command::new("sh")
        .arg(home.path(".local/share/kvojps-monitor/claude-statusline"))
        .current_dir(home.dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"rate_limits\": {}}\n\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "original: {\"rate_limits\": {}}"
    );
    assert_eq!(output.status.code(), Some(3));
    // The copy runs in the background so the status line never waits for it.
    let stdin = home.path("copier.stdin");
    for _ in 0..100 {
        if std::fs::read_to_string(&stdin).is_ok_and(|s| !s.is_empty()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        std::fs::read_to_string(stdin).unwrap(),
        "{\"rate_limits\": {}}\n\n"
    );
    assert_eq!(
        home.read("copier.args"),
        "statusline .cache/kvojps-monitor/claude-statusline.json"
    );
}

#[test]
fn a_status_line_update_publishes_both_windows_with_the_plan() {
    let home = Home::new();
    let mut claude = home.claude();
    claude.connect().unwrap();
    claude.step(false);
    assert!(!claude.step(false));
    home.clock.advance(5_000);
    home.status_line_runs(&payload());
    assert!(claude.step(false));
    assert_eq!(
        claude.data(),
        json!({
            "installed": true,
            "state": "ready",
            "plan": "pro",
            "statusLine": "connected",
            "usage": "off",
            "reading": {
                "source": "statusLine",
                "readAt": NOW + 5_000,
                "buckets": [{
                    "id": "claude",
                    "name": null,
                    "primary": {"usedPercent": 23.5, "durationMins": 300, "resetsAt": FIVE_HOUR_RESET, "reset": false},
                    "secondary": {"usedPercent": 41.2, "durationMins": 10080, "resetsAt": 1738857600000u64, "reset": false},
                }],
            },
        })
    );
    assert_eq!(claude.updated_at(), Some(NOW + 5_000));
}

#[test]
fn without_a_reading_the_module_asks_for_a_claude_code_session() {
    let home = Home::new();
    let mut claude = home.claude();
    claude.connect().unwrap();
    claude.step(false);
    assert_eq!(
        claude.data(),
        json!({"installed": true, "state": "empty", "plan": "pro", "statusLine": "connected", "usage": "off", "reading": null})
    );
}

#[test]
fn a_restarted_window_never_assumes_zero_percent() {
    let home = Home::new();
    let mut claude = home.claude();
    home.status_line_runs(&payload());
    claude.step(false);
    home.clock.advance(FIVE_HOUR_RESET - NOW + 60_000);
    let primary = &claude.data()["reading"]["buckets"][0]["primary"];
    assert_eq!(primary["reset"], true);
    assert_eq!(primary["usedPercent"], Value::Null);
    assert_eq!(primary["resetsAt"], FIVE_HOUR_RESET);
}

#[test]
fn a_restart_is_published_without_a_new_reading() {
    let home = Home::new();
    let mut claude = home.claude();
    home.status_line_runs(&payload());
    claude.step(false);
    home.clock.advance(FIVE_HOUR_RESET - NOW - 1_000);
    assert!(!claude.step(false));
    home.clock.advance(2_000);
    assert!(claude.step(false));
    assert!(!claude.step(false));
}

#[test]
fn a_window_dropped_after_its_reset_stays_shown_as_restarted() {
    let home = Home::new();
    let mut claude = home.claude();
    home.status_line_runs(&payload());
    claude.step(false);
    // Claude Code drops a window once its reset passes.
    home.clock.advance(FIVE_HOUR_RESET - NOW + 60_000);
    let mut later: Value = serde_json::from_str(&payload()).unwrap();
    later["rate_limits"]
        .as_object_mut()
        .unwrap()
        .remove("five_hour");
    later["rate_limits"]["seven_day"]["used_percentage"] = json!(45.0);
    home.status_line_runs(&later.to_string());
    claude.step(false);
    let bucket = &claude.data()["reading"]["buckets"][0];
    assert_eq!(bucket["primary"]["reset"], true);
    assert_eq!(bucket["primary"]["usedPercent"], Value::Null);
    assert_eq!(bucket["secondary"]["usedPercent"], 45.0);
}

#[test]
fn an_older_copy_never_replaces_a_newer_reading() {
    let home = Home::new();
    let mut claude = home.claude();
    home.status_line_runs(&payload());
    claude.step(false);
    let older = extract(&payload().replace("23.5", "10.0"), NOW - 60_000).unwrap();
    home.write(".cache/kvojps-monitor/claude-statusline.json", &older);
    home.changes.touch();
    assert!(!claude.step(false));
    assert_eq!(
        claude.data()["reading"]["buckets"][0]["primary"]["usedPercent"],
        23.5
    );
}

#[test]
fn without_the_cli_or_its_credentials_the_module_explains_it_is_missing() {
    let home = Home::new();
    *home.installed.0.lock().unwrap() = false;
    let mut claude = home.claude();
    claude.step(false);
    assert_eq!(claude.data()["state"], "missing");

    let home = Home::new();
    std::fs::remove_file(home.path(".claude/.credentials.json")).unwrap();
    let mut claude = home.claude();
    claude.step(false);
    assert_eq!(claude.data()["state"], "missing");
    assert_eq!(claude.data()["plan"], Value::Null);
}

#[test]
fn the_plan_comes_from_the_subscription_type() {
    let home = Home::new();
    let credentials = home
        .read(".claude/.credentials.json")
        .replace("\"pro\"", "\"max\"");
    home.write(".claude/.credentials.json", &credentials);
    let mut claude = home.claude();
    claude.step(false);
    assert_eq!(claude.data()["plan"], "max");
}

fn disk(dir: &std::path::Path) -> crate::persistence::LocalPersistence<DiskStorage, DiskStorage> {
    use crate::persistence::{Cache, LocalPersistence, Logs};
    LocalPersistence {
        cache: Cache(DiskStorage(dir.join("cache"))),
        logs: Logs(DiskStorage(dir.join("state"))),
    }
}

#[test]
fn restart_shows_the_cached_reading_and_never_the_token() {
    let home = Home::new();
    let state = tempfile::tempdir().unwrap();
    let mut claude = home.claude().with_persistence(disk(state.path()));
    home.status_line_runs(&payload());
    claude.step(false);
    let cached = std::fs::read_to_string(state.path().join("cache/ai.json")).unwrap();
    for forbidden in ["sk-ant", "Token", "token", "/current/working", "my-session"] {
        assert!(!cached.contains(forbidden), "{forbidden}");
    }
    let data = claude.data();
    for forbidden in ["sk-ant", "Token"] {
        assert!(!data.to_string().contains(forbidden), "{forbidden}");
    }
    let log = std::fs::read_to_string(state.path().join("state/info.log")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(log.trim()).unwrap(),
        json!({"source": "ai", "event": "updated", "httpStatus": null, "durationMs": 0})
    );

    home.clock.advance(3_600_000);
    let restored = home.claude().with_persistence(disk(state.path()));
    assert_eq!(restored.data()["reading"], data["reading"]);
    assert_eq!(restored.data()["plan"], "pro");
}

#[test]
fn codex_and_claude_share_one_cache_entry() {
    let state = tempfile::tempdir().unwrap();
    let shared = SharedCache::new(disk(state.path()));
    let home = Home::new();
    let mut claude = home.claude().with_persistence(shared.clone());
    home.status_line_runs(&payload());
    claude.step(false);
    shared.save(&Snapshot {
        module: Module::Ai,
        updated_at: Some(NOW - 1),
        data: json!({"refreshing": false, "codex": {"plan": "plus", "reading": null}}),
    });
    let cached = crate::persistence::Persistence::load(&disk(state.path()), Module::Ai).unwrap();
    assert_eq!(cached.data["codex"]["plan"], "plus");
    assert_eq!(cached.data["claude"]["plan"], "pro");
    assert_eq!(cached.updated_at, Some(NOW));
}

/// UI fixtures in src/lib/ai/fixtures/ are snapshots published by the core.
#[test]
fn ui_fixtures_match_published_snapshots() {
    let codex = || {
        let full: Value =
            serde_json::from_str(include_str!("../../../src/lib/ai/fixtures/full.json")).unwrap();
        Snapshot {
            module: Module::Ai,
            updated_at: Some(1793460871000),
            data: full,
        }
    };
    let connected = {
        let home = Home::new();
        let mut claude = home.claude();
        claude.connect().unwrap();
        home.status_line_runs(&payload());
        claude.step(false);
        combine(codex(), &claude)
    };
    let replaced = {
        let home = Home::new();
        let mut claude = home.claude();
        claude.connect().unwrap();
        home.status_line_runs(&payload());
        let mut settings = home.settings();
        settings["statusLine"]["command"] = json!("other-tool statusline");
        home.write(".claude/settings.json", &settings.to_string());
        claude.step(false);
        combine(codex(), &claude)
    };
    let empty = {
        let home = Home::new();
        let mut claude = home.claude();
        claude.connect().unwrap();
        claude.step(false);
        combine(codex(), &claude)
    };
    let off = {
        let home = Home::new();
        let mut claude = home.claude();
        claude.step(false);
        combine(codex(), &claude)
    };
    let missing = {
        let home = Home::new();
        *home.installed.0.lock().unwrap() = false;
        let mut claude = home.claude();
        claude.step(false);
        combine(codex(), &claude)
    };
    let usage = {
        let home = usage::home();
        let api = usage::Api::default();
        let mut claude = usage::collector(&home, &api);
        claude.set_usage(true).unwrap();
        let mut sonnet = usage::response();
        sonnet["seven_day_sonnet"] =
            json!({"utilization": 33.0, "resets_at": "2026-11-05T14:00:00+00:00"});
        api.answer(Ok(sonnet));
        claude.step(false);
        combine(codex(), &claude)
    };
    let expired = {
        let home = usage::home();
        let api = usage::Api::default();
        let mut claude = usage::collector(&home, &api);
        claude.connect().unwrap();
        home.status_line_runs(&usage::status_payload());
        claude.set_usage(true).unwrap();
        api.answer(Err(crate::ports::HttpError::response(401, None)));
        claude.step(false);
        combine(codex(), &claude)
    };
    let failure = {
        let home = usage::home();
        let api = usage::Api::default();
        let mut claude = usage::collector(&home, &api);
        claude.set_usage(true).unwrap();
        claude.step(false);
        for delay in [300_000, 30_000, 60_000] {
            api.answer(Err(crate::ports::HttpError::response(503, None)));
            home.clock.advance(delay);
            claude.step(false);
        }
        combine(codex(), &claude)
    };
    for (name, snapshot) in [
        ("claude-usage", usage),
        ("claude-failure", failure),
        ("claude-expired", expired),
        ("claude-missing", missing),
        ("claude-connected", connected),
        ("claude-replaced", replaced),
        ("claude-empty", empty),
        ("claude-off", off),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../src/lib/ai/fixtures/{name}.json"));
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

/// Opt-in `oauth/usage`, with the recorded response and a clock when it was taken.
mod usage {
    use super::*;
    use crate::ports::HttpError;

    /// Shortly before the recorded response: both windows still valid.
    const NOW: u64 = 1793401200000;
    const FIVE_HOUR_RESET: u64 = 1793413800000;
    const WEEKLY_RESET: u64 = 1793887200000;

    pub(super) fn response() -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(fixtures().join("claude/oauth-usage.json")).unwrap(),
        )
        .unwrap()
    }

    /// The status line payload at the same time, with other percentages.
    pub(super) fn status_payload() -> String {
        let mut status: Value = serde_json::from_str(&payload()).unwrap();
        status["rate_limits"] = json!({
            "five_hour": {"used_percentage": 20.0, "resets_at": FIVE_HOUR_RESET / 1000},
            "seven_day": {"used_percentage": 8.0, "resets_at": WEEKLY_RESET / 1000},
        });
        status.to_string()
    }

    /// Answers queued in order; every token received is recorded.
    #[derive(Clone, Default)]
    pub(super) struct Api {
        answers: Arc<Mutex<Vec<Result<Value, HttpError>>>>,
        tokens: Arc<Mutex<Vec<String>>>,
    }
    impl Api {
        pub(super) fn answer(&self, answer: Result<Value, HttpError>) {
            self.answers.lock().unwrap().push(answer);
        }
        fn calls(&self) -> usize {
            self.tokens.lock().unwrap().len()
        }
    }
    impl UsageApi for Api {
        fn usage(&self, token: &str) -> Result<Value, HttpError> {
            self.tokens.lock().unwrap().push(token.to_owned());
            let mut answers = self.answers.lock().unwrap();
            if answers.is_empty() {
                Ok(response())
            } else {
                answers.remove(0)
            }
        }
    }

    /// A home whose Claude Code token is still valid at NOW.
    pub(super) fn home() -> Home {
        let home = Home::new();
        home.clock.advance(NOW - super::NOW);
        home.set_token("sk-ant-oat01-exemplo", NOW + 3_600_000);
        home
    }
    impl Home {
        fn set_token(&self, token: &str, expires_at: u64) {
            let mut credentials: Value =
                serde_json::from_str(&self.read(".claude/.credentials.json")).unwrap();
            credentials["claudeAiOauth"]["accessToken"] = json!(token);
            credentials["claudeAiOauth"]["expiresAt"] = json!(expires_at);
            self.write(".claude/.credentials.json", &credentials.to_string());
        }
    }

    pub(super) fn collector(
        home: &Home,
        api: &Api,
    ) -> Claude<DiskStorage, Installed, Changes, FakeClock> {
        home.claude().with_usage(api.clone())
    }

    #[test]
    fn opting_in_reads_both_windows_with_the_access_token() {
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.step(false);
        assert_eq!(api.calls(), 0, "off by default");
        claude.set_usage(true).unwrap();
        assert!(claude.step(false));
        assert_eq!(*api.tokens.lock().unwrap(), ["sk-ant-oat01-exemplo"]);
        assert_eq!(
            claude.data(),
            json!({
                "installed": true,
                "state": "ready",
                "plan": "pro",
                "statusLine": "off",
                "usage": "on",
                "operation": {"id":1,"origin":"automatic","state":"succeeded"},
                "reading": {
                    "source": "oauthUsage",
                    "readAt": NOW,
                    "buckets": [{
                        "id": "claude",
                        "name": null,
                        "primary": {"usedPercent": 15.0, "durationMins": 300, "resetsAt": FIVE_HOUR_RESET, "reset": false},
                        "secondary": {"usedPercent": 6.0, "durationMins": 10080, "resetsAt": WEEKLY_RESET, "reset": false},
                    }],
                },
            })
        );
    }

    fn refused() -> Result<Value, HttpError> {
        Err(HttpError::response(401, None))
    }

    #[test]
    fn the_token_is_only_read_never_renewed_written_or_kept() {
        let home = home();
        let before = home.read(".claude/.credentials.json");
        let state = tempfile::tempdir().unwrap();
        let api = Api::default();
        let mut claude = collector(&home, &api).with_persistence(disk(state.path()));
        claude.set_usage(true).unwrap();
        claude.step(false);
        home.clock.advance(300_000);
        api.answer(refused());
        claude.step(true);
        home.clock.advance(300_000);
        claude.step(true);
        // Only the access token, only to oauth/usage, and the file is untouched.
        assert_eq!(*api.tokens.lock().unwrap(), ["sk-ant-oat01-exemplo"; 2]);
        assert_eq!(home.read(".claude/.credentials.json"), before);
        let cached = std::fs::read_to_string(state.path().join("cache/ai.json")).unwrap();
        let log = std::fs::read_to_string(state.path().join("state/info.log")).unwrap();
        for text in [cached, log, claude.data().to_string()] {
            for forbidden in ["sk-ant", "oat01", "ort01", "Token", "token"] {
                assert!(!text.contains(forbidden), "{forbidden} in {text}");
            }
        }
    }

    #[test]
    fn a_refused_token_falls_back_to_the_status_line_until_claude_code_renews_it() {
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        api.answer(refused());
        home.status_line_runs(&payload());
        assert!(claude.step(false));
        assert_eq!(claude.data()["usage"], "expired");
        assert_eq!(claude.data()["reading"]["source"], "statusLine");
        // Neither the timer nor ↻ sends the refused token again.
        home.clock.advance(3_600_000);
        claude.step(false);
        claude.step(true);
        assert_eq!(api.calls(), 1);
        // Claude Code renewed it: read again at the next check.
        home.set_token("sk-ant-oat01-renovado", NOW + 7_200_000);
        home.clock.advance(CHECK_INTERVAL);
        assert!(claude.step(false));
        assert_eq!(api.calls(), 2);
        assert_eq!(api.tokens.lock().unwrap()[1], "sk-ant-oat01-renovado");
        assert_eq!(claude.data()["usage"], "on");
        assert_eq!(claude.data()["reading"]["source"], "oauthUsage");
    }

    #[test]
    fn an_expired_token_is_never_sent() {
        let home = home();
        home.set_token("sk-ant-oat01-exemplo", NOW - 1);
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        assert!(claude.step(false));
        assert_eq!(api.calls(), 0);
        assert_eq!(claude.data()["usage"], "expired");
    }

    #[test]
    fn weekly_limits_per_model_appear_only_when_not_null() {
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        claude.step(false);
        assert_eq!(
            claude.data()["reading"]["buckets"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let mut max = response();
        max["seven_day_sonnet"] =
            json!({"utilization": 33.0, "resets_at": "2026-11-05T14:00:00+00:00", "tangelo": 1});
        max["cedar_ember"] = json!({"utilization": 99.0});
        api.answer(Ok(max));
        home.clock.advance(300_000);
        claude.step(false);
        assert_eq!(
            claude.data()["reading"]["buckets"][1],
            json!({
                "id": "claude-sonnet",
                "name": "Sonnet",
                "primary": null,
                "secondary": {"usedPercent": 33.0, "durationMins": 10080, "resetsAt": WEEKLY_RESET, "reset": false},
            })
        );
        assert_eq!(
            claude.data()["reading"]["buckets"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn with_both_sources_each_window_shows_its_newest_reading() {
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        let mut sonnet = response();
        sonnet["seven_day_sonnet"] =
            json!({"utilization": 33.0, "resets_at": "2026-11-05T14:00:00+00:00"});
        api.answer(Ok(sonnet));
        claude.step(false);
        // A newer status line reading replaces both main windows and keeps the
        // weekly per model, which only oauth/usage brings.
        home.clock.advance(60_000);
        home.status_line_runs(&status_payload());
        assert!(claude.step(false));
        let reading = &claude.data()["reading"];
        assert_eq!(reading["source"], "statusLine");
        assert_eq!(reading["readAt"], NOW + 60_000);
        assert_eq!(reading["buckets"][0]["primary"]["usedPercent"], 20.0);
        assert_eq!(reading["buckets"][0]["secondary"]["usedPercent"], 8.0);
        assert_eq!(reading["buckets"][1]["secondary"]["usedPercent"], 33.0);
        // The next oauth/usage reading is newer again.
        home.clock.advance(240_000);
        assert!(claude.step(false));
        let reading = &claude.data()["reading"];
        assert_eq!(reading["source"], "oauthUsage");
        assert_eq!(reading["buckets"][0]["primary"]["usedPercent"], 15.0);
        assert_eq!(claude.updated_at(), Some(NOW + 300_000));
    }

    #[test]
    fn reads_every_five_minutes_and_backs_off_on_transient_failures() {
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        claude.step(false);
        home.clock.advance(299_000);
        claude.step(false);
        assert_eq!(api.calls(), 1);
        home.clock.advance(1_000);
        claude.step(false);
        assert_eq!(api.calls(), 2);
        // 5xx, timeout, then 429 asking for more than the backoff.
        api.answer(Err(HttpError::response(503, None)));
        api.answer(Err(HttpError::default()));
        api.answer(Err(HttpError::response(429, Some("600"))));
        home.clock.advance(300_000);
        claude.step(false);
        home.clock.advance(29_000);
        claude.step(true);
        assert_eq!(api.calls(), 3, "manual refresh waits for the backoff too");
        home.clock.advance(1_000);
        claude.step(false);
        home.clock.advance(60_000);
        claude.step(false);
        assert_eq!(api.calls(), 5);
        assert_eq!(claude.data()["failureSince"], NOW + 600_000);
        assert_eq!(
            claude.data()["reading"]["source"],
            "oauthUsage",
            "last reading kept"
        );
        home.clock.advance(599_000);
        claude.step(false);
        assert_eq!(api.calls(), 5);
        home.clock.advance(1_000);
        claude.step(false);
        assert_eq!(api.calls(), 6);
        assert!(claude.data().get("failureSince").is_none());
    }

    #[test]
    fn offline_and_before_its_turn_nothing_is_read() {
        use crate::lifecycle::Status;
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        claude.lifecycle(Status {
            online: false,
            generation: 0,
            ready_at: 0,
        });
        claude.step(true);
        assert_eq!(api.calls(), 0);
        // After GitHub, Trello and Codex: 9 s after reconnecting.
        claude.lifecycle(Status {
            online: true,
            generation: 1,
            ready_at: NOW,
        });
        home.clock.advance(8_999);
        claude.step(false);
        assert_eq!(api.calls(), 0);
        home.clock.advance(1);
        claude.step(false);
        assert_eq!(api.calls(), 1);
        // Resume: the new generation reads once more without waiting five minutes.
        claude.lifecycle(Status {
            online: true,
            generation: 2,
            ready_at: NOW + 60_000,
        });
        home.clock.advance(60_000);
        claude.step(false);
        assert_eq!(api.calls(), 2);
    }

    #[test]
    fn the_opt_in_survives_a_restart_and_turning_it_off_goes_back_to_the_status_line() {
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        home.status_line_runs(&payload());
        claude.set_usage(true).unwrap();
        claude.step(false);
        assert_eq!(claude.data()["reading"]["source"], "oauthUsage");

        let mut restarted = collector(&home, &api);
        restarted.step(false);
        assert_eq!(restarted.data()["usage"], "on");
        assert_eq!(api.calls(), 2);

        restarted.set_usage(false).unwrap();
        assert_eq!(restarted.data()["usage"], "off");
        assert_eq!(restarted.data()["reading"]["source"], "statusLine");
        home.clock.advance(3_600_000);
        restarted.step(true);
        assert_eq!(api.calls(), 2);
    }

    #[test]
    fn a_refused_token_stays_blocked_after_a_restart() {
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        api.answer(refused());
        claude.step(false);
        let mut restarted = collector(&home, &api);
        restarted.step(true);
        assert_eq!(api.calls(), 1);
        assert_eq!(restarted.data()["usage"], "expired");
        // Claude Code renewed it while the app was closed.
        home.set_token("sk-ant-oat01-renovado", NOW + 7_200_000);
        let mut renewed = collector(&home, &api);
        renewed.step(false);
        assert_eq!(api.calls(), 2);
        assert_eq!(renewed.data()["usage"], "on");
    }

    #[test]
    fn a_restart_of_an_oauth_usage_window_is_published_without_a_new_reading() {
        let home = home();
        let api = Api::default();
        home.set_token("sk-ant-oat01-exemplo", WEEKLY_RESET);
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        claude.step(false);
        // Long backoff: no new reading arrives around the reset.
        api.answer(Err(HttpError::response(429, Some("86400"))));
        home.clock.advance(FIVE_HOUR_RESET - NOW - 1_000);
        claude.step(false);
        assert!(!claude.step(false));
        home.clock.advance(2_000);
        assert!(claude.step(false));
        assert_eq!(
            claude.data()["reading"]["buckets"][0]["primary"]["reset"],
            true
        );
    }

    #[test]
    fn weekly_limits_per_model_come_only_from_the_newest_oauth_usage_reading() {
        let home = home();
        let state = tempfile::tempdir().unwrap();
        let api = Api::default();
        let mut claude = collector(&home, &api).with_persistence(disk(state.path()));
        claude.set_usage(true).unwrap();
        let mut sonnet = response();
        sonnet["seven_day_sonnet"] =
            json!({"utilization": 33.0, "resets_at": "2026-11-05T14:00:00+00:00"});
        api.answer(Ok(sonnet));
        claude.step(false);
        home.clock.advance(60_000);
        home.status_line_runs(&status_payload());
        claude.step(false);
        // Restart from the cache, where the newest reading is the status line;
        // Sonnet came back null since then.
        let mut restarted = collector(&home, &api).with_persistence(disk(state.path()));
        restarted.step(false);
        let buckets = restarted.data()["reading"]["buckets"].clone();
        assert_eq!(buckets.as_array().unwrap().len(), 1, "{buckets}");
    }

    #[test]
    fn offline_the_claude_part_reports_its_own_pause() {
        use crate::lifecycle::Status;
        let home = home();
        let api = Api::default();
        let mut claude = collector(&home, &api);
        claude.set_usage(true).unwrap();
        claude.lifecycle(Status {
            online: true,
            generation: 1,
            ready_at: NOW,
        });
        claude.step(false);
        assert_eq!(claude.data()["paused"], true, "before its turn");
        home.clock.advance(9_000);
        claude.step(false);
        assert!(claude.data().get("paused").is_none());
        claude.lifecycle(Status {
            online: false,
            generation: 1,
            ready_at: NOW,
        });
        assert_eq!(claude.data()["paused"], true);
    }
}

#[test]
fn only_opted_in_queries_have_operational_results() {
    let home = usage::home();
    let api = usage::Api::default();
    let mut claude = usage::collector(&home, &api);
    home.status_line_runs(&payload());
    claude.step(true);
    assert!(claude.data().get("operation").is_none());
    claude.set_usage(true).unwrap();
    home.clock.advance(10_000);
    claude.step(true);
    assert_eq!(
        claude.data()["operation"],
        json!({"id":1,"origin":"user","state":"succeeded"})
    );
}

#[test]
fn claude_publishes_request_start_before_http_and_recovers_from_real_failure() {
    let home = usage::home();
    let api = usage::Api::default();
    let mut claude = usage::collector(&home, &api);
    claude.set_usage(true).unwrap();
    api.answer(Err(crate::ports::HttpError::response(503, None)));
    let mut starts = Vec::new();
    claude.step_with_progress(true, |data| starts.push(data));
    assert_eq!(
        starts[0]["operation"],
        json!({"id":1,"origin":"user","state":"running"})
    );
    assert_eq!(claude.data()["operation"]["state"], "failed");
    assert!(claude.data().get("failureSince").is_none());
    home.clock.advance(30_000);
    claude.step(false);
    assert_eq!(
        claude.data()["operation"],
        json!({"id":2,"origin":"automatic","state":"recovered"})
    );
    claude.set_usage(false).unwrap();
    claude.step(true);
    assert!(claude.data().get("operation").is_none());
    claude.set_usage(true).unwrap();
    claude.step(true);
    assert_eq!(claude.data()["operation"]["id"], 3);
}

#[test]
fn operational_snapshots_and_persistence_exclude_sensitive_sentinels() {
    let home = usage::home();
    let mut credentials: Value =
        serde_json::from_str(&home.read(".claude/.credentials.json")).unwrap();
    credentials["claudeAiOauth"]["accessToken"] = json!("SENTINEL_CREDENTIAL");
    home.write(".claude/.credentials.json", &credentials.to_string());
    let settings = home.settings();
    let api = usage::Api::default();
    let mut response = usage::response();
    response["prompt"] = json!("SENTINEL_PROMPT");
    response["response"] = json!("SENTINEL_RESPONSE");
    api.answer(Ok(response));
    let state = tempfile::tempdir().unwrap();
    let mut claude = usage::collector(&home, &api).with_persistence(disk(state.path()));
    claude.set_usage(true).unwrap();
    let mut starts = Vec::new();
    claude.step_with_progress(true, |data| starts.push(data));
    for text in [
        serde_json::to_string(&starts).unwrap(),
        claude.data().to_string(),
        std::fs::read_to_string(state.path().join("cache/ai.json")).unwrap(),
        std::fs::read_to_string(state.path().join("state/info.log")).unwrap(),
    ] {
        assert!(!text.contains("SENTINEL"));
    }
    let cached = std::fs::read_to_string(state.path().join("cache/ai.json")).unwrap();
    assert!(!cached.contains("operation"));
    assert_eq!(home.settings(), settings);
    assert_eq!(
        home.read(".claude/.credentials.json"),
        credentials.to_string()
    );
}

#[test]
fn claude_resumes_only_its_opted_in_source_after_pause() {
    use crate::lifecycle::Status;
    let home = usage::home();
    let api = usage::Api::default();
    let mut claude = usage::collector(&home, &api);
    claude.lifecycle(Status {
        online: false,
        generation: 1,
        ready_at: 0,
    });
    claude.step(true);
    assert!(claude.data().get("operation").is_none());
    claude.set_usage(true).unwrap();
    claude.lifecycle(Status {
        online: false,
        generation: 1,
        ready_at: 0,
    });
    claude.step(false);
    assert_eq!(claude.data()["operation"]["state"], "paused");
    claude.lifecycle(Status {
        online: true,
        generation: 2,
        ready_at: 0,
    });
    let mut starts = Vec::new();
    claude.step_with_progress(false, |data| starts.push(data));
    assert_eq!(
        starts[0]["operation"],
        json!({"id":2,"origin":"resume","state":"running"})
    );
    assert_eq!(claude.data()["operation"]["state"], "succeeded");
}

#[test]
fn a_connection_change_during_http_discards_the_response() {
    use crate::lifecycle::{Shared, Status};
    struct Interrupted(Shared);
    impl UsageApi for Interrupted {
        fn usage(&self, _: &str) -> Result<Value, crate::ports::HttpError> {
            *self.0.lock().unwrap() = Status {
                online: false,
                generation: 1,
                ready_at: 0,
            };
            Ok(usage::response())
        }
    }
    let home = usage::home();
    let status = Shared::default();
    *status.lock().unwrap() = Status {
        online: true,
        generation: 1,
        ready_at: 0,
    };
    let mut claude = home
        .claude()
        .with_usage(Interrupted(status.clone()))
        .with_lifecycle(status.clone());
    claude.set_usage(true).unwrap();
    claude.lifecycle(*status.lock().unwrap());
    claude.step(true);
    assert_eq!(claude.data()["operation"]["state"], "paused");
    assert!(claude.data()["reading"].is_null());
    assert!(claude.updated_at().is_none());
}

#[test]
fn missing_access_token_keeps_a_persistent_condition_beside_existing_readings() {
    let home = usage::home();
    let api = usage::Api::default();
    let mut claude = usage::collector(&home, &api);
    claude.set_usage(true).unwrap();
    claude.step(false);
    let previous = claude.data()["reading"].clone();
    let mut credentials: Value =
        serde_json::from_str(&home.read(".claude/.credentials.json")).unwrap();
    credentials["claudeAiOauth"]["accessToken"] = Value::Null;
    home.write(".claude/.credentials.json", &credentials.to_string());
    home.clock.advance(300_000);
    claude.step(true);
    assert_eq!(claude.data()["usageUnavailable"], true);
    assert_eq!(claude.data()["reading"], previous);
    home.clock.advance(4000);
    claude.step(false);
    assert_eq!(claude.data()["usageUnavailable"], true);
}
