//! `sakamoto run` — execute a pipeline with a task description.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};

use sakamoto_config::{LlmConfig, McpTransport, ProjectConfig};
use sakamoto_executor::local::LocalExecutor;
use sakamoto_llm::LlmBackend;
use sakamoto_llm::providers::anthropic::AnthropicBackend;
use sakamoto_llm::providers::openai_compat::OpenAiCompatBackend;
use sakamoto_tools::builtin::fs_read::FsReadTool;
use sakamoto_tools::builtin::fs_write::FsWriteTool;
use sakamoto_tools::builtin::shell::ShellTool;
use sakamoto_tools::mcp::{McpConnection, McpTool};
use sakamoto_tools::router::ToolRouter;
use sakamoto_tools::tool::Tool;

pub async fn execute(task: &str, pipeline: &str) -> anyhow::Result<()> {
    let config_path = Path::new("sakamoto.toml");
    if !config_path.exists() {
        bail!("no sakamoto.toml found — run `sakamoto init` to create one");
    }

    let config = load_config(config_path)?;
    let working_dir = std::env::current_dir().context("failed to get current directory")?;

    tracing::info!(
        project = %config.project.name,
        pipeline = %pipeline,
        task = %task,
        "starting pipeline"
    );

    let mut executor = LocalExecutor::new(config.clone(), working_dir.clone());

    // Register LLM backends
    for (name, llm_config) in &config.llm {
        match build_llm_backend(llm_config) {
            Ok(backend) => {
                tracing::info!(name = %name, provider = %llm_config.provider, "registered LLM backend");
                executor.add_llm_backend(name.clone(), backend);
            }
            Err(e) => {
                tracing::warn!(name = %name, error = %e, "skipping LLM backend");
            }
        }
    }

    // Register tool routers
    for (name, toolset_config) in &config.toolsets {
        let mut router = ToolRouter::new();

        for builtin_name in &toolset_config.builtin {
            if let Some(tool) = build_builtin_tool(builtin_name, &working_dir)
                && let Err(e) = router.register(tool)
            {
                tracing::warn!(tool = %builtin_name, error = %e, "failed to register tool");
            }
        }

        // Connect to MCP servers and register their tools
        for server_name in &toolset_config.mcp_servers {
            let Some(server_config) = config.mcp_servers.get(server_name) else {
                tracing::warn!(
                    toolset = %name,
                    server = %server_name,
                    "MCP server not found in [mcp_server] config — skipping"
                );
                continue;
            };

            let connection = match &server_config.transport {
                McpTransport::Stdio => {
                    let Some(command) = &server_config.command else {
                        tracing::warn!(
                            server = %server_name,
                            "stdio MCP server missing 'command' — skipping"
                        );
                        continue;
                    };
                    match McpConnection::connect_stdio(
                        server_name,
                        command,
                        &server_config.args,
                        &server_config.env,
                    )
                    .await
                    {
                        Ok(conn) => Arc::new(conn),
                        Err(e) => {
                            tracing::warn!(
                                server = %server_name,
                                error = %e,
                                "failed to connect to MCP server — skipping"
                            );
                            continue;
                        }
                    }
                }
                McpTransport::Http => {
                    tracing::warn!(
                        server = %server_name,
                        "HTTP MCP transport not yet implemented — skipping"
                    );
                    continue;
                }
            };

            for tool_info in connection.tools() {
                let mcp_tool = McpTool::new(Arc::clone(&connection), tool_info);
                if let Err(e) = router.register(Arc::new(mcp_tool)) {
                    tracing::warn!(
                        server = %server_name,
                        tool = %tool_info.name,
                        error = %e,
                        "failed to register MCP tool"
                    );
                }
            }
        }

        tracing::info!(name = %name, tools = router.len(), "registered toolset");
        executor.add_tool_router(name.clone(), Arc::new(router));
    }

    let result = executor
        .run_pipeline(pipeline, task)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Report results
    println!("\n── Pipeline Complete ──");
    println!("Stages: {}", result.stages_executed.join(" → "));
    if result.retries > 0 {
        println!("Retries: {}", result.retries);
    }
    let usage = &result.token_usage;
    if usage.input_tokens > 0 || usage.output_tokens > 0 {
        println!(
            "Tokens: {} in / {} out",
            usage.input_tokens, usage.output_tokens
        );
    }

    if !result.context.diagnostics.is_empty() {
        println!("\nDiagnostics:");
        for diag in &result.context.diagnostics {
            println!("  [{:?}] {}: {}", diag.severity, diag.stage, diag.message);
        }
    }

    Ok(())
}

/// Load and merge project + user configs.
pub fn load_config(config_path: &Path) -> anyhow::Result<ProjectConfig> {
    let toml_str = std::fs::read_to_string(config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;

    let project_config =
        sakamoto_config::parse_project_config(&toml_str).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Try loading user config
    let user_config_path = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("sakamoto")
        .join("config.toml");

    if user_config_path.exists() {
        let user_toml = std::fs::read_to_string(&user_config_path)
            .with_context(|| format!("failed to read {}", user_config_path.display()))?;
        let user_config =
            sakamoto_config::parse_user_config(&user_toml).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(sakamoto_config::merge_configs(project_config, &user_config))
    } else {
        Ok(project_config)
    }
}

/// Build an LLM backend from config.
fn build_llm_backend(config: &LlmConfig) -> anyhow::Result<Arc<dyn LlmBackend>> {
    match config.provider.as_str() {
        "anthropic" => {
            let api_key = match &config.api_key_env {
                Some(env_var) => std::env::var(env_var)
                    .with_context(|| format!("environment variable {env_var} not set"))?,
                None => bail!("anthropic backend requires api_key_env"),
            };

            Ok(Arc::new(AnthropicBackend::new(
                api_key,
                &config.model,
                config.base_url.clone(),
                config.max_tokens,
                config.temperature,
            )))
        }
        "openai" | "ollama" => {
            let api_key = config
                .api_key_env
                .as_ref()
                .and_then(|env_var| std::env::var(env_var).ok());

            Ok(Arc::new(OpenAiCompatBackend::new(
                api_key,
                &config.model,
                config.base_url.clone(),
                config.max_tokens,
                config.temperature,
            )))
        }
        other => bail!("unsupported LLM provider: {other}"),
    }
}

/// Build a built-in tool by name.
fn build_builtin_tool(name: &str, working_dir: &Path) -> Option<Arc<dyn Tool>> {
    match name {
        "shell" => Some(Arc::new(ShellTool::new(
            working_dir.to_path_buf(),
            Duration::from_secs(30),
        ))),
        "fs_read" => Some(Arc::new(FsReadTool::new(working_dir))),
        "fs_write" => Some(Arc::new(FsWriteTool::new(working_dir))),
        other => {
            tracing::warn!(tool = %other, "unknown built-in tool — skipping");
            None
        }
    }
}
