//! OpenAI-compatible API backend.
//!
//! Works with OpenAI, Ollama, and any other provider that implements
//! the OpenAI Chat Completions API with function calling.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use sakamoto_types::SakamotoError;
use sakamoto_types::llm::{LlmResponse, Message, ModelInfo, Role, TokenUsage, ToolCall, ToolDef};

use crate::LlmBackend;

const DEFAULT_OPENAI_URL: &str = "https://api.openai.com/v1";

/// OpenAI-compatible backend.
///
/// Works with OpenAI, Ollama (`http://localhost:11434/v1`), and any
/// provider implementing the Chat Completions API with function calling.
pub struct OpenAiCompatBackend {
    client: Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    info: ModelInfo,
}

impl OpenAiCompatBackend {
    /// Create a new OpenAI-compatible backend.
    pub fn new(
        api_key: Option<String>,
        model: impl Into<String>,
        base_url: Option<String>,
        max_tokens: Option<u64>,
        temperature: Option<f64>,
    ) -> Self {
        let model = model.into();
        let base = base_url.unwrap_or_else(|| DEFAULT_OPENAI_URL.into());
        let provider = if base.contains("localhost") || base.contains("127.0.0.1") {
            "ollama"
        } else {
            "openai"
        };

        let info = ModelInfo {
            provider: provider.into(),
            model_id: model.clone(),
            supports_tool_use: true,
            max_context_tokens: None,
            capabilities: Default::default(),
        };

        Self {
            client: Client::new(),
            api_key,
            base_url: base,
            model,
            max_tokens,
            temperature,
            info,
        }
    }

    /// Create a backend configured for local Ollama.
    pub fn ollama(model: impl Into<String>) -> Self {
        Self::new(
            None,
            model,
            Some("http://localhost:11434/v1".into()),
            None,
            None,
        )
    }

    fn build_request(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> OaiRequest {
        let mut api_messages = Vec::new();

        // Add system message if provided
        if let Some(sys) = system {
            api_messages.push(OaiMessage {
                role: "system".into(),
                content: Some(sys.into()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        for m in messages {
            if m.role == Role::System {
                continue;
            }
            api_messages.push(OaiMessage {
                role: match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                    Role::Tool => "tool".into(),
                    Role::System => unreachable!(),
                },
                content: Some(m.content.as_text()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        let oai_tools: Option<Vec<OaiTool>> = if tools.is_empty() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|t| OaiTool {
                        r#type: "function".into(),
                        function: OaiFunction {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.input_schema.clone(),
                        },
                    })
                    .collect(),
            )
        };

        OaiRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: oai_tools,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        }
    }
}

#[async_trait::async_trait]
impl LlmBackend for OpenAiCompatBackend {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> Result<(LlmResponse, TokenUsage), SakamotoError> {
        let request_body = self.build_request(messages, tools, system);
        let url = format!("{}/chat/completions", self.base_url);

        let mut req = self.client.post(&url).json(&request_body);

        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let response = req.send().await.map_err(|e| SakamotoError::LlmError {
            backend: self.info.provider.clone(),
            reason: e.to_string(),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SakamotoError::LlmError {
                backend: self.info.provider.clone(),
                reason: format!("HTTP {status}: {body}"),
            });
        }

        let api_response: OaiResponse =
            response.json().await.map_err(|e| SakamotoError::LlmError {
                backend: self.info.provider.clone(),
                reason: format!("failed to parse response: {e}"),
            })?;

        let usage = api_response
            .usage
            .map(|u| TokenUsage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
                cost_usd: None,
            })
            .unwrap_or_default();

        let choice =
            api_response
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| SakamotoError::LlmError {
                    backend: self.info.provider.clone(),
                    reason: "no choices in response".into(),
                })?;

        let llm_response = if let Some(tool_calls) = choice.message.tool_calls {
            LlmResponse::ToolCalls(
                tool_calls
                    .into_iter()
                    .map(|tc| ToolCall {
                        id: tc.id,
                        name: tc.function.name,
                        input: serde_json::from_str(&tc.function.arguments).unwrap_or_default(),
                    })
                    .collect(),
            )
        } else {
            LlmResponse::Final(choice.message.content.unwrap_or_default())
        };

        Ok((llm_response, usage))
    }

    fn model_info(&self) -> &ModelInfo {
        &self.info
    }
}

// ── API types (OpenAI Chat Completions) ────────────────────────────

#[derive(Serialize)]
struct OaiRequest {
    model: String,
    messages: Vec<OaiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OaiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Serialize)]
struct OaiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OaiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct OaiTool {
    r#type: String,
    function: OaiFunction,
}

#[derive(Serialize)]
struct OaiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct OaiResponse {
    choices: Vec<OaiChoice>,
    usage: Option<OaiUsage>,
}

#[derive(Deserialize)]
struct OaiChoice {
    message: OaiResponseMessage,
}

#[derive(Deserialize)]
struct OaiResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OaiToolCall>>,
}

#[derive(Serialize, Deserialize)]
struct OaiToolCall {
    id: String,
    function: OaiToolCallFunction,
}

#[derive(Serialize, Deserialize)]
struct OaiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OaiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sakamoto_types::llm::MessageContent;

    #[test]
    fn build_request_basic() {
        let backend =
            OpenAiCompatBackend::new(Some("sk-test".into()), "gpt-4o", None, Some(4096), None);

        let messages = vec![Message {
            role: Role::User,
            content: MessageContent::Text("Hello".into()),
        }];

        let request = backend.build_request(&messages, &[], Some("Be helpful"));
        assert_eq!(request.model, "gpt-4o");
        // system + user
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, "system");
        assert_eq!(request.messages[1].role, "user");
        assert!(request.tools.is_none());
    }

    #[test]
    fn ollama_shortcut() {
        let backend = OpenAiCompatBackend::ollama("gemma3:27b");
        assert_eq!(backend.info.provider, "ollama");
        assert_eq!(backend.model, "gemma3:27b");
        assert!(backend.api_key.is_none());
    }

    #[test]
    fn model_info_detects_provider() {
        let openai = OpenAiCompatBackend::new(Some("key".into()), "gpt-4o", None, None, None);
        assert_eq!(openai.model_info().provider, "openai");

        let ollama = OpenAiCompatBackend::ollama("llama3");
        assert_eq!(ollama.model_info().provider, "ollama");
    }

    #[test]
    fn oai_response_deserializes() {
        let json = r#"{
            "choices": [{
                "message": {
                    "content": "Hello!",
                    "tool_calls": null
                }
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        }"#;
        let response: OaiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("Hello!")
        );
    }

    #[test]
    fn oai_response_with_tool_calls() {
        let json = r#"{
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_123",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\": \"src/main.rs\"}"
                        }
                    }]
                }
            }],
            "usage": {"prompt_tokens": 20, "completion_tokens": 15}
        }"#;
        let response: OaiResponse = serde_json::from_str(json).unwrap();
        let tc = response.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].function.name, "read_file");
    }
}
