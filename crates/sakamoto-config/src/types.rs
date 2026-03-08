//! Configuration struct definitions for `sakamoto.toml`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use sakamoto_types::InteractionPolicy;

// ── Top-level configs ──────────────────────────────────────────────

/// Project-level configuration, parsed from `sakamoto.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project metadata.
    pub project: ProjectMeta,

    /// LLM backend definitions, keyed by name (e.g., "planning", "coding").
    #[serde(default)]
    pub llm: HashMap<String, LlmConfig>,

    /// Named toolsets, keyed by name (e.g., "default", "code-review").
    #[serde(default)]
    pub toolsets: HashMap<String, ToolsetConfig>,

    /// Named pipeline definitions, keyed by name (e.g., "default").
    #[serde(default, rename = "pipeline")]
    pub pipelines: HashMap<String, PipelineConfig>,

    /// Validation commands.
    #[serde(default)]
    pub validation: Option<ValidationConfig>,

    /// Output configuration.
    #[serde(default)]
    pub output: Option<OutputConfig>,

    /// Rule file configuration.
    #[serde(default)]
    pub rules: Option<RulesConfig>,
}

/// User-level configuration, parsed from `~/.config/sakamoto/config.toml`.
///
/// Provides defaults that are overridden by project-level configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserConfig {
    /// Default LLM backends.
    #[serde(default)]
    pub llm: HashMap<String, LlmConfig>,

    /// Default toolsets.
    #[serde(default)]
    pub toolsets: HashMap<String, ToolsetConfig>,

    /// Default output configuration.
    #[serde(default)]
    pub output: Option<OutputConfig>,

    /// Default validation commands.
    #[serde(default)]
    pub validation: Option<ValidationConfig>,
}

// ── Project metadata ───────────────────────────────────────────────

/// Basic project information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// Project name.
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
}

// ── LLM configuration ──────────────────────────────────────────────

/// Configuration for an LLM backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider name: "anthropic", "openai", "ollama", etc.
    pub provider: String,

    /// Model identifier (e.g., "claude-sonnet-4-6", "gpt-4o").
    pub model: String,

    /// Environment variable name containing the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,

    /// Base URL override (e.g., for local Ollama).
    #[serde(default)]
    pub base_url: Option<String>,

    /// Maximum tokens in the response.
    #[serde(default)]
    pub max_tokens: Option<u64>,

    /// Temperature (0.0 to 1.0).
    #[serde(default)]
    pub temperature: Option<f64>,
}

// ── Toolset configuration ──────────────────────────────────────────

/// A named set of tools available to pipeline stages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsetConfig {
    /// MCP server names to connect to.
    #[serde(default)]
    pub mcp_servers: Vec<String>,

    /// Built-in tool names to enable.
    #[serde(default)]
    pub builtin: Vec<String>,
}

// ── Pipeline configuration ─────────────────────────────────────────

/// Configuration for a named pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Ordered list of stage names in the pipeline.
    pub stages: Vec<String>,

    /// Maximum CI/validation retry rounds.
    #[serde(default = "default_max_ci_rounds")]
    pub max_ci_rounds: usize,

    /// Default interaction policy for all stages.
    #[serde(default)]
    pub interaction: Option<InteractionPolicy>,

    /// Per-stage configuration overrides, keyed by stage name.
    #[serde(default)]
    pub stage_overrides: HashMap<String, StageOverride>,
}

fn default_max_ci_rounds() -> usize {
    2
}

/// Per-stage overrides within a pipeline definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StageOverride {
    /// Override the LLM backend for this stage.
    #[serde(default)]
    pub llm_backend: Option<String>,

    /// Override the toolset for this stage.
    #[serde(default)]
    pub toolset: Option<String>,

    /// Override the interaction policy for this stage.
    #[serde(default)]
    pub interaction: Option<InteractionPolicy>,

    /// Override max iterations for this stage.
    #[serde(default)]
    pub max_iterations: Option<usize>,

    /// Override the shell command for this stage.
    #[serde(default)]
    pub command: Option<String>,
}

// ── Validation configuration ───────────────────────────────────────

/// Commands used for shift-left validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Lint command (e.g., "cargo clippy --workspace -- -D warnings").
    pub lint_command: String,

    /// Format check command (e.g., "cargo fmt --all --check").
    #[serde(default)]
    pub fmt_command: Option<String>,

    /// Test command (e.g., "cargo test --workspace").
    #[serde(default)]
    pub test_command: Option<String>,
}

// ── Output configuration ───────────────────────────────────────────

/// How the pipeline emits its results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Default output format.
    pub default: OutputFormat,
}

/// The format of pipeline output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Create a pull request.
    Pr,
    /// Create a git commit.
    Commit,
    /// Generate a patch file.
    Patch,
    /// Leave modified files in the working directory.
    Files,
}

// ── Rules configuration ────────────────────────────────────────────

/// Configuration for agent rule files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    /// Glob patterns for rule file paths.
    pub paths: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_config_with_all_fields() {
        let toml_str = r#"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_API_KEY"
base_url = "https://api.anthropic.com"
max_tokens = 4096
temperature = 0.7
"#;
        let config: LlmConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.provider, "anthropic");
        assert_eq!(config.model, "claude-sonnet-4-6");
        assert_eq!(config.api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
        assert_eq!(config.max_tokens, Some(4096));
        assert!((config.temperature.unwrap() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn llm_config_minimal() {
        let toml_str = r#"
provider = "ollama"
model = "gemma3:27b"
"#;
        let config: LlmConfig = toml::from_str(toml_str).unwrap();
        assert!(config.api_key_env.is_none());
        assert!(config.base_url.is_none());
        assert!(config.max_tokens.is_none());
    }

    #[test]
    fn toolset_config_empty_defaults() {
        let config: ToolsetConfig = toml::from_str("").unwrap();
        assert!(config.mcp_servers.is_empty());
        assert!(config.builtin.is_empty());
    }

    #[test]
    fn stage_override_partial() {
        let toml_str = r#"
interaction = "collaborate"
max_iterations = 30
"#;
        let over: StageOverride = toml::from_str(toml_str).unwrap();
        assert_eq!(over.interaction, Some(InteractionPolicy::Collaborate));
        assert_eq!(over.max_iterations, Some(30));
        assert!(over.llm_backend.is_none());
    }

    #[test]
    fn output_format_variants() {
        for (s, expected) in [
            ("\"pr\"", OutputFormat::Pr),
            ("\"commit\"", OutputFormat::Commit),
            ("\"patch\"", OutputFormat::Patch),
            ("\"files\"", OutputFormat::Files),
        ] {
            let parsed: OutputFormat = serde_json::from_str(s).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn validation_config_minimal() {
        let toml_str = r#"
lint_command = "cargo clippy"
"#;
        let config: ValidationConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.lint_command, "cargo clippy");
        assert!(config.fmt_command.is_none());
        assert!(config.test_command.is_none());
    }
}
