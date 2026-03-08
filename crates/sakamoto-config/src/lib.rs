//! Configuration parsing for Sakamoto.
//!
//! Reads `sakamoto.toml` project configuration and user-level defaults
//! from `~/.config/sakamoto/config.toml`. Wasm-safe: no filesystem access
//! in this crate — callers provide TOML strings.

mod types;

pub use types::*;

use sakamoto_types::SakamotoError;

/// Parse a `sakamoto.toml` string into a [`ProjectConfig`].
pub fn parse_project_config(toml_str: &str) -> Result<ProjectConfig, SakamotoError> {
    toml::from_str(toml_str).map_err(|e| SakamotoError::ConfigError(e.to_string()))
}

/// Parse a user-level config string into a [`UserConfig`].
pub fn parse_user_config(toml_str: &str) -> Result<UserConfig, SakamotoError> {
    toml::from_str(toml_str).map_err(|e| SakamotoError::ConfigError(e.to_string()))
}

/// Merge user-level defaults into a project config.
///
/// Project-level values take precedence. User-level values fill in gaps.
pub fn merge_configs(mut project: ProjectConfig, user: &UserConfig) -> ProjectConfig {
    // Fill in default LLM backends from user config
    if project.llm.is_empty() {
        project.llm.clone_from(&user.llm);
    }

    // Fill in default toolsets from user config
    if project.toolsets.is_empty() {
        project.toolsets.clone_from(&user.toolsets);
    }

    // Fill in default output format
    if project.output.is_none() {
        project.output.clone_from(&user.output);
    }

    // Fill in default validation commands
    if project.validation.is_none() {
        project.validation.clone_from(&user.validation);
    }

    project
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_CONFIG: &str = r#"
[project]
name = "test-project"
"#;

    const FULL_CONFIG: &str = r#"
[project]
name = "my-app"
description = "A sample application"

[llm.planning]
provider = "anthropic"
model = "claude-opus-4-6"

[llm.coding]
provider = "anthropic"
model = "claude-sonnet-4-6"

[llm.review]
provider = "ollama"
model = "gemma3:27b"

[toolsets.default]
mcp_servers = ["filesystem", "github"]
builtin = ["git", "fs", "shell", "lint", "test"]

[toolsets.code-review]
mcp_servers = ["github", "sourcegraph"]
builtin = ["git", "fs"]

[pipeline.default]
stages = ["context", "plan", "code", "lint", "test", "commit", "pr"]
max_ci_rounds = 2

[pipeline.default.stage_overrides.code]
interaction = "collaborate"
max_iterations = 30

[validation]
lint_command = "cargo clippy --workspace -- -D warnings"
fmt_command = "cargo fmt --all --check"
test_command = "cargo test --workspace"

[output]
default = "pr"

[rules]
paths = [".sakamoto/rules/*.md"]
"#;

    const USER_CONFIG: &str = r#"
[llm.default]
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_API_KEY"

[toolsets.default]
mcp_servers = ["filesystem"]
builtin = ["git", "fs", "shell"]

[output]
default = "commit"

[validation]
lint_command = "cargo clippy"
fmt_command = "cargo fmt --check"
test_command = "cargo test"
"#;

    #[test]
    fn parse_minimal_config() {
        let config = parse_project_config(MINIMAL_CONFIG).unwrap();
        assert_eq!(config.project.name, "test-project");
        assert!(config.llm.is_empty());
        assert!(config.toolsets.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let config = parse_project_config(FULL_CONFIG).unwrap();
        assert_eq!(config.project.name, "my-app");
        assert_eq!(
            config.project.description.as_deref(),
            Some("A sample application")
        );

        // LLM backends
        assert_eq!(config.llm.len(), 3);
        let planning = &config.llm["planning"];
        assert_eq!(planning.provider, "anthropic");
        assert_eq!(planning.model, "claude-opus-4-6");

        // Toolsets
        assert_eq!(config.toolsets.len(), 2);
        let default_tools = &config.toolsets["default"];
        assert_eq!(default_tools.mcp_servers.len(), 2);
        assert_eq!(default_tools.builtin.len(), 5);

        // Pipeline
        let pipeline = &config.pipelines["default"];
        assert_eq!(pipeline.stages.len(), 7);
        assert_eq!(pipeline.max_ci_rounds, 2);

        // Stage overrides
        let code_override = &pipeline.stage_overrides["code"];
        assert_eq!(
            code_override.interaction,
            Some(sakamoto_types::InteractionPolicy::Collaborate)
        );
        assert_eq!(code_override.max_iterations, Some(30));

        // Validation
        let validation = config.validation.as_ref().unwrap();
        assert_eq!(
            validation.lint_command,
            "cargo clippy --workspace -- -D warnings"
        );

        // Output
        let output = config.output.as_ref().unwrap();
        assert_eq!(output.default, OutputFormat::Pr);

        // Rules
        let rules = config.rules.as_ref().unwrap();
        assert_eq!(rules.paths, vec![".sakamoto/rules/*.md"]);
    }

    #[test]
    fn parse_user_config_works() {
        let config = parse_user_config(USER_CONFIG).unwrap();
        assert_eq!(config.llm.len(), 1);
        let default = &config.llm["default"];
        assert_eq!(default.api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn merge_fills_gaps() {
        let project = parse_project_config(MINIMAL_CONFIG).unwrap();
        let user = parse_user_config(USER_CONFIG).unwrap();
        let merged = merge_configs(project, &user);

        // LLM should come from user config since project had none
        assert_eq!(merged.llm.len(), 1);
        assert!(merged.llm.contains_key("default"));

        // Toolsets should come from user config
        assert!(!merged.toolsets.is_empty());

        // Output should come from user config
        assert_eq!(
            merged.output.as_ref().unwrap().default,
            OutputFormat::Commit
        );
    }

    #[test]
    fn merge_project_takes_precedence() {
        let project = parse_project_config(FULL_CONFIG).unwrap();
        let user = parse_user_config(USER_CONFIG).unwrap();
        let merged = merge_configs(project, &user);

        // Project had 3 LLM backends, user had 1 — project wins
        assert_eq!(merged.llm.len(), 3);

        // Project output was "pr", user was "commit" — project wins
        assert_eq!(merged.output.as_ref().unwrap().default, OutputFormat::Pr);
    }

    #[test]
    fn invalid_toml_returns_config_error() {
        let result = parse_project_config("this is not valid toml [[[");
        assert!(result.is_err());
        match result.unwrap_err() {
            SakamotoError::ConfigError(msg) => assert!(!msg.is_empty()),
            other => panic!("expected ConfigError, got {other:?}"),
        }
    }

    #[test]
    fn output_format_serializes() {
        let json = serde_json::to_string(&OutputFormat::Pr).unwrap();
        assert_eq!(json, "\"pr\"");
    }

    #[test]
    fn pipeline_config_defaults() {
        let toml_str = r#"
[project]
name = "test"

[pipeline.quick]
stages = ["lint", "test"]
"#;
        let config = parse_project_config(toml_str).unwrap();
        let pipeline = &config.pipelines["quick"];
        assert_eq!(pipeline.max_ci_rounds, 2); // default
        assert!(pipeline.stage_overrides.is_empty());
    }
}
