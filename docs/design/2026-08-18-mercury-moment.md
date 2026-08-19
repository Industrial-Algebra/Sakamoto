# The Mercury Moment — Sakamoto Stage Definition

**Status:** Draft v0 (2026-08-18). Evidence: the Tsume batch (5 cycles).
**Workflow skill:** `ia-toolkit/skills/ia-mercury-dispatch/SKILL.md` (the
canonical loop description — this doc defines the *Sakamoto-shaped* version).

## What this defines

A **Moment** is Sakamoto's atomic unit of delegated implementation: one
plan contract, dispatched to a fast non-interactive model, verified
against its own written tests, landed behind a human merge. It is the
Stripe-Minions "one-shot agent" idea with the fidelity problem solved the
IA way — not by a smarter generator, but by a **contract strong enough
that faithful implementation is correct implementation**, and a
**deterministic gate** behind it.

## The Moment pipeline

```
        ┌── TRIGGER (today: operator; target: a Pulse recommendation,
        │             an issue, a handoff doc with a Files+Contracts shape)
        ▼
   [1] AUTHOR    session drafts the plan contract
                 (verbatim signatures, enumerated assertions, fences)
        ▼
   [2] DISPATCH  pi -p --provider inception --model mercury-2 --no-session
                 implements the plan (~30s–2min observed)
        ▼
   [3] VERIFY    session runs the gate matrix, audits every test for
                 vacuity, refactors; commits to the plan's branch
        ▼
   [4] REVIEW    operator merges the PR (agents never self-merge)
        ▼
        └── receipts: files + tests + gate results + defect ledger entry
```

## Evidence (Tsume batch, 2026-08-18)

| Cycle | Unit | Impl time | Verify catch |
|---|---|---|---|
| 00 | envelope + Gateway trait | ~2 min | implementer: `async fn` in trait (Send dropped) |
| 01 | attribution | 47 s | cosmetics only |
| 02 | conversation registry | 48 s | author: ambiguous spec → vacuous concurrency test |
| 03 | emission seam | 32 s | author: unconditional re-export of gated type |
| 04 | session host + loop | 52 s | author: spec'd assertions on unobservable data |

Five units, a crate foundation (22 tests, full gate matrix green) in
**~30 minutes of real time**. Every defect caught pre-merge. The load-
bearing property is **not** generation quality — it is that the verify
gate caught every defect at bounded cost (~10–15 min/cycle including
authoring).

## Why this fits Sakamoto specifically

The doctrine (conformance handoff §"core already doctrine-shaped") notes
Sakamoto's `StageOutput` algebra — `Continue`/`Retry`/`Fail`/`Fork`. The
Moment maps directly:

- **Continue** — verify pass green → PR.
- **Retry** — gate red → re-dispatch with the failure appended to the
  prompt (bounded, e.g. max 2 retries before Fail).
- **Fail** — retries exhausted or verify finds contract-level defects →
  surface to the author session, not another roll.
- **Fork** — (future) parallel Moments over independent plan files.

A Moment is therefore a Sakamoto **stage** whose executor is a diffusion
model and whose validation is the gate matrix — `shift-left validation`
with the validation authored *before* the code exists, which is the whole
trick.

## Automation target (the ambition)

> Every Anima PULSE recommendation dispatches a Moment automatically.

Prerequisites, in order:

1. **Trigger adapter** — Pulse recommendations carry (repo, unit,
   acceptance) triples; a mapper drafts or selects the plan contract.
2. **Repo-state guard** — dispatch only into a clean worktree on the
   right base branch (the 2026-08-18 IA-documents incident is the
   cautionary tale: automation committed onto the wrong checked-out
   branch).
3. **Bounded Retry** — the `Retry` edge above.
4. **Human merge** — stays. The Moment automates *implementation and
   verification*, never review.

## What a Moment is NOT

- Not autonomy: no plan-writing by the executor, no self-merge, no
  unreviewed generated code on any branch.
- Not a claim about Mercury: any sufficiently instruction-following
  batch model can sit in the DISPATCH slot (the skill's smoke test
  validates a new one in minutes).
- Not a replacement for the author session: the plan IS the design work;
  the frontier of defect pressure sits there, as the ledger shows.
