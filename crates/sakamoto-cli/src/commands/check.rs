//! `sakamoto check` — validate configuration and report status.

use std::path::Path;

use crate::commands::run::load_config;

pub fn execute() -> anyhow::Result<()> {
    let config_path = Path::new("sakamoto.toml");

    if !config_path.exists() {
        println!("No sakamoto.toml found. Run `sakamoto init` to create one.");
        return Ok(());
    }

    let config = load_config(config_path)?;

    println!("Project: {}", config.project.name);

    // Check LLM backends
    if config.llm.is_empty() {
        println!("  LLM backends: none configured");
    } else {
        println!("  LLM backends:");
        for (name, llm) in &config.llm {
            let auth = if llm.api_key_env.is_some() {
                let env_var = llm.api_key_env.as_deref().unwrap_or("");
                if std::env::var(env_var).is_ok() {
                    "key set"
                } else {
                    "key MISSING"
                }
            } else {
                "no key needed"
            };
            println!("    {name}: {} ({}) [{auth}]", llm.provider, llm.model);
        }
    }

    // Check toolsets
    if config.toolsets.is_empty() {
        println!("  Toolsets: none configured");
    } else {
        println!("  Toolsets:");
        for (name, toolset) in &config.toolsets {
            println!(
                "    {name}: {} builtin, {} MCP servers",
                toolset.builtin.len(),
                toolset.mcp_servers.len()
            );
        }
    }

    // Check pipelines
    if config.pipelines.is_empty() {
        println!("  Pipelines: none configured");
    } else {
        println!("  Pipelines:");
        for (name, pipeline) in &config.pipelines {
            println!(
                "    {name}: {} stages [{}]",
                pipeline.stages.len(),
                pipeline.stages.join(" → ")
            );
        }
    }

    // Check validation
    if let Some(ref validation) = config.validation {
        println!("  Validation:");
        println!("    lint: {}", validation.lint_command);
        if let Some(ref test_cmd) = validation.test_command {
            println!("    test: {test_cmd}");
        }
    } else {
        println!("  Validation: not configured");
    }

    // Check user config
    let user_config_path = dirs_path();
    if user_config_path.exists() {
        println!("  User config: {}", user_config_path.display());
    } else {
        println!("  User config: not found (optional)");
    }

    println!("\nConfiguration is valid.");
    Ok(())
}

fn dirs_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| Path::new("~/.config").to_path_buf())
        .join("sakamoto")
        .join("config.toml")
}
