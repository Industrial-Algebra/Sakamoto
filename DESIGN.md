# Sakamoto: Design Document

## Overview

Sakamoto is a **pipeline-oriented coding agent orchestrator** built in Rust. It enables
developers to define, compose, and execute multi-stage AI-driven coding workflows that
range from fully autonomous one-shot task completion to interactive human-in-the-loop
collaboration.

Inspired by [Stripe's Minions](https://stripe.dev/blog/minions-stripes-one-shot-end-to-end-coding-agents),
Sakamoto adapts the core ideas — one-shot agents, context pre-hydration, curated MCP
toolsets, shift-left validation — into an open, extensible, and composable tool suitable
for individual developers and teams.

## Design Principles

1. **Pipeline-first**: Workflows are directed acyclic graphs (DAGs) of typed stages, not
   free-form chat. Deterministic structure with LLM filling in the gaps.
2. **Composable**: Pipelines can contain sub-pipelines. Stages, tools, and backends are
   mix-and-match.
3. **Type-driven**: Phantom types enforce valid state transitions at compile time.
   Algebraic patterns throughout.
4. **MCP-native**: Tools are first-class via Model Context Protocol. Curated toolsets
   scope tool availability per stage.
5. **Backend-agnostic**: Different LLM providers per pipeline stage. Anthropic, OpenAI,
   Ollama, Gemma, or any OpenAI-compatible API.
6. **Configurable interactivity**: Each stage declares an interaction policy — autonomous,
   confirm, collaborate, or delegate.

## Architecture

```
┌────────────────────────────────────────────────────────┐
│                    sakamoto-cli                         │
│              (Ratatui TUI / CLI entry)                  │
├────────────────────────────────────────────────────────┤
│                    sakamoto-core                        │
│         (Pipeline DAG engine, ReAct loop)               │
├──────┬──────────┬───────────┬───────────┬─────────────┤
│config│  context  │    llm    │   tools   │  executor   │
│(toml)│(hydration)│(backends) │(MCP+built)│(local/nix)  │
├──────┴──────────┴───────────┴───────────┴─────────────┤
│                   sakamoto-types                        │
│       (Phantom states, core types, errors)              │
└────────────────────────────────────────────────────────┘

        ┌──────────────────────────┐
        │     sakamoto-gui         │
        │  (Leptos + Mingot nodes) │  ← Wasm target
        │  Uses: types, core       │
        └──────────────────────────┘
```

## Wasm Boundary

The crate workspace is split into two compilation tiers:

**Wasm-safe** (compile to `wasm32-unknown-unknown`, used by GUI):
- `sakamoto-types` — no system dependencies
- `sakamoto-core` — pipeline graph logic, stage definitions (behind feature gates for
  async runtime)
- `sakamoto-config` — TOML parsing

**Native-only** (system I/O, runs on host):
- `sakamoto-llm` — HTTP calls to LLM APIs (reqwest, tokio)
- `sakamoto-tools` — MCP client via pmcp (stdio/HTTP transports)
- `sakamoto-context` — filesystem, `gh` CLI, HTTP fetches
- `sakamoto-executor` — process spawning, Nix containers
- `sakamoto-cli` — Ratatui TUI

The GUI is a pipeline **composer** (define and visualize pipelines), not a pipeline
**runner**. It communicates with a native backend via WebSocket/HTTP for execution.

## Pipeline Model

### Phantom-Typed State Machine

```rust
// States
struct Pending;
struct Hydrated;
struct Planned;
struct Executed;
struct Validated;
struct Emitted;

struct Pipeline<S> {
    config: PipelineConfig,
    context: ContextBundle,
    _state: PhantomData<S>,
}

// Only valid transitions compile
impl Pipeline<Pending>   { fn hydrate(self, ...) -> Result<Pipeline<Hydrated>>; }
impl Pipeline<Hydrated>  { fn plan(self, ...)    -> Result<Pipeline<Planned>>;  }
impl Pipeline<Planned>   { fn execute(self, ...) -> Result<Pipeline<Executed>>; }
impl Pipeline<Executed>  { fn validate(self, ..) -> Result<Pipeline<Validated>>;}
impl Pipeline<Validated> { fn emit(self, ...)    -> Result<Pipeline<Emitted>>;  }
```

### DAG Execution

Pipelines are DAGs, not linear sequences. Stages that share no data dependencies run in
parallel via rayon (CPU-bound) or tokio (async I/O).

```
                    ┌─► lint ──────┐
context ─► plan ─► code ─►         ├─► commit ─► pr
                    └─► test ──────┘
```

Failed validation stages route back to the code stage, up to `max_ci_rounds` times.

### Stage Output Algebra

```rust
enum StageOutput {
    Continue(ContextBundle),
    Retry { reason: String },
    Fail(PipelineError),
    Fork(Vec<ContextBundle>),  // fan-out to parallel stages
}
```

## Interaction Policies

Each pipeline stage declares its interactivity level:

| Policy        | Behavior                                             |
|---------------|------------------------------------------------------|
| `Autonomous`  | No human input. Runs to completion.                  |
| `Confirm`     | Presents output, waits for approval before advancing.|
| `Collaborate` | Human can edit/augment the LLM's output.             |
| `Delegate`    | Hands off to human entirely, resumes on signal.      |

## ReAct Agent Loop

The coding stage uses a ReAct-style tool-use loop:

1. LLM receives context + task + available tools
2. LLM emits a tool call (or final answer)
3. Tool executes, result appended to messages
4. Repeat until final answer or max iterations

Tool calls respect the stage's interaction policy — in `Collaborate` mode, the human can
approve, edit, or skip each tool call.

## MCP Integration

Sakamoto acts as an **MCP client**, consuming tools from external MCP servers. Tools are
organized into named **toolsets**:

```toml
[toolsets.code-review]
mcp_servers = ["github", "sourcegraph"]
builtin = ["git", "fs"]

[toolsets.infra]
mcp_servers = ["terraform", "aws"]
builtin = ["shell"]
```

Each pipeline stage references a toolset by name. Built-in tools (git, filesystem, shell,
lint, test) are always available but can be restricted per toolset.

Sakamoto can also act as an **MCP server**, exposing its pipeline execution capabilities
to other tools and agents.

## Context Pre-Hydration

Before any LLM call, the context engine:

1. **Parses** the task description for references (file paths, GitHub issues/PRs, URLs,
   crate names, symbol names)
2. **Fetches** referenced content deterministically (filesystem reads, `gh` CLI, HTTP,
   tree-sitter symbol search)
3. **Bundles** results into a `ContextBundle` that seeds the first LLM call

This reduces token waste and dramatically improves first-shot quality.

## Execution Environments

| Executor              | Isolation  | Speed    | Use Case                        |
|-----------------------|------------|----------|---------------------------------|
| `LocalExecutor`       | None       | Instant  | Quick tasks, trusted code       |
| `WorktreeExecutor`    | Git-level  | Fast     | Parallel tasks, same machine    |
| `NixContainerExecutor`| Full OS    | ~10s     | Untrusted code, reproducible    |

The Nix executor follows the pattern established in the
[Yatima](https://github.com/Industrial-Algebra/yatima) project: generates or templates a
Nix flake, builds a layered OCI image, and runs the agent inside it with bind-mounted
volumes and security hardening.

**Container architecture**: The ReAct loop and LLM calls live in `sakamoto-core` on the
host. Containers are execution sandboxes for tool calls (shell commands, linters, tests),
not autonomous agents. The orchestrator drives the loop, sends tool invocations into the
container, and gets results back. This avoids requiring authentication inside containers
for the default (proxied) case.

## LLM Routing

LLM calls from containerized stages support three routing modes, determined by the
`LlmConfig` for that stage:

| Mode       | Auth in container? | Config signal               | Use case                     |
|------------|--------------------|-----------------------------|------------------------------|
| **Proxied**| No                 | No `api_key_env`            | Claude Max, session-based auth|
| **Direct** | API key injected   | `api_key_env` is set        | API billing, CI/production   |
| **Local**  | No                 | `base_url` points to host   | Ollama, llama.cpp            |

**Proxied mode** (default): The orchestrator exposes a local Unix socket or TCP listener.
The container's LLM calls route through this proxy, which forwards them using the host's
authenticated session (e.g., Claude Max). No credentials enter the container.

**Direct mode**: The executor injects the API key (from the named env var) into the
container environment. The container calls the LLM API directly. Used when per-token API
billing is acceptable or in CI where a service account key is available.

**Local mode**: The container hits a host-exposed inference endpoint (e.g., Ollama at
`host.containers:11434`). No authentication required.

```toml
# Proxied — orchestrator handles auth (Claude Max)
[llm.claude-max]
provider = "anthropic"
model = "claude-sonnet-4-6"
# no api_key_env → proxied through orchestrator

# Direct — API key injected into container
[llm.claude-api]
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_API_KEY"

# Local — no auth, host-exposed Ollama
[llm.local-qwen]
provider = "ollama"
model = "qwen2.5-coder:32b"
base_url = "http://host.containers:11434/v1"
```

This design ensures that subscription-based auth (Claude Max) works seamlessly with
containerized execution without requiring interactive login inside containers.

## Shift-Left Validation

Following Stripe's approach, validation shifts feedback as early as possible:

1. **Local linting** (< 5 seconds) — `cargo fmt --check`, `cargo clippy`
2. **Local tests** — `cargo test` (subset or full)
3. **CI** — full test suite, at most 2 rounds
4. **Autofix** — known lint/test failures with deterministic fixes are applied
   automatically

Failed validation returns to the ReAct loop with error context. After `max_ci_rounds`,
the pipeline emits whatever it has (partial work is still valuable as a starting point).

## Configuration

Project-level configuration in `sakamoto.toml`:

```toml
[project]
name = "my-app"

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

[pipeline.default]
stages = ["context", "plan", "code", "lint", "test", "commit", "pr"]
interaction = "autonomous"
max_ci_rounds = 2

[pipeline.default.stages.code]
interaction = "collaborate"
max_iterations = 20

[validation]
lint_command = "cargo clippy --workspace -- -D warnings"
fmt_command = "cargo fmt --all --check"
test_command = "cargo test --workspace"

[output]
default = "pr"  # pr | commit | patch | files

[rules]
paths = [".sakamoto/rules/*.md"]
```

Rules files (`.sakamoto/rules/`) follow the same convention as `.cursor/rules` and
`.claude/` — markdown files with instructions that are conditionally scoped to
subdirectories.

## GUI: Leptos + Mingot Node Graph

The browser-based GUI uses [Mingot](https://github.com/Industrial-Algebra/Mingot) Phase 7
Node Components to provide a visual pipeline composer:

| Mingot Component | Sakamoto Use                                |
|------------------|---------------------------------------------|
| `NodeCanvas`     | Pipeline editor workspace                   |
| `Node`           | Pipeline stage (context, plan, code, etc.)  |
| `NodePort`       | Stage inputs/outputs with typed connections |
| `NodeConnection` | Data flow (ContextBundle) between stages    |
| NumberInput      | Stage config (iterations, timeout, model)   |

The GUI compiles `sakamoto-types` and `sakamoto-core` to Wasm for pipeline definition and
validation. Execution is delegated to the native backend.

Mingot's precision-awareness enables visualization of token counts, cost projections, and
latency estimates flowing through the pipeline graph.

## Natural Language Interface

A local Gemma model (via Ollama) can power:
- Parsing natural language into pipeline configurations
- Generating `sakamoto.toml` snippets from descriptions
- Conversational mode in the Ratatui TUI

## Entry Points

| Interface    | Description                          | Priority |
|--------------|--------------------------------------|----------|
| CLI          | `sakamoto run "task description"`    | v0.1     |
| Stdin/pipe   | `echo "task" \| sakamoto run`       | v0.1     |
| GitHub Actions| Trigger on issue labels             | v0.2     |
| TUI          | Ratatui interactive dashboard        | v0.2     |
| Web GUI      | Leptos + Mingot pipeline composer    | v0.3     |
| Slack bot    | Invoke via Slack messages            | Future   |
