//! Anthropic Claude API backend.
//!
//! Implements the Anthropic Messages API with tool use support.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use sakamoto_types::SakamotoError;
use sakamoto_types::llm::{
    ContentBlock, LlmResponse, Message, MessageContent, ModelInfo, Role, TokenUsage, ToolCall,
    ToolDef,
};

use crate::LlmBackend;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

/// Anthropic Claude backend.
pub struct AnthropicBackend {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: u64,
    temperature: Option<f64>,
    info: ModelInfo,
}

impl AnthropicBackend {
    /// Create a new Anthropic backend.
    ///
    /// # Arguments
    /// - `api_key`: Anthropic API key
    /// - `model`: Model ID (e.g., "claude-sonnet-4-6")
    /// - `base_url`: Optional base URL override
    /// - `max_tokens`: Maximum output tokens (defaults to 4096)
    /// - `temperature`: Optional temperature (0.0-1.0)
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
        max_tokens: Option<u64>,
        temperature: Option<f64>,
    ) -> Self {
        let model = model.into();
        let info = ModelInfo {
            provider: "anthropic".into(),
            model_id: model.clone(),
            supports_tool_use: true,
            max_context_tokens: Some(200_000),
            capabilities: Default::default(),
        };

        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
            model,
            max_tokens: max_tokens.unwrap_or(4096),
            temperature,
            info,
        }
    }

    /// Build the request body for the Messages API.
    fn build_request(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> ApiRequest {
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| ApiMessage {
                role: match m.role {
                    Role::User | Role::Tool => "user".into(),
                    Role::Assistant => "assistant".into(),
                    Role::System => unreachable!(),
                },
                content: convert_content(&m.content),
            })
            .collect();

        let api_tools: Vec<ApiTool> = tools
            .iter()
            .map(|t| ApiTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect();

        ApiRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            system: system.map(String::from),
            messages: api_messages,
            tools: if api_tools.is_empty() {
                None
            } else {
                Some(api_tools)
            },
        }
    }
}

#[async_trait::async_trait]
impl LlmBackend for AnthropicBackend {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
        let request_body = self.build_request(messages, tools, system);
        let url = format!("{}/v1/messages", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| SakamotoError::LlmError {
                backend: "anthropic".into(),
                reason: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SakamotoError::LlmError {
                backend: "anthropic".into(),
                reason: format!("HTTP {status}: {body}"),
            });
        }

        let api_response: ApiResponse =
            response.json().await.map_err(|e| SakamotoError::LlmError {
                backend: "anthropic".into(),
                reason: format!("failed to parse response: {e}"),
            })?;

        let usage = TokenUsage {
            input_tokens: api_response.usage.input_tokens,
            output_tokens: api_response.usage.output_tokens,
            cost_usd: None,
        };

        // Extract tool calls or final text
        let mut tool_calls = Vec::new();
        let mut text_parts = Vec::new();

        for block in &api_response.content {
            match block {
                ApiContentBlock::Text { text } => {
                    text_parts.push(text.as_str());
                }
                ApiContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
                ApiContentBlock::ToolResult { .. } => {
                    // Tool results appear in request messages, not responses.
                }
            }
        }

        let llm_response = if tool_calls.is_empty() {
            LlmResponse::Final(text_parts.join("\n"))
        } else {
            LlmResponse::ToolCalls(tool_calls)
        };

        Ok((llm_response, usage))
    }

    fn model_info(&self) -> &ModelInfo {
        &self.info
    }
}

// ── API types (Anthropic Messages API) ─────────────────────────────

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: Vec<ApiContentBlock>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Serialize)]
struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ApiContentBlock>,
    usage: ApiUsage,
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct ApiUsage {
    input_tokens: u64,
    output_tokens: u64,
}

