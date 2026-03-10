# sakamoto-llm

LLM backend trait and provider implementations for the Sakamoto orchestrator.

This crate defines the `LlmBackend` trait and provides implementations for Anthropic (Claude) and OpenAI-compatible APIs (including Ollama).

## LlmBackend Trait

```rust
#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        system: Option<&str>,
    ) -> Result<(LlmResponse, TokenUsage), SakamotoError>;

    fn model_info(&self) -> &ModelInfo;
}
```

## Providers

### Anthropic

Claude API via the Messages endpoint with native tool use support.

```rust
use sakamoto_llm::providers::anthropic::AnthropicBackend;

let backend = AnthropicBackend::new(
    api_key,
    "claude-sonnet-4-6",
    None,           // base_url override
    Some(4096),     // max_tokens
    Some(0.7),      // temperature
);
```

### OpenAI-Compatible

Works with OpenAI, Ollama, and any OpenAI-compatible API.

```rust
use sakamoto_llm::providers::openai_compat::OpenAiCompatBackend;

// Ollama (local, no API key)
let backend = OpenAiCompatBackend::new(
    None,                                           // api_key
    "qwen2.5-coder:32b",
    Some("http://localhost:11434/v1".into()),        // base_url
    None,
    None,
);
```

## LLM Routing

Three modes determined by `LlmConfig`:

| Mode | Config signal | Use case |
|------|---------------|----------|
| Proxied | No `api_key_env` | Claude Max, subscription auth handled by host |
| Direct | `api_key_env` set | API billing, CI |
| Local | `base_url` points to host | Ollama, llama.cpp |

## License

MIT
