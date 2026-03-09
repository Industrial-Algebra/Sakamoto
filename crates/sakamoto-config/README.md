# sakamoto-config

Configuration parsing for the Sakamoto orchestrator.

Reads `sakamoto.toml` project configuration and user-level defaults from `~/.config/sakamoto/config.toml`. Callers provide TOML strings — this crate performs no filesystem access.

**Wasm-safe**: compiles to `wasm32-unknown-unknown` with no system dependencies.

## Usage

```rust
use sakamoto_config::{parse_project_config, parse_user_config, merge_configs};

let project = parse_project_config(toml_str)?;
let user = parse_user_config(user_toml_str)?;
let merged = merge_configs(project, &user);
```

Project-level values take precedence. User-level values fill in gaps for LLM backends, toolsets, output format, and validation commands.

## Configuration Schema

```toml
[project]
name = "my-app"

[llm.default]
provider = "anthropic"          # anthropic, openai, ollama
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_API_KEY"  # omit for proxied mode
base_url = "https://..."        # override for local models
max_tokens = 4096
temperature = 0.7

[toolsets.default]
builtin = ["git", "fs", "shell", "lint", "test"]
mcp_servers = ["filesystem", "github"]

[pipeline.default]
stages = ["context", "code", "lint", "test", "commit"]
max_ci_rounds = 2               # default: 2
interaction = "autonomous"      # default interaction policy

[pipeline.default.stage_overrides.code]
llm_backend = "planning"        # use a different LLM for this stage
toolset = "default"
interaction = "collaborate"
max_iterations = 30
command = "..."                  # for command-based stages

[validation]
lint_command = "cargo clippy --workspace -- -D warnings"
fmt_command = "cargo fmt --all --check"
test_command = "cargo test --workspace"

[output]
default = "pr"                  # pr, commit, patch, files

[rules]
paths = [".sakamoto/rules/*.md"]
```

## Config Types

- **`ProjectConfig`** — full project-level configuration
- **`UserConfig`** — user-level defaults (`~/.config/sakamoto/config.toml`)
- **`LlmConfig`** — LLM backend definition (provider, model, auth, parameters)
- **`PipelineConfig`** — pipeline stages, retry rounds, per-stage overrides
- **`ToolsetConfig`** — named group of built-in tools and MCP servers
- **`ValidationConfig`** — lint, format, and test commands
- **`OutputFormat`** — `Pr`, `Commit`, `Patch`, `Files`

## License

MIT
