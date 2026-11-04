//! Module collectors. Notify collectors that depend on visibility when it changes.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::module::Module;
use crate::state::AppState;
use crate::system;

pub fn start(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    app.manage(system::Collector::start(app.clone()));
    #[cfg(target_os = "linux")]
    crate::network::start(app.clone());
    crate::lifecycle::start(app);
    crate::github::start(app.clone());
    crate::tasks::start(app.clone());
    crate::ai::start(app.clone());
    crate::agents::start(app.clone());
}

/// Whether module intersects the visible panel area.
pub fn is_visible(app: &AppHandle, module: Module) -> bool {
    app.state::<Mutex<AppState>>()
        .lock()
        .unwrap()
        .is_visible(module)
}

/// Call after the set of visible modules changes. Remote collectors read the
/// set on their own cadence; local collectors are told directly.
pub fn visibility_changed(app: &AppHandle) {
    let (system, network) = {
        let state = app.state::<Mutex<AppState>>();
        let state = state.lock().unwrap();
        (
            state.is_visible(Module::System),
            state.is_visible(Module::Network),
        )
    };
    if let Some(collector) = app.try_state::<system::Collector>() {
        collector.report(system);
    }
    if let Some(collector) = app.try_state::<crate::network::Collector>() {
        let _ = collector.0.send(crate::network::Command::Visible(network));
    }
}
