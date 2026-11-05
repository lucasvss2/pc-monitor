//! Read-only Restart Manager queries. No calls to stop or restart a process.
use crate::ports::LiveRollouts;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::ErrorKind,
    os::windows::ffi::OsStringExt,
    path::{Component, Path, PathBuf, Prefix},
};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, ERROR_INVALID_PARAMETER, ERROR_MORE_DATA, ERROR_NO_MORE_FILES,
            ERROR_SUCCESS, FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        System::{
            Diagnostics::ToolHelp::*, RemoteDesktop::ProcessIdToSessionId, RestartManager::*,
            Threading::*,
        },
    },
};

pub struct CodexRollouts(PathBuf);

impl CodexRollouts {
    pub fn new(executable: PathBuf) -> Self {
        Self(executable)
    }
}

impl LiveRollouts for CodexRollouts {
    fn live(&self, root: &Path, paths: &[PathBuf]) -> Result<HashMap<PathBuf, PathBuf>, ()> {
        let mut live = HashMap::new();
        if paths.is_empty() {
            return Ok(live);
        }
        let processes = cli_processes(&self.0)?;
        if processes.is_empty() {
            return Ok(live);
        }
        let root = match root.canonicalize() {
            Ok(root) => root,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(live),
            Err(_) => return Err(()),
        };
        let mut candidates = Vec::new();
        for path in paths {
            if !path.is_absolute() || path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            let canonical = match path.canonicalize() {
                Ok(path) if path.starts_with(&root) => path,
                Ok(_) => continue,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(_) => return Err(()),
            };
            if !matches!(canonical.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::VerbatimDisk(_)))
            {
                continue;
            }
            let dos = canonical
                .to_str()
                .and_then(|path| path.strip_prefix(r"\\?\"))
                .ok_or(())?;
            candidates.push(Candidate {
                original: path,
                dos: dos.encode_utf16().chain([0]).collect(),
                canonical,
            });
        }
        let mut budget = 48;
        search(&candidates, &processes, &mut budget, &mut live)?;
        Ok(live)
    }
}

struct Candidate<'a> {
    original: &'a Path,
    canonical: PathBuf,
    dos: Vec<u16>,
}

fn search(
    candidates: &[Candidate<'_>],
    processes: &HashSet<(u32, u64)>,
    budget: &mut usize,
    live: &mut HashMap<PathBuf, PathBuf>,
) -> Result<(), ()> {
    if candidates.is_empty() {
        return Ok(());
    }
    *budget = budget.checked_sub(1).ok_or(())?;
    let names: Vec<_> = candidates
        .iter()
        .map(|candidate| PCWSTR(candidate.dos.as_ptr()))
        .collect();
    if !owners(&names)?
        .iter()
        .any(|owner| processes.contains(&(owner.dwProcessId, ticks(owner.ProcessStartTime))))
    {
        return Ok(());
    }
    if let [candidate] = candidates {
        live.insert(candidate.original.to_owned(), candidate.canonical.clone());
    } else {
        let (left, right) = candidates.split_at(candidates.len() / 2);
        search(left, processes, budget, live)?;
        search(right, processes, budget, live)?;
    }
    Ok(())
}

struct RestartSession(u32);
impl Drop for RestartSession {
    fn drop(&mut self) {
        let _ = unsafe { RmEndSession(self.0) };
    }
}

fn owners(paths: &[PCWSTR]) -> Result<Vec<RM_UNIQUE_PROCESS>, ()> {
    let mut session = 0;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    if unsafe { RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr())) } != ERROR_SUCCESS {
        return Err(());
    }
    let session = RestartSession(session);
    if unsafe { RmRegisterResources(session.0, Some(paths), None, None) } != ERROR_SUCCESS {
        return Err(());
    }
    let mut rows = vec![RM_PROCESS_INFO::default(); 8];
    for _ in 0..3 {
        let mut count = rows.len() as u32;
        let (mut needed, mut reasons) = (0, 0);
        match unsafe {
            RmGetList(
                session.0,
                &mut needed,
                &mut count,
                Some(rows.as_mut_ptr()),
                &mut reasons,
            )
        } {
            ERROR_SUCCESS => {
                rows.truncate(count as usize);
                return Ok(rows.into_iter().map(|row| row.Process).collect());
            }
            ERROR_MORE_DATA if needed <= 256 && needed > count => {
                rows.resize(needed as usize, RM_PROCESS_INFO::default())
            }
            _ => return Err(()),
        }
    }
    Err(())
}

struct ProcessHandle(HANDLE);
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

fn cli_processes(executable: &Path) -> Result<HashSet<(u32, u64)>, ()> {
    let expected = executable.canonicalize().map_err(|_| ())?;
    let expected = expected.to_str().ok_or(())?;
    let expected = expected.strip_prefix(r"\\?\").unwrap_or(expected);
    let current = std::process::id();
    let mut session = 0;
    unsafe { ProcessIdToSessionId(current, &mut session) }.map_err(|_| ())?;
    let snapshot =
        ProcessHandle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.map_err(|_| ())?);
    let mut row = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    unsafe { Process32FirstW(snapshot.0, &mut row) }.map_err(|_| ())?;
    let mut processes = HashSet::new();
    loop {
        let name_len = row
            .szExeFile
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(row.szExeFile.len());
        let name = String::from_utf16_lossy(&row.szExeFile[..name_len]);
        let pid = row.th32ProcessID;
        // Exclude the monitor's own app-server used for reading usage limits.
        if name.eq_ignore_ascii_case("codex.exe") && row.th32ParentProcessID != current {
            let mut candidate_session = 0;
            match unsafe { ProcessIdToSessionId(pid, &mut candidate_session) } {
                Ok(()) if candidate_session == session => {
                    if let Some(created) = same_cli(pid, expected)? {
                        processes.insert((pid, created));
                    }
                }
                Ok(()) => {}
                Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => {}
                Err(_) => return Err(()),
            }
        }
        match unsafe { Process32NextW(snapshot.0, &mut row) } {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
            Err(_) => return Err(()),
        }
    }
    Ok(processes)
}

fn same_cli(pid: u32, expected: &str) -> Result<Option<u64>, ()> {
    let process = match unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
    } {
        Ok(process) => ProcessHandle(process),
        Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => return Ok(None),
        Err(_) => return Err(()),
    };
    match unsafe { WaitForSingleObject(process.0, 0) } {
        WAIT_TIMEOUT => {}
        WAIT_OBJECT_0 => return Ok(None),
        _ => return Err(()),
    }
    let (mut created, mut exited, mut kernel, mut user) = Default::default();
    unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) }
        .map_err(|_| ())?;
    let mut image = vec![0u16; 32768];
    let mut size = image.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(image.as_mut_ptr()),
            &mut size,
        )
    }
    .map_err(|_| ())?;
    let image = OsString::from_wide(&image[..size as usize]);
    let image = image.to_str().ok_or(())?;
    let image = image.strip_prefix(r"\\?\").unwrap_or(image);
    Ok(image
        .eq_ignore_ascii_case(expected)
        .then_some(ticks(created)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    #[test]
    fn restart_manager_only_reports_the_owner_while_the_file_is_open() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("rollout-example.jsonl");
        let writer = std::fs::File::create(&file).unwrap();
        let name: Vec<_> = file.as_os_str().encode_wide().chain([0]).collect();
        let paths = [PCWSTR(name.as_ptr())];
        let owns = || {
            owners(&paths)
                .unwrap()
                .iter()
                .any(|owner| owner.dwProcessId == std::process::id())
        };
        assert!(owns());
        drop(writer);
        assert!(!owns());
    }
}
