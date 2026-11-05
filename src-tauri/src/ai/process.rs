//! `codex app-server` as a long-lived child process speaking JSON lines on stdio.
use super::{AppServer, Connection};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

pub struct CodexCli;

impl AppServer for CodexCli {
    fn installed(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            crate::ports::Programs::installed(&crate::ports::SystemPrograms, "codex")
        }
        #[cfg(windows)]
        {
            std::env::var_os("PATH")
                .and_then(|path| super::windows::codex(&path))
                .is_some()
        }
    }

    fn spawn(&mut self) -> Result<Box<dyn Connection>, ()> {
        // stderr is discarded: it may carry account details.
        #[cfg(target_os = "linux")]
        let program = std::path::PathBuf::from("codex");
        #[cfg(windows)]
        let program = super::windows::codex(&std::env::var_os("PATH").ok_or(())?).ok_or(())?;
        let mut command = Command::new(program);
        command.arg("app-server");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ())?;
        #[cfg(windows)]
        let job = match super::windows::Job::new(&child) {
            Ok(job) => job,
            Err(()) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(());
            }
        };
        let stdin = child.stdin.take().ok_or(())?;
        let stdout = child.stdout.take().ok_or(())?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                // Lines that are not JSON are ignored, never logged.
                if let Ok(message) = serde_json::from_str::<Value>(&line) {
                    if tx.send(message).is_err() {
                        break;
                    }
                }
            }
        });
        Ok(Box::new(Process {
            #[cfg(windows)]
            _job: job,
            child,
            stdin,
            messages: rx,
        }))
    }
}

struct Process {
    #[cfg(windows)]
    _job: super::windows::Job,
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
}

impl Connection for Process {
    fn send(&mut self, message: &Value) -> Result<(), ()> {
        writeln!(self.stdin, "{message}").map_err(|_| ())?;
        self.stdin.flush().map_err(|_| ())
    }

    fn receive(&mut self) -> Vec<Value> {
        self.messages.try_iter().collect()
    }

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(all(windows, test))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "CI installs the pinned native CLI in an isolated home"]
    fn windows_npm_codex_initializes_over_stdio_without_login() {
        let mut connection = CodexCli
            .spawn()
            .expect("resolve the npm launcher to its native executable");
        connection.send(&serde_json::json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"kvojps-monitor-test","version":"0.0.0"}}})).unwrap();
        let started = std::time::Instant::now();
        loop {
            for message in connection.receive() {
                if message["id"] == 1 {
                    assert!(message["result"].is_object(), "initialization must succeed");
                    connection
                        .send(&serde_json::json!({"method":"initialized"}))
                        .unwrap();
                    return;
                }
            }
            assert!(
                connection.alive() && started.elapsed().as_secs() < 10,
                "native stdio handshake deadline"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
