//! Linux implementations of the shared I/O ports.
use crate::ports::*;
use std::io;

pub fn connectivity() -> crate::network::NetworkManagerDbus {
    crate::network::NetworkManagerDbus::connect()
}
pub fn session() -> crate::lifecycle::Logind {
    crate::lifecycle::Logind::connect()
}
pub fn agent_notifications(app: tauri::AppHandle) -> impl crate::agents::notifications::Sink {
    crate::agents::desktop::Desktop::new(app)
}
pub fn startup_storage(dir: std::path::PathBuf) -> impl crate::persistence::Storage {
    crate::persistence::DiskStorage(dir)
}

pub struct DesktopNotifier;
impl Notifier for DesktopNotifier {
    fn notify(&self, _: Notification) {
        let Ok(bus) = zbus::blocking::Connection::session() else {
            return;
        };
        let Ok(proxy) = zbus::blocking::Proxy::new(
            &bus,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        ) else {
            return;
        };
        let _: zbus::Result<u32> = proxy.call(
            "Notify",
            &(
                "kvojps-monitor",
                0u32,
                "",
                "Segurança da rede: Risco",
                "A conexão apresenta sinais de risco. Consulte o Módulo Rede.",
                Vec::<String>::new(),
                std::collections::HashMap::<String, zbus::zvariant::Value>::new(),
                -1i32,
            ),
        );
    }
}

pub struct SecretService;
impl Keyring for SecretService {
    fn read(&self, account: &str) -> Result<Option<String>, ()> {
        let entry = keyring::Entry::new("kvojps-monitor", account).map_err(|_| ())?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(()),
        }
    }
    fn save(&self, account: &str, secret: &str) -> Result<(), ()> {
        keyring::Entry::new("kvojps-monitor", account)
            .map_err(|_| ())?
            .set_password(secret)
            .map_err(|_| ())
    }
    fn delete(&self, account: &str) -> Result<(), ()> {
        let entry = keyring::Entry::new("kvojps-monitor", account).map_err(|_| ())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(()),
        }
    }
}

pub struct SystemPrograms;
impl Programs for SystemPrograms {
    fn installed(&self, name: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| {
                dir.join(name)
                    .metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
    }
}

pub struct FileWatch {
    inotify: Option<rustix::fd::OwnedFd>,
    name: std::ffi::OsString,
}
impl FileWatch {
    pub fn new(path: &std::path::Path) -> Self {
        use rustix::fs::inotify;
        let dir = path.parent().unwrap_or(std::path::Path::new("/"));
        let _ = std::fs::create_dir_all(dir);
        let inotify = inotify::init(inotify::CreateFlags::CLOEXEC | inotify::CreateFlags::NONBLOCK)
            .ok()
            .filter(|fd| {
                inotify::add_watch(
                    fd,
                    dir,
                    inotify::WatchFlags::CLOSE_WRITE | inotify::WatchFlags::MOVED_TO,
                )
                .is_ok()
            });
        FileWatch {
            inotify,
            name: path.file_name().unwrap_or_default().to_owned(),
        }
    }
}
impl Watch for FileWatch {
    fn changed(&mut self) -> bool {
        use std::os::unix::ffi::OsStrExt;
        let Some(fd) = &self.inotify else {
            return true;
        };
        let mut buffer = [std::mem::MaybeUninit::uninit(); 4096];
        let mut events = rustix::fs::inotify::Reader::new(fd, &mut buffer);
        let mut changed = false;
        while let Ok(event) = events.next() {
            changed |= event
                .file_name()
                .is_some_and(|name| name.to_bytes() == self.name.as_bytes());
        }
        changed
    }
}

pub struct UnixHookSocket(std::os::unix::net::UnixDatagram);
impl UnixHookSocket {
    pub fn sender(path: &std::path::Path) -> io::Result<Self> {
        let socket = std::os::unix::net::UnixDatagram::unbound()?;
        socket.set_nonblocking(true)?;
        socket.connect(path)?;
        Ok(Self(socket))
    }
    pub fn bind(path: &std::path::Path) -> io::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let socket = std::os::unix::net::UnixDatagram::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        socket.set_nonblocking(true)?;
        Ok(Self(socket))
    }
}
impl HookSocket for UnixHookSocket {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.0.send(bytes).map(|_| ())
    }
    fn receive(&mut self) -> Option<Vec<u8>> {
        let mut bytes = vec![0; 65536];
        let length = self.0.recv(&mut bytes).ok()?;
        bytes.truncate(length);
        Some(bytes)
    }
}

