use crate::persistence::{DiskStorage, Storage};
use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::module::Module;

// Keep the existing filename so upgrades retain user preferences.
const PREFERENCES_FILE: &str = "preferencias.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    Dark,
    Light,
    /// Follows the desktop color scheme, including later changes.
    #[default]
    System,
}

/// Visual preferences; the system reduced-motion preference prevails in the UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: Theme,
    pub reduce_motion: bool,
}

/// Daily check of the GitHub releases API; public metadata only.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct VersionCheck {
    enabled: bool,
    /// Last successful check, epoch milliseconds.
    checked_at: Option<u64>,
    available: Option<crate::releases::Release>,
}
impl Default for VersionCheck {
    fn default() -> Self {
        Self {
            enabled: true,
            checked_at: None,
            available: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Preferences {
    schema_version: u64,
    /// Last module chosen in the panel navigation; independent from visibility.
    #[serde(alias = "lastModule", alias = "ultimoModulo")]
    last_anchor: Module,
    agent_notifications: crate::agents::notifications::Preferences,
    /// First run has already enabled autostart.
    #[serde(alias = "inicializado")]
    initialized: bool,
    /// The first-run welcome was shown.
    welcomed: bool,
    appearance: Appearance,
    version_check: VersionCheck,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde_json::Value>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema_version: 2,
            last_anchor: Module::default(),
            agent_notifications: Default::default(),
            initialized: false,
            welcomed: false,
            appearance: Default::default(),
            version_check: Default::default(),
            extra: Default::default(),
        }
    }
}
fn field<T: serde::de::DeserializeOwned>(value: &serde_json::Value, default: T) -> T {
    serde_json::from_value(value.clone()).unwrap_or(default)
}
impl Preferences {
    fn load(value: &serde_json::Value) -> Self {
        let mut result = Self::default();
        result.last_anchor = field(
            value
                .get("lastAnchor")
                .or_else(|| value.get("lastModule"))
                .or_else(|| value.get("ultimoModulo"))
                .unwrap_or(&serde_json::Value::Null),
            result.last_anchor,
        );
        result.initialized = field(
            value
                .get("initialized")
                .or_else(|| value.get("inicializado"))
                .unwrap_or(&serde_json::Value::Null),
            false,
        );
        result.welcomed = field(&value["welcomed"], false);
        result.appearance.theme = field(&value["appearance"]["theme"], Theme::System);
        result.appearance.reduce_motion = field(&value["appearance"]["reduceMotion"], false);
        let notifications = &value["agentNotifications"];
        result.agent_notifications.waiting = field(&notifications["waiting"], true);
        result.agent_notifications.done = field(&notifications["done"], true);
        result.agent_notifications.failed = field(&notifications["failed"], true);
        result.version_check.enabled = field(&value["versionCheck"]["enabled"], true);
        result.version_check.checked_at = field(&value["versionCheck"]["checkedAt"], None);
        result.version_check.available = field(&value["versionCheck"]["available"], None);
        if let Some(object) = value.as_object() {
            for (key, value) in object {
                if ![
                    "schemaVersion",
                    "lastAnchor",
                    "lastModule",
                    "ultimoModulo",
                    "initialized",
                    "inicializado",
                    "welcomed",
                    "appearance",
                    "agentNotifications",
                    "versionCheck",
                ]
                .contains(&key.as_str())
                {
                    result.extra.insert(key.clone(), value.clone());
                }
            }
        }
        result
    }
}

/// Core panel state: current visibility and preferences persisted between launches.
pub struct AppState {
    storage: Box<dyn Storage>,
    preferences: Preferences,
    panel_open: bool,
    agent_target: Option<String>,
    /// Modules intersecting the visible panel area; empty when hidden or in Settings.
    visible: BTreeSet<Module>,
    preferences_error: Option<&'static str>,
}

impl AppState {
    /// Load preferences; preserve unreadable or newer files without overwriting them.
    pub fn load(dir_config: PathBuf) -> Self {
        Self::with_storage(DiskStorage(dir_config))
    }

