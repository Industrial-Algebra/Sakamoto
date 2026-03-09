//! Pipeline engine, DAG runner, and stage orchestration for Sakamoto.
//!
//! This crate defines the core execution model:
//! - [`stage::Stage`] trait for pipeline stages
//! - [`stage::LlmClient`] and [`stage::ToolExecutor`] abstractions
//! - [`react::ReactLoop`] for ReAct-style tool-use loops
//! - [`dag::PipelineDag`] for stage dependency ordering
//! - [`runner::PipelineRunner`] for end-to-end pipeline execution

pub mod dag;
pub mod react;
pub mod runner;
pub mod stage;
