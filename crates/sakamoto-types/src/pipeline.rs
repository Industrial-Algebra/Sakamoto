//! Phantom-typed pipeline state machine.
//!
//! Pipeline state transitions are encoded at the type level, preventing
//! invalid operations at compile time. Inspired by Amari's `TypedSystem`
//! multi-parameter phantom types and IA-MCP's `ApiIndex<Validated>`.

use std::marker::PhantomData;

use crate::context::ContextBundle;
use crate::error::Diagnostic;
use crate::llm::TokenUsage;
use crate::stage::StageConfig;

// ── Phantom state markers ──────────────────────────────────────────

/// Pipeline has been defined but not started.
#[derive(Debug, Clone, Copy)]
pub struct Pending;

/// Context has been gathered and pre-hydrated.
#[derive(Debug, Clone, Copy)]
pub struct Hydrated;

/// LLM has produced an execution plan.
#[derive(Debug, Clone, Copy)]
pub struct Planned;

/// Code changes have been applied.
#[derive(Debug, Clone, Copy)]
pub struct Executed;

/// Lint and tests have passed.
#[derive(Debug, Clone, Copy)]
pub struct Validated;

/// Output has been emitted (PR, commit, patch).
#[derive(Debug, Clone, Copy)]
pub struct Emitted;

/// Sealed trait for valid pipeline states.
pub trait PipelineState: private::Sealed + std::fmt::Debug {}

impl PipelineState for Pending {}
impl PipelineState for Hydrated {}
impl PipelineState for Planned {}
impl PipelineState for Executed {}
impl PipelineState for Validated {}
impl PipelineState for Emitted {}

mod private {
    pub trait Sealed {}
    impl Sealed for super::Pending {}
    impl Sealed for super::Hydrated {}
    impl Sealed for super::Planned {}
    impl Sealed for super::Executed {}
    impl Sealed for super::Validated {}
    impl Sealed for super::Emitted {}
}

// ── Pipeline struct ────────────────────────────────────────────────

/// A pipeline with a phantom-typed state.
///
/// State transitions consume `self` and return a new `Pipeline` in the
/// next state, ensuring at compile time that stages execute in order.
///
/// ```
/// use sakamoto_types::pipeline::*;
/// use sakamoto_types::context::ContextBundle;
///
/// // Only Pipeline<Pending> can transition to Hydrated
/// let pending = Pipeline::<Pending>::new("fix clippy warnings", vec![]);
/// let hydrated: Pipeline<Hydrated> = pending.advance(ContextBundle::default());
/// ```
#[derive(Debug)]
pub struct Pipeline<S: PipelineState> {
    /// The accumulated context flowing through the pipeline.
    pub context: ContextBundle,
    /// Stage configurations for this pipeline.
    pub stages: Vec<StageConfig>,
    /// Diagnostics emitted during execution.
    pub diagnostics: Vec<Diagnostic>,
    /// Accumulated token usage across all LLM calls.
    pub token_usage: TokenUsage,
    /// Phantom state marker.
    _state: PhantomData<S>,
}

impl Pipeline<Pending> {
    /// Create a new pipeline in the `Pending` state.
    pub fn new(task: impl Into<String>, stages: Vec<StageConfig>) -> Self {
        Self {
            context: ContextBundle::from_task(task),
            stages,
            diagnostics: Vec::new(),
            token_usage: TokenUsage::default(),
            _state: PhantomData,
        }
    }
}

impl<S: PipelineState> Pipeline<S> {
    /// Transition to the next state, replacing the context bundle.
    ///
    /// This is the generic state-advancement method. Specific transitions
    /// (e.g., `hydrate`, `plan`, `execute`) are implemented on the
    /// concrete state types in `sakamoto-core`, calling this internally.
    pub fn advance<T: PipelineState>(self, context: ContextBundle) -> Pipeline<T> {
        Pipeline {
            context,
            stages: self.stages,
            diagnostics: self.diagnostics,
            token_usage: self.token_usage,
            _state: PhantomData,
        }
    }

    /// Add a diagnostic message.
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Accumulate token usage from an LLM call.
    pub fn add_usage(&mut self, usage: &TokenUsage) {
        self.token_usage.accumulate(usage);
    }

