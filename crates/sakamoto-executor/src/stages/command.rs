//! Command-based stages (lint, test, commit, pr).
//!
//! These stages execute shell commands and interpret the results.
//! They are used for deterministic validation (lint, test) and
//! git operations (commit, pr).

use sakamoto_core::stage::{Stage, StageContext};
use sakamoto_types::{ContextBundle, Diagnostic, SakamotoError, Severity, StageOutput};
use std::path::PathBuf;
use tokio::process::Command;

/// A stage that runs a shell command and returns Continue on success
/// or Retry on failure (for validation stages like lint/test).
pub struct CommandStage {
    stage_name: String,
    /// If true, failure produces Retry instead of Fail.
    retriable: bool,
}

impl CommandStage {
    /// Create a lint stage (retriable on failure).
    pub fn lint() -> Self {
        Self {
            stage_name: "lint".into(),
            retriable: true,
        }
    }

    /// Create a test stage (retriable on failure).
    pub fn test() -> Self {
        Self {
            stage_name: "test".into(),
            retriable: true,
        }
    }

    /// Create a commit stage (fails on error).
    pub fn commit() -> Self {
        Self {
            stage_name: "commit".into(),
            retriable: false,
        }
    }

    /// Create a PR stage (fails on error).
    pub fn pr() -> Self {
        Self {
            stage_name: "pr".into(),
            retriable: false,
        }
    }
}

#[async_trait::async_trait]
impl Stage for CommandStage {
    fn name(&self) -> &str {
        &self.stage_name
    }

    async fn execute(&self, mut context: ContextBundle, ctx: &StageContext) -> StageOutput {
        let command = match &ctx.config.command {
            Some(cmd) => cmd.clone(),
            None => {
                return StageOutput::Fail(SakamotoError::StageFailure {
                    stage: self.stage_name.clone(),
                    reason: "no command configured".into(),
                });
            }
        };

        // Use the working directory from context metadata, or current dir
        let working_dir = context
            .metadata
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let output = match Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&working_dir)
            .output()
            .await
        {
            Ok(output) => output,
            Err(e) => {
                return StageOutput::Fail(SakamotoError::Io(e));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            if !stdout.is_empty() {
                context
                    .metadata
                    .insert(format!("{}_stdout", self.stage_name), stdout.into());
            }
            StageOutput::Continue(context)
        } else {
            let combined = if stderr.is_empty() {
                stdout.clone()
            } else {
                format!("{stdout}\n{stderr}")
            };

            context.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                stage: self.stage_name.clone(),
                message: combined.clone(),
            });

            if self.retriable {
                StageOutput::Retry {
                    context,
                    reason: format!(
                        "{} failed (exit {}): {}",
                        self.stage_name,
                        output.status.code().unwrap_or(-1),
                        combined.lines().take(5).collect::<Vec<_>>().join("\n")
                    ),
                }
            } else {
                StageOutput::Fail(SakamotoError::StageFailure {
                    stage: self.stage_name.clone(),
                    reason: combined,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sakamoto_types::stage::StageConfig;

    fn ctx_with_command(cmd: &str) -> StageContext {
        StageContext {
            llm: None,
            tools: None,
            config: StageConfig {
                command: Some(cmd.into()),
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn lint_stage_success() {
        let stage = CommandStage::lint();
        let ctx = ctx_with_command("true");
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_continue());
    }

    #[tokio::test]
    async fn lint_stage_failure_retries() {
        let stage = CommandStage::lint();
        let ctx = ctx_with_command("echo 'lint error' && exit 1");
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_retry());
    }

    #[tokio::test]
    async fn commit_stage_failure_fails() {
        let stage = CommandStage::commit();
        let ctx = ctx_with_command("exit 1");
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_fail());
    }

    #[tokio::test]
    async fn stage_without_command_fails() {
        let stage = CommandStage::lint();
        let ctx = StageContext {
            llm: None,
            tools: None,
            config: StageConfig::default(),
        };
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_fail());
    }

    #[tokio::test]
    async fn successful_command_captures_stdout() {
        let stage = CommandStage::lint();
        let ctx = ctx_with_command("echo 'all clean'");
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        let bundle = output.into_context().unwrap();
        assert!(bundle.metadata.contains_key("lint_stdout"));
    }

    #[tokio::test]
    async fn failed_command_adds_diagnostic() {
        let stage = CommandStage::test();
        let ctx = ctx_with_command("echo 'test failed' >&2 && exit 1");
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        match output {
            StageOutput::Retry { context, .. } => {
                assert!(!context.diagnostics.is_empty());
                assert_eq!(context.diagnostics[0].stage, "test");
            }
            _ => panic!("expected Retry"),
        }
    }
}
