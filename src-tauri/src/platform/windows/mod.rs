//! Windows implementations; no Linux filesystem or Secret Service emulation.
mod credentials;
mod hooks;
mod notifications;
mod startup;
pub use credentials::NativeKeyring;
pub use hooks::endpoint as hook_endpoint;
pub use hooks::{NativeHookSocket, TimedStdin};
pub use startup::startup_storage;
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
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                ["", ".exe", ".cmd", ".bat"]
                    .iter()
                    .any(|extension| dir.join(format!("{name}{extension}")).is_file())
            })
        })
    }
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
    fn follow(&self, _: &mut dyn FnMut(crate::desktop_theme::SystemTheme)) -> Result<(), ()> {
        Err(())
    }
}

pub struct WindowsConnectivity;
impl WindowsConnectivity {
    pub fn connect() -> Self {
        Self
    }
}
impl crate::network::NetworkManager for WindowsConnectivity {
    fn connection(&self) -> io::Result<crate::network::Connection> {
        let mut flags = 0;
        let online = unsafe {
            windows::Win32::Networking::WinInet::InternetGetConnectedState(&mut flags, 0)
        }
        .as_bool();
        Ok(crate::network::Connection {
            connectivity: if online { 4 } else { 1 },
            ..Default::default()
        })
    }
}
pub fn connectivity() -> WindowsConnectivity {
    WindowsConnectivity
}

pub struct WindowsSession;
impl crate::lifecycle::Session for WindowsSession {
    fn preparing_for_sleep(&self) -> Option<bool> {
        None
    }
}
pub fn session() -> WindowsSession {
    WindowsSession
}
