//! Context fetcher trait and implementations.
//!
//! Fetchers resolve [`ContextRef`]s into [`ContextEntry`]s by reading
//! from the filesystem, calling `gh` CLI, or fetching URLs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sakamoto_types::{ContextEntry, ContextRef, SakamotoError};

/// A fetcher that can resolve a specific kind of [`ContextRef`] into content.
#[async_trait::async_trait]
pub trait ContextFetcher: Send + Sync {
    /// Whether this fetcher can handle the given reference.
    fn can_fetch(&self, context_ref: &ContextRef) -> bool;

    /// Resolve the reference into a context entry.
    async fn fetch(&self, context_ref: &ContextRef) -> Result<ContextEntry, SakamotoError>;
}

// ── Filesystem fetcher ────────────────────────────────────────────

/// Fetches file contents from the local filesystem.
pub struct FilesystemFetcher {
    /// Root directory for resolving relative paths.
    root: PathBuf,
    /// Maximum file size to read (bytes).
    max_size: u64,
}

impl FilesystemFetcher {
    /// Create a new filesystem fetcher rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            max_size: 1_000_000, // 1 MB default
        }
    }

    /// Set the maximum file size to read.
    pub fn with_max_size(mut self, max_size: u64) -> Self {
        self.max_size = max_size;
        self
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }
}

#[async_trait::async_trait]
impl ContextFetcher for FilesystemFetcher {
    fn can_fetch(&self, context_ref: &ContextRef) -> bool {
        matches!(context_ref, ContextRef::FilePath { .. })
    }

    async fn fetch(&self, context_ref: &ContextRef) -> Result<ContextEntry, SakamotoError> {
        let path = match context_ref {
            ContextRef::FilePath { path } => path,
            _ => {
                return Err(SakamotoError::ContextError(
                    "FilesystemFetcher can only fetch FilePath refs".into(),
                ));
            }
        };

        let resolved = self.resolve(path);

        let metadata = tokio::fs::metadata(&resolved).await.map_err(|e| {
            SakamotoError::ContextError(format!("cannot stat '{}': {}", path.display(), e))
        })?;

        if metadata.is_dir() {
            // List directory contents instead of reading
            let mut entries = Vec::new();
            let mut read_dir = tokio::fs::read_dir(&resolved).await.map_err(|e| {
                SakamotoError::ContextError(format!(
                    "cannot read directory '{}': {}",
                    path.display(),
                    e
                ))
            })?;

            while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
                SakamotoError::ContextError(format!("error reading directory entry: {}", e))
            })? {
                let name = entry.file_name().to_string_lossy().to_string();
                let file_type = entry.file_type().await.ok();
                let suffix = if file_type.is_some_and(|ft| ft.is_dir()) {
                    "/"
                } else {
                    ""
                };
                entries.push(format!("{name}{suffix}"));
            }

            entries.sort();

            let mut meta = HashMap::new();
            meta.insert("type".into(), "directory".into());

            return Ok(ContextEntry {
                source: context_ref.clone(),
                content: entries.join("\n"),
                metadata: meta,
            });
        }

        if metadata.len() > self.max_size {
            return Err(SakamotoError::ContextError(format!(
                "file '{}' is {} bytes, exceeds max size of {} bytes",
                path.display(),
                metadata.len(),
                self.max_size
            )));
        }

        let content = tokio::fs::read_to_string(&resolved).await.map_err(|e| {
            SakamotoError::ContextError(format!("cannot read '{}': {}", path.display(), e))
        })?;

        let mut meta = HashMap::new();
        meta.insert("type".into(), "file".into());
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            meta.insert("extension".into(), ext.into());
        }
        meta.insert("lines".into(), content.lines().count().to_string());

        Ok(ContextEntry {
            source: context_ref.clone(),
            content,
            metadata: meta,
        })
    }
}

// ── GitHub fetcher ────────────────────────────────────────────────

/// Fetches GitHub issue/PR content via the `gh` CLI.
pub struct GitHubFetcher;

impl GitHubFetcher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitHubFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ContextFetcher for GitHubFetcher {
    fn can_fetch(&self, context_ref: &ContextRef) -> bool {
        matches!(
            context_ref,
            ContextRef::GitHubIssue { .. } | ContextRef::GitHubPr { .. }
        )
    }

