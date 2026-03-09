//! Stage output algebra and interaction policies.
//!
//! [`StageOutput`] models the result of executing a pipeline stage,
//! inspired by Orlando's `Step<T>` (Continue/Stop) with additional
//! variants for retry loops and fan-out parallelism.

use crate::context::ContextBundle;
use crate::error::SakamotoError;

/// The result of executing a single pipeline stage.
///
/// This is the core control-flow algebra for pipeline execution:
/// - `Continue` advances to the next stage(s)
/// - `Retry` loops back to a previous stage
/// - `Fail` halts the pipeline
/// - `Fork` fans out to parallel stages
#[derive(Debug)]
pub enum StageOutput {
    /// Advance to the next stage(s) with updated context.
    Continue(ContextBundle),

    /// Loop back to retry a previous stage.
    Retry {
        context: ContextBundle,
        reason: String,
    },

    /// Halt the pipeline with an error.
    Fail(SakamotoError),

    /// Fan out to multiple parallel stages.
    Fork(Vec<ContextBundle>),
}

impl StageOutput {
    /// Map a function over the context if this is `Continue`.
    /// Other variants pass through unchanged.
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnOnce(ContextBundle) -> ContextBundle,
    {
        match self {
            Self::Continue(ctx) => Self::Continue(f(ctx)),
            other => other,
        }
    }

    /// Monadic bind: chain a fallible operation onto a `Continue`.
    /// `Retry`, `Fail`, and `Fork` pass through unchanged.
    pub fn and_then<F>(self, f: F) -> Self
    where
        F: FnOnce(ContextBundle) -> StageOutput,
    {
        match self {
            Self::Continue(ctx) => f(ctx),
            other => other,
        }
    }

    /// Returns `true` if execution should continue to the next stage.
    pub fn is_continue(&self) -> bool {
        matches!(self, Self::Continue(_))
    }

    /// Returns `true` if the pipeline should halt.
    pub fn is_fail(&self) -> bool {
        matches!(self, Self::Fail(_))
    }

    /// Returns `true` if the stage requests a retry.
    pub fn is_retry(&self) -> bool {
        matches!(self, Self::Retry { .. })
    }

    /// Returns `true` if the stage fans out to parallel execution.
    pub fn is_fork(&self) -> bool {
        matches!(self, Self::Fork(_))
    }

    /// Extract the context from `Continue`, consuming self.
    /// Returns `None` for other variants.
    pub fn into_context(self) -> Option<ContextBundle> {
        match self {
            Self::Continue(ctx) => Some(ctx),
            _ => None,
        }
    }
}

/// How a pipeline stage interacts with the human operator.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum InteractionPolicy {
    /// No human input. Runs to completion.
    #[default]
    Autonomous,
    /// Presents output, waits for approval before advancing.
    Confirm,
    /// Human can edit or augment the LLM's output.
    Collaborate,
    /// Hands off to human entirely, resumes on signal.
    Delegate,
}

/// The kind of computation a pipeline stage performs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    /// Gather context from the task description and environment.
    Context,
    /// LLM generates an execution plan.
    Plan,
    /// ReAct loop: LLM iteratively calls tools.
    Code,
    /// Deterministic lint check.
    Lint,
    /// Deterministic test execution.
    Test,
    /// Git add + commit.
    Commit,
    /// Create a pull request.
    Pr,
    /// Custom stage with a user-defined name.
    Custom(String),
}

impl std::fmt::Display for StageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Context => write!(f, "context"),
            Self::Plan => write!(f, "plan"),
            Self::Code => write!(f, "code"),
            Self::Lint => write!(f, "lint"),
            Self::Test => write!(f, "test"),
            Self::Commit => write!(f, "commit"),
            Self::Pr => write!(f, "pr"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// Configuration for a single pipeline stage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StageConfig {
    /// Human-readable name for this stage.
    pub name: String,

    /// What kind of computation this stage performs.
    pub kind: StageKind,

    /// Which LLM backend to use (references a key in the config).
    #[serde(default)]
    pub llm_backend: Option<String>,

    /// Which toolset to use (references a key in the config).
    #[serde(default)]
    pub toolset: Option<String>,

    /// How this stage interacts with the operator.
    #[serde(default)]
    pub interaction: InteractionPolicy,

    /// Maximum iterations for ReAct-style loops.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,

    /// Maximum CI/validation retry rounds.
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// Timeout for this stage.
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// Shell command (for Lint/Test stages).
    #[serde(default)]
    pub command: Option<String>,
}

fn default_max_iterations() -> usize {
    20
}

fn default_max_retries() -> usize {
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_output_continue_maps() {
        let ctx = ContextBundle::default();
        let output = StageOutput::Continue(ctx);
        let mapped = output.map(|mut c| {
            c.task_description = "modified".into();
            c
        });
        assert!(mapped.is_continue());
        match mapped {
            StageOutput::Continue(c) => assert_eq!(c.task_description, "modified"),
            _ => panic!("expected Continue"),
        }
    }

    #[test]
    fn stage_output_fail_passes_through_map() {
        let output = StageOutput::Fail(SakamotoError::CyclicGraph);
        let mapped = output.map(|c| c);
        assert!(mapped.is_fail());
    }

    #[test]
    fn stage_output_and_then_chains() {
        let ctx = ContextBundle::default();
        let output = StageOutput::Continue(ctx);
        let chained = output.and_then(|mut c| {
            c.task_description = "chained".into();
            StageOutput::Continue(c)
        });
        assert!(chained.is_continue());
    }

    #[test]
    fn stage_output_and_then_short_circuits_on_fail() {
        let output = StageOutput::Fail(SakamotoError::CyclicGraph);
        let chained = output.and_then(|_| panic!("should not be called"));
        assert!(chained.is_fail());
    }

    #[test]
    fn stage_output_into_context() {
        let ctx = ContextBundle {
            task_description: "test task".into(),
            ..Default::default()
        };
        let output = StageOutput::Continue(ctx);
        let extracted = output.into_context().expect("should be Some");
        assert_eq!(extracted.task_description, "test task");
    }

    #[test]
    fn stage_output_into_context_returns_none_on_fail() {
        let output = StageOutput::Fail(SakamotoError::CyclicGraph);
        assert!(output.into_context().is_none());
    }

    #[test]
    fn stage_output_fork_detected() {
        let output = StageOutput::Fork(vec![ContextBundle::default()]);
        assert!(output.is_fork());
        assert!(!output.is_continue());
    }

    #[test]
    fn interaction_policy_default_is_autonomous() {
        assert_eq!(InteractionPolicy::default(), InteractionPolicy::Autonomous);
    }

    #[test]
    fn stage_kind_displays() {
        assert_eq!(StageKind::Code.to_string(), "code");
        assert_eq!(
            StageKind::Custom("deploy".into()).to_string(),
            "custom:deploy"
        );
    }

    #[test]
    fn stage_config_deserializes_with_defaults() {
        let json = r#"{"name": "lint", "kind": "lint", "command": "cargo clippy"}"#;
        let config: StageConfig = serde_json::from_str(json).expect("should parse");
        assert_eq!(config.name, "lint");
        assert_eq!(config.kind, StageKind::Lint);
        assert_eq!(config.interaction, InteractionPolicy::Autonomous);
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.command.as_deref(), Some("cargo clippy"));
    }

    #[test]
    fn interaction_policy_serializes_lowercase() {
        let json = serde_json::to_string(&InteractionPolicy::Collaborate).unwrap();
        assert_eq!(json, "\"collaborate\"");
    }
}
