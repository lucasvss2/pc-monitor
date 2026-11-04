//! A single per-user Run value; no administrator access or Startup-folder scripts.
use crate::persistence::Storage;
use std::{io, path::PathBuf};
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "kvojps-monitor";
pub struct StartupStorage {
    key_path: String,
}
pub fn startup_storage(_: PathBuf) -> StartupStorage {
    StartupStorage {
        key_path: RUN.into(),
    }
}
fn error(_: windows::core::Error) -> io::Error {
    io::Error::other("startup setting unavailable")
}
impl Storage for StartupStorage {
    fn read(&self, _: &str) -> io::Result<String> {
        match windows_registry::CURRENT_USER
            .open(&self.key_path)
            .and_then(|key| key.get_string(NAME))
        {
            Ok(value) => Ok(value),
            Err(e) if e.code() == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.to_hresult() => {
                Err(io::ErrorKind::NotFound.into())
            }
            Err(e) => Err(error(e)),
        }
    }
    fn write(&self, _: &str, value: &str) -> io::Result<()> {
        windows_registry::CURRENT_USER
            .create(&self.key_path)
            .and_then(|key| key.set_string(NAME, value))
            .map_err(error)
    }
    fn rename(&self, _: &str, _: &str) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn remove(&self, _: &str) -> io::Result<()> {
        match windows_registry::CURRENT_USER
            .open(&self.key_path)
            .and_then(|key| key.remove_value(NAME))
        {
            Ok(()) => Ok(()),
            Err(e) if e.code() == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND.to_hresult() => {
                Ok(())
            }
            Err(e) => Err(error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_startup_roundtrip_removes_only_the_test_value() {
        let key_path = format!(
            r"Software\kvojps-monitor-startup-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = windows_registry::CURRENT_USER.remove_tree(&self.0);
            }
        }
        let _cleanup = Cleanup(key_path.clone());
        let key = windows_registry::CURRENT_USER.create(&key_path).unwrap();
        key.set_string("other", "synthetic-other").unwrap();
        let storage = StartupStorage { key_path };
        storage.write("startup", "synthetic --autostart").unwrap();
        assert_eq!(storage.read("startup").unwrap(), "synthetic --autostart");
        storage.remove("startup").unwrap();
        storage.remove("startup").unwrap();
        assert_eq!(
            storage.read("startup").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(key.get_string("other").unwrap(), "synthetic-other");
    }
}
