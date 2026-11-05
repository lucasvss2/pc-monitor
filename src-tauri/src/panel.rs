//! Open and close the panel. Keeping a hidden webview exceeded 150 MB PSS,
//! so create the window on open and destroy it on close.
//!
//! The panel targets a visible area of 1100 × 700 logical pixels, bounded by the monitor
//! work area. Mutter places the window (ADR 0001): the app only chooses its size.
//!
//! On Wayland, tao draws client-side decorations for undecorated windows, and their
//! margins take 52 logical pixels per axis from the size asked for. The panel learns
//! that margin from the webview's first allocation and asks for it on top.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalSize, WebviewWindow, WebviewWindowBuilder,
};

use crate::collection;
use crate::state::AppState;

pub const LABEL: &str = "panel";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelMode {
    #[default]
    Full,
    Compact,
}

#[derive(Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelPreferences {
    pub mode: PanelMode,
    pub pinned: bool,
    pub full_size: Option<[u32; 2]>,
    pub compact_size: Option<[u32; 2]>,
    pub position: Option<[i32; 2]>,
}

impl PanelPreferences {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    fn size(self, mode: PanelMode) -> [u32; 2] {
        let (saved, default) = match mode {
            PanelMode::Full => (self.full_size, [1100, 700]),
            PanelMode::Compact => (self.compact_size, [380, 620]),
        };
        let [w, h] = saved.unwrap_or(default);
        [
            valid_dimension(w, 320, default[0]),
            valid_dimension(h, 280, default[1]),
        ]
    }
}

fn valid_dimension(value: u32, minimum: u32, default: u32) -> u32 {
    if (1..=10_000).contains(&value) {
        value.max(minimum)
    } else {
        default
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presentation {
    pub mode: PanelMode,
    pub preferred_mode: PanelMode,
    pub pinned: bool,
    pub pin_available: bool,
    pub pin_failed: bool,
}

/// Native events share the same close path as the explicit UI command.
pub fn handle_window_event(event: &tauri::WindowEvent, close: impl FnOnce()) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        close();
    }
}

/// Logical pixels the window takes from the size asked for; learned once per process.
#[derive(Default)]
pub struct Margin(Mutex<Option<(f64, f64)>>);

impl Margin {
    fn get(&self) -> (f64, f64) {
        self.0.lock().unwrap().unwrap_or((0.0, 0.0))
    }
}

pub fn open(app: &AppHandle) {
    open_with_activation_token(app, None);
}

/// Indicator activation runs on the GTK thread so the compositor's token can be
/// assigned before showing or focusing either a new or an existing window.
pub fn open_with_activation_token(app: &AppHandle, token: Option<&str>) {
    let (preferences, mode) = {
        let state = app.state::<Mutex<AppState>>();
        let mut state = state.lock().unwrap();
        state.open_panel();
        (state.panel_preferences(), state.presentation().mode)
    };
    collection::visibility_changed(app);
    if let Some(window) = app.get_webview_window(LABEL) {
        set_activation_token(&window, token);
        #[cfg(windows)]
        recover_work_area(&window);
        // A repeated request during page loading must not expose an empty webview.
        if window.is_visible().unwrap_or(false) {
            let _ = window.show();
            let _ = window.emit("panel-visibility", true);
            let _ = window.set_focus();
        }
        return;
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|j| j.label == LABEL)
        .expect("panel window missing from tauri.conf.json");
    let mut builder = match WebviewWindowBuilder::from_config(app, config) {
        Ok(builder) => builder.on_page_load(|window, payload| {
            // The first visible frame already contains the opening composition.
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = window.show();
                let _ = window.emit("panel-visibility", true);
                let _ = window.set_focus();
            }
        }),
        Err(e) => return eprintln!("panel: failed to create window: {e}"),
    };
    #[cfg(feature = "fixtures")]
    {
        builder = builder.initialization_script(crate::fixtures::CLOCK_SCRIPT);
    }
    // Start with the primary monitor size; fit() corrects it once the compositor
    // has chosen the monitor.
    if let Some(monitor) = app.primary_monitor().ok().flatten() {
        let margin = app.state::<Margin>().get();
        let size = work_area_size(&monitor, margin, preferences.size(mode))
            .to_logical::<f64>(monitor.scale_factor());
        builder = builder
            .inner_size(size.width, size.height)
            .min_inner_size(size.width.min(320.0), size.height.min(280.0));
    }
    match builder.build() {
        Ok(window) => {
            set_activation_token(&window, token);
            learn_margin(&window);
            let _ = fit(&window, preferences, mode);
            #[cfg(windows)]
            let actual = apply_pin(&window, preferences.pinned);
            #[cfg(not(windows))]
            let actual: Result<(), &'static str> = Ok(());
            app.state::<Mutex<AppState>>().lock().unwrap().apply_pin(
                cfg!(windows) && preferences.pinned && actual.is_ok(),
                actual.is_err(),
            );
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("panel: failed to create window: {e}"),
    }
}

