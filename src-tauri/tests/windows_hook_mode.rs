#![cfg(all(windows, debug_assertions))]
//! Native subprocess evidence: production command, synthetic input, private temporary files.
use std::{
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn windows_hook_command_preserves_stdin_and_special_paths_without_content_leaks() {
    use windows::{
        core::PCWSTR,
        Win32::{Foundation::*, Storage::FileSystem::*, System::Pipes::*},
    };
    let endpoint = kvojps_monitor_lib::validation_hook_endpoint().unwrap();
    let name: Vec<_> = endpoint
        .to_string_lossy()
        .encode_utf16()
        .chain([0])
        .collect();
    let server = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            65536,
            65536,
            0,
            None,
        )
    };
    assert!(server != INVALID_HANDLE_VALUE);
    struct Cleanup(HANDLE);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _cleanup = Cleanup(server);
    let root = tempfile::Builder::new()
        .prefix("monitor path ' $ % á ")
        .tempdir()
        .unwrap();
    let executable = root.path().join("monitor.exe");
    std::fs::copy(env!("CARGO_BIN_EXE_kvojps-monitor"), &executable).unwrap();
    let command = kvojps_monitor_lib::validation_hook_command(&executable.to_string_lossy());
    let encoded = command.split_whitespace().last().unwrap();
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            encoded,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(br#"{"session_id":"synthetic-session","cwd":"C:/Example","prompt":"PRIVATE_PROMPT","tool_input":"PRIVATE_TOOL"}"#).unwrap();
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            panic!("hook deadline exceeded");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    unsafe {
        let _ = ConnectNamedPipe(server, None);
    }
    let mut bytes = [0u8; 65536];
    let mut count = 0;
    unsafe { ReadFile(server, Some(&mut bytes), Some(&mut count), None) }.unwrap();
    let text = std::str::from_utf8(&bytes[..count as usize]).unwrap();
    assert!(!text.contains("PRIVATE_"));
    let metadata: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(metadata["id"] == "synthetic-session");
}
