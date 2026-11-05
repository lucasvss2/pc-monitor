//! Unstable Codex SQLite/rollout formats are confined to this adapter.
use super::{Provider, Session, SessionState};
use crate::ports::Files;
use serde::Deserialize;
#[cfg(target_os = "linux")]
use std::{collections::HashSet, path::PathBuf};

#[cfg(windows)]
#[path = "codex_windows.rs"]
pub(super) mod windows;

/// Read-only local session source; no content is persisted or logged.
pub(super) trait Source: Send {
    fn sessions(&mut self, tracked: &[String]) -> Result<Vec<(Session, bool)>, ()>;
    fn partial(&self) -> bool {
        false
    }
}
#[cfg(target_os = "linux")]
pub(super) struct Adapter<F> {
    root: PathBuf,
    files: F,
}
#[cfg(target_os = "linux")]
impl<F: Files> Adapter<F> {
    pub fn new(root: PathBuf, files: F) -> Self {
        Self { root, files }
    }

    fn live_rollouts(&self) -> Result<HashSet<PathBuf>, ()> {
        let mut paths = HashSet::new();
        for pid in self.files.list("/proc").map_err(|_| ())? {
            if !pid.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let exe = match self.files.read_link(&format!("/proc/{pid}/exe")) {
                Ok(exe) => exe,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    // Non-dumpable processes (including systemd and ssh-agent) can
                    // deny exe access even for their owner. Only Codex candidates
                    // can make this session observation incomplete.
                    let own = self.files.read("/proc/self/status").map_err(|_| ())?;
                    let status = match self.files.read(&format!("/proc/{pid}/status")) {
                        Ok(status) => status,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(_) => return Err(()),
                    };
                    if process_uid(&own)? != process_uid(&status)? {
                        continue;
                    }
                    match self.files.read(&format!("/proc/{pid}/comm")) {
                        Ok(name) if name.trim() != "codex" => continue,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        _ => return Err(()),
                    }
                }
                Err(_) => return Err(()),
            };
            if exe.file_name().is_none_or(|name| name != "codex") {
                continue;
            }
            let fd_dir = format!("/proc/{pid}/fd");
            let descriptors = match self.files.list(&fd_dir) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(()),
            };
            for fd in descriptors {
                match self.files.read_link(&format!("{fd_dir}/{fd}")) {
                    Ok(path) => {
                        paths.insert(path);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err(()),
                }
            }
        }
        Ok(paths)
    }
}
#[cfg(target_os = "linux")]
impl<F: Files> Source for Adapter<F> {
    fn sessions(&mut self, tracked: &[String]) -> Result<Vec<(Session, bool)>, ()> {
        let path = self.root.join("state_5.sqlite");
        // Absence is an empty source only before a session has been observed.
        match self.files.list(&self.root.to_string_lossy()) {
            Ok(names)
                if !names.iter().any(|name| name == "state_5.sqlite") && tracked.is_empty() =>
            {
                return Ok(vec![])
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && tracked.is_empty() => {
                return Ok(vec![])
            }
            Err(_) => return Err(()),
            _ => {}
        }
        let rows = self.files.read_sqlite(&path.to_string_lossy(), "SELECT id, rollout_path, cwd, title, created_at, updated_at, git_branch FROM threads").map_err(|_| ())?;
        let live = self.live_rollouts()?;
        let mut sessions = Vec::new();
        for row in rows {
            let thread: Thread = serde_json::from_value(row).map_err(|_| ())?;
            let running = live.contains(&PathBuf::from(&thread.rollout_path));
            if running || tracked.contains(&thread.session_id()) {
                sessions.push((thread.session(&self.files, &thread.rollout_path)?, running));
            }
        }
        Ok(sessions)
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(super) mod tests {
    use super::*;
    use crate::ports::fakes::FakeRoot;
    pub fn source() -> Adapter<FakeRoot> {
        let files = FakeRoot::from("agents/codex");
        let root = files.dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("proc/123/fd")).unwrap();
        std::os::unix::fs::symlink("/usr/bin/codex", root.join("proc/123/exe")).unwrap();
        std::os::unix::fs::symlink("/rollout.jsonl", root.join("proc/123/fd/4")).unwrap();
        Adapter::new(PathBuf::from("/"), files)
    }
}

#[derive(Clone, Deserialize)]
struct Thread {
    id: String,
    rollout_path: String,
    cwd: String,
    title: String,
    created_at: i64,
    updated_at: i64,
    git_branch: Option<String>,
}

impl Thread {
    fn session_id(&self) -> String {
        format!("codex:{}", self.id)
    }

    fn session(&self, files: &impl Files, rollout: &str) -> Result<Session, ()> {
        let Self {
            id,
            cwd,
            title,
            created_at: created,
            updated_at: updated,
            git_branch: branch,
            ..
        } = self;
        let mut state = SessionState::Done;
        let mut interrupted = false;
        let mut turn_duration = None;
        {
            let content = files.read(rollout).map_err(|_| ())?;
            for line in content.lines() {
                let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                if event["type"] != "event_msg" {
                    continue;
                }
                if matches!(
                    event["payload"]["type"].as_str(),
                    Some("task_started" | "task_complete" | "turn_aborted" | "error")
                ) {
                    interrupted = event["payload"]["type"] == "turn_aborted";
                    turn_duration = event["payload"]["duration_ms"].as_u64();
                }
                match event["payload"]["type"].as_str() {
                    Some("task_started") => state = SessionState::Working,
                    Some("task_complete" | "turn_aborted") => state = SessionState::Done,
                    Some("error") => state = SessionState::Failed,
                    _ => {}
                }
            }
        }
        Ok(Session {
            interrupted,
            turn_duration,
            id: format!("codex:{id}"),
            provider: Provider::Codex,
            project: std::path::Path::new(cwd)
                .file_name()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| cwd.clone()),
            name: Some(title.clone()),
            branch: branch.clone(),
            updated_at: Some(
                u64::try_from(*updated)
                    .map_err(|_| ())?
                    .saturating_mul(1000),
            ),
            state,
            waiting_for: None,
            started_at: u64::try_from(*created)
                .map_err(|_| ())?
                .saturating_mul(1000),
            closed_at: None,
            last_state: None,
        })
    }
}

#[cfg(all(test, target_os = "linux"))]
impl Adapter<crate::ports::fakes::FakeRoot> {
    pub(super) fn test_files(&self) -> crate::ports::fakes::FakeRoot {
        self.files.clone()
    }
    pub(super) fn test_root(&self) -> PathBuf {
        self.files.dir.path().to_path_buf()
    }
}

#[cfg(target_os = "linux")]
fn process_uid(status: &str) -> Result<&str, ()> {
    status
        .lines()
        .find_map(|line| {
            line.strip_prefix("Uid:")
                .and_then(|value| value.split_whitespace().next())
        })
        .ok_or(())
}
