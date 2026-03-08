//! The core [`LlmBackend`] trait.

use sakamoto_types::SakamotoError;
use sakamoto_types::llm::{LlmResponse, Message, ModelInfo, TokenUsage, ToolDef};

/// A backend that can complete conversations with an LLM.
///
/// Implementations exist for Anthropic, OpenAI-compatible, and Ollama APIs.
/// Each pipeline stage can use a different backend.
#[async_trait::async_trait]
pub trait LlmBackend: Send + Sync {
    /// Send a conversation to the LLM and get a response.
    ///
    /// The LLM may return tool calls (which the caller should execute
    /// and feed back) or a final text response.
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> Result<(LlmResponse, TokenUsage), SakamotoError>;

    /// Return metadata about this backend's model.
    fn model_info(&self) -> &ModelInfo;
}
