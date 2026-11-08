//! Windows implementations; no Linux filesystem or Secret Service emulation.
mod codex_sessions;
mod credentials;
mod gpu;
mod hooks;
mod network;
mod notifications;
mod protocol;
mod session;
mod startup;
mod system;
pub use codex_sessions::CodexRollouts;
pub use credentials::NativeKeyring;
pub use hooks::endpoint as hook_endpoint;
pub use hooks::{NativeHookSocket, TimedStdin};
pub use network::{WindowsNetwork, WindowsPing};
pub use startup::startup_storage;
pub use system::SystemReadings;
pub fn agent_notifications(app: tauri::AppHandle) -> impl crate::agents::notifications::Sink {
    notifications::Desktop::new(app)
}
use crate::ports::*;
use std::{io, path::Path};
use windows::core::PCWSTR;

pub fn space(path: &Path) -> io::Result<Space> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    let (mut available, mut total, mut free) = (0, 0, 0);
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(path.as_ptr()),
            Some(&mut available),
            Some(&mut total),
            Some(&mut free),
        )
    }
    .map_err(|_| io::Error::other("capacity unavailable"))?;
    Ok(Space {
        total,
        free,
        available,
    })
}

pub fn open_external(args: &[&str]) -> Result<String, ()> {
    use windows::Win32::{
        Foundation::HWND,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let [target] = args else { return Err(()) };
    if target.contains('\0') {
        return Err(());
    }
    let target: Vec<_> = target.encode_utf16().chain([0]).collect();
    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            windows::core::w!("open"),
            PCWSTR(target.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as usize > 32 {
        Ok(String::new())
    } else {
        Err(())
    }
}

pub struct SystemPrograms;
impl Programs for SystemPrograms {
    fn installed(&self, name: &str) -> bool {
        find_program(name).is_some()
    }
}

pub fn find_program(name: &str) -> Option<std::path::PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .flat_map(|dir| {
            [".exe", ".cmd", ".bat"].map(move |extension| dir.join(format!("{name}{extension}")))
        })
        .find(|path| path.is_file())
}

/// Polling fallback: the collector owns its cadence and rereads through Files.
pub struct FileWatch;
impl FileWatch {
    pub fn new(_: &Path) -> Self {
        Self
    }
}
impl Watch for FileWatch {
    fn changed(&mut self) -> bool {
        true
    }
}

pub struct PortalSettings;
impl DesktopSettings for PortalSettings {
    fn follow(&self, publish: &mut dyn FnMut(crate::desktop_theme::SystemTheme)) -> Result<(), ()> {
        use crate::desktop_theme::SystemTheme;
        let key = windows_registry::CURRENT_USER
            .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .map_err(|_| ())?;
        let mut previous = None;
        loop {
            let theme = if key.get_u32("AppsUseLightTheme").map_err(|_| ())? == 0 {
                SystemTheme::Dark
            } else {
                SystemTheme::Light
            };
            if previous != Some(theme) {
                publish(theme);
                previous = Some(theme);
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

pub struct WindowsConnectivity;
impl crate::network::NetworkManager for WindowsConnectivity {
    fn connection(&self) -> io::Result<crate::network::Connection> {
        let online = network::with_network_manager(|manager| {
            Ok(unsafe { manager.IsConnectedToInternet() }?.as_bool())
        })?;
        Ok(crate::network::Connection {
            connectivity: if online { 4 } else { 1 },
            ..Default::default()
        })
    }
}
pub fn connectivity() -> WindowsConnectivity {
    WindowsConnectivity
}

pub use session::session;
