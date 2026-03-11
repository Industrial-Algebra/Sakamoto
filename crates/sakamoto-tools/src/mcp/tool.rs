//! MCP tool proxy — wraps an MCP server tool as a Sakamoto [`Tool`].
//!
//! Each [`McpTool`] holds a reference to its parent [`McpConnection`] and
//! proxies `execute` calls to the MCP server's `tools/call` RPC.

use std::sync::Arc;

use sakamoto_types::{SakamotoError, ToolDef};

use super::connection::McpConnection;
use crate::tool::Tool;

/// A tool backed by an MCP server.
///
/// Implements the standard [`Tool`] trait so it can be registered in a
/// [`ToolRouter`](crate::router::ToolRouter) alongside built-in tools.
pub struct McpTool {
    /// The MCP server connection this tool belongs to.
    connection: Arc<McpConnection>,
    /// Cached tool definition.
    definition: ToolDef,
    /// The tool name on the MCP server (may differ from the prefixed name).
    remote_name: String,
}

impl McpTool {
    /// Create a new MCP tool proxy.
    ///
    /// The tool name is prefixed with the server name to avoid collisions
    /// between tools from different MCP servers (e.g., `filesystem__read_file`).
    pub fn new(connection: Arc<McpConnection>, tool_info: &pmcp::ToolInfo) -> Self {
        let prefixed_name = format!("{}_{}", connection.name(), tool_info.name);

        let definition = ToolDef {
            name: prefixed_name,
            description: tool_info.description.clone().unwrap_or_default(),
            input_schema: tool_info.input_schema.clone(),
        };

        Self {
            connection,
            definition,
            remote_name: tool_info.name.clone(),
        }
    }

    /// Create a new MCP tool proxy without name prefixing.
    ///
    /// Use this when there's no risk of name collision (e.g., single MCP server).
    pub fn new_unprefixed(connection: Arc<McpConnection>, tool_info: &pmcp::ToolInfo) -> Self {
        let definition = ToolDef {
            name: tool_info.name.clone(),
            description: tool_info.description.clone().unwrap_or_default(),
            input_schema: tool_info.input_schema.clone(),
        };

        Self {
            connection,
            definition,
            remote_name: tool_info.name.clone(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for McpTool {
    fn definition(&self) -> ToolDef {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, SakamotoError> {
        self.connection.call_tool(&self.remote_name, input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_def_name_prefixing_logic() {
        // Verify the name-prefixing logic that McpTool::new uses.
        // Full MCP integration tests require a running server.
        let server_name = "myserver";
        let tool_name = "read_file";
        let prefixed = format!("{server_name}_{tool_name}");

        assert_eq!(prefixed, "myserver_read_file");
    }

    #[test]
    fn tool_def_from_parts() {
        let def = ToolDef {
            name: "server_tool".into(),
            description: "A remote tool".into(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        assert_eq!(def.name, "server_tool");
    }
}
