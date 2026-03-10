//! Pipeline runner — executes a DAG of stages with retry support.
//!
//! The runner walks the DAG level by level, executing stages within each
//! level (potentially in parallel in future versions). When a validation
//! stage returns `Retry`, the runner re-runs from a configurable retry
//! target (typically the "code" stage) up to `max_retries` times.

use sakamoto_types::{
    ContextBundle, Diagnostic, SakamotoError, Severity, StageOutput, llm::TokenUsage,
    stage::StageConfig,
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::dag::PipelineDag;
use crate::stage::{LlmClient, Stage, StageContext, ToolExecutor};

/// Result of a pipeline run.
#[derive(Debug)]
pub struct RunResult {
    /// Final context after all stages have executed.
    pub context: ContextBundle,
    /// Accumulated token usage across all LLM calls.
    pub token_usage: TokenUsage,
    /// Names of stages that were executed (in order, including retries).
    pub stages_executed: Vec<String>,
    /// Number of retry rounds that occurred.
    pub retries: usize,
}

/// Configuration for a pipeline runner.
pub struct RunnerConfig {
    /// Maximum number of retry rounds (validation → code → validation).
    pub max_retries: usize,
    /// Stage name to restart from on retry (e.g., "code").
    pub retry_from: Option<String>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            retry_from: None,
        }
    }
}

/// Executes a pipeline DAG.
pub struct PipelineRunner {
    dag: PipelineDag,
    stages: HashMap<String, Arc<dyn Stage>>,
    stage_configs: HashMap<String, StageConfig>,
    llm_clients: HashMap<String, Arc<dyn LlmClient>>,
    tool_executors: HashMap<String, Arc<dyn ToolExecutor>>,
    config: RunnerConfig,
}

impl PipelineRunner {
    /// Create a new pipeline runner.
    pub fn new(dag: PipelineDag, config: RunnerConfig) -> Self {
        Self {
            dag,
            stages: HashMap::new(),
            stage_configs: HashMap::new(),
            llm_clients: HashMap::new(),
            tool_executors: HashMap::new(),
            config,
        }
    }

    /// Register a stage implementation.
    pub fn add_stage(&mut self, stage: Arc<dyn Stage>) {
        self.stages.insert(stage.name().to_string(), stage);
    }

    /// Register a stage config.
    pub fn add_stage_config(&mut self, name: String, config: StageConfig) {
        self.stage_configs.insert(name, config);
    }

    /// Register a named LLM client.
    pub fn add_llm_client(&mut self, name: String, client: Arc<dyn LlmClient>) {
        self.llm_clients.insert(name, client);
    }

    /// Register a named tool executor.
    pub fn add_tool_executor(&mut self, name: String, executor: Arc<dyn ToolExecutor>) {
        self.tool_executors.insert(name, executor);
    }

