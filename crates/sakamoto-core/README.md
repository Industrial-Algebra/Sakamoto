# sakamoto-core

Pipeline execution engine for the Sakamoto orchestrator.

This crate provides the DAG runner, ReAct loop, and trait abstractions that drive pipeline execution. It depends only on `sakamoto-types` and defines trait boundaries (`LlmClient`, `ToolExecutor`, `Stage`) that native crates implement.

**Wasm-safe**: compiles to `wasm32-unknown-unknown` with no system dependencies.

## Stage Trait

Every pipeline stage implements `Stage`:

```rust
#[async_trait]
pub trait Stage: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, context: ContextBundle, ctx: &StageContext) -> StageOutput;
}
```

`StageContext` provides optional access to an `LlmClient` and `ToolExecutor`, plus per-stage configuration.

## Abstraction Traits

`LlmClient` and `ToolExecutor` are defined here (not in native crates) so that `sakamoto-core` stays Wasm-safe:

- **`LlmClient`** — `async fn complete(messages, tools, system) -> Result<(LlmResponse, TokenUsage)>`
- **`ToolExecutor`** — `fn available_tools()` + `async fn execute_tool(name, input) -> Result<String>`

Native crates provide adapter implementations (see `sakamoto-executor`).

## ReAct Loop

`ReactLoop` drives iterative LLM tool-use cycles:

1. Send messages + available tools to LLM
2. If LLM returns tool calls, execute them and append results
3. Repeat until LLM returns a final text response or max iterations reached

```rust
let react = ReactLoop::new(10)
    .with_system_prompt("You are a coding agent.");
let result = react.run(messages, &llm, &tools).await?;
```

## Pipeline DAG

`PipelineDag` represents stage dependencies as a directed acyclic graph:

- `from_linear(stages)` — convenience constructor for sequential pipelines
- `add_stage(name)` / `add_dependency(from, to)` — build arbitrary DAGs
- `execution_levels()` — Kahn's algorithm topological sort returning parallel groups
- Cycle detection with error reporting

## Pipeline Runner

`PipelineRunner` executes a DAG with configurable retry logic:

- Runs stages level-by-level through the DAG
- On `StageOutput::Retry`, jumps back to the configured retry point (typically `"code"`)
- Tracks token usage, executed stages, and retry count in `RunResult`
- Respects `max_retries` from `RunnerConfig`

## License

MIT
