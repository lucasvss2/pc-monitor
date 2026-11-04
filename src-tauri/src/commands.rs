//! UI commands sent to the core.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::collection;
use crate::module::Module;
use crate::panel;
use crate::snapshot::{Snapshot, Snapshots};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialState {
    unavailable_modules: Vec<Module>,
    panel_visible: bool,
    last_anchor: Module,
    /// First opening: Settings shows the welcome.
    welcome: bool,
    appearance: crate::state::Appearance,
    /// Desktop color scheme; None when the portal has not answered.
    system_theme: Option<crate::desktop_theme::SystemTheme>,
    agent_target: Option<String>,
    snapshots: Vec<Snapshot>,
}

#[tauri::command]
pub fn initial_state(
    app: AppHandle,
    state: State<Mutex<AppState>>,
    snapshots: State<Snapshots>,
    theme: State<crate::desktop_theme::Current>,
) -> InitialState {
    let state = state.lock().unwrap();
    InitialState {
        unavailable_modules: if cfg!(windows) {
            vec![Module::Ai, Module::Network, Module::System]
        } else {
            vec![]
        },
        panel_visible: {
            use tauri::Manager;
            app.get_webview_window(panel::LABEL)
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false)
        },
        welcome: state.welcome_pending(),
        appearance: state.appearance(),
        system_theme: theme.get(),
        agent_target: state.agent_target(),
        last_anchor: state.opening_anchor(),
        snapshots: snapshots.all(),
    }
}

/// Modules intersecting the visible panel area; empty in Settings.
#[tauri::command]
pub fn report_visible_modules(modules: Vec<Module>, state: State<Mutex<AppState>>, app: AppHandle) {
    let changed = state.lock().unwrap().report_visible(&modules);
    if changed {
        collection::visibility_changed(&app);
    }
}

/// The last anchor chosen in the navigation, restored on the next opening.
#[tauri::command]
pub fn choose_anchor(module: Module, state: State<Mutex<AppState>>) {
    state.lock().unwrap().choose_anchor(module);
}

#[tauri::command]
pub fn close_panel(app: AppHandle) {
    panel::close(&app);
}

#[tauri::command]
pub fn refresh_github(collector: State<crate::github::Collector>) {
    let _ = collector.0.send(crate::github::Command::Refresh);
}

#[tauri::command]
pub fn refresh_ai(collector: State<crate::ai::Collector>) {
    let _ = collector.0.send(crate::ai::Command::Refresh);
}

/// Only on the user's click: changes `~/.claude/settings.json`.
#[tauri::command]
pub async fn connect_claude(
    collector: State<'_, crate::ai::Collector>,
) -> Result<(), crate::ai::ClaudeError> {
    ask_ai(collector, crate::ai::Command::ConnectClaude).await
}

#[tauri::command]
pub async fn disconnect_claude(
    collector: State<'_, crate::ai::Collector>,
) -> Result<(), crate::ai::ClaudeError> {
    ask_ai(collector, crate::ai::Command::DisconnectClaude).await
}

/// Only on the user's click, after the warning about the undocumented endpoint.
#[tauri::command]
pub async fn set_claude_usage(
    enabled: bool,
    collector: State<'_, crate::ai::Collector>,
) -> Result<(), crate::ai::ClaudeError> {
    ask_ai(collector, |reply| {
        crate::ai::Command::SetClaudeUsage(enabled, reply)
    })
    .await
}

async fn ask_ai(
    collector: State<'_, crate::ai::Collector>,
    command: impl FnOnce(
        tokio::sync::oneshot::Sender<Result<(), crate::ai::ClaudeError>>,
    ) -> crate::ai::Command,
) -> Result<(), crate::ai::ClaudeError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    collector
        .0
        .send(command(tx))
        .map_err(|_| crate::ai::ClaudeError::Unwritable)?;
    rx.await.map_err(|_| crate::ai::ClaudeError::Unwritable)?
}

#[tauri::command]
pub fn run_speed_test(collector: State<crate::network::Collector>) {
    let _ = collector.0.send(crate::network::Command::FullSpeed);
}
#[tauri::command]
pub fn set_automatic_speed(enabled: bool, collector: State<crate::network::Collector>) {
    let _ = collector
        .0
        .send(crate::network::Command::AutomaticSpeed(enabled));
}

#[tauri::command]
pub async fn save_github_token(
    token: String,
    collector: State<'_, crate::github::Collector>,
) -> Result<String, crate::github::CredentialError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    collector
        .0
        .send(crate::github::Command::SaveToken(token, tx))
        .map_err(|_| crate::github::CredentialError::KeyringUnavailable)?;
    rx.await
        .map_err(|_| crate::github::CredentialError::KeyringUnavailable)?
}

#[tauri::command]
pub fn refresh_trello(collector: State<crate::trello::Collector>) {
    let _ = collector.0.send(crate::trello::Command::Refresh);
}

/// Send a command with a reply channel to the Trello collector thread.
async fn ask_trello<T>(
    collector: State<'_, crate::trello::Collector>,
    command: impl FnOnce(
        tokio::sync::oneshot::Sender<Result<T, crate::trello::CredentialError>>,
    ) -> crate::trello::Command,
) -> Result<T, crate::trello::CredentialError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    collector
        .0
        .send(command(tx))
        .map_err(|_| crate::trello::CredentialError::KeyringUnavailable)?;
    rx.await
        .map_err(|_| crate::trello::CredentialError::KeyringUnavailable)?
}