    /// Execute the pipeline.
    pub async fn run(&self, initial_context: ContextBundle) -> Result<RunResult, SakamotoError> {
        let levels = self.dag.execution_levels()?;
        let flat_order: Vec<String> = levels.into_iter().flatten().collect();

        let mut context = initial_context;
        let total_usage = TokenUsage::default();
        let mut stages_executed = Vec::new();
        let mut retries = 0;

        // Find the retry-from index (if configured)
        let retry_from_idx = self
            .config
            .retry_from
            .as_ref()
            .and_then(|name| flat_order.iter().position(|s| s == name));

        let mut current_idx = 0;

        while current_idx < flat_order.len() {
            let stage_name = &flat_order[current_idx];

            let stage = self
                .stages
                .get(stage_name)
                .ok_or_else(|| SakamotoError::StageNotFound(stage_name.clone()))?;

            let stage_ctx = self.build_stage_context(stage_name);
            let output = stage.execute(context.clone(), &stage_ctx).await;
            stages_executed.push(stage_name.clone());

            match output {
                StageOutput::Continue(new_context) => {
                    context = new_context;
                    current_idx += 1;
                }
                StageOutput::Retry {
                    context: retry_context,
                    reason,
                } => {
                    if retries >= self.config.max_retries {
                        return Err(SakamotoError::ValidationExhausted {
                            rounds: retries,
                            reason,
                        });
                    }

                    context = retry_context;
                    context.diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        stage: stage_name.clone(),
                        message: format!("retry {}: {reason}", retries + 1),
                    });

                    retries += 1;

                    // Jump back to retry target or start
                    current_idx = retry_from_idx.unwrap_or(0);
                }
                StageOutput::Fail(err) => {
                    return Err(err);
                }
                StageOutput::Fork(_bundles) => {
                    // Fork (parallel fan-out) is planned for v0.2
                    return Err(SakamotoError::StageFailure {
                        stage: stage_name.clone(),
                        reason: "Fork not yet supported".into(),
                    });
                }
            }
        }

        Ok(RunResult {
            context,
            token_usage: total_usage,
            stages_executed,
            retries,
        })
    }

    /// Build a [`StageContext`] for the given stage, resolving the LLM
    /// client and tool executor from the stage's config.
    fn build_stage_context(&self, stage_name: &str) -> StageContext {
        let config = self
            .stage_configs
            .get(stage_name)
            .cloned()
            .unwrap_or_else(|| StageConfig {
                name: stage_name.into(),
                ..Default::default()
            });

        let llm = config
            .llm_backend
            .as_ref()
            .and_then(|name| self.llm_clients.get(name))
            .cloned();

        let tools = config
            .toolset
            .as_ref()
            .and_then(|name| self.tool_executors.get(name))
            .cloned();

        StageContext { llm, tools, config }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // -- Mock stages --

    /// A stage that always continues, appending its name to metadata.
    struct ContinueStage {
        stage_name: String,
    }

    impl ContinueStage {
        fn new(name: &str) -> Self {
            Self {
                stage_name: name.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Stage for ContinueStage {
        fn name(&self) -> &str {
            &self.stage_name
        }

        async fn execute(&self, mut context: ContextBundle, _ctx: &StageContext) -> StageOutput {
            context
                .metadata
                .insert(self.stage_name.clone(), serde_json::Value::Bool(true));
            StageOutput::Continue(context)
        }
    }

    /// A stage that fails immediately.
    struct FailStage {
        stage_name: String,
    }

    #[async_trait::async_trait]
    impl Stage for FailStage {
        fn name(&self) -> &str {
            &self.stage_name
        }

        async fn execute(&self, _context: ContextBundle, _ctx: &StageContext) -> StageOutput {
            StageOutput::Fail(SakamotoError::StageFailure {
                stage: self.stage_name.clone(),
                reason: "test failure".into(),
            })
        }
    }

    /// A stage that retries N times, then continues.
    struct RetryThenContinueStage {
        stage_name: String,
        retries_before_success: usize,
        call_count: AtomicUsize,
    }

    impl RetryThenContinueStage {
        fn new(name: &str, retries: usize) -> Self {
            Self {
                stage_name: name.into(),
                retries_before_success: retries,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl Stage for RetryThenContinueStage {
        fn name(&self) -> &str {
            &self.stage_name
        }

        async fn execute(&self, context: ContextBundle, _ctx: &StageContext) -> StageOutput {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            if count < self.retries_before_success {
                StageOutput::Retry {
                    context,
                    reason: format!("attempt {}", count + 1),
                }
            } else {
                StageOutput::Continue(context)
            }
        }
    }

    // Helper: create a runner with Continue stages
    fn runner_with_continue_stages(stage_names: &[&str]) -> PipelineRunner {
        let names: Vec<String> = stage_names.iter().map(|s| (*s).into()).collect();
        let dag = PipelineDag::from_linear(&names).unwrap();
        let mut runner = PipelineRunner::new(dag, RunnerConfig::default());
        for name in stage_names {
            runner.add_stage(Arc::new(ContinueStage::new(name)));
        }
        runner
    }

    #[tokio::test]
    async fn run_empty_pipeline() {
        let dag = PipelineDag::new();
        let runner = PipelineRunner::new(dag, RunnerConfig::default());
        let result = runner.run(ContextBundle::from_task("test")).await.unwrap();
        assert!(result.stages_executed.is_empty());
        assert_eq!(result.retries, 0);
    }

    #[tokio::test]
    async fn run_linear_all_continue() {
        let runner = runner_with_continue_stages(&["a", "b", "c"]);
        let result = runner.run(ContextBundle::from_task("test")).await.unwrap();

        assert_eq!(result.stages_executed, vec!["a", "b", "c"]);
        assert_eq!(result.retries, 0);
        // Each stage should have set its metadata flag
        assert_eq!(result.context.metadata["a"], true);
        assert_eq!(result.context.metadata["b"], true);
        assert_eq!(result.context.metadata["c"], true);
    }

    #[tokio::test]
    async fn run_stage_failure_stops_pipeline() {
        let names: Vec<String> = vec!["a", "b", "c"].into_iter().map(Into::into).collect();
        let dag = PipelineDag::from_linear(&names).unwrap();
        let mut runner = PipelineRunner::new(dag, RunnerConfig::default());
        runner.add_stage(Arc::new(ContinueStage::new("a")));
        runner.add_stage(Arc::new(FailStage {
            stage_name: "b".into(),
        }));
        runner.add_stage(Arc::new(ContinueStage::new("c")));

        let err = runner
            .run(ContextBundle::from_task("test"))
            .await
            .unwrap_err();
        assert!(matches!(err, SakamotoError::StageFailure { .. }));
    }

    #[tokio::test]
    async fn run_retry_then_succeed() {
        let names: Vec<String> = vec!["code", "lint"].into_iter().map(Into::into).collect();
        let dag = PipelineDag::from_linear(&names).unwrap();
        let mut runner = PipelineRunner::new(
            dag,
            RunnerConfig {
                max_retries: 3,
                retry_from: Some("code".into()),
            },
        );

        runner.add_stage(Arc::new(ContinueStage::new("code")));
        // Lint fails once, then succeeds
        runner.add_stage(Arc::new(RetryThenContinueStage::new("lint", 1)));

        let result = runner
            .run(ContextBundle::from_task("fix clippy"))
            .await
            .unwrap();

        assert_eq!(result.retries, 1);
        // Executed: code, lint (retry), code (again), lint (success)
        assert_eq!(result.stages_executed, vec!["code", "lint", "code", "lint"]);
    }

    #[tokio::test]
    async fn run_retry_exhausted() {
        let names: Vec<String> = vec!["code", "lint"].into_iter().map(Into::into).collect();
        let dag = PipelineDag::from_linear(&names).unwrap();
        let mut runner = PipelineRunner::new(
            dag,
            RunnerConfig {
                max_retries: 2,
                retry_from: Some("code".into()),
            },
        );

        runner.add_stage(Arc::new(ContinueStage::new("code")));
        // Lint always retries
        runner.add_stage(Arc::new(RetryThenContinueStage::new("lint", 100)));

        let err = runner
            .run(ContextBundle::from_task("fix clippy"))
            .await
            .unwrap_err();
        assert!(matches!(err, SakamotoError::ValidationExhausted { .. }));
    }

    #[tokio::test]
    async fn run_missing_stage_impl_errors() {
        let names: Vec<String> = vec!["a".into()];
        let dag = PipelineDag::from_linear(&names).unwrap();
        let runner = PipelineRunner::new(dag, RunnerConfig::default());
        // No stage registered for "a"

        let err = runner
            .run(ContextBundle::from_task("test"))
            .await
            .unwrap_err();
        assert!(matches!(err, SakamotoError::StageNotFound(_)));
    }

    #[tokio::test]
    async fn run_retry_adds_diagnostics() {
        let names: Vec<String> = vec!["code", "lint"].into_iter().map(Into::into).collect();
        let dag = PipelineDag::from_linear(&names).unwrap();
        let mut runner = PipelineRunner::new(
            dag,
            RunnerConfig {
                max_retries: 3,
                retry_from: Some("code".into()),
            },
        );

        runner.add_stage(Arc::new(ContinueStage::new("code")));
        runner.add_stage(Arc::new(RetryThenContinueStage::new("lint", 1)));

        let result = runner.run(ContextBundle::from_task("test")).await.unwrap();

        // Should have one diagnostic from the retry
        assert_eq!(result.context.diagnostics.len(), 1);
        assert_eq!(result.context.diagnostics[0].stage, "lint");
        assert!(result.context.diagnostics[0].message.contains("retry 1"));
    }
}
