//! Adapters that bridge native crate implementations into
//! `sakamoto-core`'s abstraction traits.

use sakamoto_core::stage::{LlmClient, ToolExecutor};
use sakamoto_llm::LlmBackend;
use sakamoto_tools::router::ToolRouter;
use sakamoto_types::{
    SakamotoError,
    llm::{LlmResponse, Message, TokenUsage, ToolDef},
};
use std::sync::Arc;

/// Adapts an [`LlmBackend`] into a [`LlmClient`] for use by sakamoto-core.
pub struct LlmAdapter {
    backend: Arc<dyn LlmBackend>,
}

impl LlmAdapter {
    pub fn new(backend: Arc<dyn LlmBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait::async_trait]
impl LlmClient for LlmAdapter {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
        self.backend.complete(messages, tools, system).await
    }
}

/// Adapts a [`ToolRouter`] into a [`ToolExecutor`] for use by sakamoto-core.
pub struct ToolAdapter {
    router: Arc<ToolRouter>,
}

impl ToolAdapter {
    pub fn new(router: Arc<ToolRouter>) -> Self {
        Self { router }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ToolAdapter {
    fn available_tools(&self) -> Vec<ToolDef> {
        self.router.definitions()
    }

    async fn execute_tool(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<String, SakamotoError> {
        self.router.call(name, input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_adapter_is_llm_client() {
        // Verify the adapter satisfies the trait bound at compile time.
        fn _assert_llm_client<T: LlmClient>() {}
        _assert_llm_client::<LlmAdapter>();
    }

    #[test]
    fn tool_adapter_is_tool_executor() {
        fn _assert_tool_executor<T: ToolExecutor>() {}
        _assert_tool_executor::<ToolAdapter>();
    }

    #[tokio::test]
    async fn tool_adapter_empty_router() {
        let router = Arc::new(ToolRouter::new());
        let adapter = ToolAdapter::new(router);
        assert!(adapter.available_tools().is_empty());
    }
}
