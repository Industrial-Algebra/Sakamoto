# Unit R-0 — revival hygiene: Apache-2.0 + CLA, delete the façade, errata

**Unit:** make the repo honest before the ignition: MIT → Apache-2.0 + CLA
(operator decision, ia-licensing; unreleased = clean relicense), DELETE the
phantom `Pipeline<S>` façade (operator decision: "the algebra is what
counts"), fix the two memo errata. Seam 0 (build/clippy/test) is already
verified green on 1.97.1 — do NOT touch anything mechanical beyond this.
**Depends on:** nothing.
**Conventions:** deletion is imperative (the module GOES). Gates after.

## Preconditions

- Branch `feature/revival-r0-honesty` cut from `develop` by the verifying
  session. This plan committed on it.

## Files

- **Modify** `Cargo.toml` (root): `license = "MIT"` → `license = "Apache-2.0"`.
- **Create** `LICENSE` — the standard Apache License 2.0 text, verbatim
  (canonical text; do not paraphrase), copyright line
  `Copyright 2026 Industrial Algebra`.
- **Create** `CONTRIBUTING.md` — short form: contributions welcome via PR;
  by contributing you agree to the CLA; pointer to CLA.md.
- **Create** `CLA.md` — the IA Apache-2.0 Contributor License Agreement
  short form (individual contributor; copyright license + patent grant to
  Industrial Algebra; match the form used across IA repos — e.g. see
  Mingot/Knopper CONTRIBUTING if available locally; else the standard
  Apache CLA individual text adapted).
- **Modify** every `crates/*/src/**/*.rs`: prepend, if not present:

```rust
// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0
```

(script it; do not hand-edit 40 files; verify with
`grep -rL 'SPDX-License-Identifier' crates/*/src | wc -l` → 0)

- **Delete** `crates/sakamoto-types/src/pipeline.rs` entirely; remove
  `pub mod pipeline;` (+ any `pub use pipeline::…`) from
  `crates/sakamoto-types/src/lib.rs`; remove its tests (in-module `#[cfg(test)]`
  goes with the file; remove any `tests/` file that exists solely for it —
  check `crates/sakamoto-types/tests/`).
- **Modify** `README.md`: principle #3 currently claims phantom types
  enforce pipeline transitions — reword to the truth: **"Type-driven: the
  `StageOutput` algebra (Continue/Retry/Fail/Fork) is the load-bearing
  contract; pipeline progression is runner-enforced."** Remove any other
  `Pipeline<S>` references.
- **Modify** `DESIGN.md` (if it references the named transitions/"implemented
  on the concrete state types"): same correction — the phantom machine is
  deleted; the algebra and the DAG runner are the design.
- **Modify** `docs/handoff/2026-08-17-anima-conformance.md` (the merged
  conformance sketch): errata — `rmcp` → `pmcp`; Seam 0 note: "verified
  complete 2026-08-27 (223/223 green, zero clippy on 1.97.1)".
- **Modify** `ROADMAP.md`: check the boxes for `ContextFetcher`/
  `ContextEngine` (implemented in PR #8) if unchecked.

## Tests

No new tests. Existing suite must stay green MINUS the deleted phantom's
tests: expect `cargo test --workspace` to drop from 223 by exactly the
number of in-file pipeline tests (report the before/after count).

## Completion commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo fmt --check
grep -rL 'SPDX-License-Identifier' crates/*/src | wc -l   # must print 0
```

## Report

Files touched, test count before/after, any pipeline.rs references found
beyond the plan's list (disclose), CLA text source used.
