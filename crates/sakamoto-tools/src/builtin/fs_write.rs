//! Structured file writing tool.
//!
//! Provides the LLM with a file writing tool that supports full writes
//! and partial edits (find-and-replace). Safer than `echo >` for code
//! modification — validates paths and creates parent directories.

use std::path::{Path, PathBuf};

use sakamoto_types::{SakamotoError, ToolDef};

use crate::tool::Tool;

/// A tool that writes or edits file contents.
pub struct FsWriteTool {
    /// Root directory — all paths are resolved relative to this.
    root: PathBuf,
}

impl FsWriteTool {
    /// Create a new fs_write tool rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve and validate a path, preventing directory traversal.
    fn resolve_path(&self, file_path: &str) -> Result<PathBuf, SakamotoError> {
        let requested = Path::new(file_path);

        let resolved = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.root.join(requested)
        };

        // For writes, the file may not exist yet, so canonicalize the parent
        let parent = resolved
            .parent()
            .ok_or_else(|| SakamotoError::ToolCallFailed {
                tool: "fs_write".into(),
                reason: format!("invalid path: '{}'", file_path),
            })?;

        // Parent must exist (or we create it) and be under root
        let canonical_parent = if parent.exists() {
            parent
                .canonicalize()
                .map_err(|e| SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!("cannot resolve parent of '{}': {}", file_path, e),
                })?
        } else {
            // Walk up to find an existing ancestor and check it's under root
            let mut ancestor = parent.to_path_buf();
            while !ancestor.exists() {
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| SakamotoError::ToolCallFailed {
                        tool: "fs_write".into(),
                        reason: format!("no valid ancestor for '{}'", file_path),
                    })?
                    .to_path_buf();
            }
            let canonical_ancestor =
                ancestor
                    .canonicalize()
                    .map_err(|e| SakamotoError::ToolCallFailed {
                        tool: "fs_write".into(),
                        reason: format!("cannot resolve ancestor: {}", e),
                    })?;

            let canonical_root =
                self.root
                    .canonicalize()
                    .map_err(|e| SakamotoError::ToolCallFailed {
                        tool: "fs_write".into(),
                        reason: format!("cannot resolve root: {}", e),
                    })?;

            if !canonical_ancestor.starts_with(&canonical_root) {
                return Err(SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!("path '{}' is outside the working directory", file_path),
                });
            }

            // Return the resolved (non-canonical) path since parent doesn't exist yet
            return Ok(resolved);
        };

        let canonical_root =
            self.root
                .canonicalize()
                .map_err(|e| SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!("cannot resolve root: {}", e),
                })?;

        if !canonical_parent.starts_with(&canonical_root) {
            return Err(SakamotoError::ToolCallFailed {
                tool: "fs_write".into(),
                reason: format!("path '{}' is outside the working directory", file_path),
            });
        }

        Ok(canonical_parent.join(resolved.file_name().ok_or_else(|| {
            SakamotoError::ToolCallFailed {
                tool: "fs_write".into(),
                reason: format!("path '{}' has no filename", file_path),
            }
        })?))
    }
}