    pub fn with_storage(storage: impl Storage + 'static) -> Self {
        let loaded = storage.read(PREFERENCES_FILE);
        let value = loaded
            .as_ref()
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            .filter(serde_json::Value::is_object);
        let preferences_error = if value
            .as_ref()
            .is_some_and(|v| v["schemaVersion"].as_u64().is_some_and(|v| v > 2))
        {
            Some("futureSchema")
        } else if value.is_none()
            && !loaded
                .as_ref()
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            Some("unreadable")
        } else {
            None
        };
        let mut preferences = value.as_ref().map(Preferences::load).unwrap_or_default();
        if preferences_error.is_some() {
            preferences.initialized = true;
            preferences.welcomed = true;
        }
        let mut state = AppState {
            storage: Box::new(storage),
            preferences,
            panel_open: false,
            agent_target: None,
            visible: BTreeSet::new(),
            preferences_error,
        };
        if value.as_ref().is_some_and(|v| v["schemaVersion"] != 2)
            && preferences_error.is_none()
            && state.try_save().is_err()
        {
            state.preferences_error = Some("migrationFailed");
        }
        state
    }
    pub fn preferences_error(&self) -> Option<&'static str> {
        self.preferences_error
    }

    pub fn last_anchor(&self) -> Module {
        self.preferences.last_anchor
    }

    /// The user chose module in the panel navigation.
    pub fn choose_anchor(&mut self, module: Module) {
        self.agent_target = None;
        if self.preferences.last_anchor != module {
            self.preferences.last_anchor = module;
            self.save();
        }
    }

    /// Replace the set of modules intersecting the visible panel area. Returns
    /// whether it changed, so repeated intersection reports trigger nothing.
    pub fn report_visible(&mut self, modules: &[Module]) -> bool {
        if !self.panel_open {
            return false;
        }
        let visible: BTreeSet<Module> = modules.iter().copied().collect();
        if visible == self.visible {
            return false;
        }
        self.visible = visible;
        true
    }

    pub fn first_execution(&self) -> bool {
        !self.preferences.initialized
    }

    pub fn complete_first_execution(&mut self) {
        self.preferences.initialized = true;
        self.save();
    }

    /// Recreate the panel at the last anchor on each opening. The UI scrolls to
    /// it, so it counts as visible until the UI reports the real set. An already
    /// open panel keeps its set: the UI reports again if it scrolls.
    pub fn open_panel(&mut self) {
        if self.panel_open {
            return;
        }
        self.panel_open = true;
        self.visible = BTreeSet::from([self.opening_anchor()]);
    }

    pub fn close_panel(&mut self) {
        self.panel_open = false;
        self.agent_target = None;
        self.visible.clear();
    }

    /// Some collectors run faster or only while their module is visible.
    pub fn is_visible(&self, module: Module) -> bool {
        self.visible.contains(&module)
    }

    #[cfg(test)]
    pub fn visible_modules(&self) -> Vec<Module> {
        self.visible.iter().copied().collect()
    }

    pub fn welcome_pending(&self) -> bool {
        !self.preferences.welcomed
    }
    pub fn complete_welcome(&mut self) {
        if !self.preferences.welcomed {
            self.preferences.welcomed = true;
            self.save();
        }
    }

    pub fn appearance(&self) -> Appearance {
        self.preferences.appearance
    }
    pub fn set_appearance(&mut self, appearance: Appearance) -> std::io::Result<()> {
        let previous = self.preferences.appearance;
        self.preferences.appearance = appearance;
        if let Err(error) = self.try_save() {
            self.preferences.appearance = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn version_check(&self) -> bool {
        self.preferences.version_check.enabled
    }
    pub fn set_version_check(&mut self, enabled: bool) -> std::io::Result<()> {
        let previous = self.preferences.version_check.clone();
        self.preferences.version_check.enabled = enabled;
        if !enabled {
            self.preferences.version_check.available = None;
        }
        if let Err(error) = self.try_save() {
            self.preferences.version_check = previous;
            return Err(error);
        }
        Ok(())
    }

    /// A newer release found by the daily check, while the check is enabled.
    pub fn available_release(&self) -> Option<crate::releases::Release> {
        let check = &self.preferences.version_check;
        check.enabled.then(|| check.available.clone()).flatten()
    }
    pub fn version_checked_at(&self) -> Option<u64> {
        self.preferences.version_check.checked_at
    }
    pub fn record_version_check(&mut self, at: u64, available: Option<crate::releases::Release>) {
        self.preferences.version_check.checked_at = Some(at);
        self.preferences.version_check.available = available;
        self.save();
    }
    /// Forget a known release once it stops being newer, e.g. after upgrading.
    pub fn keep_release_if(&mut self, newer: impl Fn(&crate::releases::Release) -> bool) {
        let check = &mut self.preferences.version_check;
        if check.available.as_ref().is_some_and(|r| !newer(r)) {
            check.available = None;
            self.save();
        }
    }

    pub fn agent_notifications(&self) -> crate::agents::notifications::Preferences {
        self.preferences.agent_notifications
    }
    pub fn set_agent_notifications(
        &mut self,
        preferences: crate::agents::notifications::Preferences,
    ) {
        self.preferences.agent_notifications = preferences;
        self.save();
    }
    pub fn target_agent(&mut self, session: String) {
        self.agent_target = Some(session);
    }
    pub fn agent_target(&self) -> Option<String> {
        self.agent_target.clone()
    }
    pub fn opening_anchor(&self) -> Module {
        if self.agent_target.is_some() {
            Module::Agents
        } else {
            self.last_anchor()
        }
    }
    fn try_save(&self) -> std::io::Result<()> {
        if self.preferences_error.is_some() {
            return Err(std::io::ErrorKind::Unsupported.into());
        }
        let json = serde_json::to_string_pretty(&self.preferences)?;
        self.storage.write("preferencias.json.tmp", &json)?;
        self.storage
            .rename("preferencias.json.tmp", PREFERENCES_FILE)
    }

    fn save(&self) {
        // A write failure loses preferences; the app keeps running.
        let _ = self.try_save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn invalid_theme_preserves_legacy_anchor_and_other_valid_preferences() {
        let d = dir();
        fs::write(d.path().join(PREFERENCES_FILE), r#"{"lastAnchor":"trello","initialized":true,"welcomed":true,"appearance":{"theme":"invalid","reduceMotion":true},"versionCheck":{"enabled":false}}"#).unwrap();
        let state = AppState::load(d.path().into());
        assert_eq!(state.last_anchor(), Module::Tasks);
        assert!(state.appearance().reduce_motion);
        assert_eq!(state.appearance().theme, Theme::System);
        assert!(!state.welcome_pending());
        assert!(!state.first_execution());
        assert!(!state.version_check());
    }

    #[test]
    fn first_run_opens_ai_module() {
        let d = dir();
        assert_eq!(AppState::load(d.path().into()).last_anchor(), Module::Ai);
    }

    #[test]
    fn last_anchor_persists_between_launches() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.choose_anchor(Module::Network);

        assert_eq!(
            AppState::load(d.path().into()).last_anchor(),
            Module::Network
        );
    }

    #[test]
    fn unreadable_preferences_are_preserved_and_reported_without_startup_changes() {
        let d = dir();
        fs::write(d.path().join(PREFERENCES_FILE), "{ invalid json").unwrap();
        let mut state = AppState::load(d.path().into());
        assert_eq!(state.last_anchor(), Module::Ai);
        assert_eq!(state.preferences_error(), Some("unreadable"));
        assert!(!state.first_execution());
        assert!(!state.welcome_pending());
        assert!(state.set_version_check(false).is_err());
        state.choose_anchor(Module::Tasks);
        assert_eq!(
            fs::read_to_string(d.path().join(PREFERENCES_FILE)).unwrap(),
            "{ invalid json"
        );
    }

    #[test]
    fn future_preferences_remain_byte_identical_when_a_setting_is_requested() {
        let d = dir();
        let original = r#"{"schemaVersion":999,"lastAnchor":"trello","initialized":true,"futureChoice":"keep"}"#;
        fs::write(d.path().join(PREFERENCES_FILE), original).unwrap();
        let mut state = AppState::load(d.path().into());
        assert_eq!(state.last_anchor(), Module::Tasks);
        assert_eq!(state.preferences_error(), Some("futureSchema"));
        assert!(state.set_version_check(false).is_err());
        assert_eq!(
            fs::read_to_string(d.path().join(PREFERENCES_FILE)).unwrap(),
            original
        );
    }

    #[test]
    fn failed_atomic_migration_keeps_the_original_file_and_valid_choices() {
        struct RenameDenied(DiskStorage);
        impl Storage for RenameDenied {
            fn read(&self, p: &str) -> std::io::Result<String> {
                self.0.read(p)
            }
            fn write(&self, p: &str, data: &str) -> std::io::Result<()> {
                self.0.write(p, data)
            }
            fn rename(&self, _: &str, _: &str) -> std::io::Result<()> {
                Err(std::io::ErrorKind::PermissionDenied.into())
            }
            fn remove(&self, p: &str) -> std::io::Result<()> {
                self.0.remove(p)
            }
        }
        let d = dir();
        let original = r#"{"lastAnchor":"trello","initialized":true,"welcomed":true,"appearance":{"theme":"dark","reduceMotion":true}}"#;
        fs::write(d.path().join(PREFERENCES_FILE), original).unwrap();
        let mut state = AppState::with_storage(RenameDenied(DiskStorage(d.path().into())));
        assert_eq!(state.last_anchor(), Module::Tasks);
        assert_eq!(state.preferences_error(), Some("migrationFailed"));
        assert_eq!(
            state.appearance(),
            Appearance {
                theme: Theme::Dark,
                reduce_motion: true
            }
        );
        assert!(state.set_version_check(false).is_err());
        assert_eq!(
            fs::read_to_string(d.path().join(PREFERENCES_FILE)).unwrap(),
            original
        );
    }

    #[test]
    fn closed_panel_has_no_visible_module() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.report_visible(&[Module::System, Module::Ai]);

        assert!(state.visible_modules().is_empty());
    }

    #[test]
    fn every_module_intersecting_the_panel_counts_as_visible() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.open_panel();

        state.report_visible(&[Module::Ai, Module::Agents, Module::Github]);
        assert!(state.is_visible(Module::Ai));
        assert!(state.is_visible(Module::Agents));
        assert!(state.is_visible(Module::Github));
        assert!(!state.is_visible(Module::Network));

        // Scrolling moves modules in and out of the visible area.
        state.report_visible(&[Module::Network, Module::System]);
        assert!(!state.is_visible(Module::Ai));
        assert!(state.is_visible(Module::Network));
        assert!(state.is_visible(Module::System));
    }

    #[test]
    fn repeated_reports_of_the_same_set_are_not_a_change() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.open_panel();

        assert!(state.report_visible(&[Module::Ai, Module::System]));
        assert!(!state.report_visible(&[Module::System, Module::Ai]));
        assert!(!state.report_visible(&[Module::System, Module::Ai, Module::Ai]));
        assert!(state.report_visible(&[Module::System]));
    }

    #[test]
    fn visibility_does_not_change_the_last_anchor() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.choose_anchor(Module::Tasks);
        state.open_panel();
        state.report_visible(&[Module::Ai, Module::Network]);

        assert_eq!(AppState::load(d.path().into()).last_anchor(), Module::Tasks);
    }

    #[test]
    fn opening_panel_starts_with_the_restored_anchor_visible() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.choose_anchor(Module::System);
        state.open_panel();
        assert_eq!(state.visible_modules(), vec![Module::System]);

        state.report_visible(&[Module::Tasks, Module::Github]);
        state.close_panel();
        assert!(state.visible_modules().is_empty());

        // Closing destroys the webview; reopening restores the last anchor.
        state.open_panel();
        assert_eq!(state.visible_modules(), vec![Module::System]);
    }

    #[test]
    fn settings_reports_an_empty_set() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.choose_anchor(Module::Github);
        state.open_panel();
        state.report_visible(&[Module::Github, Module::Ai]);

        state.report_visible(&[]);

        assert!(state.visible_modules().is_empty());
        assert_eq!(
            AppState::load(d.path().into()).last_anchor(),
            Module::Github
        );
    }

    #[test]
    fn reopening_an_open_panel_keeps_the_visible_set() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.open_panel();
        state.report_visible(&[Module::Agents, Module::System, Module::Network]);

        // A notification or the indicator opens the panel that is already open.
        state.target_agent("session-1".into());
        state.open_panel();

        assert_eq!(
            state.visible_modules(),
            vec![Module::Agents, Module::Network, Module::System]
        );
    }

    #[test]
    fn reports_after_closing_do_not_make_modules_visible() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.open_panel();
        state.close_panel();

        assert!(!state.report_visible(&[Module::Ai]));
        assert!(state.visible_modules().is_empty());
    }

    #[test]
    fn first_run_ends_when_completed() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        assert!(state.first_execution());

        state.complete_first_execution();

        assert!(!AppState::load(d.path().into()).first_execution());
    }

    #[test]
    fn completing_first_run_preserves_last_anchor() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state.choose_anchor(Module::Agents);
        state.complete_first_execution();

        assert_eq!(
            AppState::load(d.path().into()).last_anchor(),
            Module::Agents
        );
    }
    #[test]
    fn legacy_preferences_preserve_last_anchor_and_first_run_status() {
        let d = dir();
        fs::write(
            d.path().join(PREFERENCES_FILE),
            r#"{"ultimoModulo":"rede","inicializado":true}"#,
        )
        .unwrap();
        let mut state = AppState::load(d.path().into());
        assert_eq!(state.last_anchor(), Module::Network);
        assert!(!state.first_execution());
        state.choose_anchor(Module::System);
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(d.path().join(PREFERENCES_FILE)).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({
                "schemaVersion": 2,
                "lastAnchor": "system",
                "initialized": true,
                "agentNotifications": {"waiting": true, "done": true, "failed": true},
                "welcomed": false,
                "appearance": {"theme": "system", "reduceMotion": false},
                "versionCheck": {"enabled": true, "checkedAt": null, "available": null}
            })
        );
        let reloaded = AppState::load(d.path().into());
        assert_eq!(reloaded.last_anchor(), Module::System);
        assert!(!reloaded.first_execution());
    }
}

