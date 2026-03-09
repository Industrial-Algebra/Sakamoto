# sakamoto-tools

Tool trait, router, and built-in tools for the Sakamoto orchestrator.

This crate provides the `Tool` abstraction, a `ToolRouter` for dispatching tool calls by name, and built-in tool implementations.

## Tool Trait

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDef;
    async fn execute(&self, input: serde_json::Value) -> Result<String, SakamotoError>;
}
```

## ToolRouter

Registry that maps tool names to implementations:

```rust
use sakamoto_tools::router::ToolRouter;

let mut router = ToolRouter::new();
router.register(Arc::new(ShellTool::new(".", Duration::from_secs(30))))?;

let defs = router.definitions();          // Vec<ToolDef> for LLM
let result = router.call("shell", input).await?;
```

Duplicate tool names are rejected at registration time.

## Built-in Tools

### Shell

Executes shell commands in a subprocess with configurable working directory and timeout.

```rust
use sakamoto_tools::builtin::shell::ShellTool;

let tool = ShellTool::new("/path/to/project", Duration::from_secs(30));
```

Accepts JSON input: `{"command": "cargo test"}`. Returns stdout on success, error with stderr on failure. Commands that exceed the timeout are killed.

### Planned (v0.2)

- `git` — status, diff, add, commit, branch, push
- `fs` — read, write, search (glob), grep
- `lint` — run lint command, parse structured output
- `test` — run test command, parse structured output
- MCP server integration via pmcp

## License

MIT
