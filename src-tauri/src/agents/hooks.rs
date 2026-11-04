//! Opt-in hook configuration and metadata-only transport.
use crate::persistence::Storage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io;

const MANIFEST: &str = ".config/kvojps-monitor/agent-hooks.json";

fn hook_command(executable: &str, provider: &str, event: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        format!(
            "'{}' hook {provider} {event}",
            executable.replace('\'', "'\\''")
        )
    }
    #[cfg(windows)]
    {
        use base64::Engine;
        // EncodedCommand avoids interpretation by the caller's cmd/Bash shell.
        let script = format!(
            "& '{}' hook {provider} {event}",
            executable.replace('\'', "''")
        );
        let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        format!(
            "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }
}

#[cfg(debug_assertions)]
pub(crate) fn validation_command(executable: &str) -> String {
    hook_command(executable, "codex", "UserPromptSubmit")
}
const PROVIDERS: &[(&str, &str, &[&str])] = &[
    (
        "claude",
        ".claude/settings.json",
        &[
            "UserPromptSubmit",
            "PermissionRequest",
            "Notification",
            "Stop",
            "StopFailure",
            "SessionEnd",
        ],
    ),
    (
        "codex",
        ".codex/hooks.json",
        &[
            "SessionStart",
            "SessionEnd",
            "UserPromptSubmit",
            "PermissionRequest",
            "PostToolUse",
            "Stop",
            "Interrupt",
        ],
    ),
];
#[derive(Serialize, Deserialize)]
struct AddedFile {
    path: String,
    missing_file: bool,
    missing_hooks: bool,
    entries: Vec<(String, Value, bool)>,
}

/// Only the explicit settings command calls set_enabled. Construction is read-only.
pub struct Configuration<S> {
    storage: S,
    executable: String,
}
impl<S: Storage> Configuration<S> {
    pub fn new(storage: S, executable: String) -> Self {
        Self {
            storage,
            executable,
        }
    }
    pub fn enabled(&self) -> bool {
        self.storage
            .read(MANIFEST)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<AddedFile>>(&s).ok())
            .is_some()
    }
    pub fn set_enabled(&mut self, enabled: bool) -> io::Result<()> {
        if enabled == self.enabled() {
            return Ok(());
        }
        let mut changes = Vec::new();
        let mut files = Vec::new();
        if enabled {
            for (provider, path, events) in PROVIDERS {
                let original = read_optional(&self.storage, path)?;
                let mut value: Value = match &original {
                    Some(text) => serde_json::from_str(text).map_err(io::Error::other)?,
                    None => json!({}),
                };
                let object = value.as_object_mut().ok_or_else(invalid)?;
                let missing_hooks = !object.contains_key("hooks");
                let hooks = object
                    .entry("hooks")
                    .or_insert(json!({}))
                    .as_object_mut()
                    .ok_or_else(invalid)?;
                let mut entries = vec![];
                for event in *events {
                    // Windows also starts PowerShell; allow bounded cold startup.
                    // The hook binary still reads stdin for at most 100 ms.
                    let timeout = if cfg!(windows) { 3 } else { 1 };
                    let entry = json!({"hooks":[{"type":"command","command":hook_command(&self.executable, provider, event),"timeout":timeout}]});
                    let missing = !hooks.contains_key(*event);
                    let list = hooks
                        .entry(*event)
                        .or_insert(json!([]))
                        .as_array_mut()
                        .ok_or_else(invalid)?;
                    // Never claim a pre-existing entry as ours.
                    if !list.contains(&entry) {
                        list.push(entry.clone());
                        entries.push((event.to_string(), entry, missing));
                    }
                }
                files.push(AddedFile {
                    path: path.to_string(),
                    missing_file: original.is_none(),
                    missing_hooks,
                    entries,
                });
                changes.push((path.to_string(), original, Some(value.to_string())));
            }
            changes.push((MANIFEST.into(), None, Some(serde_json::to_string(&files)?)));
        } else {
            files =
                serde_json::from_str(&self.storage.read(MANIFEST)?).map_err(io::Error::other)?;
            for file in files {
                let original = read_optional(&self.storage, &file.path)?;
                let Some(text) = &original else {
                    continue;
                };
                let mut value: Value = serde_json::from_str(text).map_err(io::Error::other)?;
                let object = value.as_object_mut().ok_or_else(invalid)?;
                if let Some(hooks) = object.get_mut("hooks").and_then(Value::as_object_mut) {
                    for (event, entry, missing) in file.entries {
                        if let Some(list) = hooks.get_mut(&event).and_then(Value::as_array_mut) {
                            // Remove one exact entry, preserving later third-party edits.
                            if let Some(index) = list.iter().position(|e| e == &entry) {
                                list.remove(index);
                            }
                            if missing && list.is_empty() {
                                hooks.remove(&event);
                            }
                        }
                    }
                    if file.missing_hooks && hooks.is_empty() {
                        object.remove("hooks");
                    }
                }
                let replacement = if file.missing_file && object.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
                changes.push((file.path, original, replacement));
            }
            changes.push((
                MANIFEST.into(),
                read_optional(&self.storage, MANIFEST)?,
                None,
            ));
        }
        // Validate all files before writing; roll back completed writes on error.
        for (index, (path, _, replacement)) in changes.iter().enumerate() {
            if let Err(error) = replace(&self.storage, path, replacement.as_deref()) {
                for (path, original, _) in changes[..=index].iter().rev() {
                    let _ = replace(&self.storage, path, original.as_deref());
                }
                return Err(error);
            }
        }
        Ok(())
    }
}
fn invalid() -> io::Error {
    io::Error::other("Invalid hook configuration")
}
fn read_optional(storage: &impl Storage, path: &str) -> io::Result<Option<String>> {
    match storage.read(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
fn replace(storage: &impl Storage, path: &str, text: Option<&str>) -> io::Result<()> {
    match text {
        Some(text) => {
            let temporary = format!("{path}.kvojps-monitor.tmp");
            storage.write(&temporary, text)?;
            let result = storage.rename(&temporary, path);
            if result.is_err() {
                let _ = storage.remove(&temporary);
            }
            result
        }
        None => storage.remove(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{DiskStorage, Storage};

    #[test]
    fn installation_requires_a_command_and_removal_preserves_third_party_hooks() {
        let root = tempfile::tempdir().unwrap();
        let disk = DiskStorage(root.path().into());
        let original = r#"{"other":true,"hooks":{"Stop":[{"hooks":[{"type":"command","command":"orca stop"}]}]}}"#;
        disk.write(".claude/settings.json", original).unwrap();
        disk.write(".codex/hooks.json", original).unwrap();
        let mut config = Configuration::new(disk, "/opt/monitor's app".into());
        assert!(!config.enabled());
        assert_eq!(
            config.storage.read(".claude/settings.json").unwrap(),
            original
        );
        config.set_enabled(true).unwrap();
        config.set_enabled(true).unwrap();
        let installed: serde_json::Value =
            serde_json::from_str(&config.storage.read(".claude/settings.json").unwrap()).unwrap();
        assert_eq!(installed["hooks"]["Stop"].as_array().unwrap().len(), 2);
        // Another app adds a hook after installation.
        let mut edited = installed;
        edited["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"hooks":[{"type":"command","command":"another app"}]}));
        config
            .storage
            .write(".claude/settings.json", &edited.to_string())
            .unwrap();
        config.set_enabled(false).unwrap();
        let removed: serde_json::Value =
            serde_json::from_str(&config.storage.read(".claude/settings.json").unwrap()).unwrap();
        assert_eq!(removed["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(
            removed["hooks"]["Stop"][0]["hooks"][0]["command"],
            "orca stop"
        );
        assert_eq!(removed["hooks"].as_object().unwrap().len(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &config.storage.read(".codex/hooks.json").unwrap()
            )
            .unwrap(),
            serde_json::from_str::<serde_json::Value>(original).unwrap()
        );
    }
    #[test]
    fn missing_files_are_restored_and_malformed_peer_prevents_all_writes() {
        let root = tempfile::tempdir().unwrap();
        let disk = DiskStorage(root.path().into());
        disk.write(".codex/hooks.json", "invalid").unwrap();
        let mut config = Configuration::new(disk, "/opt/monitor".into());
        assert!(config.set_enabled(true).is_err());
        assert!(config.storage.read(".claude/settings.json").is_err());
        config.storage.remove(".codex/hooks.json").unwrap();
        config.set_enabled(true).unwrap();
        config.set_enabled(false).unwrap();
        assert!(config.storage.read(".claude/settings.json").is_err());
        assert!(config.storage.read(".codex/hooks.json").is_err());
        assert!(!config.enabled());
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::ports::HookSocket;
    #[derive(Default)]
    struct Socket(Vec<Vec<u8>>);
    impl crate::ports::HookSocket for Socket {
        fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.0.push(bytes.to_vec());
            Ok(())
        }
        fn receive(&mut self) -> Option<Vec<u8>> {
            self.0.pop()
        }
    }
    #[test]
    fn bounded_input_failures_are_silent_and_post_tool_resumes_codex() {
        struct Input(bool);
        impl crate::ports::HookInput for Input {
            fn read(&mut self, budget_ms: u64, limit: usize) -> io::Result<String> {
                assert_eq!((budget_ms, limit), (100, 65536));
                if self.0 {
                    Ok(include_str!("../../fixtures/agents/hooks/codex-post-tool.json").into())
                } else {
                    Err(io::ErrorKind::TimedOut.into())
                }
            }
        }
        let mut socket = Socket::default();
        deliver("codex", "PostToolUse", &mut Input(false), &mut socket);
        assert!(socket.receive().is_none());
        deliver("codex", "PostToolUse", &mut Input(true), &mut socket);
        let bytes = socket.receive().unwrap();
        let event: Metadata = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(event.state, super::super::SessionState::Working);
        assert!(!String::from_utf8(bytes).unwrap().contains("PRIVATE"));
    }
    #[test]
    fn forwarding_strips_content_and_ignores_unrelated_notifications() {
        let mut socket = Socket::default();
        forward(
            "codex",
            "PermissionRequest",
            include_str!("../../fixtures/agents/hooks/codex-permission.json"),
            &mut socket,
        );
        let sent = String::from_utf8(socket.receive().unwrap()).unwrap();
        assert!(!sent.contains("PRIVATE"));
        let event: Metadata = serde_json::from_str(&sent).unwrap();
        assert_eq!(event.id, "hook-codex");
        assert_eq!(event.state, super::super::SessionState::Waiting);
        forward(
            "claude",
            "Notification",
            r#"{"session_id":"abc","cwd":"/tmp","notification_type":"auth_success"}"#,
            &mut socket,
        );
        assert!(socket.receive().is_none());
    }
}

#[derive(Serialize, Deserialize)]
pub struct Metadata {
    pub(super) provider: super::Provider,
    pub(super) id: String,
    pub(super) state: super::SessionState,
    pub(super) tool: Option<String>,
    pub(super) cwd: String,
    pub(super) interrupted: bool,
}
/// Deserialize only the allowlisted fields; arbitrary hook payload never leaves memory.
pub fn forward(
    provider: &str,
    event: &str,
    input: &str,
    socket: &mut impl crate::ports::HookSocket,
) {
    #[derive(Deserialize)]
    struct Payload {
        session_id: String,
        cwd: String,
        tool_name: Option<String>,
        notification_type: Option<String>,
    }
    let Ok(payload) = serde_json::from_str::<Payload>(input) else {
        return;
    };
    let provider = match provider {
        "claude" => super::Provider::Claude,
        "codex" => super::Provider::Codex,
        _ => return,
    };
    use super::SessionState::*;
    let state = match (provider, event) {
        (_, "UserPromptSubmit") | (super::Provider::Codex, "SessionStart" | "PostToolUse") => {
            Working
        }
        (_, "PermissionRequest") => Waiting,
        (super::Provider::Claude, "Notification")
            if matches!(
                payload.notification_type.as_deref(),
                Some("permission_prompt" | "idle_prompt" | "elicitation_dialog")
            ) =>
        {
            Waiting
        }
        (_, "Stop") | (super::Provider::Codex, "Interrupt") => Done,
        (super::Provider::Claude, "StopFailure") => Failed,
        (_, "SessionEnd") => Closed,
        _ => return,
    };
    if payload.session_id.is_empty() || payload.session_id.len() > 256 || payload.cwd.len() > 4096 {
        return;
    }
    let metadata = Metadata {
        provider,
        id: payload.session_id,
        state,
        cwd: payload.cwd,
        tool: payload.tool_name.filter(|s| s.len() <= 128),
        interrupted: event == "Interrupt",
    };
    if let Ok(bytes) = serde_json::to_vec(&metadata) {
        let _ = socket.send(&bytes);
    }
}
pub fn socket_path() -> Option<std::path::PathBuf> {
    crate::platform::hook_endpoint()
}

/// The core requests bounded input through a port; adapters own OS time and stdin.
pub fn deliver(
    provider: &str,
    event: &str,
    input: &mut impl crate::ports::HookInput,
    socket: &mut impl crate::ports::HookSocket,
) {
    if let Ok(payload) = input.read(100, 65536) {
        forward(provider, event, &payload, socket);
    }
}
pub fn run(provider: &str, event: &str) {
    let Some(path) = socket_path() else {
        return;
    };
    if let Ok(mut socket) = crate::ports::NativeHookSocket::sender(&path) {
        deliver(provider, event, &mut crate::ports::TimedStdin, &mut socket);
    }
}