#[cfg(test)]
mod agent_notification_tests {
    use super::*;
    use std::{
        io,
        sync::{Arc, Mutex},
    };
    #[derive(Clone, Default)]
    struct MemoryStorage(Arc<Mutex<Option<String>>>);
    impl Storage for MemoryStorage {
        fn read(&self, _: &str) -> io::Result<String> {
            self.0
                .lock()
                .unwrap()
                .clone()
                .ok_or(io::ErrorKind::NotFound.into())
        }
        fn write(&self, _: &str, data: &str) -> io::Result<()> {
            *self.0.lock().unwrap() = Some(data.into());
            Ok(())
        }
        fn rename(&self, _: &str, _: &str) -> io::Result<()> {
            Ok(())
        }
        fn remove(&self, _: &str) -> io::Result<()> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
    #[test]
    fn failed_visual_and_version_preferences_keep_the_confirmed_values() {
        struct ReadOnly(MemoryStorage);
        impl Storage for ReadOnly {
            fn read(&self, path: &str) -> io::Result<String> {
                self.0.read(path)
            }
            fn write(&self, _: &str, _: &str) -> io::Result<()> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
            fn rename(&self, _: &str, _: &str) -> io::Result<()> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
            fn remove(&self, _: &str) -> io::Result<()> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        }
        let mut state = AppState::with_storage(ReadOnly(MemoryStorage::default()));
        assert!(state
            .set_appearance(Appearance {
                theme: Theme::Light,
                reduce_motion: true
            })
            .is_err());
        assert_eq!(state.appearance(), Appearance::default());
        assert!(state.set_version_check(false).is_err());
        assert!(state.version_check());
    }

