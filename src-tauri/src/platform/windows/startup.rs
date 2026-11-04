//! A single per-user Run value; no administrator access or Startup-folder scripts.
use crate::persistence::Storage;
use std::{io, path::PathBuf};
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "kvojps-monitor";
pub struct StartupStorage;
pub fn startup_storage(_: PathBuf) -> StartupStorage {
    StartupStorage
}
fn error(_: windows::core::Error) -> io::Error {
    io::Error::other("startup setting unavailable")
}
impl Storage for StartupStorage {
    fn read(&self, _: &str) -> io::Result<String> {
        match windows_registry::CURRENT_USER
            .open(RUN)
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
            .create(RUN)
            .and_then(|key| key.set_string(NAME, value))
            .map_err(error)
    }
    fn rename(&self, _: &str, _: &str) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn remove(&self, _: &str) -> io::Result<()> {
        match windows_registry::CURRENT_USER
            .open(RUN)
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