#[cfg(target_os = "linux")]
fn set_activation_token(window: &WebviewWindow, token: Option<&str>) {
    use gtk::prelude::*;
    if let Some(token) = token {
        if let Ok(native) = window.gtk_window() {
            native.set_startup_id(token);
        }
    }
}

/// Resize to the work area of the monitor the window is on.
fn fit(window: &WebviewWindow, prefs: PanelPreferences, mode: PanelMode) -> tauri::Result<()> {
    #[cfg(windows)]
    let position = prefs
        .position
        .or_else(|| window.outer_position().ok().map(|p| [p.x, p.y]));
    #[cfg(windows)]
    let saved_monitor = position.and_then(|[x, y]| {
        window
            .monitor_from_point(f64::from(x), f64::from(y))
            .ok()
            .flatten()
    });
    #[cfg(not(windows))]
    let saved_monitor = None;
    let monitor = saved_monitor
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let margin = window.state::<Margin>().get();
        let size = work_area_size(&monitor, margin, prefs.size(mode));
        #[cfg(windows)]
        if let Some(position) = position {
            let area = monitor.work_area();
            let [x, y] = clamp_position(
                position,
                [area.position.x, area.position.y],
                [area.size.width, area.size.height],
                [size.width, size.height],
            );
            window.set_position(tauri::PhysicalPosition::new(x, y))?;
        }
        window.set_size(size)?;
    } else {
        let [width, height] = prefs.size(mode);
        window.set_size(tauri::LogicalSize::new(width, height))?;
    }
    Ok(())
}

#[cfg(windows)]
pub fn recover_work_area(window: &WebviewWindow) {
    if window.is_maximized().unwrap_or(false) || window.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    else {
        return;
    };
    let area = monitor.work_area();
    let fitted = PhysicalSize::new(
        size.width.min(area.size.width),
        size.height.min(area.size.height),
    );
    if fitted != size {
        let _ = window.set_size(fitted);
    }
    let [x, y] = clamp_position(
        [position.x, position.y],
        [area.position.x, area.position.y],
        [area.size.width, area.size.height],
        [fitted.width, fitted.height],
    );
    if [x, y] != [position.x, position.y] {
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

#[cfg(any(windows, test))]
fn clamp_position(
    position: [i32; 2],
    origin: [i32; 2],
    area: [u32; 2],
    size: [u32; 2],
) -> [i32; 2] {
    std::array::from_fn(|axis| {
        let upper = i64::from(origin[axis]) + i64::from(area[axis].saturating_sub(size[axis]));
        i64::from(position[axis])
            .clamp(i64::from(origin[axis]), upper)
            .min(i64::from(i32::MAX)) as i32
    })
}

pub fn remember_geometry(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    if window.is_maximized().unwrap_or(false) || window.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) else {
        return;
    };
    let logical = size.to_logical::<u32>(scale);
    if logical.width < 100 || logical.height < 100 {
        return;
    }
    #[cfg(windows)]
    let position = window.outer_position().ok().map(|p| [p.x, p.y]);
    #[cfg(not(windows))]
    let position = None;
    app.state::<Mutex<AppState>>()
        .lock()
        .unwrap()
        .remember_panel_geometry([logical.width, logical.height], position);
}

pub fn set_mode(
    app: &AppHandle,
    mode: PanelMode,
    remember: bool,
) -> Result<Presentation, &'static str> {
    let window = app.get_webview_window(LABEL).ok_or("window")?;
    let state = app.state::<Mutex<AppState>>();
    let previous = state.lock().unwrap().presentation().mode;
    if previous != mode {
        remember_geometry(app);
        let preferences = state.lock().unwrap().panel_preferences();
        if window.is_maximized().unwrap_or(false) {
            window.unmaximize().map_err(|_| "window")?;
        }
        fit(&window, preferences, mode).map_err(|_| "window")?;
    }
    let saved = state.lock().unwrap().present_mode(mode, remember);
    if saved.is_err() {
        if previous != mode {
            let preferences = state.lock().unwrap().panel_preferences();
            let _ = fit(&window, preferences, previous);
        }
        return Err("save");
    }
    let result = state.lock().unwrap().presentation();
    Ok(result)
}