    #[test]
    fn notification_target_overrides_opening_anchor_until_panel_closes() {
        let storage = MemoryStorage::default();
        let mut state = AppState::with_storage(storage.clone());
        state.choose_anchor(Module::Network);
        state.target_agent("session-1".into());
        state.open_panel();
        assert_eq!(state.visible_modules(), vec![Module::Agents]);
        assert_eq!(state.agent_target().as_deref(), Some("session-1"));
        assert_eq!(state.last_anchor(), Module::Network);
        state.close_panel();
        state.open_panel();
        assert_eq!(state.visible_modules(), vec![Module::Network]);
        assert!(state.agent_target().is_none());
    }
    #[test]
    fn agent_switches_default_to_enabled_and_persist_without_session_content() {
        let storage = MemoryStorage::default();
        let mut state = AppState::with_storage(storage.clone());
        let defaults = state.agent_notifications();
        assert!(defaults.waiting && defaults.done && defaults.failed);
        state.target_agent("PRIVATE_SESSION".into());
        state.set_agent_notifications(crate::agents::notifications::Preferences {
            waiting: false,
            done: true,
            failed: false,
        });
        let reloaded = AppState::with_storage(storage.clone());
        let saved = reloaded.agent_notifications();
        assert!(!saved.waiting && saved.done && !saved.failed);
        assert!(!storage.read(PREFERENCES_FILE).unwrap().contains("PRIVATE_"));
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn appearance_defaults_to_the_system_theme_with_motion_and_persists() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        assert_eq!(
            state.appearance(),
            Appearance {
                theme: Theme::System,
                reduce_motion: false
            }
        );

        state
            .set_appearance(Appearance {
                theme: Theme::Light,
                reduce_motion: true,
            })
            .unwrap();

        assert_eq!(
            AppState::load(d.path().into()).appearance(),
            Appearance {
                theme: Theme::Light,
                reduce_motion: true
            }
        );
    }

