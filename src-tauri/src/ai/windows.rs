//! Native Windows launchers. Resolve npm's native Codex binary rather than
//! starting a shell shim whose descendants would outlive the collector.
use std::path::{Path, PathBuf};

/// Windows closes this job if the monitor exits, including an abrupt GUI exit.
/// The app-server and its descendants must not survive as residual processes.
pub struct Job(windows::Win32::Foundation::HANDLE);
// Owned kernel handle with no thread affinity; only this adapter closes it.
unsafe impl Send for Job {}
impl Job {
    pub fn new(child: &std::process::Child) -> Result<Self, ()> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
        let job = Self(unsafe { CreateJobObjectW(None, None) }.map_err(|_| ())?);
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
            .map_err(|_| ())?;
            AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())).map_err(|_| ())?;
        }
        Ok(job)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

pub fn codex(paths: &std::ffi::OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(paths) {
        let direct = dir.join("codex.exe");
        if direct.is_file() {
            return Some(direct);
        }
        if !dir.join("codex.cmd").is_file() {
            continue;
        }
        for package in [
            dir.join("node_modules/@openai/codex"),
            dir.join("../@openai/codex"),
        ] {
            let Ok(package) = package.canonicalize() else {
                continue;
            };
            // npm releases ship either the platform dependency or vendor inline.
            for vendor in [
                package.join("node_modules/@openai/codex-win32-x64/vendor"),
                package.join("../codex-win32-x64/vendor"),
                package.join("vendor"),
            ] {
                for folder in ["bin", "codex"] {
                    let binary = vendor.join(format!("x86_64-pc-windows-msvc/{folder}/codex.exe"));
                    if binary.is_file() {
                        return Some(binary);
                    }
                }
            }
        }
    }
    None
}

/// Match Claude Code's documented Git Bash preference, without choosing WSL's bash.
pub fn git_bash() -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if dir.join("git.exe").is_file() {
                candidates.extend([dir.join("bash.exe"), dir.join("../bin/bash.exe")]);
            }
        }
    }
    for base in ["ProgramFiles", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(base) {
            let base = Path::new(&base);
            candidates.extend([
                base.join("Git/bin/bash.exe"),
                base.join("Programs/Git/bin/bash.exe"),
            ]);
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
}

pub fn encoded(script: &str) -> String {
    use base64::Engine;
    let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

pub fn command(wrapper: &str) -> String {
    format!(
        "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {}",
        encoded(&format!("& {}", literal(wrapper)))
    )
}

pub fn wrapper(
    executable: &str,
    cache: &str,
    original: Option<&str>,
    bash: Option<&str>,
) -> String {
    let original = original
        .map(|original| match bash {
            Some(bash) => format!(
                "Invoke-StatusCommand {} {} $payload $true",
                literal(bash),
                literal(&format!("-c {}", windows_arg(original)))
            ),
            None => format!(
                "Invoke-StatusCommand 'powershell.exe' {} $payload $true",
                literal(&format!(
                    "-NoLogo -NoProfile -NonInteractive -EncodedCommand {}",
                    encoded(original)
                ))
            ),
        })
        .unwrap_or_default();
    // stdin/stdout are byte streams: no PowerShell pipeline conversion, extra
    // newlines or ANSI stripping. Only the Rust helper projects cache fields.
    format!(
        r#"﻿# kvojps-monitor: disconnect in the app to restore the previous status line.
$ErrorActionPreference = 'Stop'
function Invoke-StatusCommand($file, $arguments, $payload, $show) {{
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $file
    $info.Arguments = $arguments
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $child = New-Object System.Diagnostics.Process
    $child.StartInfo = $info
    try {{
        if (!$child.Start()) {{ return }}
        $output = New-Object System.IO.MemoryStream
        $copy = $child.StandardOutput.BaseStream.CopyToAsync($output)
        $errors = $child.StandardError.ReadToEndAsync()
        $child.StandardInput.BaseStream.Write($payload, 0, $payload.Length)
        $child.StandardInput.Close()
        if (!$child.WaitForExit(2000)) {{ $child.Kill(); $child.WaitForExit(); return }}
        $copy.GetAwaiter().GetResult()
        if ($show) {{ $bytes = $output.ToArray(); [Console]::OpenStandardOutput().Write($bytes, 0, $bytes.Length) }}
    }} catch {{ }} finally {{ $child.Dispose() }}
}}
try {{
    $stream = [Console]::OpenStandardInput()
    $buffer = New-Object byte[] 8192
    $inputCopy = New-Object System.IO.MemoryStream
    while (($count = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {{
        if ($inputCopy.Length + $count -gt 1048576) {{ exit 0 }}
        $inputCopy.Write($buffer, 0, $count)
    }}
    $payload = $inputCopy.ToArray()
    Invoke-StatusCommand {executable} {arguments} $payload $false
    {original}
}} catch {{ }}
"#,
        executable = literal(executable),
        arguments = literal(&format!("statusline {}", windows_arg(cache))),
    )
}

/// CommandLineToArgvW quoting for an argument passed directly to a native exe.
fn windows_arg(text: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in text.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        result.extend(std::iter::repeat_n(
            '\\',
            if ch == '"' { slashes * 2 + 1 } else { slashes },
        ));
        result.push(ch);
        slashes = 0;
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}
