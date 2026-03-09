//! Stage trait and orchestration abstractions.
//!
//! These traits define the interfaces that `sakamoto-core` programs against.
//! Concrete implementations live in native-only crates (`sakamoto-llm`,
//! `sakamoto-tools`, etc.), keeping core Wasm-safe.

use sakamoto_types::{
    ContextBundle, SakamotoError, StageOutput,
    llm::{LlmResponse, Message, TokenUsage, ToolDef},
    stage::StageConfig,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Abstraction traits
// ---------------------------------------------------------------------------

/// Abstraction over LLM completion, allowing sakamoto-core to remain
/// independent of specific backend implementations.
///
/// Implemented by adapters wrapping `sakamoto-llm` backends.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a conversation to the LLM and get a response.
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> Result<(LlmResponse, TokenUsage), SakamotoError>;
}

/// Abstraction over tool execution.
///
/// Implemented by adapters wrapping `sakamoto-tools::ToolRouter`.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// List all available tool definitions.
    fn available_tools(&self) -> Vec<ToolDef>;

    /// Execute a tool by name with the given input.
    async fn execute_tool(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<String, SakamotoError>;
}

// ---------------------------------------------------------------------------
// Stage context and trait
// ---------------------------------------------------------------------------

/// Runtime context provided to each stage during execution.
pub struct StageContext {
    /// LLM client for stages that need LLM interaction (plan, code).
    pub llm: Option<Arc<dyn LlmClient>>,

    /// Tool executor for stages that need tool access (code, lint, test).
    pub tools: Option<Arc<dyn ToolExecutor>>,

    /// Configuration for this specific stage.
    pub config: StageConfig,
}

/// A pipeline stage that transforms a [`ContextBundle`].
///
/// Stages are the building blocks of a pipeline DAG. Each stage receives
/// the accumulated context from upstream stages and produces a [`StageOutput`]
/// that determines how the pipeline continues.
#[async_trait::async_trait]
pub trait Stage: Send + Sync {
    /// The stage's unique name within the pipeline.
    fn name(&self) -> &str;

    /// Execute the stage, transforming the context.
    async fn execute(&self, context: ContextBundle, stage_ctx: &StageContext) -> StageOutput;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sakamoto_types::stage::{InteractionPolicy, StageKind};

    // -- Mock implementations for trait testing --

    struct MockLlm;

    #[async_trait::async_trait]
    impl LlmClient for MockLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDef],
            _system: Option<&str>,
        ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
            Ok((LlmResponse::Final("done".into()), TokenUsage::default()))
        }
    }

    struct MockTools;

    #[async_trait::async_trait]
    impl ToolExecutor for MockTools {
        fn available_tools(&self) -> Vec<ToolDef> {
            vec![]
        }

        async fn execute_tool(
            &self,
            _name: &str,
            _input: serde_json::Value,
        ) -> Result<String, SakamotoError> {
            Ok("ok".into())
        }
    }

    struct PassthroughStage;

    #[async_trait::async_trait]
    impl Stage for PassthroughStage {
        fn name(&self) -> &str {
            "passthrough"
        }

        async fn execute(&self, context: ContextBundle, _stage_ctx: &StageContext) -> StageOutput {
            StageOutput::Continue(context)
        }
    }

    fn test_stage_config() -> StageConfig {
        StageConfig {
            name: "test".into(),
            kind: StageKind::Code,
            llm_backend: None,
            toolset: None,
            interaction: InteractionPolicy::Autonomous,
            max_iterations: 10,
            max_retries: 2,
            timeout_secs: None,
            command: None,
        }
    }

    #[test]
    fn llm_client_is_object_safe() {
        let _: Box<dyn LlmClient> = Box::new(MockLlm);
    }

    #[test]
    fn tool_executor_is_object_safe() {
        let _: Box<dyn ToolExecutor> = Box::new(MockTools);
    }

    #[test]
    fn stage_is_object_safe() {
        let _: Box<dyn Stage> = Box::new(PassthroughStage);
    }

    #[tokio::test]
    async fn stage_context_with_llm_and_tools() {
        let ctx = StageContext {
            llm: Some(Arc::new(MockLlm)),
            tools: Some(Arc::new(MockTools)),
            config: test_stage_config(),
        };

        let stage: Box<dyn Stage> = Box::new(PassthroughStage);
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_continue());
    }

    #[tokio::test]
    async fn stage_context_without_optional_deps() {
        let ctx = StageContext {
            llm: None,
            tools: None,
            config: test_stage_config(),
        };

        let stage = PassthroughStage;
        let bundle = ContextBundle::from_task("test");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_continue());
    }
}
