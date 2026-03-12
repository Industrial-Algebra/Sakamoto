//! Code stage — ReAct loop wrapper.
//!
//! Runs a ReAct loop using the stage's LLM client and tool executor,
//! then merges the result into the context bundle.

use sakamoto_core::react::ReactLoop;
use sakamoto_core::stage::{Stage, StageContext};
use sakamoto_types::{
    ContextBundle, SakamotoError, StageOutput,
    llm::{Message, MessageContent, Role},
};

/// Stage that runs a ReAct loop for code generation/modification.
pub struct CodeStage;

#[async_trait::async_trait]
impl Stage for CodeStage {
    fn name(&self) -> &str {
        "code"
    }

    async fn execute(&self, mut context: ContextBundle, ctx: &StageContext) -> StageOutput {
        let llm = match &ctx.llm {
            Some(llm) => llm,
            None => {
                return StageOutput::Fail(SakamotoError::StageFailure {
                    stage: "code".into(),
                    reason: "no LLM client configured".into(),
                });
            }
        };

        let tools = match &ctx.tools {
            Some(tools) => tools,
            None => {
                return StageOutput::Fail(SakamotoError::StageFailure {
                    stage: "code".into(),
                    reason: "no tool executor configured".into(),
                });
            }
        };

        // Build the initial user message from context
        let mut prompt = format!("Task: {}\n", context.task_description);

        if let Some(plan) = &context.plan {
            prompt.push_str(&format!("\nPlan:\n{plan}\n"));
        }

        if !context.diagnostics.is_empty() {
            prompt.push_str("\nPrevious issues to fix:\n");
            for diag in &context.diagnostics {
                prompt.push_str(&format!("- {diag}\n"));
            }
        }

        let messages = vec![Message {
            role: Role::User,
            content: MessageContent::Text(prompt),
        }];

        let default_prompt = "You are a coding agent. Use the available tools to complete the task. When done, respond with a summary of changes made.";
        let system_prompt = ctx
            .config
            .system_prompt
            .as_deref()
            .unwrap_or(default_prompt);

        let react = ReactLoop::new(ctx.config.max_iterations).with_system_prompt(system_prompt);

        match react.run(messages, llm.as_ref(), tools.as_ref()).await {
            Ok(result) => {
                context
                    .metadata
                    .insert("code_result".into(), result.final_text.into());
                StageOutput::Continue(context)
            }
            Err(e) => StageOutput::Fail(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sakamoto_core::stage::StageContext;
    use sakamoto_types::{
        llm::{LlmResponse, TokenUsage, ToolDef},
        stage::StageConfig,
    };
    use std::sync::Arc;

    struct MockLlm;

    #[async_trait::async_trait]
    impl sakamoto_core::stage::LlmClient for MockLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDef],
            _system: Option<&str>,
        ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
            Ok((
                LlmResponse::Final("changes applied".into()),
                TokenUsage::default(),
            ))
        }
    }

    struct MockTools;

    #[async_trait::async_trait]
    impl sakamoto_core::stage::ToolExecutor for MockTools {
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

    #[tokio::test]
    async fn code_stage_runs_react_loop() {
        let stage = CodeStage;
        let ctx = StageContext {
            llm: Some(Arc::new(MockLlm)),
            tools: Some(Arc::new(MockTools)),
            config: StageConfig {
                max_iterations: 5,
                ..Default::default()
            },
        };

        let bundle = ContextBundle::from_task("fix clippy warnings");
        let output = stage.execute(bundle, &ctx).await;
        let result = output.into_context().unwrap();
        assert_eq!(result.metadata["code_result"], "changes applied");
    }

    #[tokio::test]
    async fn code_stage_fails_without_llm() {
        let stage = CodeStage;
        let ctx = StageContext {
            llm: None,
            tools: Some(Arc::new(MockTools)),
            config: StageConfig::default(),
        };

        let bundle = ContextBundle::from_task("task");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_fail());
    }

    #[tokio::test]
    async fn code_stage_fails_without_tools() {
        let stage = CodeStage;
        let ctx = StageContext {
            llm: Some(Arc::new(MockLlm)),
            tools: None,
            config: StageConfig::default(),
        };

        let bundle = ContextBundle::from_task("task");
        let output = stage.execute(bundle, &ctx).await;
        assert!(output.is_fail());
    }
}
