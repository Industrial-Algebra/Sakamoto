# Sakamoto

A pipeline-oriented coding agent orchestrator built in Rust.

Sakamoto enables developers to define, compose, and execute multi-stage AI-driven coding
workflows — from fully autonomous one-shot task completion to interactive
human-in-the-loop collaboration.

Inspired by [Stripe's Minions](https://stripe.dev/blog/minions-stripes-one-shot-end-to-end-coding-agents),
Sakamoto adapts the core ideas — one-shot agents, context pre-hydration, curated MCP
toolsets, shift-left validation — into an open, extensible, and composable tool.

## Quick Start

```bash
# Install git hooks
bash hooks/install.sh

# Build
cargo build --workspace

# Generate a config file
cargo run -p sakamoto-cli -- init

# Validate your config
cargo run -p sakamoto-cli -- check

# Run a pipeline
cargo run -p sakamoto-cli -- run "fix clippy warnings"
```

## Configuration

Create a `sakamoto.toml` in your project root (or run `sakamoto init`):

```toml
[project]
name = "my-app"

[llm.default]
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_API_KEY"

[toolsets.default]
builtin = ["shell"]

[pipeline.default]
stages = ["context", "code", "lint", "test", "commit"]
max_ci_rounds = 2

[validation]
lint_command = "cargo clippy --workspace -- -D warnings"
test_command = "cargo test --workspace"
```

User-level defaults can be set in `~/.config/sakamoto/config.toml` — project config takes precedence.

## Architecture

```
┌────────────────────────────────────────────────────────┐
│                    sakamoto-cli                         │
│                  (CLI entry point)                      │
├────────────────────────────────────────────────────────┤
│                  sakamoto-executor                      │
│        (LocalExecutor, adapters, built-in stages)      │
├──────┬──────────┬───────────┬───────────┬─────────────┤
│config│  context  │    llm    │   tools   │    core     │
│(toml)│(hydration)│(backends) │(MCP+built)│(DAG,ReAct)  │
├──────┴──────────┴───────────┴───────────┴─────────────┤
│                   sakamoto-types                        │
│       (Phantom states, core types, errors)              │
└────────────────────────────────────────────────────────┘
```

### Crates

| Crate | Description | Wasm-safe |
|-------|-------------|-----------|
| [`sakamoto-types`](crates/sakamoto-types/) | Phantom-typed pipeline states, `StageOutput` algebra, LLM/tool types, errors | Yes |
| [`sakamoto-core`](crates/sakamoto-core/) | `Stage` trait, `ReactLoop`, `PipelineDag`, `PipelineRunner` | Yes |
| [`sakamoto-config`](crates/sakamoto-config/) | `sakamoto.toml` parsing, user config merging | Yes |
| [`sakamoto-llm`](crates/sakamoto-llm/) | `LlmBackend` trait, Anthropic and OpenAI-compatible providers | No |
| [`sakamoto-tools`](crates/sakamoto-tools/) | `Tool` trait, `ToolRouter`, built-in shell tool | No |
| [`sakamoto-context`](crates/sakamoto-context/) | Context reference parsing (file paths, GitHub refs, URLs, symbols) | No |
| [`sakamoto-executor`](crates/sakamoto-executor/) | `LocalExecutor`, adapter layer bridging llm/tools into core traits | No |
| [`sakamoto-cli`](crates/sakamoto-cli/) | `sakamoto run`, `sakamoto init`, `sakamoto check` | No |

### Wasm Boundary

`sakamoto-types`, `sakamoto-core`, and `sakamoto-config` compile to `wasm32-unknown-unknown` with no system dependencies. This enables the future web GUI to validate and compose pipelines in the browser.

Native-only crates (`sakamoto-llm`, `sakamoto-tools`, `sakamoto-context`, `sakamoto-executor`, `sakamoto-cli`) perform I/O: HTTP calls, process spawning, filesystem access.

## Pipeline Model

Pipelines are DAGs of typed stages. Stages that share no data dependencies can run in parallel.

```
                    ┌─► lint ──────┐
context ─► plan ─► code ─►         ├─► commit ─► pr
                    └─► test ──────┘
```

Failed validation stages route back to the code stage, up to `max_ci_rounds` times.

### Stage Output Algebra

Every stage returns a `StageOutput`:

- **Continue** — pass the context bundle to the next stage
- **Retry** — loop back to a retry point with diagnostic feedback
- **Fail** — halt the pipeline with an error
- **Fork** — fan out to parallel sub-pipelines (v0.2)

### Interaction Policies

Each stage declares its interactivity level:

| Policy | Behavior |
|--------|----------|
| `autonomous` | Runs to completion with no human input |
| `confirm` | Presents output, waits for approval |
| `collaborate` | Human can edit or augment LLM output |
| `delegate` | Hands off to human entirely |

### LLM Routing

Different LLM backends per pipeline stage. Three routing modes for containerized execution:

| Mode | Auth | Use case |
|------|------|----------|
| Proxied | Host handles auth | Claude Max, subscription-based |
| Direct | API key injected | API billing, CI |
| Local | None | Ollama, llama.cpp |

## Development

### Prerequisites

- Rust 2024 edition (1.85+)
- [cargo](https://rustup.rs/)

### Git Workflow

`main` ← release PRs ← `develop` ← `feature/*`, `chore/*`, `fix/*` branches

### Pre-commit Hooks

```bash
bash hooks/install.sh
```

Runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on every commit.

### Running Tests

```bash
cargo test --workspace
```

166 tests across 8 crates.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full development plan.

- **v0.1** (current) — End-to-end pipeline execution: context → code → lint → test → commit
- **v0.2** — Ratatui TUI, `WorktreeExecutor`, GitHub Actions integration, pipeline templates
- **v0.3** — Leptos + Mingot web GUI, `NixContainerExecutor`, sub-pipeline composition
- **v0.4** — Plugin system, community pipeline registry, editor integrations

## Design

See [DESIGN.md](DESIGN.md) for the full design document covering phantom-typed state machines,
ReAct agent loops, MCP integration, context pre-hydration, and execution environments.

## License

MIT
