//! Structured file reading tool.
//!
//! Provides the LLM with a file reading tool that supports line ranges,
//! output truncation, and line numbering — better for context management
//! than raw `cat`.

use std::path::{Path, PathBuf};

use sakamoto_types::{SakamotoError, ToolDef};

use crate::tool::Tool;

/// Maximum number of lines to return by default.
const DEFAULT_MAX_LINES: usize = 2000;

/// Maximum line length before truncation.
const MAX_LINE_LENGTH: usize = 2000;

/// A tool that reads file contents with optional line ranges.
pub struct FsReadTool {
    /// Root directory — all paths are resolved relative to this.
    root: PathBuf,
}

impl FsReadTool {
    /// Create a new fs_read tool rooted at the given directory.
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

        // Canonicalize to resolve .. and symlinks, then check it's under root
        let canonical = resolved
            .canonicalize()
            .map_err(|e| SakamotoError::ToolCallFailed {
                tool: "fs_read".into(),
                reason: format!("cannot resolve path '{}': {}", file_path, e),
            })?;

        let canonical_root =
            self.root
                .canonicalize()
                .map_err(|e| SakamotoError::ToolCallFailed {
                    tool: "fs_read".into(),
                    reason: format!("cannot resolve root: {}", e),
                })?;

        if !canonical.starts_with(&canonical_root) {
            return Err(SakamotoError::ToolCallFailed {
                tool: "fs_read".into(),
                reason: format!("path '{}' is outside the working directory", file_path),
            });
        }

        Ok(canonical)
    }
}

#[async_trait::async_trait]
impl Tool for FsReadTool {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "fs_read".into(),
            description: "Read a file's contents with optional line range. Returns numbered lines."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path (relative to working directory or absolute)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (1-based, default: 1)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to return (default: 2000)"
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
                tool: "fs_read".into(),
                reason: "missing 'path' field".into(),
            })?;

        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().unwrap_or(DEFAULT_MAX_LINES as u64) as usize;

        let resolved = self.resolve_path(file_path)?;

        let content = tokio::fs::read_to_string(&resolved).await.map_err(|e| {
            SakamotoError::ToolCallFailed {
                tool: "fs_read".into(),
                reason: format!("cannot read '{}': {}", file_path, e),
            }
        })?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Convert to 0-based index
        let start = (offset - 1).min(total_lines);
        let end = (start + limit).min(total_lines);

        let mut output = String::new();

        for (i, line) in lines[start..end].iter().enumerate() {
            let line_num = start + i + 1;
            let truncated = if line.len() > MAX_LINE_LENGTH {
                format!("{}... [truncated]", &line[..MAX_LINE_LENGTH])
            } else {
                line.to_string()
            };
            output.push_str(&format!("{:>6}\t{}\n", line_num, truncated));
        }

        if end < total_lines {
            output.push_str(&format!(
                "\n[{} more lines not shown — use offset/limit to read further]\n",
                total_lines - end
            ));
        }

        if output.is_empty() {
            output = format!("[file is empty: {} total lines]\n", total_lines);
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FsReadTool) {
        let dir = TempDir::new().unwrap();
        let tool = FsReadTool::new(dir.path());
        (dir, tool)
    }

    fn write_test_file(dir: &TempDir, name: &str, content: &str) {
        fs::write(dir.path().join(name), content).unwrap();
    }

    #[test]
    fn definition_has_correct_name() {
        let (_dir, tool) = setup();
        assert_eq!(tool.definition().name, "fs_read");
    }

    #[tokio::test]
    async fn read_entire_file() {
        let (dir, tool) = setup();
        write_test_file(&dir, "hello.txt", "line one\nline two\nline three\n");

        let result = tool
            .execute(serde_json::json!({"path": "hello.txt"}))
            .await
            .unwrap();

        assert!(result.contains("1\tline one"));
        assert!(result.contains("2\tline two"));
        assert!(result.contains("3\tline three"));
    }

    #[tokio::test]
    async fn read_with_offset() {
        let (dir, tool) = setup();
        write_test_file(&dir, "lines.txt", "a\nb\nc\nd\ne\n");

        let result = tool
            .execute(serde_json::json!({"path": "lines.txt", "offset": 3}))
            .await
            .unwrap();

        assert!(!result.contains("1\ta"));
        assert!(!result.contains("2\tb"));
        assert!(result.contains("3\tc"));
        assert!(result.contains("4\td"));
    }

    #[tokio::test]
    async fn read_with_limit() {
        let (dir, tool) = setup();
        write_test_file(&dir, "lines.txt", "a\nb\nc\nd\ne\n");

        let result = tool
            .execute(serde_json::json!({"path": "lines.txt", "limit": 2}))
            .await
            .unwrap();

        assert!(result.contains("1\ta"));
        assert!(result.contains("2\tb"));
        assert!(!result.contains("3\tc"));
        assert!(result.contains("more lines not shown"));
    }

    #[tokio::test]
    async fn read_with_offset_and_limit() {
        let (dir, tool) = setup();
        write_test_file(&dir, "lines.txt", "a\nb\nc\nd\ne\n");

        let result = tool
            .execute(serde_json::json!({"path": "lines.txt", "offset": 2, "limit": 2}))
            .await
            .unwrap();

        assert!(!result.contains("1\ta"));
        assert!(result.contains("2\tb"));
        assert!(result.contains("3\tc"));
        assert!(!result.contains("4\td"));
    }

    #[tokio::test]
    async fn read_empty_file() {
        let (dir, tool) = setup();
        write_test_file(&dir, "empty.txt", "");

        let result = tool
            .execute(serde_json::json!({"path": "empty.txt"}))
            .await
            .unwrap();

        assert!(result.contains("file is empty"));
    }

    #[tokio::test]
    async fn read_nonexistent_file() {
        let (_dir, tool) = setup();

        let result = tool
            .execute(serde_json::json!({"path": "no_such_file.txt"}))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_missing_path_field() {
        let (_dir, tool) = setup();

        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn directory_traversal_blocked() {
        let (dir, tool) = setup();
        // Create a file outside the root
        write_test_file(&dir, "inside.txt", "ok");

        let result = tool
            .execute(serde_json::json!({"path": "../../etc/passwd"}))
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("outside the working directory") || err.contains("cannot resolve"),
            "expected traversal error, got: {err}"
        );
    }

    #[tokio::test]
    async fn long_lines_are_truncated() {
        let (dir, tool) = setup();
        let long_line = "x".repeat(3000);
        write_test_file(&dir, "long.txt", &long_line);

        let result = tool
            .execute(serde_json::json!({"path": "long.txt"}))
            .await
            .unwrap();

        assert!(result.contains("[truncated]"));
        // Output should be shorter than the original
        assert!(result.len() < 3000);
    }
}
