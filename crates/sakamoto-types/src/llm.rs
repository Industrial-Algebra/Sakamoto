//! LLM conversation types: messages, tool calls, and tool definitions.
//!
//! These types are backend-agnostic — they represent the common structure
//! shared by Anthropic, OpenAI, Ollama, and other LLM APIs.

use std::collections::HashMap;

/// A role in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in a conversation with an LLM.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

/// The content of a message — either plain text or a sequence of content blocks.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content.
    Text(String),
    /// Structured content blocks (text + tool use + tool results).
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    /// Extract plain text content, joining blocks if necessary.
    pub fn as_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// A content block within a message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text.
    Text { text: String },

    /// A tool use request from the assistant.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// A tool result returned to the assistant.
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

/// A tool call extracted from an LLM response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// Unique ID for this tool call (used to match results).
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Arguments as a JSON object.
    pub input: serde_json::Value,
}

/// The result of executing a tool call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    /// The ID of the tool call this result corresponds to.
    pub tool_use_id: String,
    /// The output content.
    pub content: String,
    /// Whether this result represents an error.
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn success(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    /// Create an error tool result.
    pub fn error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: true,
        }
    }
}

/// Definition of a tool that can be provided to an LLM.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDef {
    /// The tool name (must be unique within a toolset).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// The response from an LLM after processing a conversation.
#[derive(Debug, Clone)]
pub enum LlmResponse {
    /// The LLM wants to call one or more tools.
    ToolCalls(Vec<ToolCall>),
    /// The LLM has produced a final text response.
    Final(String),
}

impl LlmResponse {
    /// Returns `true` if the LLM wants to call tools.
    pub fn is_tool_calls(&self) -> bool {
        matches!(self, Self::ToolCalls(_))
    }

    /// Returns `true` if this is a final response.
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Final(_))
    }

    /// Extract tool calls, if any.
    pub fn into_tool_calls(self) -> Option<Vec<ToolCall>> {
        match self {
            Self::ToolCalls(calls) => Some(calls),
            _ => None,
        }
    }
}

/// Usage statistics from an LLM call.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TokenUsage {
    /// Input tokens consumed.
    pub input_tokens: u64,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Estimated cost in USD (if available).
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

impl TokenUsage {
    /// Total tokens (input + output).
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// Accumulate usage from another call.
    pub fn accumulate(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        match (self.cost_usd, other.cost_usd) {
            (Some(a), Some(b)) => self.cost_usd = Some(a + b),
            (None, Some(b)) => self.cost_usd = Some(b),
            _ => {}
        }
    }
}

/// Metadata about a model's capabilities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    /// Provider name (e.g., "anthropic", "openai", "ollama").
    pub provider: String,
    /// Model identifier (e.g., "claude-sonnet-4-6").
    pub model_id: String,
    /// Whether this model supports tool use / function calling.
    #[serde(default)]
    pub supports_tool_use: bool,
    /// Maximum context window in tokens.
    #[serde(default)]
    pub max_context_tokens: Option<u64>,
    /// Optional extra capabilities.
    #[serde(default)]
    pub capabilities: HashMap<String, bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_content_text_extracts() {
        let content = MessageContent::Text("hello".into());
        assert_eq!(content.as_text(), "hello");
    }

    #[test]
    fn message_content_blocks_extracts_text_only() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "line 1".into(),
            },
            ContentBlock::ToolUse {
                id: "tc_1".into(),
                name: "read".into(),
                input: serde_json::json!({"path": "foo.rs"}),
            },
            ContentBlock::Text {
                text: "line 2".into(),
            },
        ]);
        assert_eq!(content.as_text(), "line 1\nline 2");
    }

    #[test]
    fn tool_result_success_and_error() {
        let ok = ToolResult::success("tc_1", "file contents");
        assert!(!ok.is_error);
        assert_eq!(ok.content, "file contents");

        let err = ToolResult::error("tc_2", "file not found");
        assert!(err.is_error);
    }

    #[test]
    fn tool_call_roundtrips_json() {
        let call = ToolCall {
            id: "tc_1".into(),
            name: "shell".into(),
            input: serde_json::json!({"command": "cargo clippy"}),
        };
        let json = serde_json::to_string(&call).unwrap();
        let parsed: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "shell");
        assert_eq!(parsed.input["command"], "cargo clippy");
    }

    #[test]
    fn tool_def_roundtrips_json() {
        let def = ToolDef {
            name: "read_file".into(),
            description: "Read a file from the filesystem".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        };
        let json = serde_json::to_string(&def).unwrap();
        let parsed: ToolDef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "read_file");
    }

    #[test]
    fn llm_response_variant_checks() {
        let tool_resp = LlmResponse::ToolCalls(vec![ToolCall {
            id: "tc_1".into(),
            name: "read".into(),
            input: serde_json::json!({}),
        }]);
        assert!(tool_resp.is_tool_calls());
        assert!(!tool_resp.is_final());

        let final_resp = LlmResponse::Final("done".into());
        assert!(final_resp.is_final());
        assert!(!final_resp.is_tool_calls());
    }

    #[test]
    fn llm_response_into_tool_calls() {
        let resp = LlmResponse::ToolCalls(vec![ToolCall {
            id: "tc_1".into(),
            name: "read".into(),
            input: serde_json::json!({}),
        }]);
        let calls = resp.into_tool_calls().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");

        let final_resp = LlmResponse::Final("done".into());
        assert!(final_resp.into_tool_calls().is_none());
    }

    #[test]
    fn token_usage_accumulates() {
        let mut a = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: Some(0.01),
        };
        let b = TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cost_usd: Some(0.02),
        };
        a.accumulate(&b);
        assert_eq!(a.input_tokens, 300);
        assert_eq!(a.output_tokens, 150);
        assert_eq!(a.total(), 450);
        assert!((a.cost_usd.unwrap() - 0.03).abs() < f64::EPSILON);
    }

    #[test]
    fn token_usage_accumulates_with_none_cost() {
        let mut a = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: None,
        };
        let b = TokenUsage {
            input_tokens: 200,
            output_tokens: 100,
            cost_usd: Some(0.02),
        };
        a.accumulate(&b);
        assert_eq!(a.cost_usd, Some(0.02));
    }

    #[test]
    fn content_block_serializes_tagged() {
        let block = ContentBlock::ToolUse {
            id: "tc_1".into(),
            name: "read".into(),
            input: serde_json::json!({"path": "foo.rs"}),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["name"], "read");
    }

    #[test]
    fn role_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), "\"tool\"");
    }
}
