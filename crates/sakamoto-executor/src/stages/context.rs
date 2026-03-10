//! Context pre-hydration stage.
//!
//! Parses the task description for references, then resolves them
//! using the context engine (filesystem fetcher, GitHub fetcher).

use std::path::PathBuf;

use sakamoto_context::engine::ContextEngine;
use sakamoto_context::fetcher::{FilesystemFetcher, GitHubFetcher};
use sakamoto_core::stage::{Stage, StageContext};
use sakamoto_types::{ContextBundle, StageOutput};

/// Stage that extracts and resolves context references from the task description.
pub struct ContextStage;

#[async_trait::async_trait]
impl Stage for ContextStage {
    fn name(&self) -> &str {
        "context"
    }

    async fn execute(&self, mut context: ContextBundle, _ctx: &StageContext) -> StageOutput {
        let working_dir = context
            .metadata
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut engine = ContextEngine::new();
        engine.add_fetcher(FilesystemFetcher::new(&working_dir));
        engine.add_fetcher(GitHubFetcher::new());

        engine.hydrate(&mut context).await;

        StageOutput::Continue(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sakamoto_types::{ContextRef, stage::StageConfig};
    use std::path::PathBuf;

    fn empty_stage_ctx() -> StageContext {
        StageContext {
            llm: None,
            tools: None,
            config: StageConfig::default(),
        }
    }

    #[tokio::test]
    async fn context_stage_extracts_refs() {
        let stage = ContextStage;
        let bundle = ContextBundle::from_task("Fix src/main.rs and see owner/repo#42");
        let ctx = empty_stage_ctx();

        let output = stage.execute(bundle, &ctx).await;
        let bundle = output.into_context().unwrap();

        assert!(!bundle.refs.is_empty());
        assert!(bundle.refs.contains(&ContextRef::FilePath {
            path: PathBuf::from("src/main.rs"),
        }));
    }

    #[tokio::test]
    async fn context_stage_empty_description() {
        let stage = ContextStage;
        let bundle = ContextBundle::from_task("");
        let ctx = empty_stage_ctx();

        let output = stage.execute(bundle, &ctx).await;
        let bundle = output.into_context().unwrap();
        assert!(bundle.refs.is_empty());
    }

    #[tokio::test]
    async fn context_stage_fetches_real_files() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub mod foo;\n").unwrap();

        let stage = ContextStage;
        let mut bundle = ContextBundle::from_task("Check lib.rs");
        bundle.metadata.insert(
            "working_dir".into(),
            dir.path().display().to_string().into(),
        );
        let ctx = empty_stage_ctx();

        let output = stage.execute(bundle, &ctx).await;
        let bundle = output.into_context().unwrap();

        assert!(
            bundle
                .entries
                .iter()
                .any(|e| e.content.contains("pub mod foo"))
        );
    }
}
