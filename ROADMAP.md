# Sakamoto: Roadmap

## v0.1.0 — Foundation (Current)

**Goal**: End-to-end execution of a single pipeline that can fix clippy warnings in a
Rust project and open a PR.

### Crate Milestones

#### sakamoto-types
- [ ] Core error types (`SakamotoError` enum)
- [ ] Pipeline state phantom types (`Pending`, `Hydrated`, `Planned`, `Executed`,
      `Validated`, `Emitted`)
- [ ] `Pipeline<S>` struct with state transitions
- [ ] `StageOutput` algebra (`Continue`, `Retry`, `Fail`, `Fork`)
- [ ] `InteractionPolicy` enum
- [ ] `ContextBundle` type (accumulated context for LLM calls)
- [ ] `ContextRef` enum (file paths, GitHub issues, URLs, symbols)
- [ ] `Message` / `ToolCall` / `ToolResult` types for LLM conversation
- [ ] `ToolDef` type (tool name, description, JSON schema for parameters)

#### sakamoto-config
- [ ] `sakamoto.toml` schema definition
- [ ] Parse project config: LLM backends, toolsets, pipeline definitions, validation
      commands, output format
- [ ] Parse rule files (`.sakamoto/rules/*.md`) with subdirectory scoping
- [ ] Config validation and defaults
- [ ] Config merging (project + user-level `~/.config/sakamoto/config.toml`)

#### sakamoto-llm
- [ ] `LlmBackend` trait (`complete`, `stream`, `model_id`, `supports_tool_use`)
- [ ] Anthropic implementation (Messages API with tool use)
- [ ] OpenAI-compatible implementation (Chat Completions with function calling)
- [ ] Ollama implementation (local models)
- [ ] Streaming response support
- [ ] Token counting / cost estimation

#### sakamoto-tools
- [ ] `Tool` trait and `ToolRouter` for dispatching tool calls
- [ ] Built-in tools:
  - [ ] `git` — status, diff, add, commit, branch, push
  - [ ] `fs` — read, write, search (glob), grep
  - [ ] `shell` — execute commands with timeout and output capture
  - [ ] `lint` — run configurable lint command, parse output
  - [ ] `test` — run configurable test command, parse output
- [ ] MCP client integration via pmcp
  - [ ] Connect to MCP servers (stdio and HTTP transports)
  - [ ] Discover and register tools from MCP servers
  - [ ] Execute MCP tool calls and return results
- [ ] Toolset management (named subsets of tools per stage)

#### sakamoto-context
- [ ] `ContextParser` trait — extract `ContextRef`s from task text
  - [ ] File path parser (absolute and relative)
  - [ ] GitHub issue/PR URL parser
  - [ ] Generic URL parser
  - [ ] Symbol name parser (function/type references)
- [ ] `ContextFetcher` trait — resolve `ContextRef` to content
  - [ ] Filesystem fetcher (read files, directory listings)
  - [ ] GitHub fetcher (via `gh` CLI — issues, PRs, comments)
  - [ ] HTTP fetcher (generic URL content)
- [ ] `ContextEngine` — orchestrate parsing and fetching into a `ContextBundle`
- [ ] Pre-hydration: run fetchers before LLM loop starts

#### sakamoto-core
- [ ] `Stage` trait — unit of pipeline execution
- [ ] Built-in stages:
  - [ ] `ContextStage` — runs context pre-hydration
  - [ ] `PlanStage` — LLM generates an execution plan
  - [ ] `CodeStage` — ReAct loop with tool use
  - [ ] `LintStage` — deterministic lint check
  - [ ] `TestStage` — deterministic test execution
  - [ ] `CommitStage` — git add + commit
  - [ ] `PrStage` — create PR via `gh`
- [ ] `ReactLoop` — iterative LLM + tool-use execution engine
- [ ] DAG pipeline runner
  - [ ] Topological sort of stages
  - [ ] Parallel execution of independent stages (rayon + tokio)
  - [ ] Retry routing on validation failure (up to `max_ci_rounds`)
- [ ] Interaction policy enforcement per stage

#### sakamoto-executor
- [ ] `Executor` trait (`spawn`, `status`, `output`)
- [ ] `LocalExecutor` — run in current working directory

#### sakamoto-cli
- [ ] `sakamoto run "task description"` — execute a pipeline
- [ ] `sakamoto init` — generate `sakamoto.toml` in current project
- [ ] `sakamoto check` — validate config and tool connectivity
- [ ] Basic stdout logging with tracing

### Infrastructure
- [x] Cargo workspace with all crates
- [x] CI: GitHub Actions (fmt, clippy, test)
- [x] Git hooks (pre-commit: fmt, clippy, test)
- [x] Dependabot
- [x] Gitflow branching (main ← develop ← feature/*)
- [ ] Nix flake (dev shell + build)
- [ ] CLAUDE.md / .sakamoto/rules for agent-assisted development

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
  - [ ] Templated flake generation per project type
  - [ ] OCI image builds (following Yatima pattern)
  - [ ] Security hardening (read-only mounts, memory limits)
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
