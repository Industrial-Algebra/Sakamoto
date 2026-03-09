# sakamoto-cli

Command-line interface for the Sakamoto pipeline orchestrator.

## Commands

### `sakamoto run`

Execute a pipeline with a task description:

```bash
sakamoto run "fix clippy warnings"
sakamoto run -p with_lint "add tests for auth module"
```

Loads `sakamoto.toml` from the current directory, builds LLM backends and tool routers from config, creates a `LocalExecutor`, and runs the named pipeline.

### `sakamoto init`

Generate a starter `sakamoto.toml` in the current directory:

```bash
sakamoto init
```

### `sakamoto check`

Validate configuration and report status:

```bash
sakamoto check
```

Reports:
- Project name
- LLM backends with API key status (set/missing/not needed)
- Registered toolsets
- Pipeline definitions with stage lists
- Validation commands
- User config location

## Configuration

The CLI loads config from two sources:

1. `sakamoto.toml` in the current directory (required for `run` and `check`)
2. `~/.config/sakamoto/config.toml` (optional user-level defaults)

Project config takes precedence. User config fills in gaps.

## Logging

Set the `RUST_LOG` environment variable to control log output:

```bash
RUST_LOG=info sakamoto run "task"
RUST_LOG=debug sakamoto run "task"
```

## License

MIT
