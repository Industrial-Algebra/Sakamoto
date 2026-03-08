//! Core types, phantom states, and algebraic patterns for Sakamoto.
//!
//! This crate defines the foundational types shared across all Sakamoto
//! crates. It is Wasm-safe: no filesystem, network, or process dependencies.
//!
//! # Modules
//!
//! - [`error`] — Error types and diagnostics.
//! - [`context`] — Context bundle and reference types for pre-hydration.
//! - [`llm`] — LLM conversation types (messages, tool calls, tool definitions).
//! - [`stage`] — Stage output algebra and interaction policies.
//! - [`pipeline`] — Phantom-typed pipeline state machine.

pub mod context;
pub mod error;
pub mod llm;
pub mod pipeline;
pub mod stage;

// Re-export the most-used types at crate root for ergonomics.
pub use context::{ContextBundle, ContextEntry, ContextRef};
pub use error::{Diagnostic, Result, SakamotoError, Severity};
pub use llm::{LlmResponse, Message, ModelInfo, Role, TokenUsage, ToolCall, ToolDef, ToolResult};
pub use pipeline::{
    Emitted, Executed, Hydrated, Pending, Pipeline, PipelineOutput, PipelineResult, PipelineState,
    Planned, Validated,
};
pub use stage::{InteractionPolicy, StageConfig, StageKind, StageOutput};