    async fn fetch(&self, context_ref: &ContextRef) -> Result<ContextEntry, SakamotoError> {
        let (owner, repo, number, kind) = match context_ref {
            ContextRef::GitHubIssue {
                owner,
                repo,
                number,
            } => (owner, repo, number, "issue"),
            ContextRef::GitHubPr {
                owner,
                repo,
                number,
            } => (owner, repo, number, "pr"),
            _ => {
                return Err(SakamotoError::ContextError(
                    "GitHubFetcher can only fetch GitHub refs".into(),
                ));
            }
        };

        let output = tokio::process::Command::new("gh")
            .args([
                kind,
                "view",
                &number.to_string(),
                "--repo",
                &format!("{owner}/{repo}"),
                "--json",
                "title,body,state,labels,comments",
            ])
            .output()
            .await
            .map_err(|e| {
                SakamotoError::ContextError(format!(
                    "failed to run `gh {kind} view` for {owner}/{repo}#{number}: {e}"
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SakamotoError::ContextError(format!(
                "`gh {kind} view` failed for {owner}/{repo}#{number}: {stderr}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        let mut meta = HashMap::new();
        meta.insert("type".into(), kind.into());
        meta.insert("owner".into(), owner.clone());
        meta.insert("repo".into(), repo.clone());
        meta.insert("number".into(), number.to_string());

        // Try to extract title and state from the JSON for metadata
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(title) = json["title"].as_str() {
                meta.insert("title".into(), title.into());
            }
            if let Some(state) = json["state"].as_str() {
                meta.insert("state".into(), state.into());
            }
        }

        Ok(ContextEntry {
            source: context_ref.clone(),
            content: stdout,
            metadata: meta,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[tokio::test]
    async fn filesystem_fetcher_reads_file() {
        let dir = test_dir();
        fs::write(dir.path().join("hello.rs"), "fn main() {}\n").unwrap();

        let fetcher = FilesystemFetcher::new(dir.path());
        let context_ref = ContextRef::FilePath {
            path: PathBuf::from("hello.rs"),
        };

        assert!(fetcher.can_fetch(&context_ref));
        let entry = fetcher.fetch(&context_ref).await.unwrap();

        assert_eq!(entry.content, "fn main() {}\n");
        assert_eq!(entry.metadata["type"], "file");
        assert_eq!(entry.metadata["extension"], "rs");
        assert_eq!(entry.metadata["lines"], "1");
    }

    #[tokio::test]
    async fn filesystem_fetcher_reads_directory() {
        let dir = test_dir();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let fetcher = FilesystemFetcher::new(dir.path());
        let context_ref = ContextRef::FilePath {
            path: PathBuf::from("."),
        };

        let entry = fetcher.fetch(&context_ref).await.unwrap();

        assert_eq!(entry.metadata["type"], "directory");
        assert!(entry.content.contains("a.txt"));
        assert!(entry.content.contains("b.txt"));
        assert!(entry.content.contains("subdir/"));
    }

    #[tokio::test]
    async fn filesystem_fetcher_nonexistent_file() {
        let dir = test_dir();
        let fetcher = FilesystemFetcher::new(dir.path());
        let context_ref = ContextRef::FilePath {
            path: PathBuf::from("missing.txt"),
        };

        let result = fetcher.fetch(&context_ref).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn filesystem_fetcher_rejects_oversized() {
        let dir = test_dir();
        let big_content = "x".repeat(2000);
        fs::write(dir.path().join("big.txt"), &big_content).unwrap();

        let fetcher = FilesystemFetcher::new(dir.path()).with_max_size(100);
        let context_ref = ContextRef::FilePath {
            path: PathBuf::from("big.txt"),
        };

        let result = fetcher.fetch(&context_ref).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds max size"));
    }

    #[tokio::test]
    async fn filesystem_fetcher_rejects_wrong_ref_type() {
        let dir = test_dir();
        let fetcher = FilesystemFetcher::new(dir.path());
        let context_ref = ContextRef::Url {
            url: "https://example.com".into(),
        };

        assert!(!fetcher.can_fetch(&context_ref));
        let result = fetcher.fetch(&context_ref).await;
        assert!(result.is_err());
    }

    #[test]
    fn github_fetcher_can_fetch_issues_and_prs() {
        let fetcher = GitHubFetcher::new();

        assert!(fetcher.can_fetch(&ContextRef::GitHubIssue {
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
        }));

        assert!(fetcher.can_fetch(&ContextRef::GitHubPr {
            owner: "o".into(),
            repo: "r".into(),
            number: 1,
        }));

        assert!(!fetcher.can_fetch(&ContextRef::FilePath {
            path: PathBuf::from("foo"),
        }));
    }

    // Note: GitHubFetcher.fetch() tests are integration tests — they
    // require `gh` CLI to be installed and authenticated. We test the
    // can_fetch dispatch and error paths here; actual gh calls are
    // tested in the integration test suite.
}
