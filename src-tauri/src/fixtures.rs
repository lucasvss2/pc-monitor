//! Development-only fixture mode (Cargo feature `fixtures`): the native panel shows
//! the same snapshots as prototype B, published through the regular snapshot path.
//!
//! No collector starts: nothing reads the system, the network, the keyring or the
//! agent configuration files, and no notification is sent. Write commands answer
//! in memory. Fixtures are read from `src/lib/<module>/fixtures/` at startup, so
//! they are never embedded in a binary.

#[cfg(not(debug_assertions))]
compile_error!("the fixtures feature is for development builds only");

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::module::Module;
use crate::snapshot::Snapshot;

/// Fixed clock: the instant the `claude-usage` fixture was published, so both AI
/// windows are still open as in prototype B.
pub const NOW: u64 = 1_793_401_200_000;

/// Pins `Date.now()` and `new Date()` in the panel webview to `NOW`.
pub const CLOCK_SCRIPT: &str = "(() => {
  const now = 1793401200000;
  const Real = Date;
  class FixedDate extends Real {
    constructor(...args) { super(...(args.length ? args : [now])); }
    static now() { return now; }
  }
  globalThis.Date = FixedDate;
})();";

/// Environment variable choosing other fixtures, e.g. `ai=claude-failure,network=poor`.
pub const SELECTION_VARIABLE: &str = "KVOJPS_FIXTURES";

/// The fixtures shown by prototype B.
const B: [(Module, &str); 6] = [
    (Module::Ai, "claude-usage"),
    (Module::Agents, "states"),
    (Module::Github, "full"),
    (Module::Tasks, "full"),
    (Module::Network, "full"),
    (Module::System, "notebook"),
];

/// Fixture name per module: B's, overridden by `module=name` pairs.
pub fn selection(spec: &str) -> Result<BTreeMap<Module, String>, String> {
    let mut chosen: BTreeMap<Module, String> =
        B.iter().map(|(m, name)| (*m, (*name).to_owned())).collect();
    for pair in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (module, name) = pair
            .split_once('=')
            .ok_or_else(|| format!("expected module=fixture, got {pair:?}"))?;
        let module: Module = serde_json::from_value(Value::String(module.trim().into()))
            .map_err(|_| format!("unknown module {module:?}"))?;
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("invalid fixture name {name:?}"));
        }
        chosen.insert(module, name.to_owned());
    }
    Ok(chosen)
}

/// UI fixtures directory of the repository.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib")
}

/// Read one fixture. Files holding a whole snapshot are used as is; files holding
/// only the module data are wrapped, updated at the fixed clock.
fn load(root: &Path, module: Module, name: &str) -> Result<Snapshot, String> {
    let path = root
        .join(if module == Module::Tasks {
            "trello"
        } else {
            module.id()
        })
        .join("fixtures")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("invalid {}: {e}", path.display()))?;
    let (updated_at, data) = match value {
        Value::Object(mut whole) if whole.contains_key("module") && whole.contains_key("data") => (
            whole.get("updatedAt").and_then(Value::as_u64),
            whole.remove("data").unwrap_or(Value::Null),
        ),
        data => (Some(NOW), data),
    };
    Ok(Snapshot {
        module,
        updated_at,
        data,
    })
}

/// The snapshots shown in fixture mode, changed only in memory.
pub struct Fixtures(BTreeMap<Module, Snapshot>);

impl Fixtures {
    pub fn load(selection: &BTreeMap<Module, String>) -> Result<Self, String> {
        let root = fixtures_root();
        selection
            .iter()
            .map(|(module, name)| Ok((*module, load(&root, *module, name)?)))
            .collect::<Result<_, String>>()
            .map(Fixtures)
    }

    pub fn get(&self, module: Module) -> Snapshot {
        self.0[&module].clone()
    }

    fn data(&mut self, module: Module) -> &mut Value {
        &mut self
            .0
            .get_mut(&module)
            .expect("every module has a fixture")
            .data
    }

    /// Move a card to the bottom of another list of the shown board, like Trello.
    pub fn move_card(&mut self, card: &str, list: &str) -> Result<(), crate::trello::MoveError> {
        use crate::trello::MoveError;
        let lists = self.data(Module::Tasks)["lists"]
            .as_array_mut()
            .ok_or(MoveError)?;
        let to = lists
            .iter()
            .position(|l| l["id"] == list)
            .ok_or(MoveError)?;
        let (from, index) = lists
            .iter()
            .enumerate()
            .find_map(|(from, source)| {
                let cards = source["cards"].as_array()?;
                cards
                    .iter()
                    .position(|c| c["id"] == card)
                    .map(|i| (from, i))
            })
            .ok_or(MoveError)?;
        if from == to {
            return Err(MoveError);
        }
        let moved = lists[from]["cards"]
            .as_array_mut()
            .ok_or(MoveError)?
            .remove(index);
        lists[to]["cards"]
            .as_array_mut()
            .ok_or(MoveError)?
            .push(moved);
        Ok(())
    }

