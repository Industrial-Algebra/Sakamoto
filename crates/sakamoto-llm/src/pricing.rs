//! Static model pricing table and cost calculation.

use sakamoto_types::llm::{ModelPricing, TokenUsage};

/// Look up pricing for a known model.
///
/// Returns `None` for unknown models (e.g., local Ollama models).
pub fn lookup(provider: &str, model: &str) -> Option<ModelPricing> {
    match (provider, model) {
        // Anthropic
        ("anthropic", m) if m.starts_with("claude-opus") => Some(ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
        }),
        ("anthropic", m) if m.starts_with("claude-sonnet") => Some(ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        }),
        ("anthropic", m) if m.starts_with("claude-haiku") => Some(ModelPricing {
            input_per_mtok: 0.80,
            output_per_mtok: 4.0,
        }),

        // OpenAI
        ("openai", "gpt-4o") => Some(ModelPricing {
            input_per_mtok: 2.50,
            output_per_mtok: 10.0,
        }),
        ("openai", "gpt-4o-mini") => Some(ModelPricing {
            input_per_mtok: 0.15,
            output_per_mtok: 0.60,
        }),
        ("openai", m) if m.starts_with("o3") => Some(ModelPricing {
            input_per_mtok: 10.0,
            output_per_mtok: 40.0,
        }),

        _ => None,
    }
}

/// Calculate cost in USD given pricing and token usage.
pub fn calculate_cost(pricing: &ModelPricing, usage: &TokenUsage) -> f64 {
    let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * pricing.input_per_mtok;
    let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * pricing.output_per_mtok;
    input_cost + output_cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_anthropic_sonnet() {
        let pricing = lookup("anthropic", "claude-sonnet-4-6").unwrap();
        assert!((pricing.input_per_mtok - 3.0).abs() < f64::EPSILON);
        assert!((pricing.output_per_mtok - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_anthropic_opus() {
        let pricing = lookup("anthropic", "claude-opus-4-6").unwrap();
        assert!((pricing.input_per_mtok - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_anthropic_haiku() {
        let pricing = lookup("anthropic", "claude-haiku-4-5-20251001").unwrap();
        assert!((pricing.input_per_mtok - 0.80).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_openai_gpt4o() {
        let pricing = lookup("openai", "gpt-4o").unwrap();
        assert!((pricing.input_per_mtok - 2.50).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_openai_gpt4o_mini() {
        let pricing = lookup("openai", "gpt-4o-mini").unwrap();
        assert!((pricing.input_per_mtok - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_openai_o3() {
        let pricing = lookup("openai", "o3-mini").unwrap();
        assert!((pricing.input_per_mtok - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lookup_unknown_returns_none() {
        assert!(lookup("ollama", "llama3").is_none());
        assert!(lookup("anthropic", "unknown-model").is_none());
    }

    #[test]
    fn calculate_cost_basic() {
        let pricing = ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        };
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cost_usd: None,
        };
        let cost = calculate_cost(&pricing, &usage);
        // 1000/1M * 3.0 + 500/1M * 15.0 = 0.003 + 0.0075 = 0.0105
        assert!((cost - 0.0105).abs() < 1e-10);
    }

    #[test]
    fn calculate_cost_zero_tokens() {
        let pricing = ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        };
        let usage = TokenUsage::default();
        assert!((calculate_cost(&pricing, &usage)).abs() < f64::EPSILON);
    }
}
