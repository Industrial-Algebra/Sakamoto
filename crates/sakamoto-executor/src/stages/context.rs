//! Context pre-hydration stage.
//!
//! Parses the task description for references and populates
//! the context bundle's `refs` field.

use sakamoto_context::parser;
use sakamoto_core::stage::{Stage, StageContext};
use sakamoto_types::{ContextBundle, StageOutput};

/// Stage that extracts context references from the task description.
pub struct ContextStage;

#[async_trait::async_trait]
impl Stage for ContextStage {
    fn name(&self) -> &str {
        "context"
    }

    async fn execute(&self, mut context: ContextBundle, _ctx: &StageContext) -> StageOutput {
        let refs = parser::parse_all(&context.task_description);
        context.refs.extend(refs);
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
}