/// Convert our `MessageContent` into Anthropic API content blocks.
fn convert_content(content: &MessageContent) -> Vec<ApiContentBlock> {
    match content {
        MessageContent::Text(s) => vec![ApiContentBlock::Text { text: s.clone() }],
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .map(|b| match b {
                ContentBlock::Text { text } => ApiContentBlock::Text { text: text.clone() },
                ContentBlock::ToolUse { id, name, input } => ApiContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                },
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => ApiContentBlock::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: content.clone(),
                    is_error: *is_error,
                },
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_request_basic() {
        let backend = AnthropicBackend::new("test-key", "claude-sonnet-4-6", None, None, None);

        let messages = vec![Message {
            role: Role::User,
            content: MessageContent::Text("Hello".into()),
        }];

        let request = backend.build_request(&messages, &[], Some("You are helpful"));
        assert_eq!(request.model, "claude-sonnet-4-6");
        assert_eq!(request.max_tokens, 4096);
        assert_eq!(request.system.as_deref(), Some("You are helpful"));
        assert_eq!(request.messages.len(), 1);
        assert!(request.tools.is_none());
    }

    #[test]
    fn build_request_with_tools() {
        let backend = AnthropicBackend::new("test-key", "claude-sonnet-4-6", None, None, None);

        let tools = vec![ToolDef {
            name: "read_file".into(),
            description: "Read a file".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }];

        let request = backend.build_request(&[], &tools, None);
        assert!(request.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), 1);
        assert_eq!(request.tools.as_ref().unwrap()[0].name, "read_file");
    }

    #[test]
    fn build_request_filters_system_messages() {
        let backend = AnthropicBackend::new("test-key", "claude-sonnet-4-6", None, None, None);

        let messages = vec![
            Message {
                role: Role::System,
                content: MessageContent::Text("system prompt".into()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("hello".into()),
            },
        ];

        let request = backend.build_request(&messages, &[], None);
        // System messages are filtered out (system goes in the `system` field)
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
    }

    #[test]
    fn build_request_with_temperature() {
        let backend =
            AnthropicBackend::new("test-key", "claude-sonnet-4-6", None, Some(8192), Some(0.3));

        let request = backend.build_request(&[], &[], None);
        assert_eq!(request.max_tokens, 8192);
        assert!((request.temperature.unwrap() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn model_info_correct() {
        let backend = AnthropicBackend::new("test-key", "claude-opus-4-6", None, None, None);
        let info = backend.model_info();
        assert_eq!(info.provider, "anthropic");
        assert_eq!(info.model_id, "claude-opus-4-6");
        assert!(info.supports_tool_use);
    }

    #[test]
    fn convert_text_content() {
        let content = MessageContent::Text("hello".into());
        let blocks = convert_content(&content);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ApiContentBlock::Text { text } if text == "hello"));
    }

    #[test]
    fn convert_mixed_content() {
        let content = MessageContent::Blocks(vec![
            ContentBlock::Text {
                text: "please read".into(),
            },
            ContentBlock::ToolUse {
                id: "tc_1".into(),
                name: "read".into(),
                input: serde_json::json!({"path": "foo.rs"}),
            },
        ]);
        let blocks = convert_content(&content);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn api_response_deserializes() {
        let json = r#"{
            "content": [
                {"type": "text", "text": "Here's the plan"},
                {"type": "tool_use", "id": "tc_1", "name": "read_file", "input": {"path": "src/main.rs"}}
            ],
            "usage": {"input_tokens": 100, "output_tokens": 50},
            "stop_reason": "tool_use"
        }"#;
        let response: ApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.content.len(), 2);
        assert_eq!(response.usage.input_tokens, 100);
        assert_eq!(response.usage.output_tokens, 50);
    }

    #[test]
    fn api_response_text_only() {
        let json = r#"{
            "content": [{"type": "text", "text": "Done!"}],
            "usage": {"input_tokens": 50, "output_tokens": 10},
            "stop_reason": "end_turn"
        }"#;
        let response: ApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.content.len(), 1);
        assert!(matches!(
            &response.content[0],
            ApiContentBlock::Text { text } if text == "Done!"
        ));
    }
}
