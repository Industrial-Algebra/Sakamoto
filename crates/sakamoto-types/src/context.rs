//! Context bundle and reference types for pre-hydration.
//!
//! The context engine parses task descriptions for references (file paths,
//! GitHub issues, URLs, symbols) and resolves them into content that seeds
//! the LLM's initial context.

use std::collections::HashMap;
use std::path::PathBuf;

/// A reference extracted from a task description, to be resolved by a fetcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextRef {
    /// A filesystem path (absolute or relative).
    FilePath { path: PathBuf },

    /// A GitHub issue reference.
    GitHubIssue {
        owner: String,
        repo: String,
        number: u64,
    },

    /// A GitHub pull request reference.
    GitHubPr {
        owner: String,
        repo: String,
        number: u64,
    },

    /// A generic URL to fetch.
    Url { url: String },

    /// A crate name to look up documentation for.
    Crate { name: String },

    /// A symbol name (function, type, trait) to search for in the codebase.
    Symbol { name: String },
}

impl std::fmt::Display for ContextRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FilePath { path } => write!(f, "file:{}", path.display()),
            Self::GitHubIssue {
                owner,
                repo,
                number,
            } => write!(f, "issue:{owner}/{repo}#{number}"),
            Self::GitHubPr {
                owner,
                repo,
                number,
            } => write!(f, "pr:{owner}/{repo}#{number}"),
            Self::Url { url } => write!(f, "url:{url}"),
            Self::Crate { name } => write!(f, "crate:{name}"),
            Self::Symbol { name } => write!(f, "symbol:{name}"),
        }
    }
}

/// A piece of resolved context content, keyed by its source reference.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextEntry {
    /// The reference that was resolved to produce this content.
    pub source: ContextRef,

    /// The resolved content (file contents, issue body, page text, etc.).
    pub content: String,

    /// Optional metadata (e.g., file language, issue labels, HTTP status).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Accumulated context passed between pipeline stages.
///
/// This is the primary data structure that flows through the pipeline DAG.
/// It accumulates context as stages execute and is the input to each stage.
///
/// As a monoidal type, two `ContextBundle`s can be merged (e.g., after
/// parallel stages fan back in).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ContextBundle {
    /// The original task description from the user.
    pub task_description: String,

    /// References extracted from the task description.
    #[serde(default)]
    pub refs: Vec<ContextRef>,

    /// Resolved context entries (pre-hydrated content).
    #[serde(default)]
    pub entries: Vec<ContextEntry>,

    /// The execution plan produced by the planning stage.
    #[serde(default)]
    pub plan: Option<String>,

    /// Files that have been modified during execution.
    #[serde(default)]
    pub modified_files: Vec<PathBuf>,

    /// Diagnostic messages accumulated during execution.
    #[serde(default)]
    pub diagnostics: Vec<crate::error::Diagnostic>,

    /// Arbitrary key-value metadata for extensibility.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ContextBundle {
    /// Create a new context bundle from a task description.
    pub fn from_task(description: impl Into<String>) -> Self {
        Self {
            task_description: description.into(),
            ..Default::default()
        }
    }

    /// Merge another context bundle into this one.
    ///
    /// This is the monoidal append operation: refs, entries, modified files,
    /// and diagnostics are concatenated. Plan is taken from `other` if `self`
    /// has no plan. Metadata is merged with `other` taking precedence.
    pub fn merge(&mut self, other: Self) {
        self.refs.extend(other.refs);
        self.entries.extend(other.entries);
        self.modified_files.extend(other.modified_files);
        self.diagnostics.extend(other.diagnostics);

        if self.plan.is_none() {
            self.plan = other.plan;
        }

        for (k, v) in other.metadata {
            self.metadata.entry(k).or_insert(v);
        }
    }

    /// Consume two bundles and produce a merged result.
    pub fn combine(mut self, other: Self) -> Self {
        self.merge(other);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_ref_display() {
        let file_ref = ContextRef::FilePath {
            path: PathBuf::from("src/main.rs"),
        };
        assert_eq!(file_ref.to_string(), "file:src/main.rs");

        let issue_ref = ContextRef::GitHubIssue {
            owner: "Industrial-Algebra".into(),
            repo: "Sakamoto".into(),
            number: 42,
        };
        assert_eq!(
            issue_ref.to_string(),
            "issue:Industrial-Algebra/Sakamoto#42"
        );
    }

    #[test]
    fn context_ref_serializes_tagged() {
        let r = ContextRef::Symbol {
            name: "Pipeline".into(),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["type"], "symbol");
        assert_eq!(json["name"], "Pipeline");
    }

    #[test]
    fn context_bundle_from_task() {
        let bundle = ContextBundle::from_task("fix clippy warnings");
        assert_eq!(bundle.task_description, "fix clippy warnings");
        assert!(bundle.refs.is_empty());
        assert!(bundle.entries.is_empty());
    }

    #[test]
    fn context_bundle_merge_concatenates() {
        let mut a = ContextBundle::from_task("task a");
        a.refs.push(ContextRef::Symbol { name: "Foo".into() });
        a.modified_files.push(PathBuf::from("a.rs"));

        let mut b = ContextBundle::from_task("task b");
        b.refs.push(ContextRef::Symbol { name: "Bar".into() });
        b.modified_files.push(PathBuf::from("b.rs"));
        b.plan = Some("do the thing".into());

        a.merge(b);
        assert_eq!(a.refs.len(), 2);
        assert_eq!(a.modified_files.len(), 2);
        assert_eq!(a.plan.as_deref(), Some("do the thing"));
    }

    #[test]
    fn context_bundle_merge_preserves_existing_plan() {
        let mut a = ContextBundle::from_task("task a");
        a.plan = Some("plan a".into());

        let mut b = ContextBundle::from_task("task b");
        b.plan = Some("plan b".into());

        a.merge(b);
        assert_eq!(a.plan.as_deref(), Some("plan a"));
    }

    #[test]
    fn context_bundle_combine_is_associative() {
        let a = ContextBundle::from_task("a");
        let mut b = ContextBundle::from_task("b");
        b.refs.push(ContextRef::Symbol { name: "X".into() });
        let mut c = ContextBundle::from_task("c");
        c.refs.push(ContextRef::Symbol { name: "Y".into() });

        // (a.combine(b)).combine(c) should have same refs as a.combine(b.combine(c))
        let ab_c = a.clone().combine(b.clone()).combine(c.clone());
        let a_bc = a.combine(b.combine(c));

        assert_eq!(ab_c.refs.len(), a_bc.refs.len());
    }

    #[test]
    fn context_bundle_default_is_empty() {
        let bundle = ContextBundle::default();
        assert!(bundle.task_description.is_empty());
        assert!(bundle.refs.is_empty());
        assert!(bundle.entries.is_empty());
        assert!(bundle.plan.is_none());
        assert!(bundle.modified_files.is_empty());
        assert!(bundle.diagnostics.is_empty());
        assert!(bundle.metadata.is_empty());
    }

    #[test]
    fn context_bundle_metadata_merge_does_not_overwrite() {
        let mut a = ContextBundle::from_task("a");
        a.metadata
            .insert("key".into(), serde_json::Value::from("from_a"));

        let mut b = ContextBundle::from_task("b");
        b.metadata
            .insert("key".into(), serde_json::Value::from("from_b"));
        b.metadata
            .insert("other".into(), serde_json::Value::from("only_b"));

        a.merge(b);
        assert_eq!(a.metadata["key"], "from_a");
        assert_eq!(a.metadata["other"], "only_b");
    }
}
