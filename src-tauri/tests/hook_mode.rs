#![cfg(target_os = "linux")]
//! Real subprocess boundary: no Tauri startup, no output, bounded even without EOF.
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// Keep process-startup timing independent from competing GUI loader startups.
static PROCESS_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn hook_mode_exits_zero_under_200ms_with_app_down_and_stalled_stdin() {
    let _serial = PROCESS_TEST
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().unwrap();
    for input in [
        Some(r#"{"session_id":"abc","cwd":"/tmp","prompt":"PRIVATE"}"#),
        Some("invalid"),
        None,
    ] {
        let start = Instant::now();
        let mut child = Command::new(env!("CARGO_BIN_EXE_kvojps-monitor"))
            .args(["hook", "codex", "UserPromptSubmit"])
            .env("XDG_RUNTIME_DIR", root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(input) = input {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
        }
        // Retain the pipe in the parent for the stalled-stdin case.
        let pipe = child.stdin.take();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "elapsed {:?}",
            start.elapsed()
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        drop(pipe);
    }
}

#[test]
fn hook_subprocess_delivers_only_metadata_over_the_runtime_socket() {
    let _serial = PROCESS_TEST
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().unwrap();
    let socket =
        std::os::unix::net::UnixDatagram::bind(root.path().join("kvojps-monitor.sock")).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_kvojps-monitor"))
        .args(["hook", "claude", "PermissionRequest"])
        .env("XDG_RUNTIME_DIR", root.path())
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(include_bytes!(
            "../fixtures/agents/hooks/claude-permission.json"
        ))
        .unwrap();
    assert!(child.wait().unwrap().success());
    let mut bytes = [0; 8192];
    let length = socket.recv(&mut bytes).unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(&bytes[..length]).unwrap();
    assert_eq!(metadata["id"], "hook-claude");
    assert_eq!(metadata["state"], "waiting");
    assert!(!String::from_utf8_lossy(&bytes[..length]).contains("PRIVATE"));
}

#[test]
fn active_socket_still_bounds_stalled_stdin() {
    let _serial = PROCESS_TEST
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = tempfile::tempdir().unwrap();
    let _socket =
        std::os::unix::net::UnixDatagram::bind(root.path().join("kvojps-monitor.sock")).unwrap();
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_kvojps-monitor"))
        .args(["hook", "codex", "UserPromptSubmit"])
        .env("XDG_RUNTIME_DIR", root.path())
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let pipe = child.stdin.take();
    assert!(child.wait().unwrap().success());
    assert!(start.elapsed() < Duration::from_millis(200));
    drop(pipe);
}
