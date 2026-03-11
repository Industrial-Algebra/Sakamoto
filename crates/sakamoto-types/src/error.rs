//! Error types for Sakamoto.
//!
//! Library crates use [`SakamotoError`] via `thiserror`.
//! Only `sakamoto-cli` should use `anyhow`.

use std::fmt;

/// Top-level error type for all Sakamoto operations.
#[derive(Debug, thiserror::Error)]
pub enum SakamotoError {
    /// A pipeline stage failed.
    #[error("stage `{stage}` failed: {reason}")]
    StageFailure { stage: String, reason: String },

    /// A pipeline stage exceeded its maximum iteration count.
    #[error("stage `{stage}` exceeded max iterations ({max})")]
    MaxIterationsExceeded { stage: String, max: usize },

    /// A pipeline stage exceeded its timeout.
    #[error("stage `{stage}` timed out after {elapsed:?}")]
    Timeout {
        stage: String,
        elapsed: std::time::Duration,
    },

    /// Validation (lint/test) failed after all retry rounds.
    #[error("validation failed after {rounds} round(s): {reason}")]
    ValidationExhausted { rounds: usize, reason: String },

    /// The pipeline graph contains a cycle.
    #[error("pipeline graph contains a cycle")]
    CyclicGraph,

    /// A referenced stage was not found in the pipeline graph.
    #[error("stage `{0}` not found in pipeline")]
    StageNotFound(String),

    /// A referenced toolset was not found.
    #[error("toolset `{0}` not found")]
    ToolsetNotFound(String),

    /// An MCP tool call failed.
    #[error("tool `{tool}` failed: {reason}")]
    ToolCallFailed { tool: String, reason: String },

    /// An MCP server connection or protocol error.
    #[error("MCP error ({server}): {reason}")]
    McpError { server: String, reason: String },

    /// An LLM backend returned an error.
    #[error("LLM error ({backend}): {reason}")]
    LlmError { backend: String, reason: String },

    /// Configuration is invalid.
    #[error("config error: {0}")]
    ConfigError(String),

    /// Context fetching failed.
    #[error("context error: {0}")]
    ContextError(String),

    /// I/O error (filesystem, network, process).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias used throughout Sakamoto library crates.
pub type Result<T> = std::result::Result<T, SakamotoError>;

/// Severity level for diagnostic messages within a pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A diagnostic message emitted during pipeline execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub stage: String,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.stage, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_failure_displays_correctly() {
        let err = SakamotoError::StageFailure {
            stage: "code".into(),
            reason: "LLM returned empty response".into(),
        };
        assert_eq!(
            err.to_string(),
            "stage `code` failed: LLM returned empty response"
        );
    }

    #[test]
    fn max_iterations_displays_correctly() {
        let err = SakamotoError::MaxIterationsExceeded {
            stage: "code".into(),
            max: 20,
        };
        assert_eq!(err.to_string(), "stage `code` exceeded max iterations (20)");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: SakamotoError = io_err.into();
        assert!(matches!(err, SakamotoError::Io(_)));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn diagnostic_displays_correctly() {
        let diag = Diagnostic {
            severity: Severity::Warning,
            stage: "lint".into(),
            message: "unused import on line 42".into(),
        };
        assert_eq!(diag.to_string(), "[warning] lint: unused import on line 42");
    }
}
