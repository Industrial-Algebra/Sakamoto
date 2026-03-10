//! The [`Executor`] trait — abstraction over execution environments.

use sakamoto_core::runner::RunResult;
use sakamoto_types::SakamotoError;

/// An execution environment that can run a pipeline.
///
/// Different executors provide different isolation levels:
/// - [`LocalExecutor`](super::local::LocalExecutor): runs in the current working directory
/// - `WorktreeExecutor`: runs in a git worktree (v0.2)
/// - `NixContainerExecutor`: runs in an OCI container built from a Nix flake (v0.3)
#[async_trait::async_trait]
pub trait Executor: Send + Sync {
    /// Execute a pipeline with the given task description.
    async fn run(&self, task: &str) -> Result<RunResult, SakamotoError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_is_object_safe() {
        fn _assert<T: Executor>() {}
    }
}
