//! The core [`Tool`] trait for Sakamoto tools.
//!
//! All tools — built-in and MCP-proxied — implement this trait,
//! providing a uniform interface for the ReAct loop executor.

use sakamoto_types::{SakamotoError, ToolDef};

/// A tool that can be invoked by the LLM during a ReAct loop.
///
/// Implementors must be `Send + Sync` so they can be shared across
/// async tasks. The trait is object-safe via `async_trait`.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Return the tool definition (name, description, input schema).
    ///
    /// This is sent to the LLM so it knows what tools are available
    /// and how to call them.
    fn definition(&self) -> ToolDef;

    /// Execute the tool with the given input arguments.
    ///
    /// Returns the tool's output as a string on success, or a
    /// `SakamotoError` on failure. The caller is responsible for
    /// wrapping the result into a `ToolResult`.
    async fn execute(&self, input: serde_json::Value) -> Result<String, SakamotoError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal test tool for verifying the trait works.
    struct EchoTool;

    #[async_trait::async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDef {
            ToolDef {
                name: "echo".into(),
                description: "Echoes the input back".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }),
            }
        }

        async fn execute(&self, input: serde_json::Value) -> Result<String, SakamotoError> {
            let message =
                input["message"]
                    .as_str()
                    .ok_or_else(|| SakamotoError::ToolCallFailed {
                        tool: "echo".into(),
                        reason: "missing 'message' field".into(),
                    })?;
            Ok(message.to_string())
        }
    }

    #[tokio::test]
    async fn echo_tool_definition() {
        let tool = EchoTool;
        let def = tool.definition();
        assert_eq!(def.name, "echo");
        assert!(!def.description.is_empty());
    }

    #[tokio::test]
    async fn echo_tool_execute_success() {
        let tool = EchoTool;
        let result = tool.execute(serde_json::json!({"message": "hello"})).await;
        assert_eq!(result.ok(), Some("hello".into()));
    }

    #[tokio::test]
    async fn echo_tool_execute_missing_field() {
        let tool = EchoTool;
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn tool_is_object_safe() {
        // Verify that Tool can be used as a trait object.
        let tool: Box<dyn Tool> = Box::new(EchoTool);
        let def = tool.definition();
        assert_eq!(def.name, "echo");

        let result = tool.execute(serde_json::json!({"message": "test"})).await;
        assert!(result.is_ok());
    }
}
