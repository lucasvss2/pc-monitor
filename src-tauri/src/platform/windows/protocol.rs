//! Per-user URI association. Never replace a protocol owned by another app.
use std::path::Path;
use windows::core::{Error, Result};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, E_ACCESSDENIED, E_INVALIDARG};
const KEY: &str = r"Software\Classes\kvojps-monitor";
const OWNER: &str = "dev.kvojps.monitor";

pub(super) fn register(executable: &Path) -> Result<()> {
    register_at(KEY, executable)
}

fn register_at(path: &str, executable: &Path) -> Result<()> {
    let executable = executable.to_str().ok_or(Error::from(E_INVALIDARG))?;
    if executable.contains(['\"', '\0']) {
        return Err(E_INVALIDARG.into());
    }
    let user = &windows_registry::CURRENT_USER;
    match user.open(path) {
        Ok(key) if key.get_string("MonitorOwner").ok().as_deref() != Some(OWNER) => {
            return Err(E_ACCESSDENIED.into());
        }
        Ok(_) => {}
        Err(error) if error.code() == ERROR_FILE_NOT_FOUND.to_hresult() => {}
        Err(error) => return Err(error),
    }
    let key = user.create(path)?;
    key.set_string("MonitorOwner", OWNER)?;
    key.set_string("", "URL:kvojps-monitor")?;
    key.set_string("URL Protocol", "")?;
    let command = key.create(r"shell\open\command")?;
    command.set_string(
        "",
        format!("\"{executable}\" {} \"%1\"", crate::activation::FLAG),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_protocol_preserves_other_owners_and_quotes_the_executable() {
        let path = format!(
            r"Software\kvojps-monitor-protocol-test-{}-{}",
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
        let _cleanup = Cleanup(path.clone());
        let user = &windows_registry::CURRENT_USER;
        let executable = Path::new(r"C:\Example App\monitor.exe");
        let key = user.create(&path).unwrap();
        key.set_string("", "other application").unwrap();
        assert!(register_at(&path, executable).is_err());
        assert_eq!(key.get_string("").unwrap(), "other application");
        assert!(key.open(r"shell\open\command").is_err());
        user.remove_tree(&path).unwrap();
        register_at(&path, executable).unwrap();
        register_at(&path, executable).unwrap();
        let key = user.open(&path).unwrap();
        assert_eq!(key.get_string("URL Protocol").unwrap(), "");
        assert_eq!(
            key.open(r"shell\open\command")
                .unwrap()
                .get_string("")
                .unwrap(),
            r#""C:\Example App\monitor.exe" --notification-activate "%1""#
        );
        assert!(register_at(&path, Path::new("bad\"path")).is_err());
    }
}
