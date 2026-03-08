# Sakamoto — Agent Rules

## Project

Sakamoto is a pipeline-oriented coding agent orchestrator. See DESIGN.md and ROADMAP.md.

## Coding Standards

- Idiomatic Rust. Follow standard Rust API guidelines.
- Test-Driven Development: write tests before implementation.
- Use phantom types for state machines. Use algebraic types (enums) for modeling.
- Prefer `thiserror` for library errors, `anyhow` only in `sakamoto-cli`.
- Use `rayon` for CPU-bound parallelism, `tokio` for async I/O.
- No unwrap/expect in library crates. Return `Result` or `Option`.

## Wasm Boundary

- `sakamoto-types`, `sakamoto-core`, `sakamoto-config` MUST compile to
  `wasm32-unknown-unknown`. No system dependencies (no filesystem, no network, no
  process spawning) in these crates.
- Feature-gate any async runtime dependency in Wasm-safe crates.

## Git Workflow

- Branch from `develop`, not `main`.
- Branch naming: `feature/description`, `chore/description`, `fix/description`.
- PRs target `develop`. Release PRs target `main`.
- Pre-commit hooks run fmt, clippy, and tests. Do not skip them.

## Architecture

- Pipeline stages are composable DAG nodes, not a fixed linear sequence.
- Each stage can use a different LLM backend and toolset.
- MCP is the primary tool integration mechanism (via pmcp crate).
- Interaction policies (autonomous/confirm/collaborate/delegate) are per-stage.
