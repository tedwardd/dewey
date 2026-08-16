pub mod jsonrpc;

use crate::errors::CliError;
use crate::module::jsonrpc::{decode_response, encode_request, Response};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ModuleHost {
    pub name: String,
    command: Vec<String>,
    cwd: PathBuf,
}

impl ModuleHost {
    pub fn new(name: String, manifest_command: &[String], cwd: PathBuf) -> ModuleHost {
        let command = resolve_command(manifest_command, &cwd);
        ModuleHost {
            name,
            command,
            cwd,
        }
    }

    pub fn call(&self, method: &str, params: Value, id: u64) -> Result<Value, CliError> {
        self.call_with_timeout(method, params, id, EXCHANGE_TIMEOUT)
    }

    pub fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        id: u64,
        timeout: Duration,
    ) -> Result<Value, CliError> {
        let mut child = Command::new(&self.command[0])
            .args(&self.command[1..])
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                CliError::Module(format!(
                    "module {}: cannot spawn {:?}: {e}",
                    self.name, self.command[0]
                ))
            })?;

        let mut stdin = child.stdin.take().expect("stdin piped");
        let line = encode_request(id, method, &params);
        if let Err(e) = stdin.write_all(line.as_bytes()) {
            let _ = child.kill();
            return Err(CliError::Module(format!(
                "module {}: failed to send request: {e}",
                self.name
            )));
        }
        drop(stdin); // EOF so the module sees a complete request

        let stdout = child.stdout.take().expect("stdout piped");
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let n = reader.read_line(&mut line);
            let _ = tx.send(n.map(|n| if n == 0 { None } else { Some(line) }));
        });

        let outcome = match rx.recv_timeout(timeout) {
            Ok(Ok(Some(line))) => {
                let resp: Response = decode_response(line.trim_end())?;
                if let Some(err) = resp.error {
                    return Err(CliError::Module(format!(
                        "module {}: {} (code {})",
                        self.name, err.message, err.code
                    )));
                }
                match resp.result {
                    Some(value) => Ok(value),
                    None => Err(CliError::Module(format!(
                        "module {}: response has neither result nor error",
                        self.name
                    ))),
                }
            }
            Ok(Ok(None)) => Err(CliError::Module(format!(
                "module {}: exited without a response",
                self.name
            ))),
            Ok(Err(e)) => Err(CliError::Module(format!(
                "module {}: read error: {e}",
                self.name
            ))),
            Err(_) => Err(CliError::Module(format!(
                "module {}: timed out after {}s",
                self.name,
                timeout.as_secs()
            ))),
        };

        let _ = handle.join();
        match outcome {
            Ok(v) => {
                let _ = finish(child, Duration::from_secs(2));
                Ok(v)
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(e)
            }
        }
    }
}

fn resolve_command(cmd: &[String], dir: &Path) -> Vec<String> {
    let program = if Path::new(&cmd[0]).components().count() > 1 || dir.join(&cmd[0]).is_file() {
        dir.join(&cmd[0]).to_string_lossy().into_owned()
    } else {
        cmd[0].clone()
    };
    let mut out = vec![program];
    for arg in &cmd[1..] {
        let p = Path::new(arg);
        out.push(if p.is_absolute() {
            arg.clone()
        } else {
            dir.join(arg).to_string_lossy().into_owned()
        });
    }
    out
}

fn finish(mut child: Child, grace: Duration) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + grace;
    loop {
        if let Some(status) = child.try_wait()? {
            let _ = status;
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(script: &str) -> ModuleHost {
        ModuleHost::new(
            "fake".into(),
            &["python3".into(), script.into()],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        )
    }

    #[test]
    fn returns_result_value() {
        let v = host("fake-ok.py").call("ping", json!({}), 1).unwrap();
        assert_eq!(v, json!({"ok": true}));
    }

    #[test]
    fn surfaces_module_error() {
        let err = host("fake-error.py").call("ping", json!({}), 1).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("boom"), "got: {err}");
    }

    #[test]
    fn crash_without_response_is_module_error() {
        let err = host("fake-crash.py").call("ping", json!({}), 1).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("exited without a response"), "got: {err}");
    }

    #[test]
    fn timeout_kills_module() {
        let err = host("fake-sleep.py")
            .call_with_timeout("ping", json!({}), 1, Duration::from_secs(1))
            .unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("timed out"), "got: {err}");
    }

    #[test]
    fn invalid_response_is_protocol_error() {
        let err = host("fake-badjson.py").call("ping", json!({}), 1).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("module protocol"), "got: {err}");
    }

    #[test]
    fn response_with_neither_result_nor_error_is_module_error() {
        let err = host("fake-null.py").call("ping", json!({}), 1).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("neither result nor error"), "got: {err}");
    }
}
