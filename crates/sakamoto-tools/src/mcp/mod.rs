//! MCP client integration for Sakamoto.
//!
//! Provides [`McpConnection`] for connecting to MCP servers (via stdio or HTTP
//! transports), discovering tools, and executing tool calls. Each discovered
//! tool is wrapped as an [`McpTool`] implementing the standard [`Tool`](crate::tool::Tool)
//! trait so it can be registered in a [`ToolRouter`](crate::router::ToolRouter).

mod connection;
mod tool;
mod transport;

pub use connection::McpConnection;
pub use tool::McpTool;
pub use transport::ChildProcessTransport;
