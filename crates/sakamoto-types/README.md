# sakamoto-types

Foundational types for the Sakamoto pipeline orchestrator.

This crate defines the core data model shared across all Sakamoto crates: phantom-typed pipeline states, the stage output algebra, LLM conversation primitives, context references, and error types.

**Wasm-safe**: compiles to `wasm32-unknown-unknown` with no system dependencies.

## Phantom-Typed Pipeline

The `Pipeline<S>` struct uses phantom types to enforce valid state transitions at compile time:

```
Pending → Hydrated → Planned → Executed → Validated → Emitted
```

Only valid transitions compile — you cannot execute a pipeline that hasn't been planned.

## Stage Output Algebra

`StageOutput` models every possible stage result:

| Variant | Meaning |
|---------|---------|
| `Continue(ContextBundle)` | Pass context to the next stage |
| `Retry { context, reason }` | Loop back with diagnostic feedback |
| `Fail(SakamotoError)` | Halt the pipeline |
| `Fork(Vec<ContextBundle>)` | Fan out to parallel sub-pipelines |

`StageOutput` supports `map` and `and_then` for functional composition.

## Key Types

- **`ContextBundle`** — accumulated context passed between stages (task description, resolved references, diagnostics, plan, metadata)
- **`ContextRef`** — references extracted from task text (file paths, GitHub issues/PRs, URLs, symbols)
- **`Message`**, **`ToolCall`**, **`ToolResult`**, **`ToolDef`** — LLM conversation primitives
- **`LlmResponse`** — either tool calls to execute or a final text response
- **`TokenUsage`** — input/output token counts with optional cost
- **`StageConfig`** — per-stage settings (LLM backend, toolset, interaction policy, max iterations)
- **`InteractionPolicy`** — `Autonomous`, `Confirm`, `Collaborate`, `Delegate`
- **`SakamotoError`** — top-level error enum using `thiserror`
- **`Diagnostic`** — structured diagnostic messages with severity levels

## License

MIT
