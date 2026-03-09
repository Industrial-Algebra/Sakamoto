//! Pipeline DAG — topological ordering and parallel level computation.
//!
//! Wraps a simple adjacency-list graph that tracks stage dependencies.
//! Provides topological sort (via Kahn's algorithm) and groups stages
//! into parallel execution levels.
//!
//! Future versions will integrate with Cliffy's `DataflowGraph` for
//! richer graph operations and reactive `Behavior<T>` progress tracking.

use sakamoto_types::SakamotoError;
use std::collections::HashMap;

/// A directed acyclic graph of pipeline stages.
///
/// Each stage is identified by name. Edges represent data dependencies:
/// if stage B depends on stage A, then A must complete before B starts.
pub struct PipelineDag {
    /// Stage names in insertion order.
    stages: Vec<String>,
    /// Map from stage name to index in `stages`.
    index: HashMap<String, usize>,
    /// Forward adjacency list: stage index → indices of stages that depend on it.
    dependents: Vec<Vec<usize>>,
    /// Reverse adjacency list: stage index → indices of stages it depends on.
    dependencies: Vec<Vec<usize>>,
}

impl PipelineDag {
    /// Create an empty DAG.
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            index: HashMap::new(),
            dependents: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Create a linear DAG where each stage depends on the previous one.
    pub fn from_linear(stage_names: &[String]) -> Result<Self, SakamotoError> {
        let mut dag = Self::new();
        for name in stage_names {
            dag.add_stage(name.clone())?;
        }
        for i in 1..stage_names.len() {
            dag.add_dependency(&stage_names[i], &stage_names[i - 1])?;
        }
        Ok(dag)
    }

    /// Add a stage to the DAG. Returns an error if a stage with the same
    /// name already exists.
    pub fn add_stage(&mut self, name: String) -> Result<usize, SakamotoError> {
        if self.index.contains_key(&name) {
            return Err(SakamotoError::ConfigError(format!(
                "duplicate stage: {name}"
            )));
        }
        let idx = self.stages.len();
        self.index.insert(name.clone(), idx);
        self.stages.push(name);
        self.dependents.push(Vec::new());
        self.dependencies.push(Vec::new());
        Ok(idx)
    }

    /// Declare that `stage` depends on `depends_on` (i.e., `depends_on`
    /// must complete before `stage` can start).
    pub fn add_dependency(&mut self, stage: &str, depends_on: &str) -> Result<(), SakamotoError> {
        let stage_idx = *self
            .index
            .get(stage)
            .ok_or_else(|| SakamotoError::StageNotFound(stage.into()))?;
        let dep_idx = *self
            .index
            .get(depends_on)
            .ok_or_else(|| SakamotoError::StageNotFound(depends_on.into()))?;

        self.dependents[dep_idx].push(stage_idx);
        self.dependencies[stage_idx].push(dep_idx);
        Ok(())
    }

    /// Whether the DAG contains a stage with the given name.
    pub fn has_stage(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    /// Number of stages in the DAG.
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// Whether the DAG has no stages.
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// Compute execution levels via Kahn's algorithm.
    ///
    /// Returns a vector of levels, where each level is a vector of stage
    /// names that can execute in parallel. Stages in level N depend only
    /// on stages in levels 0..N-1.
    ///
    /// Returns `CyclicGraph` if the DAG contains a cycle.
    pub fn execution_levels(&self) -> Result<Vec<Vec<String>>, SakamotoError> {
        let n = self.stages.len();
        if n == 0 {
            return Ok(Vec::new());
        }

        // Compute in-degree for each node
        let mut in_degree: Vec<usize> = self.dependencies.iter().map(|deps| deps.len()).collect();

        // Start with all nodes that have no dependencies
        let mut current_level: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();

        let mut levels = Vec::new();
        let mut visited = 0;

        while !current_level.is_empty() {
            // Record this level's stage names
            let level_names: Vec<String> = current_level
                .iter()
                .map(|&i| self.stages[i].clone())
                .collect();
            levels.push(level_names);
            visited += current_level.len();

            // Compute the next level
            let mut next_level = Vec::new();
            for &node in &current_level {
                for &dependent in &self.dependents[node] {
                    in_degree[dependent] -= 1;
                    if in_degree[dependent] == 0 {
                        next_level.push(dependent);
                    }
                }
            }

            current_level = next_level;
        }

        if visited != n {
            return Err(SakamotoError::CyclicGraph);
        }

        Ok(levels)
    }

    /// Flat topological order (all stages, respecting dependencies).
    pub fn topological_order(&self) -> Result<Vec<String>, SakamotoError> {
        Ok(self.execution_levels()?.into_iter().flatten().collect())
    }
}

impl Default for PipelineDag {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dag() {
        let dag = PipelineDag::new();
        assert!(dag.is_empty());
        assert_eq!(dag.len(), 0);
        let levels = dag.execution_levels().unwrap();
        assert!(levels.is_empty());
    }