#[cfg(windows)]
fn apply_pin(window: &WebviewWindow, pinned: bool) -> Result<(), &'static str> {
    window.set_always_on_top(pinned).map_err(|_| "window")?;
    (window.is_always_on_top().map_err(|_| "window")? == pinned)
        .then_some(())
        .ok_or("window")
}

#[cfg(windows)]
pub fn set_pinned(app: &AppHandle, pinned: bool) -> Result<Presentation, &'static str> {
    let window = app.get_webview_window(LABEL).ok_or("window")?;
    let state = app.state::<Mutex<AppState>>();
    let previous = state.lock().unwrap().presentation().pinned;
    apply_pin(&window, pinned)?;
    let saved = state.lock().unwrap().remember_pin(pinned);
    if saved.is_err() {
        if apply_pin(&window, previous).is_err() {
            let actual = window.is_always_on_top().unwrap_or(false);
            state.lock().unwrap().apply_pin(actual, true);
        }
        return Err("save");
    }
    let result = state.lock().unwrap().presentation();
    Ok(result)
}

/// Once the content allocation settles on the window size GTK chose, compare it with
/// the size asked for and, if the window took a margin, remember it and ask again.
#[cfg(target_os = "linux")]
fn learn_margin(window: &WebviewWindow) {
    use gtk::prelude::*;
    if window.state::<Margin>().0.lock().unwrap().is_some() {
        return;
    }
    let Ok(content) = window.default_vbox() else {
        return;
    };
    let window = window.clone();
    content.connect_size_allocate(move |_, allocation| {
        // Never hold the lock across Tauri or GTK calls: they can allocate again and
        // re-enter this callback on the same thread.
        if window.state::<Margin>().0.lock().unwrap().is_some() {
            return;
        }
        let Some(monitor) = window.current_monitor().ok().flatten() else {
            return;
        };
        let scale = monitor.scale_factor();
        let state = window.state::<Mutex<AppState>>();
        let logical = {
            let state = state.lock().unwrap();
            state.panel_preferences().size(state.presentation().mode)
        };
        let asked = work_area_size(&monitor, (0.0, 0.0), logical).to_logical::<f64>(scale);
        let visible = (allocation.width(), allocation.height());
        // The first allocation still holds the size asked for; the real one matches
        // the window size GTK settled on.
        if window.gtk_window().map(|w| w.size()).ok() != Some(visible) {
            return;
        }
        let visible = (f64::from(visible.0), f64::from(visible.1));
        let Some(margin) = margin((asked.width, asked.height), visible) else {
            return;
        };
        let first = window
            .state::<Margin>()
            .0
            .lock()
            .unwrap()
            .get_or_insert(margin)
            == &margin;
        if first && margin != (0.0, 0.0) {
            let _ = window.set_size(work_area_size(&monitor, margin, logical));
        }
    });
}

/// Margin between the logical size asked for and the visible area obtained. None for
/// allocations that are not the real one yet (tiny, or larger than asked for).
#[cfg(any(test, target_os = "linux"))]
fn margin(asked: (f64, f64), visible: (f64, f64)) -> Option<(f64, f64)> {
    const MAX: f64 = 200.0;
    let taken = (asked.0 - visible.0, asked.1 - visible.1);
    let plausible = |t: f64| (0.0..=MAX).contains(&t);
    (plausible(taken.0) && plausible(taken.1)).then_some(taken)
}

fn work_area_size(monitor: &Monitor, margin: (f64, f64), logical: [u32; 2]) -> PhysicalSize<u32> {
    let area = monitor.work_area().size;
    let (width, height) = panel_size(
        area.width,
        area.height,
        monitor.scale_factor(),
        margin,
        logical,
    );
    PhysicalSize::new(width, height)
}