pub struct TimedStdin;
impl HookInput for TimedStdin {
    fn read(&mut self, budget_ms: u64, limit: usize) -> io::Result<String> {
        read_hook_input(0, budget_ms, limit)
    }
}

fn read_hook_input(fd: libc::c_int, budget_ms: u64, limit: usize) -> io::Result<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(budget_ms);
    let mut input = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let mut descriptor = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // The fixed deadline bounds poll; read consumes only ready stdin bytes.
        if unsafe { libc::poll(&mut descriptor, 1, remaining.as_millis() as i32) } <= 0 {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let mut buffer = [0u8; 8192];
        let length = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        if length < 0 {
            return Err(io::Error::last_os_error());
        }
        if length == 0 {
            return String::from_utf8(input).map_err(io::Error::other);
        }
        input.extend_from_slice(&buffer[..length as usize]);
        if input.len() > limit {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
}

pub struct PortalSettings;
impl DesktopSettings for PortalSettings {
    fn follow(&self, publish: &mut dyn FnMut(crate::desktop_theme::SystemTheme)) -> Result<(), ()> {
        use crate::desktop_theme::{scheme, setting_changed, KEY, NAMESPACE};
        let mut run = || -> zbus::Result<()> {
            let bus = zbus::blocking::Connection::session()?;
            let proxy = zbus::blocking::Proxy::new(
                &bus,
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings",
            )?;
            let signals = proxy.receive_signal("SettingChanged")?;
            let initial: zbus::zvariant::OwnedValue = proxy.call("ReadOne", &(NAMESPACE, KEY))?;
            if let Ok(value) = u32::try_from(initial) {
                publish(scheme(value));
            }
            for message in signals {
                if let Some(theme) = setting_changed(&message) {
                    publish(theme);
                }
            }
            Ok(())
        };
        run().map_err(|_| ())
    }
}

pub use SecretService as NativeKeyring;
pub use UnixHookSocket as NativeHookSocket;
pub fn hook_endpoint() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|dir| std::path::PathBuf::from(dir).join("kvojps-monitor.sock"))
}
pub fn space(path: &std::path::Path) -> io::Result<Space> {
    let s = rustix::fs::statvfs(path)?;
    Ok(Space {
        total: s.f_blocks * s.f_frsize,
        free: s.f_bfree * s.f_frsize,
        available: s.f_bavail * s.f_frsize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_hook_input_times_out_without_waiting_for_eof() {
        use std::os::fd::AsRawFd;
        let (reader, _writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let start = std::time::Instant::now();
        let error = read_hook_input(reader.as_raw_fd(), 100, 65536).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < std::time::Duration::from_millis(200));
    }

    #[test]
    fn file_watch_sees_a_replaced_file_once_and_ignores_its_neighbours() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cache/claude-statusline.json");
        let mut watch = FileWatch::new(&file);
        assert!(!watch.changed());
        std::fs::write(dir.path().join("cache/other.json"), "{}").unwrap();
        assert!(!watch.changed());
        let temporary = dir.path().join("cache/claude-statusline.json.tmp");
        std::fs::write(&temporary, "{}").unwrap();
        std::fs::rename(&temporary, &file).unwrap();
        assert!(watch.changed());
        assert!(!watch.changed());
    }
}