#[tauri::command]
pub async fn connect_trello(
    key: String,
    token: String,
    collector: State<'_, crate::trello::Collector>,
) -> Result<String, crate::trello::CredentialError> {
    ask_trello(collector, |reply| {
        crate::trello::Command::Connect(key, token, reply)
    })
    .await
}

#[tauri::command]
pub async fn select_trello_board(
    board: String,
    collector: State<'_, crate::trello::Collector>,
) -> Result<(), crate::trello::CredentialError> {
    ask_trello(collector, |reply| {
        crate::trello::Command::SelectBoard(board, reply)
    })
    .await
}

#[tauri::command]
pub async fn disconnect_trello(
    collector: State<'_, crate::trello::Collector>,
) -> Result<(), crate::trello::CredentialError> {
    ask_trello(collector, crate::trello::Command::Disconnect).await
}

/// The collector remembers the key, since the browser closes the panel.
#[tauri::command]
pub async fn open_trello_authorization(
    key: String,
    collector: State<'_, crate::trello::Collector>,
) -> Result<(), crate::trello::CredentialError> {
    ask_trello(collector, |reply| {
        crate::trello::Command::Authorize(key, reply)
    })
    .await
}

/// Rejects when Trello refuses or cannot be reached; the panel then undoes its
/// optimistic move.
#[tauri::command]
pub async fn move_trello_card(
    card: String,
    list: String,
    collector: State<'_, crate::trello::Collector>,
) -> Result<(), crate::trello::MoveError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    collector
        .0
        .send(crate::trello::Command::Move(card, list, tx))
        .map_err(|_| crate::trello::MoveError)?;
    rx.await.map_err(|_| crate::trello::MoveError)?
}

// Async commands run off the main thread while xdg-open hands the URL over.
#[tauri::command]
pub async fn open_trello_card(url: String) {
    crate::trello::open_card(&crate::ports::SystemProcesses, &url);
}

#[tauri::command]
pub async fn open_trello_admin() {
    crate::trello::open_admin(&crate::ports::SystemProcesses);
}

#[tauri::command]
pub fn set_agent_notifications(
    preferences: crate::agents::notifications::Preferences,
    state: State<Mutex<AppState>>,
) {
    state.lock().unwrap().set_agent_notifications(preferences);
}

/// Hook files change only in response to the explicit settings click.
#[tauri::command]
pub async fn set_agent_hooks(
    enabled: bool,
    collector: State<'_, crate::agents::Collector>,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    collector
        .0
        .send((enabled, tx))
        .map_err(|_| "collectorUnavailable".to_string())?;
    rx.await.map_err(|_| "collectorUnavailable".to_string())?
}

/// Where the autostart entry lives and what it launches.
pub struct Startup {
    pub dir: std::path::PathBuf,
    pub execution: crate::autostart::Execution,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct General {
    version: &'static str,
    autostart: bool,
    version_check: bool,
    release: Option<crate::releases::Release>,
}

pub(crate) fn general(state: &AppState, startup: &Startup) -> General {
    General {
        version: env!("CARGO_PKG_VERSION"),
        autostart: crate::autostart::enabled(&crate::platform::startup_storage(
            startup.dir.clone(),
        )),
        version_check: state.version_check(),
        release: state.available_release(),
    }
}

#[tauri::command]
pub fn general_settings(state: State<Mutex<AppState>>, startup: State<Startup>) -> General {
    general(&state.lock().unwrap(), &startup)
}

/// Rejects with "unwritable" when the autostart entry cannot change.
#[tauri::command]
pub fn set_autostart(
    enabled: bool,
    state: State<Mutex<AppState>>,
    startup: State<Startup>,
) -> Result<General, String> {
    crate::autostart::set(
        &crate::platform::startup_storage(startup.dir.clone()),
        &startup.execution,
        enabled,
    )
    .map_err(|_| "unwritable".to_string())?;
    Ok(general(&state.lock().unwrap(), &startup))
}

#[tauri::command]
pub fn set_version_check(
    enabled: bool,
    state: State<Mutex<AppState>>,
    startup: State<Startup>,
) -> Result<General, ()> {
    let mut state = state.lock().unwrap();
    state.set_version_check(enabled).map_err(|_| ())?;
    Ok(general(&state, &startup))
}

#[tauri::command]
pub async fn open_release(state: State<'_, Mutex<AppState>>) -> Result<(), ()> {
    let release = state.lock().unwrap().available_release();
    crate::general::open_release(&crate::ports::SystemProcesses, release.as_ref())
        .then_some(())
        .ok_or(())
}

#[tauri::command]
pub async fn open_logs() -> Result<(), ()> {
    let dir = dirs::state_dir().ok_or(())?.join("kvojps-monitor");
    crate::general::open_logs(
        &crate::ports::RootFilesystem::system(),
        &crate::ports::SystemProcesses,
        &dir,
    )
    .then_some(())
    .ok_or(())
}

#[tauri::command]
pub fn set_appearance(
    appearance: crate::state::Appearance,
    state: State<Mutex<AppState>>,
) -> Result<(), ()> {
    state
        .lock()
        .unwrap()
        .set_appearance(appearance)
        .map_err(|_| ())
}

#[tauri::command]
pub fn complete_welcome(state: State<Mutex<AppState>>) {
    state.lock().unwrap().complete_welcome();
}
