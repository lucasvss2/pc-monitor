//! Optional silent per-user startup. First launch never opts the user in.
use crate::persistence::Storage;
use std::{io, path::PathBuf};
pub struct Execution {
    pub executable: PathBuf,
}
pub fn enabled(storage: &impl Storage) -> bool {
    storage.read("startup").is_ok()
}
pub fn synchronize(storage: &impl Storage, execution: &Execution, _: bool) -> io::Result<()> {
    match storage.read("startup") {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(_) => set(storage, execution, true),
    }
}
pub fn set(storage: &impl Storage, execution: &Execution, enabled: bool) -> io::Result<()> {
    if enabled {
        storage.write(
            "startup",
            &format!("\"{}\" --autostart", execution.executable.display()),
        )
    } else {
        storage.remove("startup")
    }
}