#[async_trait::async_trait]
impl Tool for FsWriteTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "fs_write".into(),
            description: "Write content to a file, or edit a file by replacing a specific string. \
                           Creates parent directories if needed."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (relative to working directory or absolute)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file content to write (for complete file writes)"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "String to find in the existing file (for edits)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "Replacement string (for edits)"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, SakamotoError> {
        let file_path = input["path"]
            .as_str()
            .ok_or_else(|| SakamotoError::ToolCallFailed {
                tool: "fs_write".into(),
                reason: "missing 'path' field".into(),
            })?;

        let resolved = self.resolve_path(file_path)?;

        let has_content = input.get("content").is_some_and(|v| !v.is_null());
        let has_old_string = input.get("old_string").is_some_and(|v| !v.is_null());

        if has_content && has_old_string {
            return Err(SakamotoError::ToolCallFailed {
                tool: "fs_write".into(),
                reason: "provide either 'content' for full write or 'old_string'/'new_string' for edit, not both".into(),
            });
        }

        if has_old_string {
            // Edit mode: find and replace
            let old_string =
                input["old_string"]
                    .as_str()
                    .ok_or_else(|| SakamotoError::ToolCallFailed {
                        tool: "fs_write".into(),
                        reason: "'old_string' must be a string".into(),
                    })?;

            let new_string = input["new_string"].as_str().unwrap_or("");

            let existing = tokio::fs::read_to_string(&resolved).await.map_err(|e| {
                SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!("cannot read '{}' for editing: {}", file_path, e),
                }
            })?;

            let count = existing.matches(old_string).count();
            if count == 0 {
                return Err(SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!(
                        "old_string not found in '{}' — provide a unique string that exists in the file",
                        file_path
                    ),
                });
            }
            if count > 1 {
                return Err(SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!(
                        "old_string found {} times in '{}' — provide a more specific string to match exactly once",
                        count, file_path
                    ),
                });
            }

            let updated = existing.replacen(old_string, new_string, 1);
            tokio::fs::write(&resolved, &updated).await.map_err(|e| {
                SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!("cannot write '{}': {}", file_path, e),
                }
            })?;

            Ok(format!("edited {}", file_path))
        } else if has_content {
            // Full write mode
            let content =
                input["content"]
                    .as_str()
                    .ok_or_else(|| SakamotoError::ToolCallFailed {
                        tool: "fs_write".into(),
                        reason: "'content' must be a string".into(),
                    })?;

            // Create parent directories if needed
            if let Some(parent) = resolved.parent()
                && !parent.exists()
            {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    SakamotoError::ToolCallFailed {
                        tool: "fs_write".into(),
                        reason: format!("cannot create directories for '{}': {}", file_path, e),
                    }
                })?;
            }

            tokio::fs::write(&resolved, content).await.map_err(|e| {
                SakamotoError::ToolCallFailed {
                    tool: "fs_write".into(),
                    reason: format!("cannot write '{}': {}", file_path, e),
                }
            })?;

            Ok(format!("wrote {} ({} bytes)", file_path, content.len()))
        } else {
            Err(SakamotoError::ToolCallFailed {
                tool: "fs_write".into(),
                reason:
                    "provide either 'content' for full write or 'old_string'/'new_string' for edit"
                        .into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FsWriteTool) {
        let dir = TempDir::new().unwrap();
        let tool = FsWriteTool::new(dir.path());
        (dir, tool)
    }

    #[test]
    fn definition_has_correct_name() {
        let (_dir, tool) = setup();
        assert_eq!(tool.definition().name, "fs_write");
    }

    #[tokio::test]
    async fn write_new_file() {
        let (dir, tool) = setup();

        let result = tool
            .execute(serde_json::json!({
                "path": "new.txt",
                "content": "hello world"
            }))
            .await
            .unwrap();

        assert!(result.contains("wrote"));
        let content = fs::read_to_string(dir.path().join("new.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn write_creates_parent_dirs() {
        let (dir, tool) = setup();

        tool.execute(serde_json::json!({
            "path": "sub/dir/file.txt",
            "content": "nested"
        }))
        .await
        .unwrap();

        let content = fs::read_to_string(dir.path().join("sub/dir/file.txt")).unwrap();
        assert_eq!(content, "nested");
    }

    #[tokio::test]
    async fn edit_find_and_replace() {
        let (dir, tool) = setup();
        fs::write(
            dir.path().join("code.rs"),
            "fn foo() {\n    println!(\"old\");\n}\n",
        )
        .unwrap();

        let result = tool
            .execute(serde_json::json!({
                "path": "code.rs",
                "old_string": "println!(\"old\")",
                "new_string": "println!(\"new\")"
            }))
            .await
            .unwrap();

        assert!(result.contains("edited"));
        let content = fs::read_to_string(dir.path().join("code.rs")).unwrap();
        assert!(content.contains("println!(\"new\")"));
        assert!(!content.contains("println!(\"old\")"));
    }

    #[tokio::test]
    async fn edit_old_string_not_found() {
        let (dir, tool) = setup();
        fs::write(dir.path().join("file.txt"), "hello world").unwrap();

        let result = tool
            .execute(serde_json::json!({
                "path": "file.txt",
                "old_string": "nonexistent",
                "new_string": "replacement"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn edit_ambiguous_match_rejected() {
        let (dir, tool) = setup();
        fs::write(dir.path().join("file.txt"), "aaa bbb aaa").unwrap();

        let result = tool
            .execute(serde_json::json!({
                "path": "file.txt",
                "old_string": "aaa",
                "new_string": "ccc"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("2 times"));
    }

    #[tokio::test]
    async fn content_and_old_string_both_rejected() {
        let (dir, tool) = setup();
        fs::write(dir.path().join("file.txt"), "hello").unwrap();

        let result = tool
            .execute(serde_json::json!({
                "path": "file.txt",
                "content": "full write",
                "old_string": "hello",
                "new_string": "bye"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not both"));
    }

    #[tokio::test]
    async fn neither_content_nor_old_string_rejected() {
        let (_dir, tool) = setup();

        let result = tool.execute(serde_json::json!({"path": "file.txt"})).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn missing_path_field() {
        let (_dir, tool) = setup();

        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn directory_traversal_blocked() {
        let (_dir, tool) = setup();

        let result = tool
            .execute(serde_json::json!({
                "path": "../../etc/evil",
                "content": "bad"
            }))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn overwrite_existing_file() {
        let (dir, tool) = setup();
        fs::write(dir.path().join("existing.txt"), "old content").unwrap();

        tool.execute(serde_json::json!({
            "path": "existing.txt",
            "content": "new content"
        }))
        .await
        .unwrap();

        let content = fs::read_to_string(dir.path().join("existing.txt")).unwrap();
        assert_eq!(content, "new content");
    }
}
