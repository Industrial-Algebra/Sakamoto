//! ReAct loop — iterative LLM + tool-use execution engine.
//!
//! The ReAct (Reasoning + Acting) loop is the core of the coding agent.
//! It sends messages to an LLM, executes any tool calls the LLM requests,
//! appends the results, and repeats until the LLM produces a final answer
//! or the iteration limit is reached.

use sakamoto_types::{
    SakamotoError,
    llm::{ContentBlock, LlmResponse, Message, MessageContent, Role, TokenUsage, ToolDef},
};

use crate::stage::{LlmClient, ToolExecutor};

/// Configuration for a ReAct loop execution.
pub struct ReactLoop {
    /// Maximum number of LLM round-trips before giving up.
    pub max_iterations: usize,
    /// Optional system prompt prepended to the conversation.
    pub system_prompt: Option<String>,
}

/// The result of a completed ReAct loop.
#[derive(Debug)]
pub struct ReactResult {
    /// The LLM's final text response.
    pub final_text: String,
    /// The full message history including tool calls and results.
    pub messages: Vec<Message>,
    /// Accumulated token usage across all LLM calls.
    pub token_usage: TokenUsage,
    /// Number of LLM round-trips performed.
    pub iterations: usize,
}

impl ReactLoop {
    /// Create a new ReAct loop with the given iteration limit.
    pub fn new(max_iterations: usize) -> Self {
        Self {
            max_iterations,
            system_prompt: None,
        }
    }

    /// Set the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Run the ReAct loop to completion.
    ///
    /// Takes initial messages (typically the user's task + context), an LLM
    /// client, and a tool executor. Returns the final result or an error.
    pub async fn run(
        &self,
        initial_messages: Vec<Message>,
        llm: &dyn LlmClient,
        tools: &dyn ToolExecutor,
    ) -> Result<ReactResult, SakamotoError> {
        let tool_defs = tools.available_tools();
        let mut messages = initial_messages;
        let mut total_usage = TokenUsage::default();

        for iteration in 0..self.max_iterations {
            let (response, usage) = llm
                .complete(&messages, &tool_defs, self.system_prompt.as_deref())
                .await?;
            total_usage.accumulate(&usage);

            match response {
                LlmResponse::Final(text) => {
                    return Ok(ReactResult {
                        final_text: text,
                        messages,
                        token_usage: total_usage,
                        iterations: iteration + 1,
                    });
                }
                LlmResponse::ToolCalls(calls) => {
                    // Build assistant message with tool-use blocks
                    let tool_use_blocks: Vec<ContentBlock> = calls
                        .iter()
                        .map(|call| ContentBlock::ToolUse {
                            id: call.id.clone(),
                            name: call.name.clone(),
                            input: call.input.clone(),
                        })
                        .collect();

                    messages.push(Message {
                        role: Role::Assistant,
                        content: MessageContent::Blocks(tool_use_blocks),
                    });

                    // Execute each tool and collect results
                    let mut result_blocks = Vec::new();
                    for call in &calls {
                        match tools.execute_tool(&call.name, call.input.clone()).await {
                            Ok(output) => {
                                result_blocks.push(ContentBlock::ToolResult {
                                    tool_use_id: call.id.clone(),
                                    content: output,
                                    is_error: false,
                                });
                            }
                            Err(e) => {
                                result_blocks.push(ContentBlock::ToolResult {
                                    tool_use_id: call.id.clone(),
                                    content: e.to_string(),
                                    is_error: true,
                                });
                            }
                        }
                    }

                    messages.push(Message {
                        role: Role::User,
                        content: MessageContent::Blocks(result_blocks),
                    });
                }
            }
        }

        Err(SakamotoError::MaxIterationsExceeded {
            stage: "react".into(),
            max: self.max_iterations,
        })
    }
}

