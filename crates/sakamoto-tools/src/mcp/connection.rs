// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0

//! MCP connection lifecycle management.
//!
//! Manages the full lifecycle of an MCP server connection: spawn transport,
//! initialize the protocol, discover tools, execute calls, and shut down.
//! Supports both stdio (child process) and HTTP (streamable) transports.

use std::collections::HashMap;

use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::{Client, ClientCapabilities, Implementation, ToolInfo};
use tokio::sync::Mutex;

use sakamoto_types::SakamotoError;

use super::transport::ChildProcessTransport;

/// Internal enum to hold either transport's client type.
enum McpClient {
    Stdio(Client<ChildProcessTransport>),
    Http(Client<StreamableHttpTransport>),
}

/// An active connection to an MCP server.
///
/// Wraps a pmcp [`Client`] over either a stdio or HTTP transport and manages
/// the protocol lifecycle. Once initialized, the connection exposes the
/// server's tools and can execute calls.
pub struct McpConnection {
    /// Human-readable name for this connection (from config key).
    name: String,
    /// The pmcp client — `None` after close.
    client: Mutex<Option<McpClient>>,
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
            "connected to MCP server (stdio)"
        );

        Ok(Self {
            name: name.to_string(),
            client: Mutex::new(Some(McpClient::Stdio(client))),
            tools,
        })
    }

    /// Connect to an MCP server via HTTP (streamable) transport.
    ///
    /// Performs the MCP initialize handshake over HTTP and discovers
    /// available tools.
    pub async fn connect_http(name: &str, url: &str) -> Result<Self, SakamotoError> {
        let parsed_url: url::Url = url.parse().map_err(|e| SakamotoError::McpError {
            server: name.to_string(),
            reason: format!("invalid URL: {e}"),
        })?;

        let config = StreamableHttpTransportConfigBuilder::new(parsed_url).build();

        let transport = StreamableHttpTransport::new(config);

        let mut client: Client<StreamableHttpTransport> = Client::with_info(
            transport,
            Implementation {
                name: "sakamoto".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        );

        client
            .initialize(ClientCapabilities::default())
            .await
            .map_err(|e| SakamotoError::McpError {
                server: name.to_string(),
                reason: format!("initialization failed: {e}"),
            })?;

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
            "connected to MCP server (http)"
        );

        Ok(Self {
            name: name.to_string(),
            client: Mutex::new(Some(McpClient::Http(client))),
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

        let result = match client {
            McpClient::Stdio(c) => c.call_tool(tool_name.to_string(), arguments).await,
            McpClient::Http(c) => c.call_tool(tool_name.to_string(), arguments).await,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_http_invalid_url_returns_error() {
        let result = McpConnection::connect_http("test", "not a url").await;
        assert!(result.is_err());
        match result {
            Err(SakamotoError::McpError { reason, .. }) => {
                assert!(reason.contains("invalid URL"), "got: {reason}");
            }
            Err(other) => panic!("expected McpError, got: {other}"),
            Ok(_) => panic!("expected error"),
        }
    }
}
