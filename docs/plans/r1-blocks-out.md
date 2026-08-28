# Unit R-1 — blocks-out: Sakamoto emits §2.7 blocks (the ignition Moment)

**Unit:** the revival's first real act — an adapter crate mapping
`StageOutput` to lonis-schema Blocks, plus one E2E demo writing a block
stream to file. **This unit is Sakamoto's revival executed by its own
pattern**: dispatched as a Mercury Moment, verified against its own tests,
landed behind a human merge.
**Depends on:** R-0 branch NOT required (independent); `lonis-schema 0.1`
from crates.io (published — verify with `cargo add`/lock).
**Conventions:** lonis-schema's exact Rust API is discovered from the
dependency itself (`cargo doc -p lonis-schema --open`-style reading of
`~/.cargo/registry/src/**/lonis-schema-0.1*/src`) — the CONTRACT below pins
SEMANTICS and observable JSON shape, not guessed type paths. Disclose the
API shapes you found. NO `#[must_use]`. Bounded awaits only.

## Context

Doctrine §2.7 blocks are the ecosystem's semantic stream; Sakamoto's
`StageOutput` algebra is its pipeline-native vocabulary. Mapping (v0,
deliberately simple — the conformance sketch's scope):

| StageOutput | Block kind | Content |
|---|---|---|
| `Continue(ctx)` | `Action` | "stage <name>: continue" |
| `Retry { reason, .. }` | `Action` | "stage <name>: retry — <reason>" |
| `Fail(e)` | `Result` | "stage <name>: failed — <e>" |
| `Fork(ctxs)` | `Plan` | "stage <name>: fork into N branches" (N = len) |

Attribution on every block: producer `sakamoto`, identity
`sakamoto:<pipeline-name>` — using lonis-schema's actual Attribution shape
(string fields per its published schema). Stage lifecycle *start* events
also map: `stage_started(name)` → `Intent` ("stage <name>: started").

## Preconditions

- Branch `feature/revival-r1-blocks` cut from `develop` by the verifying
  session. This plan committed on it.

## Files

- **Modify** root `Cargo.toml`: members += `"crates/sakamoto-blocks"`.
- **Create** `crates/sakamoto-blocks/Cargo.toml` — name `sakamoto-blocks`,
  workspace-inherited version/license/edition, deps: `sakamoto-types`
  (path), `lonis-schema = "0.1"`, `serde_json = "1"` (dev + runtime for the
  writer), `serde` if lonis-schema requires it for payload impls.
- **Create** `crates/sakamoto-blocks/src/lib.rs` — license header + docs +
  `pub mod adapter; pub mod writer;` + re-exports.
- **Create** `crates/sakamoto-blocks/src/adapter.rs` — the mapping:

```rust
// Behavioral contract:
// pub fn stage_started(name: &str) -> Block<P>          // Intent
// pub fn stage_output(pipeline: &str, stage: &str, out: &StageOutput) -> Vec<Block<P>>
//   Continue → 1 Action block; Retry → 1 Action; Fail → 1 Result; Fork → 1 Plan
// Every block: producer "sakamoto", identity "sakamoto:<pipeline>",
//   kind per the table above, content per the table.
// P = lonis-schema's simplest string payload (or its canonical payload
// type — use what the crate actually ships; disclose the choice).
```

- **Create** `crates/sakamoto-blocks/src/writer.rs` —

```rust
// pub struct BlockStreamWriter { /* wraps a std::io::Write, e.g. File */ }
// impl BlockStreamWriter {
//   pub fn create(path) -> io::Result<Self>
//   pub fn write(&mut self, block: &Block<P>) -> io::Result<()>  // one JSON line
// }
```

- **Create** `crates/sakamoto-blocks/tests/blocks_out.rs` — the tests + the
  E2E demo (below).

## Tests — all in `tests/blocks_out.rs` (value oracles; exact strings)

1. `continue_maps_to_action` — `stage_output("p1", "build",
   &Continue(ctx))` → exactly 1 block; serialized JSON: kind is `Action`
   (compare against lonis-schema's kind enum serialized form), content
   string contains `"stage build: continue"`, attribution producer ==
   `"sakamoto"`, identity == `"sakamoto:p1"`.
2. `retry_maps_to_action_with_reason` — Retry with reason `"gate red"` →
   1 Action block; content contains `"retry"` and `"gate red"`.
3. `fail_maps_to_result` — Fail(err) → 1 Result block; content contains
   `"failed"`.
4. `fork_maps_to_plan_with_count` — Fork of 3 contexts → 1 Plan block;
   content contains `"fork"` and `"3 branches"`.
5. `stage_started_maps_to_intent` — Intent block, content contains
   `"stage build: started"`.
6. `e2e_demo_writes_parseable_stream` — simulate a fictional 3-stage run
   (started/Continue for stage 1; started/Retry-then-Continue for stage 2;
   started/Continue for stage 3) → write via BlockStreamWriter to a
   pid-suffixed temp file → read back: every line parses as JSON; exactly 7
   lines; kinds in order: Intent, Action, Intent, Action, Action, Intent,
   Action; every line's producer == `"sakamoto"`.
7. `round_trips_through_lonis_schema` — for one block of each produced
   kind: serialize via lonis-schema's serde impl, deserialize back, assert
   equality (uses the crate's real round-trip surface — proves we're using
   its types correctly, not fabricating JSON).

## Completion commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo fmt --check
```

## Report

Files touched, tests added (expect 7), the lonis-schema API shapes you
found and used (payload type, Block construction, kind enum name,
Attribution fields), any mapping disclosures.
