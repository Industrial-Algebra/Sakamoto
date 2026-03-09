//! `sakamoto init` — generate a default sakamoto.toml.

use std::path::Path;

use anyhow::{Context, bail};

const DEFAULT_CONFIG: &str = r#"[project]
name = ""  # TODO: set your project name

# LLM backends
# [llm.default]
# provider = "anthropic"
# model = "claude-sonnet-4-6"
# api_key_env = "ANTHROPIC_API_KEY"

# Tool configuration
# [toolsets.default]
# builtin = ["git", "fs", "shell", "lint", "test"]
# mcp_servers = []

# Default pipeline
[pipeline.default]
stages = ["context", "code", "lint", "test", "commit"]

# Validation commands
# [validation]
# lint_command = "cargo clippy --workspace -- -D warnings"
# test_command = "cargo test --workspace"
"#;

pub fn execute() -> anyhow::Result<()> {
    let config_path = Path::new("sakamoto.toml");

    if config_path.exists() {
        bail!("sakamoto.toml already exists in this directory");
    }

    std::fs::write(config_path, DEFAULT_CONFIG).context("failed to write sakamoto.toml")?;

    tracing::info!("created sakamoto.toml");
    println!("Created sakamoto.toml — edit it to configure your project.");
    Ok(())
}
