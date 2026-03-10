//! Tool router — registry and dispatch for available tools.
//!
//! The [`ToolRouter`] holds a set of [`Tool`] implementations and provides
//! lookup by name, listing of definitions, and execution dispatch. It is
//! the main entry point used by the ReAct loop executor.

use std::collections::HashMap;
use std::sync::Arc;

use sakamoto_types::{SakamotoError, ToolDef};

use crate::tool::Tool;

/// A registry that maps tool names to their implementations and dispatches
/// calls by name.
pub struct ToolRouter {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Returns an error if a tool with the same name
    /// is already registered.
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), SakamotoError> {
        let name = tool.definition().name;
        if self.tools.contains_key(&name) {
            return Err(SakamotoError::ToolCallFailed {
                tool: name,
                reason: "tool already registered".into(),
            });
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// Return definitions for all registered tools.
    ///
    /// This is sent to the LLM so it knows what tools are available.
    pub fn definitions(&self) -> Vec<ToolDef> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Execute a tool by name with the given input arguments.
    ///
    /// Returns `ToolsetNotFound` if the tool is not registered.
    pub async fn call(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<String, SakamotoError> {
        let tool = self
            .get(name)
            .ok_or_else(|| SakamotoError::ToolsetNotFound(name.to_string()))?;
        tool.execute(input).await
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the router has no tools registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal tool for testing the router.
    struct AddTool;

    #[async_trait::async_trait]
    impl Tool for AddTool {
        fn definition(&self) -> ToolDef {
            ToolDef {
                name: "add".into(),
                description: "Adds two numbers".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "a": { "type": "number" },
                        "b": { "type": "number" }
                    },
                    "required": ["a", "b"]
                }),
            }
        }

        async fn execute(&self, input: serde_json::Value) -> Result<String, SakamotoError> {
            let a = input["a"]
                .as_f64()
                .ok_or_else(|| SakamotoError::ToolCallFailed {
                    tool: "add".into(),
                    reason: "missing 'a'".into(),
                })?;
            let b = input["b"]
                .as_f64()
                .ok_or_else(|| SakamotoError::ToolCallFailed {
                    tool: "add".into(),
                    reason: "missing 'b'".into(),
                })?;
            Ok(format!("{}", a + b))
        }
    }

    #[test]
    fn router_starts_empty() {
        let router = ToolRouter::new();
        assert!(router.is_empty());
        assert_eq!(router.len(), 0);
        assert!(router.definitions().is_empty());
    }

    #[test]
    fn router_register_and_lookup() {
        let mut router = ToolRouter::new();
        router.register(Arc::new(AddTool)).unwrap();
        assert_eq!(router.len(), 1);
        assert!(router.get("add").is_some());
        assert!(router.get("nonexistent").is_none());
    }

    #[test]
    fn router_duplicate_registration_fails() {
        let mut router = ToolRouter::new();
        router.register(Arc::new(AddTool)).unwrap();
        let result = router.register(Arc::new(AddTool));
        assert!(result.is_err());
    }

    #[test]
    fn router_definitions_lists_all() {
        let mut router = ToolRouter::new();
        router.register(Arc::new(AddTool)).unwrap();
        let defs = router.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "add");
    }

    #[tokio::test]
    async fn router_call_success() {
        let mut router = ToolRouter::new();
        router.register(Arc::new(AddTool)).unwrap();
        let result = router
            .call("add", serde_json::json!({"a": 2, "b": 3}))
            .await;
        assert_eq!(result.ok(), Some("5".into()));
    }

    #[tokio::test]
    async fn router_call_unknown_tool() {
        let router = ToolRouter::new();
        let result = router.call("nonexistent", serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn router_call_tool_error_propagates() {
        let mut router = ToolRouter::new();
        router.register(Arc::new(AddTool)).unwrap();
        let result = router.call("add", serde_json::json!({})).await;
        assert!(result.is_err());
    }
}
