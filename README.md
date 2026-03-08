# Sakamoto

A pipeline-oriented coding agent orchestrator built in Rust.

Sakamoto enables developers to define, compose, and execute multi-stage AI-driven coding
workflows — from fully autonomous one-shot task completion to interactive
human-in-the-loop collaboration.

## Quick Start

```bash
# Install git hooks
bash hooks/install.sh

# Build
cargo build --workspace

# Run
cargo run -p sakamoto-cli -- run "fix clippy warnings"
```

## Architecture

See [DESIGN.md](DESIGN.md) for the full design document and [ROADMAP.md](ROADMAP.md)
for the development roadmap.

### Crates

| Crate | Description | Wasm |
|-------|-------------|------|
| `sakamoto-types` | Core types, phantom states, error types | Yes |
| `sakamoto-core` | Pipeline DAG engine, ReAct loop | Yes |
| `sakamoto-config` | Configuration parsing (`sakamoto.toml`) | Yes |
| `sakamoto-llm` | LLM backend trait + implementations | No |
| `sakamoto-tools` | Built-in tools + MCP client/server | No |
| `sakamoto-context` | Context parsing and pre-hydration | No |
| `sakamoto-executor` | Execution environments (local, worktree, Nix) | No |
| `sakamoto-cli` | CLI and TUI (Ratatui) | No |
| `sakamoto-gui` | Web GUI (Leptos + Mingot) | Yes |

## Development

### Git Workflow

`main` ← release PRs ← `develop` ← `feature/*`, `chore/*`, `fix/*` branches

### Pre-commit Hooks

```bash
bash hooks/install.sh
```

Runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on every commit.

## License

MIT
