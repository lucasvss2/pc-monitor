//! Windows notification-area icon. All callbacks run on Tauri's event loop.
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub fn start(app: &tauri::AppHandle) {
    let result = (|| -> tauri::Result<()> {
        let open = MenuItem::with_id(app, "open", "Abrir Painel", true, None::<&str>)?;
        let exit = MenuItem::with_id(app, "exit", "Sair", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &exit])?;
        let pixels = crate::icon::draw(Default::default(), 32);
        let rgba: Vec<_> = pixels
            .argb
            .chunks_exact(4)
            .flat_map(|p| [p[1], p[2], p[3], p[0]])
            .collect();
        let tray = TrayIconBuilder::with_id("monitor")
            .tooltip("kvojps-monitor")
            .icon(tauri::image::Image::new_owned(rgba, 32, 32))
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "open" => crate::panel::open(app),
                "exit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                ) {
                    crate::panel::open(tray.app_handle());
                }
            })
            .build(app)?;
        app.manage(tray);
        Ok(())
    })();
    if result.is_err() {
        eprintln!("indicator: registration failed");
    }
}
