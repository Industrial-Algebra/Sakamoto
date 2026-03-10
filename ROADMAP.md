# Sakamoto: Roadmap

## v0.1.0 — Foundation (Current)

**Goal**: End-to-end execution of a single pipeline that can fix clippy warnings in a
Rust project and open a PR.

### Crate Milestones

#### sakamoto-types
- [x] Core error types (`SakamotoError` enum)
- [x] Pipeline state phantom types (`Pending`, `Hydrated`, `Planned`, `Executed`,
      `Validated`, `Emitted`)
- [x] `Pipeline<S>` struct with state transitions
- [x] `StageOutput` algebra (`Continue`, `Retry`, `Fail`, `Fork`)
- [x] `InteractionPolicy` enum
- [x] `ContextBundle` type (accumulated context for LLM calls)
- [x] `ContextRef` enum (file paths, GitHub issues, URLs, symbols)
- [x] `Message` / `ToolCall` / `ToolResult` types for LLM conversation
- [x] `ToolDef` type (tool name, description, JSON schema for parameters)

#### sakamoto-config
- [x] `sakamoto.toml` schema definition
- [x] Parse project config: LLM backends, toolsets, pipeline definitions, validation
      commands, output format
- [ ] Parse rule files (`.sakamoto/rules/*.md`) with subdirectory scoping
- [x] Config validation and defaults
- [x] Config merging (project + user-level `~/.config/sakamoto/config.toml`)

#### sakamoto-llm
- [x] `LlmBackend` trait (`complete`, `model_info`)
- [x] Anthropic implementation (Messages API with tool use)
- [x] OpenAI-compatible implementation (Chat Completions with function calling)
- [x] Ollama implementation (via OpenAI-compatible backend)
- [ ] Streaming response support
- [ ] Token counting / cost estimation

#### sakamoto-tools
- [x] `Tool` trait and `ToolRouter` for dispatching tool calls
- [x] Built-in tools:
  - [x] `shell` — execute commands with timeout and output capture
  - [ ] `fs_read` — structured file reading with line ranges and truncation
  - [ ] `fs_write` — atomic file writes with path validation
- [ ] MCP client integration via pmcp
  - [ ] Connect to MCP servers (stdio and HTTP transports)
  - [ ] Discover and register tools from MCP servers
  - [ ] Execute MCP tool calls and return results
- [ ] Toolset management (named subsets of tools per stage)

> **Design decision**: `git`, linters, test runners, and language-specific tools are
> accessed via the `shell` tool and the execution environment (developer's PATH or Nix
> flake), not reimplemented as Rust built-in tools. See DESIGN.md "Tooling Philosophy".

#### sakamoto-context
- [x] Context reference parsers — extract `ContextRef`s from task text
  - [x] File path parser (absolute and relative)
  - [x] GitHub issue/PR URL parser
  - [x] Generic URL parser
  - [x] Symbol name parser (function/type references)
- [ ] `ContextFetcher` trait — resolve `ContextRef` to content
  - [ ] Filesystem fetcher (read files, directory listings)
  - [ ] GitHub fetcher (via `gh` CLI — issues, PRs, comments)
  - [ ] HTTP fetcher (generic URL content)
- [ ] `ContextEngine` — orchestrate parsing and fetching into a `ContextBundle`
- [ ] Pre-hydration: run fetchers before LLM loop starts

