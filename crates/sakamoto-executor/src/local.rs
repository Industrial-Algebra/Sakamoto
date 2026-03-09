//! Local executor — runs pipelines in the current working directory.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use sakamoto_config::{PipelineConfig, ProjectConfig};
use sakamoto_core::dag::PipelineDag;
use sakamoto_core::runner::{PipelineRunner, RunResult, RunnerConfig};
use sakamoto_core::stage::{LlmClient, Stage, ToolExecutor};
use sakamoto_types::stage::StageConfig;
use sakamoto_types::{ContextBundle, SakamotoError};

use crate::adapters::{LlmAdapter, ToolAdapter};
use crate::executor::Executor;
use crate::stages::code::CodeStage;
use crate::stages::command::CommandStage;
use crate::stages::context::ContextStage;

/// Executes pipelines in the current working directory with no isolation.
pub struct LocalExecutor {
    /// Project configuration.
    config: ProjectConfig,
    /// Working directory for command execution.
    working_dir: PathBuf,
    /// LLM clients keyed by config name.
    llm_clients: HashMap<String, Arc<dyn LlmClient>>,
    /// Tool executors keyed by toolset name.
    tool_executors: HashMap<String, Arc<dyn ToolExecutor>>,
}

impl LocalExecutor {
    /// Create a new local executor.
    pub fn new(config: ProjectConfig, working_dir: PathBuf) -> Self {
        Self {
            config,
            working_dir,
            llm_clients: HashMap::new(),
            tool_executors: HashMap::new(),
        }
    }

    /// Register an LLM backend (already constructed by the caller).
    pub fn add_llm_client(&mut self, name: String, client: Arc<dyn LlmClient>) {
        self.llm_clients.insert(name, client);
    }

    /// Register an LLM backend from a sakamoto-llm implementation.
    pub fn add_llm_backend(&mut self, name: String, backend: Arc<dyn sakamoto_llm::LlmBackend>) {
        self.llm_clients
            .insert(name, Arc::new(LlmAdapter::new(backend)));
    }

    /// Register a tool executor (already constructed by the caller).
    pub fn add_tool_executor(&mut self, name: String, executor: Arc<dyn ToolExecutor>) {
        self.tool_executors.insert(name, executor);
    }

    /// Register a tool router from sakamoto-tools.
    pub fn add_tool_router(
        &mut self,
        name: String,
        router: Arc<sakamoto_tools::router::ToolRouter>,
    ) {
        self.tool_executors
            .insert(name, Arc::new(ToolAdapter::new(router)));
    }

    /// Build a stage implementation for the given stage name.
    fn build_stage(&self, name: &str) -> Arc<dyn Stage> {
        match name {
            "context" => Arc::new(ContextStage),
            "code" => Arc::new(CodeStage),
            "lint" => Arc::new(CommandStage::lint()),
            "test" => Arc::new(CommandStage::test()),
            "commit" => Arc::new(CommandStage::commit()),
            "pr" => Arc::new(CommandStage::pr()),
            _ => Arc::new(CodeStage), // Default unknown stages to code
        }
    }

    /// Build a StageConfig for a named stage using the pipeline config.
    fn build_stage_config(&self, stage_name: &str, pipeline: &PipelineConfig) -> StageConfig {
        let mut config = StageConfig {
            name: stage_name.into(),
            ..Default::default()
        };

        // Apply stage overrides from pipeline config
        if let Some(overrides) = pipeline.stage_overrides.get(stage_name) {
            if let Some(ref llm) = overrides.llm_backend {
                config.llm_backend = Some(llm.clone());
            }
            if let Some(ref toolset) = overrides.toolset {
                config.toolset = Some(toolset.clone());
            }
            if let Some(interaction) = overrides.interaction {
                config.interaction = interaction;
            }
            if let Some(max_iter) = overrides.max_iterations {
                config.max_iterations = max_iter;
            }
            if let Some(ref cmd) = overrides.command {
                config.command = Some(cmd.clone());
            }
        }

        // Apply validation commands for lint/test stages
        if config.command.is_none()
            && let Some(ref validation) = self.config.validation
        {
            match stage_name {
                "lint" => {
                    config.command = Some(validation.lint_command.clone());
                }
                "test" => {
                    config.command = validation.test_command.clone();
                }
                _ => {}
            }
        }

        config
    }

    /// Execute a named pipeline from the project config.
    pub async fn run_pipeline(
        &self,
        pipeline_name: &str,
        task: &str,
    ) -> Result<RunResult, SakamotoError> {
        let pipeline_config = self
            .config
            .pipelines
            .get(pipeline_name)
            .ok_or_else(|| {
                SakamotoError::ConfigError(format!("pipeline '{pipeline_name}' not found"))
            })?
            .clone();

        let dag = PipelineDag::from_linear(&pipeline_config.stages)?;

        let mut runner = PipelineRunner::new(
            dag,
            RunnerConfig {
                max_retries: pipeline_config.max_ci_rounds,
                retry_from: Some("code".into()),
            },
        );

        // Register stages and their configs
        for stage_name in &pipeline_config.stages {
            runner.add_stage(self.build_stage(stage_name));
            let stage_config = self.build_stage_config(stage_name, &pipeline_config);
            runner.add_stage_config(stage_name.clone(), stage_config);
        }

        // Register LLM clients
        for (name, client) in &self.llm_clients {
            runner.add_llm_client(name.clone(), Arc::clone(client));
        }

        // Register tool executors
        for (name, executor) in &self.tool_executors {
            runner.add_tool_executor(name.clone(), Arc::clone(executor));
        }

        // Build initial context
        let mut context = ContextBundle::from_task(task);
        context.metadata.insert(
            "working_dir".into(),
            self.working_dir.display().to_string().into(),
        );

        runner.run(context).await
    }
}

#[async_trait::async_trait]
impl Executor for LocalExecutor {
    async fn run(&self, task: &str) -> Result<RunResult, SakamotoError> {
        self.run_pipeline("default", task).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_config() -> ProjectConfig {
        let toml = r#"
            [project]
            name = "test"

            [pipeline.default]
            stages = ["context"]

            [pipeline.with_lint]
            stages = ["context", "lint"]
        "#;
        sakamoto_config::parse_project_config(toml).unwrap()
    }

    #[tokio::test]
    async fn local_executor_runs_context_only_pipeline() {
        let config = minimal_config();
        let executor = LocalExecutor::new(config, PathBuf::from("."));

        let result = executor
            .run_pipeline("default", "Fix src/main.rs")
            .await
            .unwrap();

        assert_eq!(result.stages_executed, vec!["context"]);
        // Context stage should have extracted the file path ref
        assert!(!result.context.refs.is_empty());
    }

    #[tokio::test]
    async fn local_executor_unknown_pipeline_errors() {
        let config = minimal_config();
        let executor = LocalExecutor::new(config, PathBuf::from("."));

        let err = executor
            .run_pipeline("nonexistent", "task")
            .await
            .unwrap_err();
        assert!(matches!(err, SakamotoError::ConfigError(_)));
    }

    #[tokio::test]
    async fn local_executor_sets_working_dir() {
        let config = minimal_config();
        let executor = LocalExecutor::new(config, PathBuf::from("/tmp"));

        let result = executor.run_pipeline("default", "task").await.unwrap();

        assert_eq!(result.context.metadata["working_dir"], "/tmp");
    }
}