    /// A simulated speed test: the last result, measured now.
    pub fn speed_test(&mut self) {
        let network = self.data(Module::Network);
        network["speedRunning"] = false.into();
        network["speedFailed"] = false.into();
        if let Some(speed) = network.get_mut("speed").filter(|s| s.is_object()) {
            speed["measuredAt"] = NOW.into();
        }
    }

    pub fn automatic_speed(&mut self, enabled: bool) {
        self.data(Module::Network)["automaticSpeed"] = enabled.into();
    }

    /// Claude status line and `oauth/usage` switches, without touching any file.
    pub fn claude_switch(&mut self, key: &str, on: bool) {
        if let Some(claude) = self.data(Module::Ai).get_mut("claude") {
            claude[key] = (if on { "on" } else { "off" }).into();
        }
    }

    fn login(&self, module: Module) -> String {
        let data = &self.0[&module].data;
        [&data["login"], &data["account"]["login"]]
            .into_iter()
            .find_map(Value::as_str)
            .unwrap_or("fixture")
            .to_owned()
    }
}

type Shared = Arc<Mutex<Fixtures>>;

/// Publish every fixture and answer the UI commands in memory.
pub fn start(app: &AppHandle) {
    let spec = std::env::var(SELECTION_VARIABLE).unwrap_or_default();
    let fixtures = selection(&spec)
        .and_then(|chosen| Fixtures::load(&chosen))
        .unwrap_or_else(|e| panic!("fixture mode: {e}"));
    let shared: Shared = Arc::new(Mutex::new(fixtures));
    for (module, _) in B {
        crate::snapshot::publish(app, shared.lock().unwrap().get(module));
    }

    app.manage(crate::system::Collector::detached());
    app.manage(crate::network::Collector(serve(
        app,
        &shared,
        |f, command| match command {
            crate::network::Command::FullSpeed => {
                f.speed_test();
                Some(Module::Network)
            }
            crate::network::Command::AutomaticSpeed(enabled) => {
                f.automatic_speed(enabled);
                Some(Module::Network)
            }
            crate::network::Command::Visible(_) | crate::network::Command::Resume => None,
        },
    )));
    app.manage(crate::ai::Collector(serve(app, &shared, |f, command| {
        use crate::ai::Command;
        match command {
            Command::Refresh => {}
            Command::ConnectClaude(reply) => {
                f.claude_switch("statusLine", true);
                let _ = reply.send(Ok(()));
            }
            Command::DisconnectClaude(reply) => {
                f.claude_switch("statusLine", false);
                let _ = reply.send(Ok(()));
            }
            Command::SetClaudeUsage(enabled, reply) => {
                f.claude_switch("usage", enabled);
                let _ = reply.send(Ok(()));
            }
        }
        Some(Module::Ai)
    })));
    app.manage(crate::github::Collector(serve(
        app,
        &shared,
        |f, command| {
            if let crate::github::Command::SaveToken(_, reply) = command {
                let _ = reply.send(Ok(f.login(Module::Github)));
            }
            Some(Module::Github)
        },
    )));
    app.manage(crate::tasks::Collector(serve(app, &shared, |f, command| {
        use crate::tasks::{Command, Provider, Error};
        match command {
            Command::Refresh(_) | Command::Open(_, _) => {}
            Command::Connect(_, _, reply) => { let _ = reply.send(Ok(f.login(Module::Tasks))); }
            Command::Switch(_, provider, reply) => {
                let generation = f.get(Module::Tasks).data["contextGeneration"].as_u64().unwrap_or(0) + 1;
                let mut snapshot = if provider == Provider::Trello { load(&fixtures_root(), Module::Tasks, "full").unwrap() } else {
                    Snapshot { module: Module::Tasks, updated_at: None, data: serde_json::json!({"state":"missingCredential","account":null,"boards":[],"board":null,"columns":[],"issues":[],"refreshing":false}) }
                };
                snapshot.data["provider"] = serde_json::json!(provider);
                snapshot.data["contextGeneration"] = serde_json::json!(generation);
                f.0.insert(Module::Tasks, snapshot);
                let _ = reply.send(Ok(()));
            }
            Command::SelectBoard(_, _, reply) | Command::Disconnect(_, reply) | Command::Authorize(_, _, reply) | Command::RetryCleanup(_, reply) => { let _ = reply.send(Ok(())); }
            Command::Transitions(_, _, reply) => { let _ = reply.send(Ok(vec![])); }
            Command::Perform(_, action, reply) => {
                let result = match action { crate::tasks::Action::Move {item,list} => f.move_card(&item,&list).map_err(|_| Error::Refused), _ => Err(Error::Refused) };
                let _ = reply.send(result);
            }
        }
        Some(Module::Tasks)
    })));
    app.manage(crate::agents::Collector(serve(
        app,
        &shared,
        |_, (_, reply): (bool, tokio::sync::oneshot::Sender<Result<(), String>>)| {
            // Hook files stay untouched.
            let _ = reply.send(Ok(()));
            None
        },
    )));
}