#### sakamoto-core
- [x] `Stage` trait — unit of pipeline execution
- [x] `LlmClient` / `ToolExecutor` abstraction traits (Wasm-safe boundary)
- [x] `ReactLoop` — iterative LLM + tool-use execution engine
- [x] DAG pipeline runner
  - [x] Topological sort of stages (Kahn's algorithm)
  - [ ] Parallel execution of independent stages (rayon + tokio)
  - [x] Retry routing on validation failure (up to `max_ci_rounds`)
- [ ] `PlanStage` — LLM generates an execution plan
- [ ] Interaction policy enforcement per stage

> **Note**: Built-in stages (ContextStage, CodeStage, CommandStage for lint/test/commit/pr)
> live in `sakamoto-executor`, not `sakamoto-core`, to keep core Wasm-safe.

#### sakamoto-executor
- [x] `Executor` trait
- [x] `LocalExecutor` — run in current working directory
- [x] `LlmAdapter` / `ToolAdapter` — bridge native crates into core traits
- [x] Built-in stages: `ContextStage`, `CodeStage`, `CommandStage` (lint/test/commit/pr)

#### sakamoto-cli
- [x] `sakamoto run "task description"` — execute a pipeline
- [x] `sakamoto init` — generate `sakamoto.toml` in current project
- [x] `sakamoto check` — validate config and tool connectivity
- [x] Basic stdout logging with tracing

### Infrastructure
- [x] Cargo workspace with all crates
- [x] CI: GitHub Actions (fmt, clippy, test)
- [x] Git hooks (pre-commit: fmt, clippy, test)
- [x] Dependabot
- [x] Gitflow branching (main ← develop ← feature/*)
- [ ] Nix flake (dev shell + build)
- [x] CLAUDE.md for agent-assisted development
- [ ] `.sakamoto/rules` example rule files

### First End-to-End Test Case

**"Fix Clippy Warnings" pipeline:**
1. Context: read `cargo clippy` output, identify affected files
2. Plan: LLM produces a list of fixes
3. Code: ReAct loop applies fixes using fs tools
4. Lint: re-run `cargo clippy`, verify clean
5. Test: run `cargo test`, verify passing
6. Commit: stage and commit changes
7. PR: open PR via `gh pr create`

---

## v0.2.0 — Interactivity & Polish

- [ ] Ratatui TUI dashboard
  - [ ] Pipeline progress visualization
  - [ ] Stage output inspection
  - [ ] Interactive `Confirm` / `Collaborate` / `Delegate` modes
- [ ] `WorktreeExecutor` — git worktree isolation for parallel tasks
- [ ] Multiple concurrent pipeline execution
- [ ] GitHub Actions integration (trigger on issue labels)
- [ ] Pipeline templates (predefined pipelines for common tasks)
  - [ ] Fix clippy warnings
  - [ ] Add tests for uncovered code
  - [ ] Implement feature from GitHub issue
  - [ ] Code review / PR feedback
- [ ] Enhanced context pre-hydration
  - [ ] Crate documentation fetcher
  - [ ] Tree-sitter symbol search
  - [ ] Dependency graph analysis
- [ ] Cost tracking and token budget enforcement

---

## v0.3.0 — GUI & Composition

- [ ] Leptos + Mingot web GUI
  - [ ] Node-based pipeline composer (Mingot Phase 7)
  - [ ] Pipeline template library
  - [ ] Execution monitoring dashboard
  - [ ] Configuration editor
- [ ] Sub-pipeline composition (pipelines as stages within pipelines)
- [ ] Pipeline serialization/sharing (export/import pipeline definitions)
- [ ] Sakamoto as MCP server (expose pipeline execution to other tools)
- [ ] `NixContainerExecutor` — full OS isolation via Nix flakes
  - [ ] Templated flake generation per project type (rust, typescript, python, go, generic)
  - [ ] Python 3 included in all environment templates as baseline
  - [ ] OCI image builds (following Yatima pattern)
  - [ ] Security hardening (read-only mounts, memory limits)
  - [ ] `[environment]` config section for selecting flake templates
- [ ] Gemma-powered natural language pipeline configuration

---

## v0.4.0 — Ecosystem

- [ ] Plugin system for custom stages
- [ ] Community pipeline template registry
- [ ] Slack bot entry point
- [ ] Multi-project orchestration (monorepo support)
- [ ] Metrics and analytics (success rates, token usage, time savings)
- [ ] wgpu acceleration for batch context processing
- [ ] Editor integrations (VS Code, Neovim)

---

## Design Constraints

### Coding Standards
- Idiomatic Rust throughout
- Test-Driven Development — tests written before implementation
- Phantom types and algebraic patterns for state machines and data modeling
- rayon for CPU-bound concurrency
- tokio for async I/O
- wgpu reserved for future GPU acceleration

### Git Workflow
- `main` ← release PRs ← `develop` ← feature/chore/fix branches
- Pre-commit hooks: `cargo fmt`, `cargo clippy`, `cargo test`
- CI on all PRs: fmt, clippy, test
- Dependabot enabled for cargo and GitHub Actions

### Wasm Compatibility
- `sakamoto-types`, `sakamoto-core`, `sakamoto-config` must compile to
  `wasm32-unknown-unknown`
- No system dependencies in Wasm-safe crates
- Feature gates for async runtime selection where needed
