//! Context engine — orchestrates parsing and fetching.
//!
//! The engine takes a task description, parses it for references,
//! resolves those references using registered fetchers, and produces
//! a hydrated [`ContextBundle`].

use sakamoto_types::{ContextBundle, ContextRef, SakamotoError};

use crate::fetcher::ContextFetcher;
use crate::parser;

/// Orchestrates context pre-hydration: parse refs, then fetch content.
pub struct ContextEngine {
    fetchers: Vec<Box<dyn ContextFetcher>>,
}

impl ContextEngine {
    /// Create a new empty context engine.
    pub fn new() -> Self {
        Self {
            fetchers: Vec::new(),
        }
    }

    /// Register a fetcher.
    pub fn add_fetcher(&mut self, fetcher: impl ContextFetcher + 'static) {
        self.fetchers.push(Box::new(fetcher));
    }

    /// Parse a task description for references.
    pub fn parse(&self, text: &str) -> Vec<ContextRef> {
        parser::parse_all(text)
    }

    /// Fetch a single context reference using registered fetchers.
    ///
    /// Returns `None` if no fetcher can handle this reference.
    /// Returns `Some(Err(...))` if a fetcher fails.
    pub async fn fetch_one(
        &self,
        context_ref: &ContextRef,
    ) -> Option<Result<sakamoto_types::ContextEntry, SakamotoError>> {
        for fetcher in &self.fetchers {
            if fetcher.can_fetch(context_ref) {
                return Some(fetcher.fetch(context_ref).await);
            }
        }
        None
    }

    /// Hydrate a context bundle: parse refs from the task description,
    /// then fetch all resolvable references.
    ///
    /// References that fail to fetch are logged but do not fail the
    /// overall hydration — partial context is better than none.
    pub async fn hydrate(&self, bundle: &mut ContextBundle) {
        let refs = self.parse(&bundle.task_description);
        bundle.refs.extend(refs);

        // Deduplicate refs
        let unique_refs: Vec<ContextRef> = {
            let mut seen = std::collections::HashSet::new();
            bundle
                .refs
                .iter()
                .filter(|r| seen.insert((*r).clone()))
                .cloned()
                .collect()
        };
        bundle.refs = unique_refs;

        for context_ref in &bundle.refs {
            match self.fetch_one(context_ref).await {
                Some(Ok(entry)) => {
                    tracing::debug!(ref_ = %context_ref, "fetched context");
                    bundle.entries.push(entry);
                }
                Some(Err(e)) => {
                    tracing::warn!(ref_ = %context_ref, error = %e, "failed to fetch context");
                }
                None => {
                    tracing::debug!(ref_ = %context_ref, "no fetcher available");
                }
            }
        }
    }
}

impl Default for ContextEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::FilesystemFetcher;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn test_dir_with_files() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub mod foo;\n").unwrap();
        dir
    }

    #[test]
    fn engine_parses_refs() {
        let engine = ContextEngine::new();
        let refs = engine.parse("Fix src/main.rs and owner/repo#42");
        assert!(!refs.is_empty());
    }

    #[tokio::test]
    async fn engine_hydrates_file_refs() {
        let dir = test_dir_with_files();
        let mut engine = ContextEngine::new();
        engine.add_fetcher(FilesystemFetcher::new(dir.path()));

        let mut bundle = ContextBundle::from_task("Fix main.rs");
        // Manually add a ref that points to a real file
        bundle.refs.push(ContextRef::FilePath {
            path: PathBuf::from("main.rs"),
        });

        engine.hydrate(&mut bundle).await;

        assert!(!bundle.entries.is_empty());
        assert_eq!(bundle.entries[0].content, "fn main() {}\n");
    }

    #[tokio::test]
    async fn engine_hydrate_parses_and_fetches() {
        let dir = test_dir_with_files();
        let mut engine = ContextEngine::new();
        engine.add_fetcher(FilesystemFetcher::new(dir.path()));

        let mut bundle = ContextBundle::from_task("Look at main.rs");
        engine.hydrate(&mut bundle).await;

        // Parser should have found "main.rs" as a file path ref
        assert!(bundle.refs.iter().any(|r| matches!(
            r,
            ContextRef::FilePath { path } if path == &PathBuf::from("main.rs")
        )));

        // Fetcher should have resolved it
        assert!(
            bundle
                .entries
                .iter()
                .any(|e| e.content.contains("fn main()"))
        );
    }

    #[tokio::test]
    async fn engine_skips_unfetchable_refs() {
        let engine = ContextEngine::new(); // no fetchers registered

        let mut bundle = ContextBundle::from_task("test");
        bundle.refs.push(ContextRef::FilePath {
            path: PathBuf::from("whatever.rs"),
        });

        engine.hydrate(&mut bundle).await;

        // No fetchers, so no entries, but no panic either
        assert!(bundle.entries.is_empty());
    }

    #[tokio::test]
    async fn engine_tolerates_fetch_errors() {
        let dir = test_dir_with_files();
        let mut engine = ContextEngine::new();
        engine.add_fetcher(FilesystemFetcher::new(dir.path()));

        let mut bundle = ContextBundle::from_task("test");
        bundle.refs.push(ContextRef::FilePath {
            path: PathBuf::from("nonexistent.rs"),
        });
        bundle.refs.push(ContextRef::FilePath {
            path: PathBuf::from("main.rs"),
        });

        engine.hydrate(&mut bundle).await;

        // Should have fetched main.rs despite nonexistent.rs failing
        assert_eq!(bundle.entries.len(), 1);
        assert!(bundle.entries[0].content.contains("fn main()"));
    }

    #[tokio::test]
    async fn engine_deduplicates_refs() {
        let mut engine = ContextEngine::new();
        let dir = test_dir_with_files();
        engine.add_fetcher(FilesystemFetcher::new(dir.path()));

        let mut bundle = ContextBundle::from_task("test");
        bundle.refs.push(ContextRef::FilePath {
            path: PathBuf::from("main.rs"),
        });
        bundle.refs.push(ContextRef::FilePath {
            path: PathBuf::from("main.rs"),
        });

        engine.hydrate(&mut bundle).await;

        // Refs should be deduplicated
        assert_eq!(
            bundle
                .refs
                .iter()
                .filter(|r| matches!(r, ContextRef::FilePath { path } if path == &PathBuf::from("main.rs")))
                .count(),
            1
        );
        // Only one entry fetched
        assert_eq!(bundle.entries.len(), 1);
    }

    #[tokio::test]
    async fn fetch_one_returns_none_when_no_fetcher() {
        let engine = ContextEngine::new();
        let context_ref = ContextRef::Url {
            url: "https://example.com".into(),
        };

        assert!(engine.fetch_one(&context_ref).await.is_none());
    }
}
