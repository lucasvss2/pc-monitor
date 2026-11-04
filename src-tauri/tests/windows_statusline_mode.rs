#![cfg(all(windows, debug_assertions))]
//! Native Windows process test. All paths, commands and sensitive sentinels are synthetic.
use std::{
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn run_wrapper(bash: Option<&str>) {
    let root = tempfile::Builder::new()
        .prefix("monitor line ' $ % á ")
        .tempdir()
        .unwrap();
    let executable = root.path().join("monitor.exe");
    let binary = std::env::var_os("KVOJPS_MONITOR_TEST_BINARY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_kvojps-monitor").into());
    std::fs::copy(binary, &executable).unwrap();
    let cache = root.path().join("limits.json");
    let wrapper = root.path().join("status line.ps1");
    // The previous command verifies every input byte, including trailing LF and
    // Unicode, then emits ANSI output which must be preserved byte-for-byte.
    let payload = "{\"rate_limits\":{\"five_hour\":{\"used_percentage\":23,\"resets_at\":2000000000}},\"prompt\":\"PRIVATE_PROMPT_á\",\"token\":\"PRIVATE_TOKEN\"}\n";
    let original = if bash.is_some() {
        r"cat >/dev/null; printf '\033[32moriginal á\033[0m\n'".to_owned()
    } else {
        use base64::Engine;
        let expected = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
        format!(
            r#"$m = New-Object System.IO.MemoryStream; [Console]::OpenStandardInput().CopyTo($m); if ([Convert]::ToBase64String($m.ToArray()) -ne '{expected}') {{ exit 4 }}; $o = [Text.Encoding]::UTF8.GetBytes(([char]27 + '[32moriginal á' + [char]27 + "[0m`n")); [Console]::OpenStandardOutput().Write($o,0,$o.Length)"#
        )
    };
    let (command, script) = kvojps_monitor_lib::validation_status_line(
        &executable.to_string_lossy(),
        &cache.to_string_lossy(),
        &wrapper.to_string_lossy(),
        &original,
        bash,
    );
    std::fs::write(&wrapper, script).unwrap();
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            command.split_whitespace().last().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("status line deadline exceeded");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "wrapper must not disclose input or errors"
    );
    assert_eq!(output.stdout, "\u{1b}[32moriginal á\u{1b}[0m\n".as_bytes());
    let stored = std::fs::read_to_string(&cache).expect("native helper must copy the limits");
    assert!(
        !stored.contains("PRIVATE_") && !stored.contains("prompt") && !stored.contains("token")
    );
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["rate_limits"]["five_hour"]["used_percentage"], 23.0);
    assert_eq!(stored.as_object().unwrap().len(), 2);
}

#[test]
fn windows_statusline_preserves_original_output_and_projects_limits() {
    run_wrapper(None);
}

#[test]
fn windows_statusline_chains_git_bash_without_using_wsl() {
    let bash =
        std::env::var("KVOJPS_MONITOR_TEST_GIT_BASH").expect("CI supplies Git Bash explicitly");
    run_wrapper(Some(&bash));
}
