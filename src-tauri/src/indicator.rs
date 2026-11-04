//! StatusNotifierItem indicator through ksni, without a GNOME Shell extension.
//! AppIndicator opens the menu on a single click; double click invokes activate and
//! middle click invokes secondary_activate (ADR 0001).

use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, ToolTip, Tray};
use tauri::AppHandle;

use crate::icon::{self, IndicatorState};
use crate::panel;
use crate::strings::STRINGS;

/// Sizes offered to the host, which chooses the closest match for its bar.
const ICON_SIZES: [i32; 3] = [22, 32, 44];

pub fn start(app: &AppHandle) {
    use ksni::TrayMethods;
    use tauri::Manager;
    let indicator = Indicator {
        app: app.clone(),
        state: Default::default(),
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match indicator.spawn().await {
            Ok(handle) => {
                app.manage(handle);
            }
            Err(_) => eprintln!("indicator: registration failed"),
        }
    });
}

pub struct Indicator {
    pub app: AppHandle,
    pub state: IndicatorState,
}

impl Indicator {
    fn open_panel(&self) {
        // Read inside the ksni callback: its activation token is event-local.
        // GTK must receive it on the UI thread, before the window is presented.
        let token = ksni::current_xdg_activation_token();
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            panel::open_with_activation_token(&app, token.as_deref());
        });
    }
}

impl Tray for Indicator {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn title(&self) -> String {
        STRINGS.indicator.title.clone()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: STRINGS.indicator.title.clone(),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        ICON_SIZES
            .iter()
            .map(|&side| {
                let p = icon::draw(self.state, side);
                Icon {
                    width: p.side,
                    height: p.side,
                    data: p.argb,
                }
            })
            .collect()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.open_panel();
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.open_panel();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![StandardItem {
            label: STRINGS.indicator.open_panel.clone(),
            activate: Box::new(|this: &mut Self| this.open_panel()),
            ..Default::default()
        }
        .into()]
    }
}