/// Convert the logical target plus the window margin to physical pixels before
/// clamping to the work area.
fn panel_size(
    width: u32,
    height: u32,
    scale: f64,
    margin: (f64, f64),
    logical: [u32; 2],
) -> (u32, u32) {
    (
        width.min(((f64::from(logical[0]) + margin.0) * scale).round() as u32),
        height.min(((f64::from(logical[1]) + margin.1) * scale).round() as u32),
    )
}

pub fn close(app: &AppHandle) {
    remember_geometry(app);
    app.state::<Mutex<AppState>>().lock().unwrap().close_panel();
    collection::visibility_changed(app);
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.emit("panel-visibility", false);
        let _ = window.destroy();
    }
}

/// Notification clicks present Agents without changing the saved anchor.
/// An empty session targets the module without a particular session identity.
pub fn open_agent(app: &AppHandle, session: String) {
    use tauri::Emitter;
    app.state::<Mutex<AppState>>()
        .lock()
        .unwrap()
        .target_agent(session.clone());
    if app.get_webview_window(LABEL).is_some() {
        let _ = set_mode(app, PanelMode::Full, false);
    }
    open(app);
    let _ = app.emit_to(LABEL, "agent-target", session);
}

#[cfg(windows)]
fn set_activation_token(_: &WebviewWindow, _: Option<&str>) {}
#[cfg(windows)]
fn learn_margin(_: &WebviewWindow) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn losing_focus_does_not_close_the_panel_or_discard_a_connection_draft() {
        let closed = std::cell::Cell::new(false);
        handle_window_event(&tauri::WindowEvent::Focused(false), || closed.set(true));
        assert!(
            !closed.get(),
            "switching to another app must keep the panel open"
        );
    }

    #[test]
    fn panel_targets_logical_size_and_fits_scaled_work_area() {
        let none = (0.0, 0.0);
        assert_eq!(panel_size(1920, 1048, 1.0, none, [1100, 700]), (1100, 700));
        assert_eq!(panel_size(2880, 1736, 2.0, none, [1100, 700]), (2200, 1400));
        assert_eq!(panel_size(1920, 1048, 1.5, none, [1100, 700]), (1650, 1048));
        assert_eq!(panel_size(800, 600, 1.0, none, [1100, 700]), (800, 600));
        assert_eq!(panel_size(1600, 1200, 2.0, none, [1100, 700]), (1600, 1200));
    }

    #[test]
    fn compact_geometry_is_independent_and_recovers_from_a_missing_monitor() {
        let preferences = PanelPreferences {
            full_size: Some([900, 610]),
            ..Default::default()
        };
        assert_eq!(preferences.size(PanelMode::Full), [900, 610]);
        assert_eq!(preferences.size(PanelMode::Compact), [380, 620]);
        let compact = panel_size(
            1440,
            900,
            1.5,
            (0.0, 0.0),
            preferences.size(PanelMode::Compact),
        );
        assert_eq!(compact, (570, 900));
        assert_eq!(
            clamp_position([-2000, 1000], [0, 0], [1440, 900], [compact.0, compact.1]),
            [0, 0]
        );
    }

    #[test]
    fn window_margin_is_asked_on_top_of_the_visible_target_within_the_work_area() {
        // Wayland, 1920 × 1080 at scale 1: tao's decorations take 52 px per axis.
        assert_eq!(
            panel_size(1920, 1048, 1.0, (52.0, 52.0), [1100, 700]),
            (1152, 752)
        );
        assert_eq!(
            panel_size(2880, 1736, 2.0, (52.0, 52.0), [1100, 700]),
            (2304, 1504)
        );
        assert_eq!(
            panel_size(1100, 720, 1.0, (52.0, 52.0), [1100, 700]),
            (1100, 720)
        );
    }

    #[test]
    fn margin_is_learned_only_from_a_plausible_allocation() {
        assert_eq!(margin((1100.0, 700.0), (1048.0, 648.0)), Some((52.0, 52.0)));
        assert_eq!(margin((1100.0, 700.0), (1100.0, 700.0)), Some((0.0, 0.0)));
        assert_eq!(margin((1100.0, 700.0), (1.0, 1.0)), None);
        assert_eq!(margin((1100.0, 700.0), (1200.0, 700.0)), None);
    }
}
