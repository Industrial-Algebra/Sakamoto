//! MCP connection lifecycle management.
//!
//! Manages the full lifecycle of an MCP server connection: spawn transport,
//! initialize the protocol, discover tools, execute calls, and shut down.

use std::collections::HashMap;

use pmcp::{Client, ClientCapabilities, Implementation, ToolInfo};
use tokio::sync::Mutex;

use sakamoto_types::SakamotoError;

use super::transport::ChildProcessTransport;

/// An active connection to an MCP server.
///
/// Wraps a pmcp [`Client`] over a [`ChildProcessTransport`] and manages the
/// protocol lifecycle. Once initialized, the connection exposes the server's
/// tools and can execute calls.
pub struct McpConnection {
    /// Human-readable name for this connection (from config key).
    name: String,
    /// The pmcp client — `None` after close.
    client: Mutex<Option<Client<ChildProcessTransport>>>,
    /// Tools discovered from the server after initialization.
    tools: Vec<ToolInfo>,
}

impl McpConnection {
    /// Connect to an MCP server via stdio transport.
    ///
    /// Spawns the child process, performs the MCP initialize handshake,
    /// and discovers available tools.
    pub async fn connect_stdio(
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, SakamotoError> {
        let transport = ChildProcessTransport::spawn(command, args, env)?;

        let mut client = Client::with_info(
            transport,
            Implementation {
                name: "sakamoto".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        );

        // Initialize the MCP protocol handshake
        client
            .initialize(ClientCapabilities::default())
            .await
            .map_err(|e| SakamotoError::McpError {
                server: name.to_string(),
                reason: format!("initialization failed: {e}"),
            })?;

        // Discover available tools
        let tools_result = client
            .list_tools(None)
            .await
            .map_err(|e| SakamotoError::McpError {
                server: name.to_string(),
                reason: format!("failed to list tools: {e}"),
            })?;

        let tools = tools_result.tools;

        tracing::info!(
            server = %name,
            tool_count = tools.len(),
            tools = ?tools.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "connected to MCP server"
        );

        Ok(Self {
            name: name.to_string(),
            client: Mutex::new(Some(client)),
            tools,
        })
    }

    /// The server name (from config).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Tools discovered from this server.
    pub fn tools(&self) -> &[ToolInfo] {
        &self.tools
    }

    /// Call a tool on this MCP server.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, SakamotoError> {
        let mut guard = self.client.lock().await;
        let client = guard.as_mut().ok_or_else(|| SakamotoError::McpError {
            server: self.name.clone(),
            reason: "connection is closed".into(),
        })?;

        let result = client
            .call_tool(tool_name.to_string(), arguments)
            .await
            .map_err(|e| SakamotoError::McpError {
                server: self.name.clone(),
                reason: format!("tool call `{tool_name}` failed: {e}"),
            })?;

        // Convert MCP Content items to a single string result
        let mut output = String::new();
        for content in &result.content {
            match content {
                pmcp::Content::Text { text } => {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(text);
                }
                pmcp::Content::Image { mime_type, .. } => {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&format!("[image: {mime_type}]"));
                }
                pmcp::Content::Resource { uri, text, .. } => {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    if let Some(text) = text {
                        output.push_str(text);
                    } else {
                        output.push_str(&format!("[resource: {uri}]"));
                    }
                }
            }
        }

        if result.is_error {
            return Err(SakamotoError::McpError {
                server: self.name.clone(),
                reason: format!("tool `{tool_name}` returned error: {output}"),
            });
        }

        Ok(output)
    }

    /// Shut down the connection and release resources.
    ///
    /// The child process transport will kill the child when dropped.
    pub async fn close(&self) {
        let mut guard = self.client.lock().await;
        guard.take();
    }
}