    /// Get the current task description.
    pub fn task(&self) -> &str {
        &self.context.task_description
    }
}

/// The result of a completed pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineResult {
    /// The final context bundle with all accumulated data.
    pub context: ContextBundle,
    /// All diagnostics from the pipeline run.
    pub diagnostics: Vec<Diagnostic>,
    /// Total token usage.
    pub token_usage: TokenUsage,
    /// The output that was emitted (PR URL, commit SHA, etc.).
    pub output: PipelineOutput,
}

/// What a pipeline produced as its final output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipelineOutput {
    /// A pull request was created.
    PullRequest { url: String, number: u64 },
    /// A commit was made.
    Commit { sha: String, message: String },
    /// A patch file was generated.
    Patch { path: String },
    /// Files were modified in the working directory.
    Files { paths: Vec<String> },
    /// The pipeline produced no output (e.g., validation-only run).
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::{StageConfig, StageKind};

    fn sample_stages() -> Vec<StageConfig> {
        vec![StageConfig {
            name: "lint".into(),
            kind: StageKind::Lint,
            llm_backend: None,
            toolset: None,
            interaction: Default::default(),
            max_iterations: 20,
            max_retries: 2,
            timeout_secs: None,
            command: Some("cargo clippy".into()),
        }]
    }

    #[test]
    fn pipeline_starts_in_pending() {
        let p = Pipeline::<Pending>::new("fix clippy", sample_stages());
        assert_eq!(p.task(), "fix clippy");
        assert_eq!(p.stages.len(), 1);
    }

    #[test]
    fn pipeline_advances_through_states() {
        let pending = Pipeline::<Pending>::new("task", vec![]);

        let hydrated: Pipeline<Hydrated> = pending.advance(ContextBundle::from_task("task"));
        assert_eq!(hydrated.task(), "task");

        let planned: Pipeline<Planned> = hydrated.advance(ContextBundle::from_task("task"));
        let executed: Pipeline<Executed> = planned.advance(ContextBundle::from_task("task"));
        let validated: Pipeline<Validated> = executed.advance(ContextBundle::from_task("task"));
        let _emitted: Pipeline<Emitted> = validated.advance(ContextBundle::from_task("task"));
    }

    #[test]
    fn pipeline_preserves_stages_across_transitions() {
        let pending = Pipeline::<Pending>::new("task", sample_stages());
        let hydrated: Pipeline<Hydrated> = pending.advance(ContextBundle::from_task("task"));
        assert_eq!(hydrated.stages.len(), 1);
        assert_eq!(hydrated.stages[0].name, "lint");
    }

    #[test]
    fn pipeline_accumulates_diagnostics() {
        let mut pending = Pipeline::<Pending>::new("task", vec![]);
        pending.add_diagnostic(Diagnostic {
            severity: crate::error::Severity::Info,
            stage: "context".into(),
            message: "found 3 references".into(),
        });
        assert_eq!(pending.diagnostics.len(), 1);

        let hydrated: Pipeline<Hydrated> = pending.advance(ContextBundle::from_task("task"));
        assert_eq!(hydrated.diagnostics.len(), 1);
    }

    #[test]
    fn pipeline_accumulates_token_usage() {
        let mut pending = Pipeline::<Pending>::new("task", vec![]);
        pending.add_usage(&TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: Some(0.01),
        });
        pending.add_usage(&TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cost_usd: Some(0.02),
        });
        assert_eq!(pending.token_usage.total(), 450);
    }

    #[test]
    fn pipeline_output_serializes() {
        let output = PipelineOutput::PullRequest {
            url: "https://github.com/org/repo/pull/1".into(),
            number: 1,
        };
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["type"], "pull_request");
        assert_eq!(json["number"], 1);
    }

    #[test]
    fn pipeline_result_roundtrips() {
        let result = PipelineResult {
            context: ContextBundle::from_task("test"),
            diagnostics: vec![],
            token_usage: TokenUsage::default(),
            output: PipelineOutput::None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: PipelineResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.context.task_description, "test");
    }
}
