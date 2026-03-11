//! Stdio transport over a child process.
//!
//! pmcp's built-in [`StdioTransport`](pmcp::StdioTransport) uses the current
//! process's stdin/stdout. For MCP servers spawned as subprocesses, this module
//! provides a transport that communicates over the child's piped stdin/stdout.

use std::collections::HashMap;
use std::process::Stdio;

use pmcp::StdioTransport;
use pmcp::error::{Result, TransportError};
use pmcp::shared::transport::{Transport, TransportMessage};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// A transport that communicates with a child process via piped stdin/stdout.
///
/// The child process is spawned when the transport is created and killed
/// when it is closed.
pub struct ChildProcessTransport {
    stdin: Mutex<tokio::process::ChildStdin>,
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    child: Mutex<Child>,
}

impl std::fmt::Debug for ChildProcessTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildProcessTransport").finish()
    }
}

impl ChildProcessTransport {
    /// Spawn a child process and create a transport connected to its stdin/stdout.
    pub fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> std::result::Result<Self, sakamoto_types::SakamotoError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| sakamoto_types::SakamotoError::McpError {
                server: command.to_string(),
                reason: format!("failed to spawn: {e}"),
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| sakamoto_types::SakamotoError::McpError {
                server: command.to_string(),
                reason: "failed to capture child stdin".into(),
            })?;

        let stdout =
            child
                .stdout
                .take()
                .ok_or_else(|| sakamoto_types::SakamotoError::McpError {
                    server: command.to_string(),
                    reason: "failed to capture child stdout".into(),
                })?;

        Ok(Self {
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            child: Mutex::new(child),
        })
    }
}

#[async_trait::async_trait]
impl Transport for ChildProcessTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        let json_bytes = StdioTransport::serialize_message(&message)?;
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(&json_bytes)
            .await
            .map_err(|e| -> pmcp::Error { TransportError::Io(e.to_string()).into() })?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| -> pmcp::Error { TransportError::Io(e.to_string()).into() })?;
        stdin
            .flush()
            .await
            .map_err(|e| -> pmcp::Error { TransportError::Io(e.to_string()).into() })?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = stdout
                .read_line(&mut line)
                .await
                .map_err(|e| -> pmcp::Error { TransportError::Io(e.to_string()).into() })?;

            if bytes_read == 0 {
                return Err(TransportError::ConnectionClosed.into());
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            return StdioTransport::parse_message(trimmed.as_bytes());
        }
    }

    async fn close(&mut self) -> Result<()> {
        // Drop stdin to signal EOF to the child, then kill
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn transport_type(&self) -> &'static str {
        "child-process-stdio"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_nonexistent_command_fails() {
        let result = ChildProcessTransport::spawn(
            "nonexistent-binary-that-does-not-exist",
            &[],
            &HashMap::new(),
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn spawn_and_close() {
        let result = ChildProcessTransport::spawn("cat", &[], &HashMap::new());
        if let Ok(mut transport) = result {
            assert!(transport.close().await.is_ok());
        }
    }

    #[tokio::test]
    async fn spawn_with_env() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".into(), "test_value".into());
        let result = ChildProcessTransport::spawn("cat", &[], &env);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn debug_format() {
        let transport = ChildProcessTransport::spawn("cat", &[], &HashMap::new()).unwrap();
        let debug = format!("{transport:?}");
        assert!(debug.contains("ChildProcessTransport"));
    }
}