    #[test]
    fn single_stage() {
        let mut dag = PipelineDag::new();
        dag.add_stage("a".into()).unwrap();
        let levels = dag.execution_levels().unwrap();
        assert_eq!(levels, vec![vec!["a"]]);
    }

    #[test]
    fn linear_pipeline() {
        let names: Vec<String> = vec!["context", "plan", "code", "lint", "test"]
            .into_iter()
            .map(Into::into)
            .collect();
        let dag = PipelineDag::from_linear(&names).unwrap();

        let levels = dag.execution_levels().unwrap();
        assert_eq!(levels.len(), 5);
        assert_eq!(levels[0], vec!["context"]);
        assert_eq!(levels[1], vec!["plan"]);
        assert_eq!(levels[2], vec!["code"]);
        assert_eq!(levels[3], vec!["lint"]);
        assert_eq!(levels[4], vec!["test"]);
    }

    #[test]
    fn parallel_stages() {
        // context → plan → code → { lint, test } → commit
        let mut dag = PipelineDag::new();
        for name in &["context", "plan", "code", "lint", "test", "commit"] {
            dag.add_stage((*name).into()).unwrap();
        }
        dag.add_dependency("plan", "context").unwrap();
        dag.add_dependency("code", "plan").unwrap();
        dag.add_dependency("lint", "code").unwrap();
        dag.add_dependency("test", "code").unwrap();
        dag.add_dependency("commit", "lint").unwrap();
        dag.add_dependency("commit", "test").unwrap();

        let levels = dag.execution_levels().unwrap();
        assert_eq!(levels.len(), 5);
        assert_eq!(levels[0], vec!["context"]);
        assert_eq!(levels[1], vec!["plan"]);
        assert_eq!(levels[2], vec!["code"]);
        // lint and test are at the same level (parallel)
        assert_eq!(levels[3].len(), 2);
        assert!(levels[3].contains(&"lint".to_string()));
        assert!(levels[3].contains(&"test".to_string()));
        assert_eq!(levels[4], vec!["commit"]);
    }

    #[test]
    fn cycle_detected() {
        let mut dag = PipelineDag::new();
        dag.add_stage("a".into()).unwrap();
        dag.add_stage("b".into()).unwrap();
        dag.add_stage("c".into()).unwrap();
        dag.add_dependency("b", "a").unwrap();
        dag.add_dependency("c", "b").unwrap();
        dag.add_dependency("a", "c").unwrap();

        let err = dag.execution_levels().unwrap_err();
        assert!(matches!(err, SakamotoError::CyclicGraph));
    }

    #[test]
    fn duplicate_stage_rejected() {
        let mut dag = PipelineDag::new();
        dag.add_stage("a".into()).unwrap();
        let err = dag.add_stage("a".into()).unwrap_err();
        assert!(matches!(err, SakamotoError::ConfigError(_)));
    }

    #[test]
    fn dependency_on_unknown_stage_rejected() {
        let mut dag = PipelineDag::new();
        dag.add_stage("a".into()).unwrap();
        let err = dag.add_dependency("a", "nonexistent").unwrap_err();
        assert!(matches!(err, SakamotoError::StageNotFound(_)));
    }

    #[test]
    fn has_stage_works() {
        let mut dag = PipelineDag::new();
        dag.add_stage("a".into()).unwrap();
        assert!(dag.has_stage("a"));
        assert!(!dag.has_stage("b"));
    }

    #[test]
    fn topological_order_flattens() {
        let names: Vec<String> = vec!["a", "b", "c"].into_iter().map(Into::into).collect();
        let dag = PipelineDag::from_linear(&names).unwrap();
        let order = dag.topological_order().unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn independent_stages_all_at_level_zero() {
        let mut dag = PipelineDag::new();
        dag.add_stage("a".into()).unwrap();
        dag.add_stage("b".into()).unwrap();
        dag.add_stage("c".into()).unwrap();
        // No dependencies — all parallel

        let levels = dag.execution_levels().unwrap();
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].len(), 3);
    }

    #[test]
    fn diamond_dependency() {
        //   a
        //  / \
        // b   c
        //  \ /
        //   d
        let mut dag = PipelineDag::new();
        for name in &["a", "b", "c", "d"] {
            dag.add_stage((*name).into()).unwrap();
        }
        dag.add_dependency("b", "a").unwrap();
        dag.add_dependency("c", "a").unwrap();
        dag.add_dependency("d", "b").unwrap();
        dag.add_dependency("d", "c").unwrap();

        let levels = dag.execution_levels().unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec!["a"]);
        assert_eq!(levels[1].len(), 2); // b and c parallel
        assert_eq!(levels[2], vec!["d"]);
    }
}