/// Extract tool definitions from a [`ToolExecutor`].
pub fn collect_tool_defs(tools: &dyn ToolExecutor) -> Vec<ToolDef> {
    tools.available_tools()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sakamoto_types::llm::ToolCall;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // -- Mock LLM that returns Final immediately --

    struct ImmediateLlm {
        response_text: String,
    }

    #[async_trait::async_trait]
    impl crate::stage::LlmClient for ImmediateLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDef],
            _system: Option<&str>,
        ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
            Ok((
                LlmResponse::Final(self.response_text.clone()),
                TokenUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cost_usd: None,
                },
            ))
        }
    }

    // -- Mock LLM that calls a tool once, then returns Final --

    struct ToolThenFinalLlm {
        call_count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::stage::LlmClient for ToolThenFinalLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDef],
            _system: Option<&str>,
        ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            let usage = TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cost_usd: None,
            };

            if count == 0 {
                // First call: request a tool call
                Ok((
                    LlmResponse::ToolCalls(vec![ToolCall {
                        id: "call_1".into(),
                        name: "echo".into(),
                        input: serde_json::json!({"message": "hello"}),
                    }]),
                    usage,
                ))
            } else {
                // Second call: return final
                Ok((LlmResponse::Final("done after tool".into()), usage))
            }
        }
    }

    // -- Mock LLM that always requests tool calls (for max iterations test) --

    struct InfiniteToolLlm;

    #[async_trait::async_trait]
    impl crate::stage::LlmClient for InfiniteToolLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDef],
            _system: Option<&str>,
        ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
            Ok((
                LlmResponse::ToolCalls(vec![ToolCall {
                    id: "call_inf".into(),
                    name: "echo".into(),
                    input: serde_json::json!({"message": "loop"}),
                }]),
                TokenUsage::default(),
            ))
        }
    }

    // -- Mock LLM that returns an error --

    struct ErrorLlm;

    #[async_trait::async_trait]
    impl crate::stage::LlmClient for ErrorLlm {
        async fn complete(
            &self,
            _messages: &[Message],
            _tools: &[ToolDef],
            _system: Option<&str>,
        ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
            Err(SakamotoError::LlmError {
                backend: "mock".into(),
                reason: "test error".into(),
            })
        }
    }

    // -- Mock tool executor --

    struct MockTools {
        call_count: AtomicUsize,
    }

    impl MockTools {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl crate::stage::ToolExecutor for MockTools {
        fn available_tools(&self) -> Vec<ToolDef> {
            vec![ToolDef {
                name: "echo".into(),
                description: "Echoes input".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }]
        }

        async fn execute_tool(
            &self,
            _name: &str,
            input: serde_json::Value,
        ) -> Result<String, SakamotoError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(input.to_string())
        }
    }

    // -- Mock tool executor that fails --

    struct FailingTools;

    #[async_trait::async_trait]
    impl crate::stage::ToolExecutor for FailingTools {
        fn available_tools(&self) -> Vec<ToolDef> {
            vec![ToolDef {
                name: "echo".into(),
                description: "Echoes input".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }]
        }

        async fn execute_tool(
            &self,
            name: &str,
            _input: serde_json::Value,
        ) -> Result<String, SakamotoError> {
            Err(SakamotoError::ToolCallFailed {
                tool: name.to_string(),
                reason: "tool error".into(),
            })
        }
    }

    fn user_message(text: &str) -> Message {
        Message {
            role: Role::User,
            content: MessageContent::Text(text.into()),
        }
    }

    // -- Tests --

    #[tokio::test]
    async fn react_immediate_final() {
        let react = ReactLoop::new(10);
        let llm = ImmediateLlm {
            response_text: "answer".into(),
        };
        let tools = MockTools::new();

        let result = react
            .run(vec![user_message("question")], &llm, &tools)
            .await
            .unwrap();

        assert_eq!(result.final_text, "answer");
        assert_eq!(result.iterations, 1);
        assert_eq!(result.token_usage.input_tokens, 100);
        assert_eq!(result.token_usage.output_tokens, 50);
    }

    #[tokio::test]
    async fn react_tool_then_final() {
        let react = ReactLoop::new(10);
        let llm = ToolThenFinalLlm {
            call_count: AtomicUsize::new(0),
        };
        let tools = MockTools::new();

        let result = react
            .run(vec![user_message("task")], &llm, &tools)
            .await
            .unwrap();

        assert_eq!(result.final_text, "done after tool");
        assert_eq!(result.iterations, 2);
        // Tool was called once
        assert_eq!(tools.call_count.load(Ordering::SeqCst), 1);
        // Two LLM calls worth of tokens
        assert_eq!(result.token_usage.input_tokens, 200);
    }

    #[tokio::test]
    async fn react_messages_include_tool_exchange() {
        let react = ReactLoop::new(10);
        let llm = ToolThenFinalLlm {
            call_count: AtomicUsize::new(0),
        };
        let tools = MockTools::new();

        let result = react
            .run(vec![user_message("task")], &llm, &tools)
            .await
            .unwrap();

        // Messages: user, assistant (tool use), user (tool result)
        assert_eq!(result.messages.len(), 3);
        assert_eq!(result.messages[0].role, Role::User);
        assert_eq!(result.messages[1].role, Role::Assistant);
        assert_eq!(result.messages[2].role, Role::User);

        // Verify assistant message has tool use block
        if let MessageContent::Blocks(blocks) = &result.messages[1].content {
            assert!(matches!(blocks[0], ContentBlock::ToolUse { .. }));
        } else {
            panic!("expected Blocks content");
        }

        // Verify user message has tool result block
        if let MessageContent::Blocks(blocks) = &result.messages[2].content {
            assert!(matches!(blocks[0], ContentBlock::ToolResult { .. }));
            if let ContentBlock::ToolResult { is_error, .. } = &blocks[0] {
                assert!(!is_error);
            }
        } else {
            panic!("expected Blocks content");
        }
    }

    #[tokio::test]
    async fn react_max_iterations_exceeded() {
        let react = ReactLoop::new(3);
        let llm = InfiniteToolLlm;
        let tools = MockTools::new();

        let err = react
            .run(vec![user_message("task")], &llm, &tools)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            SakamotoError::MaxIterationsExceeded { max: 3, .. }
        ));
    }

    #[tokio::test]
    async fn react_llm_error_propagates() {
        let react = ReactLoop::new(10);
        let llm = ErrorLlm;
        let tools = MockTools::new();

        let err = react
            .run(vec![user_message("task")], &llm, &tools)
            .await
            .unwrap_err();

        assert!(matches!(err, SakamotoError::LlmError { .. }));
    }

    #[tokio::test]
    async fn react_tool_error_becomes_error_result() {
        let react = ReactLoop::new(10);
        // LLM calls tool once, then sees the error result and returns Final
        let llm = ToolThenFinalLlm {
            call_count: AtomicUsize::new(0),
        };
        let tools = FailingTools;

        let result = react
            .run(vec![user_message("task")], &llm, &tools)
            .await
            .unwrap();

        // The loop still completes — tool errors don't abort the loop,
        // they're sent back to the LLM as error results
        assert_eq!(result.final_text, "done after tool");

        // Verify the tool result was marked as error
        if let MessageContent::Blocks(blocks) = &result.messages[2].content
            && let ContentBlock::ToolResult { is_error, .. } = &blocks[0]
        {
            assert!(is_error);
        }
    }

    #[tokio::test]
    async fn react_with_system_prompt() {
        struct SystemCaptureLlm {
            captured: Arc<tokio::sync::Mutex<Option<String>>>,
        }

        #[async_trait::async_trait]
        impl crate::stage::LlmClient for SystemCaptureLlm {
            async fn complete(
                &self,
                _messages: &[Message],
                _tools: &[ToolDef],
                system: Option<&str>,
            ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
                *self.captured.lock().await = system.map(String::from);
                Ok((LlmResponse::Final("ok".into()), TokenUsage::default()))
            }
        }

        let captured = Arc::new(tokio::sync::Mutex::new(None));
        let llm = SystemCaptureLlm {
            captured: Arc::clone(&captured),
        };
        let tools = MockTools::new();

        let react = ReactLoop::new(10).with_system_prompt("You are a coding agent.");

        react
            .run(vec![user_message("task")], &llm, &tools)
            .await
            .unwrap();

        assert_eq!(
            captured.lock().await.as_deref(),
            Some("You are a coding agent.")
        );
    }

    #[test]
    fn collect_tool_defs_delegates() {
        let tools = MockTools::new();
        let defs = collect_tool_defs(&tools);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "echo");
    }
}
