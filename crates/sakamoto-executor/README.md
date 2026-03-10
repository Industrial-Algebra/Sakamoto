# sakamoto-executor

Execution environments and adapter layer for the Sakamoto orchestrator.

This crate bridges native implementations (`sakamoto-llm`, `sakamoto-tools`) into `sakamoto-core`'s abstract traits, and provides concrete executor implementations.

## LocalExecutor

Runs pipelines in the current working directory with no isolation:

```rust
use sakamoto_executor::local::LocalExecutor;

let mut executor = LocalExecutor::new(config, working_dir);
executor.add_llm_backend("default".into(), backend);
executor.add_tool_router("default".into(), router);

let result = executor.run_pipeline("default", "fix clippy warnings").await?;
println!("Stages: {:?}", result.stages_executed);
println!("Retries: {}", result.retries);
```

The executor:

1. Looks up the named pipeline in `ProjectConfig`
2. Builds a `PipelineDag` from the stage list
3. Constructs a `PipelineRunner` with registered stages, LLM clients, and tool executors
4. Applies stage overrides and validation commands from config
5. Runs the pipeline and returns a `RunResult`

## Adapters

- **`LlmAdapter`** — wraps `Arc<dyn LlmBackend>` into `sakamoto-core`'s `LlmClient` trait
- **`ToolAdapter`** — wraps `Arc<ToolRouter>` into `sakamoto-core`'s `ToolExecutor` trait

This adapter pattern keeps `sakamoto-core` Wasm-safe while allowing native crates to provide implementations.

## Built-in Stages

### ContextStage

Runs `sakamoto-context` parsers on the task description and populates the context bundle's `refs` field.

### CodeStage

Wraps a `ReactLoop` — sends the task description, plan, and any previous diagnostics to the LLM, then iterates tool calls until completion.

### CommandStage

Executes shell commands for validation and git operations:

| Factory | Retriable | Use case |
|---------|-----------|----------|
| `CommandStage::lint()` | Yes | Run lint command, retry on failure |
| `CommandStage::test()` | Yes | Run test command, retry on failure |
| `CommandStage::commit()` | No | Git commit, fail on error |
| `CommandStage::pr()` | No | Create PR, fail on error |

Retriable stages return `StageOutput::Retry` with diagnostic context on failure. Non-retriable stages return `StageOutput::Fail`.

## Executor Trait

```rust
#[async_trait]
pub trait Executor: Send + Sync {
    async fn run(&self, task: &str) -> Result<RunResult, SakamotoError>;
}
```

Planned executors (v0.2+): `WorktreeExecutor` (git worktree isolation), `NixContainerExecutor` (full OS isolation via Nix flakes).

## License

MIT
