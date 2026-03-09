//! Pluggable execution environments for Sakamoto.
//!
//! This crate provides the [`Executor`] trait and concrete implementations
//! that wire together the core pipeline runner with LLM backends, tool
//! routers, and built-in stages.

pub mod adapters;
pub mod executor;
pub mod local;
pub mod stages;
