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

use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalSize, WebviewWindow, WebviewWindowBuilder,
};

use crate::collection;
use crate::state::AppState;

pub const LABEL: &str = "panel";

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
    app.state::<Mutex<AppState>>().lock().unwrap().open_panel();
    collection::visibility_changed(app);
    if let Some(window) = app.get_webview_window(LABEL) {
        set_activation_token(&window, token);
        fit(&window);
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
        let size = work_area_size(&monitor, margin).to_logical::<f64>(monitor.scale_factor());
        builder = builder.inner_size(size.width, size.height);
    }
    match builder.build() {
        Ok(window) => {
            set_activation_token(&window, token);
            learn_margin(&window);
            fit(&window);
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
fn fit(window: &WebviewWindow) {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let margin = window.state::<Margin>().get();
        let _ = window.set_size(work_area_size(&monitor, margin));
    }
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
        let asked = work_area_size(&monitor, (0.0, 0.0)).to_logical::<f64>(scale);
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
            let _ = window.set_size(work_area_size(&monitor, margin));
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

fn work_area_size(monitor: &Monitor, margin: (f64, f64)) -> PhysicalSize<u32> {
    let area = monitor.work_area().size;
    let (width, height) = panel_size(area.width, area.height, monitor.scale_factor(), margin);
    PhysicalSize::new(width, height)
}

/// Convert the logical target plus the window margin to physical pixels before
/// clamping to the work area.
fn panel_size(width: u32, height: u32, scale: f64, margin: (f64, f64)) -> (u32, u32) {
    (
        width.min(((1100.0 + margin.0) * scale).round() as u32),
        height.min(((700.0 + margin.1) * scale).round() as u32),
    )
}

pub fn close(app: &AppHandle) {
    app.state::<Mutex<AppState>>().lock().unwrap().close_panel();
    collection::visibility_changed(app);
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.emit("panel-visibility", false);
        let _ = window.destroy();
    }
}

/// Notification clicks override the remembered module, also in an already open panel.
pub fn open_agent(app: &AppHandle, session: String) {
    use tauri::Emitter;
    app.state::<Mutex<AppState>>()
        .lock()
        .unwrap()
        .target_agent(session.clone());
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
    fn panel_targets_logical_size_and_fits_scaled_work_area() {
        let none = (0.0, 0.0);
        assert_eq!(panel_size(1920, 1048, 1.0, none), (1100, 700));
        assert_eq!(panel_size(2880, 1736, 2.0, none), (2200, 1400));
        assert_eq!(panel_size(1920, 1048, 1.5, none), (1650, 1048));
        assert_eq!(panel_size(800, 600, 1.0, none), (800, 600));
        assert_eq!(panel_size(1600, 1200, 2.0, none), (1600, 1200));
    }

    #[test]
    fn window_margin_is_asked_on_top_of_the_visible_target_within_the_work_area() {
        // Wayland, 1920 × 1080 at scale 1: tao's decorations take 52 px per axis.
        assert_eq!(panel_size(1920, 1048, 1.0, (52.0, 52.0)), (1152, 752));
        assert_eq!(panel_size(2880, 1736, 2.0, (52.0, 52.0)), (2304, 1504));
        assert_eq!(panel_size(1100, 720, 1.0, (52.0, 52.0)), (1100, 720));
    }

    #[test]
    fn margin_is_learned_only_from_a_plausible_allocation() {
        assert_eq!(margin((1100.0, 700.0), (1048.0, 648.0)), Some((52.0, 52.0)));
        assert_eq!(margin((1100.0, 700.0), (1100.0, 700.0)), Some((0.0, 0.0)));
        assert_eq!(margin((1100.0, 700.0), (1.0, 1.0)), None);
        assert_eq!(margin((1100.0, 700.0), (1200.0, 700.0)), None);
    }
}
