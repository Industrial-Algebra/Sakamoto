//! LLM backend trait and provider implementations for Sakamoto.
//!
//! Each LLM provider implements the [`LlmBackend`] trait, providing
//! a uniform interface for conversation completion with tool use.

mod backend;
pub mod providers;

pub use backend::LlmBackend;
