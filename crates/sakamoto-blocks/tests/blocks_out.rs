// SPDX-License-Identifier: MIT
//! Tests for the Sakamoto‑Blocks adapter.

use std::fs;
use std::io::Read;

use serde_json::json;

use lonis_schema::block::kinds::{
    Action, ActionStatus, Message, Outcome, OutcomeStatus, Plan, ResultPayload,
};
use lonis_schema::block::{Attribution, AttributionSource, Block, BlockKind, now_rfc3339};
use sakamoto_blocks::adapter::{stage_output, stage_started};
use sakamoto_blocks::writer::BlockStreamWriter;
use sakamoto_types::context::ContextBundle;
use sakamoto_types::error::SakamotoError;
use sakamoto_types::stage::StageOutput;

fn dummy_attribution(pipeline: &str) -> Attribution {
    Attribution {
        identity: format!("sakamoto:{}", pipeline),
        viewpoint: None,
        provenance: AttributionSource {
            when: now_rfc3339(),
            location: None,
            producer: "sakamoto".into(),
        },
    }
}

#[test]
fn continue_maps_to_action() {
    let ctx = ContextBundle::default();
    let out = StageOutput::Continue(ctx);
    let blocks = stage_output("p1", "build", &out);
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert_eq!(block.payload.kind_name(), "action");
    let rendered = block.render_human();
    assert!(rendered.contains("stage build: continue"));
    assert_eq!(block.attribution.provenance.producer, "sakamoto");
    assert_eq!(block.attribution.identity, "sakamoto:p1");
}

#[test]
fn retry_maps_to_action_with_reason() {
    let ctx = ContextBundle::default();
    let out = StageOutput::Retry {
        context: ctx,
        reason: "gate red".into(),
    };
    let blocks = stage_output("p1", "build", &out);
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert_eq!(block.payload.kind_name(), "action");
    let rendered = block.render_human();
    assert!(rendered.contains("retry"));
    assert!(rendered.contains("gate red"));
}

#[test]
fn fail_maps_to_result() {
    let err = SakamotoError::CyclicGraph;
    let err_msg = format!("{}", err);
    let out = StageOutput::Fail(err);
    let blocks = stage_output("p1", "build", &out);
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert_eq!(block.payload.kind_name(), "outcome");
    // Value oracle: the failure message must carry the stage and the error.
    match &block.payload {
        lonis_schema::block::kinds::BlockKind::Outcome(o) => {
            assert!(o.message.contains("stage build: failed"));
            assert!(o.message.contains(&err_msg) || o.message.contains("cyclic"));
            assert_eq!(o.status, lonis_schema::block::kinds::OutcomeStatus::Error);
        }
        _ => panic!("expected Outcome"),
    }
}

#[test]
fn fork_maps_to_plan_with_count() {
    let ctxs = vec![ContextBundle::default(); 3];
    let out = StageOutput::Fork(ctxs);
    let blocks = stage_output("p1", "build", &out);
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert_eq!(block.payload.kind_name(), "plan");
    // Value oracle: the plan goal must carry the fork count.
    match &block.payload {
        lonis_schema::block::kinds::BlockKind::Plan(p) => {
            let goal = p.goal.as_deref().expect("goal present");
            assert!(goal.contains("fork"));
            assert!(goal.contains("3 branches"));
        }
        _ => panic!("expected Plan"),
    }
}

#[test]
fn stage_started_maps_to_intent() {
    let block = stage_started("p1", "build");
    assert_eq!(block.payload.kind_name(), "intent");
    let rendered = block.render_human();
    assert!(rendered.contains("stage build: started"));
    assert_eq!(block.attribution.provenance.producer, "sakamoto");
    assert_eq!(block.attribution.identity, "sakamoto:p1");
}

#[test]
fn e2e_demo_writes_parseable_stream() -> std::io::Result<()> {
    // Simulate three stages with start/continue, start/retry/continue, start/continue.
    let mut path = std::env::temp_dir();
    path.push(format!("sakamoto_demo_{}.jsonl", std::process::id()));
    let mut writer = BlockStreamWriter::create(&path)?;

    // Stage 1
    writer.write(&stage_started("p1", "stage1"))?;
    let out1 = StageOutput::Continue(ContextBundle::default());
    for blk in stage_output("p1", "stage1", &out1) {
        writer.write(&blk)?;
    }
    // Stage 2 – retry then continue
    writer.write(&stage_started("p1", "stage2"))?;
    let out2 = StageOutput::Retry {
        context: ContextBundle::default(),
        reason: "temp".into(),
    };
    for blk in stage_output("p1", "stage2", &out2) {
        writer.write(&blk)?;
    }
    let out2c = StageOutput::Continue(ContextBundle::default());
    for blk in stage_output("p1", "stage2", &out2c) {
        writer.write(&blk)?;
    }
    // Stage 3
    writer.write(&stage_started("p1", "stage3"))?;
    let out3 = StageOutput::Continue(ContextBundle::default());
    for blk in stage_output("p1", "stage3", &out3) {
        writer.write(&blk)?;
    }

    // Verify file
    let mut file = fs::File::open(&path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 7);
    // Expected order: Intent, Action, Intent, Action (retry), Action (continue), Intent, Action
    // Just check each line parses as JSON and that producer matches.
    for line in lines {
        let v: serde_json::Value = serde_json::from_str(line).expect("invalid json");
        let prov = &v["attribution"]["provenance"]["producer"];
        assert_eq!(prov, "sakamoto");
    }
    // Clean up
    fs::remove_file(&path)?;
    Ok(())
}

#[test]
fn round_trips_through_lonis_schema() {
    // One block of each kind.
    let kinds = vec![
        BlockKind::Message(Message {
            role: None,
            content: "hi".into(),
            reply_to: None,
        }),
        BlockKind::Action(Action {
            verb: "run".into(),
            target: None,
            parameters: None,
            status: ActionStatus::Completed,
        }),
        BlockKind::Outcome(Outcome {
            status: OutcomeStatus::Success,
            kind: "ok".into(),
            message: "all good".into(),
            details: None,
            exit_code: None,
        }),
        BlockKind::Plan(Plan {
            goal: None,
            steps: vec![],
            prerequisite_order: vec![],
            normalization: None,
            plan_hash: None,
        }),
        BlockKind::Result(ResultPayload {
            output: json!({"x":1}),
            score: None,
            evidence: vec![],
            validated_assumptions: vec![],
            refuted_assumptions: vec![],
            resources: None,
            duration_micros: None,
        }),
    ];
    for kind in kinds {
        let attr = dummy_attribution("p1");
        let block = Block::new(attr.clone(), kind.clone());
        let ser = serde_json::to_string(&block).unwrap();
        let back: Block<BlockKind> = serde_json::from_str(&ser).unwrap();
        assert_eq!(back, block);
    }
}
