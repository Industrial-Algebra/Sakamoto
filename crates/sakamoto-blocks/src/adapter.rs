// SPDX-License-Identifier: MIT
//! Adapter from `sakamoto-types::StageOutput` to `lonis-schema` blocks.
//!
//! This module implements the mapping described in `docs/plans/r1-blocks-out.md`.
//!
//! * `stage_started(name, pipeline)` → an `Intent` block.
//! * `stage_output(pipeline, stage, out)` → a `Block` whose payload depends on the
//!   `StageOutput` variant:
//!   * `Continue` → `Action` with content "stage <stage>: continue".
//!   * `Retry { reason, .. }` → `Action` with content "stage <stage>: retry — <reason>".
//!   * `Fail(e)` → `Outcome` with content "stage <stage>: failed — <e>".
//!   * `Fork(ctxs)` → `Plan` with content "stage <stage>: fork into N branches" (N = len).
//!
//! Every block is attributed to producer `"sakamoto"` and identity
//! `"sakamoto:<pipeline>"`.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use serde_json;

use lonis_schema::block::kinds::{Action, ActionStatus, Intent, Outcome, OutcomeStatus, Plan};
use lonis_schema::block::{Attribution, AttributionSource, Block, BlockKind, now_rfc3339};

use sakamoto_types::stage::StageOutput;

/// Create an `Intent` block for a stage start event.
pub fn stage_started(pipeline: &str, name: &str) -> Block<BlockKind> {
    let attribution = Attribution {
        identity: format!("sakamoto:{}", pipeline),
        viewpoint: None,
        provenance: AttributionSource {
            when: now_rfc3339(),
            location: None,
            producer: "sakamoto".into(),
        },
    };
    let intent = Intent {
        statement: format!("stage {}: started", name),
        constraints: vec![],
    };
    Block::new(attribution, BlockKind::Intent(intent))
}

/// Map a `StageOutput` into one or more `lonis-schema` blocks.
pub fn stage_output(pipeline: &str, stage: &str, out: &StageOutput) -> Vec<Block<BlockKind>> {
    let attribution = Attribution {
        identity: format!("sakamoto:{}", pipeline),
        viewpoint: None,
        provenance: AttributionSource {
            when: now_rfc3339(),
            location: None,
            producer: "sakamoto".into(),
        },
    };
    match out {
        StageOutput::Continue(_) => {
            let action = Action {
                verb: format!("stage {}: continue", stage),
                target: None,
                parameters: None,
                status: ActionStatus::Completed,
            };
            vec![Block::new(attribution, BlockKind::Action(action))]
        }
        StageOutput::Retry { reason, .. } => {
            let action = Action {
                verb: format!("stage {}: retry — {}", stage, reason),
                target: None,
                parameters: None,
                status: ActionStatus::Running,
            };
            vec![Block::new(attribution, BlockKind::Action(action))]
        }
        StageOutput::Fail(e) => {
            let outcome = Outcome {
                status: OutcomeStatus::Error,
                kind: "failure".into(),
                message: format!("stage {}: failed — {}", stage, e),
                details: None,
                exit_code: None,
            };
            vec![Block::new(attribution, BlockKind::Outcome(outcome))]
        }
        StageOutput::Fork(ctxs) => {
            let plan = Plan {
                goal: Some(format!(
                    "stage {}: fork into {} branches",
                    stage,
                    ctxs.len()
                )),
                steps: vec![],
                prerequisite_order: vec![],
                normalization: None,
                plan_hash: None,
            };
            vec![Block::new(attribution, BlockKind::Plan(plan))]
        }
    }
}

/// Simple writer that appends one JSON‑encoded block per line.
pub struct BlockStreamWriter {
    inner: File,
}

impl BlockStreamWriter {
    /// Open a file for writing (creates it if missing).
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let f = File::create(path)?;
        Ok(Self { inner: f })
    }

    /// Serialize a block as a JSON line.
    pub fn write(&mut self, block: &Block<BlockKind>) -> io::Result<()> {
        let line = serde_json::to_string(block).map_err(std::io::Error::other)?;
        self.inner.write_all(line.as_bytes())?;
        self.inner.write_all(b"\n")?;
        Ok(())
    }
}
