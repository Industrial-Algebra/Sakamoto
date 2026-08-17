# Sakamoto ↔ Anima Doctrine Conformance — Handoff Sketch

**Date:** 2026-08-17
**For:** the Sakamoto revival session.
**Sources:** `ANIMA_ECOSYSTEM_DOCTRINE.md` (IA-documents, Aug 12 state-ownership
precision), `PULSE_2026-08-17_Anima.md`, Sakamoto DESIGN.md/ROADMAP.md as of
`3a87652` (frozen 158 days).

## Where Sakamoto sits in the doctrine

Sakamoto is one of the three IA harnesses (**Wallace, Tsume, Sakamoto**) — planes that
*execute work*. Per the doctrine (and §8.E's candidate resolution):

- A harness **owns its session/workspace state intrinsically** (Sakamoto: pipeline
  runs, stage outputs, validation results, rule files) — and keeps **no private copy
  of ecosystem durable memory** (that is Ijima's job, optional to consume).
- Sakamoto's niche is distinct and worth protecting: **autonomous pipeline
  orchestration** (Minions-style one-shot runs, curated toolsets, shift-left
  validation) vs. Wallace (interactive collaborative workspace) vs. Tsume (gateway).
  Doctrine conformance does **not** mean becoming Wallace.

The good news: Sakamoto's core model is already doctrine-*shaped* — typed stages,
phantom state machines, the `StageOutput` algebra (`Continue`/`Retry`/`Fail`/`Fork`),
deterministic DAGs with LLM filling gaps. **The gaps are all at the boundaries, not
in the core.**

## Current state (verified 2026-08-17)

- 9 crates; v0.1.0 foundation substantially complete per ROADMAP (types, config,
  llm backends, tools, executor, context all checked); last commits: MCP client (#9),
  rule-file parsing (#10), 2026-03-12.
- **Zero Anima-internal dependencies** — predates the doctrine, fully standalone.
- UI: Ratatui TUI (cli) + planned Leptos/Mingot wasm GUI.
- Frozen 158 days → revival hygiene needed before any conformance work (below).

## Conformance seams (in recommended order)

### 0. Revival hygiene (do first, unblocks everything)

- Toolchain drift: stable rustc rolled forward ~5 months since March; expect new
  clippy lints (Dominic and Ijima both hit `manual_noop_waker`/`needless_borrow`
  class changes in August). Run the full gate: `cargo fmt`, `clippy --all-features
  --all-targets -D warnings`, `test --all-features`, `cargo doc`.
- Dependency refresh (rmcp/MCP SDK, ratatui, provider SDKs) — bit-rot risk was
  flagged in two consecutive Anima pulses.
- CI state unknown after 158 days — verify workflows still green.

### 1. Blocks out (§2.7) — cheapest, highest leverage

Stage results and pipeline events should be emitted as **§2.7 blocks**
(`lonis-schema`'s `Block = { envelope, attribution, bounds, payload }`; the 14 seed
kinds — a stage result is a `result` block, a failure an `outcome`/`error` block, a
plan a `plan` block). This single seam makes Sakamoto **legible to the ecosystem**:

- Ijima can mine Sakamoto runs (the miner cannot operate on opaque strings — §8.C).
- Wallace can render Sakamoto run projections (block stream → transcript pane).
- Replay/audit comes free (§2.7 replayability: content hashes).

Minimal version: an adapter crate/module (`sakamoto-blocks`?) mapping
`StageOutput`/`PipelineEvent` → blocks. `lonis-schema` 0.1.0 is staged for release —
depend on it once tagged.

### 2. Capabilities via Schubert

Today agents authenticate with raw `api_key_env`. Conformance: agents act under
**Schubert `GrantToken`s** (0.4 on crates.io — multi-cap bearer, `GrantVerifier::may`
geometric containment; Dominic M3 is the reference consumer). Grant lifecycle
(expiry/revocation) is Schubert roadmap #20 / v0.5 — design the seam now, don't block
on it.

### 3. Tools via Lonis

MCP-native today (`[mcp_server.*]`, curated toolsets). Doctrine: **Lonis owns tool
exposure** — "Sakamoto declares, Lonis governs" (same statement Wallace 05 makes).
Pragmatic path: keep the MCP client short-term (Lonis is MCP-adjacent, not
MCP-hostile; a Lonis↔MCP bridge may exist), long-term offer Lonis providers as
first-class tool sources whose results arrive as blocks natively (seam 1 makes this
natural).

### 4. Dispatch via Dominic (when the plane is ready)

Cross-plane work ("run this pipeline on that repo, report to memory") becomes a
Dominic `DispatchRequest` — Sakamoto as a **dispatch origin**, results flowing back as
`DispatchOutcome`. Dominic M4 (pi dispatcher) is in flight now; Sakamoto integration
is REQUIREMENTS-listed after Tsume (§5 #6). Do not block on this.

### 5. Memory via Ijima (optional, per §8.E)

Promote run artifacts (stage outputs, PRs, validation reports) to Ijima ecosystem
memory for cross-session/cross-harness recall. Optional by doctrine — Sakamoto must
work fully standalone. The block↔Ijima promotion boundary (which of the 14 kinds are
memory-promotable) is an open doctrine question (PULSE 08-17 rec #6) — align with
whatever it resolves to.

### 6. Identity (§8.B)

Participants/agents get canonical `ParticipantId`s from Dominic's identity registry,
with Sakamoto-local view types embedding them. Low urgency; lands with seam 4.

## Interface-plane decision (flag for the operator, not settled here)

The doctrine says **Wallace owns the interface**; Knopper is its substrate. Long-term,
Sakamoto runs render as Wallace projections (block stream → Wallace transcript — seam
1 is the prerequisite). Short-term, the standalone Ratatui TUI is pragmatic and fine.
`sakamoto-gui` (Leptos/Mingot wasm) is the odd one out — revisit whether it survives
revival at all.

## Suggested first revival milestone

**"Sakamoto emits blocks"** — hygiene (seam 0) + seam 1 + one E2E demo: run a
pipeline, emit its event log as a §2.7 block stream, write it to a file (and
optionally to a local Ijima). That single milestone makes Sakamoto an Anima citizen
without touching its core model.