    #[test]
    fn welcome_is_pending_only_until_completed() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        // Autostart completes the first execution before the panel ever opens.
        state.complete_first_execution();
        assert!(state.welcome_pending());

        state.complete_welcome();

        assert!(!state.welcome_pending());
        assert!(!AppState::load(d.path().into()).welcome_pending());
    }

    #[test]
    fn version_check_starts_enabled_and_can_be_disabled() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        assert!(state.version_check());

        state.set_version_check(false).unwrap();

        assert!(!AppState::load(d.path().into()).version_check());
    }

    #[test]
    fn preferences_keep_only_choices_never_credentials_or_content() {
        let d = dir();
        let mut state = AppState::load(d.path().into());
        state
            .set_appearance(Appearance {
                theme: Theme::Dark,
                reduce_motion: false,
            })
            .unwrap();
        state.target_agent("PRIVATE_SESSION".into());
        state.complete_welcome();
        state.complete_first_execution();

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(d.path().join(PREFERENCES_FILE)).unwrap())
                .unwrap();
        let mut keys: Vec<_> = saved.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "agentNotifications",
                "appearance",
                "initialized",
                "lastAnchor",
                "schemaVersion",
                "versionCheck",
                "welcomed"
            ]
        );
        assert!(!saved.to_string().contains("PRIVATE_"));
    }
}