/// A collector stand-in: applies each command to the fixtures and republishes the
/// module it changed.
fn serve<C: Send + 'static>(
    app: &AppHandle,
    shared: &Shared,
    mut handle: impl FnMut(&mut Fixtures, C) -> Option<Module> + Send + 'static,
) -> mpsc::Sender<C> {
    let (tx, rx) = mpsc::channel();
    let (app, shared) = (app.clone(), shared.clone());
    std::thread::spawn(move || {
        for command in rx {
            let snapshot = {
                let mut fixtures = shared.lock().unwrap();
                handle(&mut fixtures, command).map(|module| fixtures.get(module))
            };
            if let Some(snapshot) = snapshot {
                crate::snapshot::publish(&app, snapshot);
            }
        }
    });
    tx
}

/// Preferences and the autostart entry live outside the user's configuration.
pub fn config_dir() -> PathBuf {
    std::env::temp_dir().join("kvojps-monitor-fixtures")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b() -> Fixtures {
        Fixtures::load(&selection("").unwrap()).unwrap()
    }

    #[test]
    fn b_fixtures_by_default_and_overrides_per_module() {
        let chosen = selection(" ai=claude-failure , network=poor").unwrap();
        assert_eq!(chosen[&Module::Ai], "claude-failure");
        assert_eq!(chosen[&Module::Network], "poor");
        assert_eq!(chosen[&Module::Agents], "states");
        assert_eq!(chosen[&Module::System], "notebook");
        assert!(Fixtures::load(&chosen).is_ok());
    }

    #[test]
    fn rejects_unknown_modules_and_names_outside_the_fixture_directory() {
        assert!(selection("weather=sunny").is_err());
        assert!(selection("ai").is_err());
        assert!(selection("ai=../../secrets").is_err());
        assert!(Fixtures::load(&selection("ai=missing-fixture-name").unwrap()).is_err());
    }

    #[test]
    fn data_fixtures_are_wrapped_at_the_fixed_clock_and_snapshots_kept() {
        let fixtures = b();
        let ai = fixtures.get(Module::Ai);
        assert_eq!(ai.updated_at, Some(NOW));
        assert!(ai.data["claude"].is_object());
        let system = fixtures.get(Module::System);
        assert_eq!(system.updated_at, Some(1_793_457_124_000));
        assert!(system.data["cpu"].is_object());
        assert!(fixtures.get(Module::Network).data["speed"].is_object());
    }

    fn counts(fixtures: &Fixtures) -> Vec<usize> {
        fixtures.get(Module::Tasks).data["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["cards"].as_array().unwrap().len())
            .collect()
    }

    #[test]
    fn moves_a_card_to_the_bottom_of_another_list_and_back() {
        let mut fixtures = b();
        let before = counts(&fixtures);
        let card = "000000000000000000000004";
        let (first, second) = ("000000000000000000000003", "00000000000000000000000a");
        fixtures.move_card(card, second).unwrap();
        let after = counts(&fixtures);
        assert_eq!((after[0], after[1]), (before[0] - 1, before[1] + 1));
        let lists = &fixtures.get(Module::Tasks).data["lists"];
        assert_eq!(
            lists[1]["cards"].as_array().unwrap().last().unwrap()["id"],
            card
        );
        assert!(fixtures.move_card(card, second).is_err());
        assert!(fixtures.move_card("unknown", first).is_err());
        assert!(fixtures.move_card(card, "unknown").is_err());
        fixtures.move_card(card, first).unwrap();
        assert_eq!(counts(&fixtures), before);
    }

    #[test]
    fn simulated_speed_test_and_claude_switches_change_memory_only() {
        let mut fixtures = b();
        fixtures.speed_test();
        let network = fixtures.get(Module::Network).data;
        assert_eq!(network["speed"]["measuredAt"], NOW);
        assert_eq!(network["speedRunning"], false);
        fixtures.claude_switch("statusLine", true);
        assert_eq!(fixtures.get(Module::Ai).data["claude"]["statusLine"], "on");
        assert_eq!(fixtures.login(Module::Github), "exemplo");
        assert_eq!(fixtures.login(Module::Tasks), "exemplo");
    }

    #[test]
    fn clock_script_pins_the_fixture_instant() {
        assert!(CLOCK_SCRIPT.contains(&NOW.to_string()));
    }

    /// Guardrail: fixture mode never reaches a real port.
    #[test]
    fn production_code_uses_no_real_port() {
        let source = include_str!("fixtures.rs");
        let code = source.split("#[cfg(test)]").next().unwrap();
        for real in [
            "SecretService",
            "SystemProcesses",
            "GithubHttp",
            "TrelloHttp",
            "DesktopNotifier",
            "RootFilesystem",
            "SystemClock",
            "Dbus",
            "::start(",
            "settings.json",
            "hooks.json",
        ] {
            assert!(!code.contains(real), "fixture mode references {real}");
        }
    }
}
