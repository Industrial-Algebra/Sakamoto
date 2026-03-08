//! Shell command execution tool.
//!
//! This tool runs a shell command in a specified working directory and
//! returns its stdout/stderr. It is the primary mechanism by which the
//! coding agent runs linters, test suites, and build commands.

use sakamoto_types::{SakamotoError, ToolDef};
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

use crate::tool::Tool;

/// A tool that executes shell commands in a subprocess.
pub struct ShellTool {
    /// Working directory for command execution.
    working_dir: PathBuf,
    /// Maximum time a command may run before being killed.
    timeout: Duration,
}

impl ShellTool {
    /// Create a new shell tool with the given working directory and timeout.
    pub fn new(working_dir: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            working_dir: working_dir.into(),
            timeout,
        }
    }
}

#[async_trait::async_trait]
impl Tool for ShellTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "shell".into(),
            description: "Execute a shell command and return its output".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, SakamotoError> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| SakamotoError::ToolCallFailed {
                tool: "shell".into(),
                reason: "missing 'command' field".into(),
            })?;

        let output = tokio::time::timeout(
            self.timeout,
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&self.working_dir)
                .output(),
        )
        .await
        .map_err(|_| SakamotoError::Timeout {
            stage: "shell".into(),
            elapsed: self.timeout,
        })?
        .map_err(SakamotoError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str("[stderr]\n");
            result.push_str(&stderr);
        }

        if output.status.success() {
            Ok(result)
        } else {
            let code = output.status.code().unwrap_or(-1);
            Err(SakamotoError::ToolCallFailed {
                tool: "shell".into(),
                reason: format!("exit code {code}\n{result}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn test_shell() -> ShellTool {
        ShellTool::new(
            env::current_dir().expect("has cwd"),
            Duration::from_secs(10),
        )
    }

    #[tokio::test]
    async fn shell_definition() {
        let tool = test_shell();
        let def = tool.definition();
        assert_eq!(def.name, "shell");
    }

    #[tokio::test]
    async fn shell_echo() {
        let tool = test_shell();
        let result = tool
            .execute(serde_json::json!({"command": "echo hello"}))
            .await;
        assert_eq!(
            result.ok().map(|s| s.trim().to_string()),
            Some("hello".into())
        );
    }

    #[tokio::test]
    async fn shell_failing_command() {
        let tool = test_shell();
        let result = tool.execute(serde_json::json!({"command": "exit 1"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn shell_missing_command_field() {
        let tool = test_shell();
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn shell_captures_stderr() {
        let tool = test_shell();
        let result = tool
            .execute(serde_json::json!({"command": "echo err >&2"}))
            .await;
        let output = result.unwrap();
        assert!(output.contains("err"));
    }

    #[tokio::test]
    async fn shell_timeout() {
        let tool = ShellTool::new(
            env::current_dir().expect("has cwd"),
            Duration::from_millis(100),
        );
        let result = tool
            .execute(serde_json::json!({"command": "sleep 10"}))
            .await;
        assert!(result.is_err());
    }
}
