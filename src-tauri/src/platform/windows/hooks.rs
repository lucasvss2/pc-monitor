//! User-only, local named pipe. Neither endpoint stores hook payloads on disk.
use crate::ports::{HookInput, HookSocket};
use std::{io, os::windows::ffi::OsStrExt, path::Path};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::{Pipes::*, Threading::*},
    },
};

struct Handle(HANDLE);
// These kernel handles have no thread affinity and are owned by this adapter.
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn failure() -> io::Error {
    io::Error::other("local hook transport unavailable")
}

fn user_sid() -> io::Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(|_| failure())?;
        let token = Handle(token);
        let mut length = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut length);
        let mut storage = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            length,
            &mut length,
        )
        .map_err(|_| failure())?;
        let user = &*storage.as_ptr().cast::<TOKEN_USER>();
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).map_err(|_| failure())?;
        let sid = text.to_string().map_err(|_| failure());
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        sid
    }
}
pub fn endpoint() -> Option<std::path::PathBuf> {
    Some(format!(r"\\.\pipe\kvojps-monitor-{}", user_sid().ok()?).into())
}

pub struct NativeHookSocket {
    handle: Handle,
    server: bool,
}
impl NativeHookSocket {
    pub fn sender(path: &Path) -> io::Result<Self> {
        let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                GENERIC_WRITE.0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|_| failure())?;
        // A busy or absent monitor fails immediately; never wait for a server.
        let handle = Handle(handle);
        unsafe { SetNamedPipeHandleState(handle.0, Some(&PIPE_NOWAIT), None, None) }
            .map_err(|_| failure())?;
        Ok(Self {
            handle,
            server: false,
        })
    }
    pub fn bind(path: &Path) -> io::Result<Self> {
        let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
        let descriptor: Vec<_> = format!("D:P(A;;GA;;;{})", user_sid()?)
            .encode_utf16()
            .chain([0])
            .collect();
        unsafe {
            let mut security = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(descriptor.as_ptr()),
                1,
                &mut security,
                None,
            )
            .map_err(|_| failure())?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: security.0,
                bInheritHandle: false.into(),
            };
            let handle = CreateNamedPipeW(
                PCWSTR(path.as_ptr()),
                PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE
                    | PIPE_READMODE_MESSAGE
                    | PIPE_NOWAIT
                    | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                65536,
                65536,
                0,
                Some(&attributes),
            );
            let _ = LocalFree(Some(HLOCAL(security.0)));
            if handle == INVALID_HANDLE_VALUE {
                return Err(failure());
            }
            Ok(Self {
                handle: Handle(handle),
                server: true,
            })
        }
    }
}
impl HookSocket for NativeHookSocket {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.server || bytes.len() > 65536 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut written = 0;
        unsafe { WriteFile(self.handle.0, Some(bytes), Some(&mut written), None) }
            .map_err(|_| failure())?;
        if written as usize != bytes.len() {
            return Err(failure());
        }
        Ok(())
    }
    fn receive(&mut self) -> Option<Vec<u8>> {
        if !self.server {
            return None;
        }
        unsafe {
            let _ = ConnectNamedPipe(self.handle.0, None);
        }
        let mut bytes = vec![0u8; 65536];
        let mut count = 0;
        let result = unsafe { ReadFile(self.handle.0, Some(&mut bytes), Some(&mut count), None) };
        match result {
            Ok(()) if count > 0 => {
                bytes.truncate(count as usize);
                Some(bytes)
            }
            Err(error)
                if error.code() == ERROR_NO_DATA.to_hresult()
                    || error.code() == ERROR_BROKEN_PIPE.to_hresult()
                    || error.code() == ERROR_MORE_DATA.to_hresult() =>
            {
                unsafe {
                    let _ = DisconnectNamedPipe(self.handle.0);
                }
                None
            }
            _ => None,
        }
    }
}

pub struct TimedStdin;
impl HookInput for TimedStdin {
    fn read(&mut self, budget_ms: u64, limit: usize) -> io::Result<String> {
        // The hook subprocess exits after the deadline, also ending a blocked reader.
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            use std::io::Read;
            let mut bytes = Vec::new();
            let result = std::io::stdin()
                .take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .and_then(|_| {
                    if bytes.len() > limit {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData.into())
                });
            let _ = send.send(result);
        });
        receive
            .recv_timeout(std::time::Duration::from_millis(budget_ms))
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_pipe_is_exclusive_local_and_delivers_without_disk_payload() {
        let path: std::path::PathBuf =
            format!(r"\\.\pipe\kvojps-monitor-test-{}", std::process::id()).into();
        let mut server = NativeHookSocket::bind(&path).unwrap();
        assert!(NativeHookSocket::bind(&path).is_err());
        let mut client = NativeHookSocket::sender(&path).unwrap();
        client.send(b"synthetic-metadata").unwrap();
        assert!(server.receive().unwrap() == b"synthetic-metadata");
        drop(client);
        let _ = server.receive();
        let mut client = NativeHookSocket::sender(&path).unwrap();
        client.send(b"second").unwrap();
        assert!(server.receive().unwrap() == b"second");
    }
}
