//! Rule file loading and directory scoping.
//!
//! Loads Markdown rule files from `.sakamoto/rules/` and merges them into
//! a system prompt. Rules are scoped by directory — a rule file at
//! `.sakamoto/rules/backend/rust.md` only applies to stages working on
//! files under `backend/`.

use std::path::{Path, PathBuf};

/// A loaded rule file with its scope path.
#[derive(Debug, Clone)]
pub struct Rule {
    /// The path of the rule file relative to the rules directory.
    ///
    /// For `.sakamoto/rules/general.md` this would be `general.md`.
    /// For `.sakamoto/rules/backend/rust.md` this would be `backend/rust.md`.
    pub relative_path: PathBuf,

    /// The scope directory. Rules in the root of the rules directory have
    /// an empty scope (apply everywhere). Rules in subdirectories only
    /// apply to files under that subdirectory.
    ///
    /// For `backend/rust.md`, the scope is `backend`.
    pub scope: Option<PathBuf>,

    /// The content of the rule file.
    pub content: String,
}

impl Rule {
    /// Returns `true` if this rule applies to the given working path.
    ///
    /// Rules with no scope (in the rules root) always apply.
    /// Scoped rules apply when the working path starts with the scope.
    pub fn applies_to(&self, working_path: &Path) -> bool {
        match &self.scope {
            None => true,
            Some(scope) => working_path.starts_with(scope),
        }
    }
}

/// Load all rule files matching the given glob patterns, relative to the
/// project root directory.
///
/// Returns a sorted list of rules. Files that cannot be read are logged
/// and skipped.
pub fn load_rules(project_root: &Path, patterns: &[String]) -> Vec<Rule> {
    let mut rules = Vec::new();

    for pattern in patterns {
        let full_pattern = project_root.join(pattern);
        let full_pattern_str = full_pattern.display().to_string();

        let entries = match glob::glob(&full_pattern_str) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(pattern = %pattern, error = %e, "invalid glob pattern");
                continue;
            }
        };

        for entry in entries {
            let path = match entry {
                Ok(path) => path,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to expand glob entry");
                    continue;
                }
            };

            if !path.is_file() {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "failed to read rule file");
                    continue;
                }
            };

            // Determine the rules base directory from the glob pattern.
            // For ".sakamoto/rules/*.md", the base is ".sakamoto/rules".
            // For ".sakamoto/rules/**/*.md", the base is ".sakamoto/rules".
            let rules_dir = find_rules_base_dir(project_root, pattern);

            let relative = path.strip_prefix(&rules_dir).unwrap_or(&path).to_path_buf();

            let scope = relative.parent().and_then(|p| {
                if p.as_os_str().is_empty() {
                    None
                } else {
                    Some(p.to_path_buf())
                }
            });

            rules.push(Rule {
                relative_path: relative,
                scope,
                content,
            });
        }
    }

    // Sort by path for deterministic ordering
    rules.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    rules
}

/// Merge loaded rules into a system prompt string.
///
/// Global rules (no scope) are always included. Scoped rules are included
/// only if their scope matches the working directory.
///
/// Each rule is separated by a header showing its source path.
pub fn merge_rules_to_prompt(rules: &[Rule], working_dir: Option<&Path>) -> Option<String> {
    let applicable: Vec<&Rule> = rules
        .iter()
        .filter(|r| match working_dir {
            Some(dir) => r.applies_to(dir),
            None => r.scope.is_none(), // No working dir → only global rules
        })
        .collect();

    if applicable.is_empty() {
        return None;
    }

    let mut prompt = String::new();

    for rule in &applicable {
        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(&format!(
            "# Rules: {}\n\n{}",
            rule.relative_path.display(),
            rule.content.trim()
        ));
    }

    Some(prompt)
}

/// Find the base directory of the rules from a glob pattern.
///
/// Strips glob characters to find the deepest concrete directory.
/// For `.sakamoto/rules/*.md` → `.sakamoto/rules`
/// For `.sakamoto/rules/**/*.md` → `.sakamoto/rules`
fn find_rules_base_dir(project_root: &Path, pattern: &str) -> PathBuf {
    let path = Path::new(pattern);
    let mut base = project_root.to_path_buf();

    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if s.contains('*') || s.contains('?') || s.contains('[') {
            break;
        }
        base.push(component);
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_global_applies_to_everything() {
        let rule = Rule {
            relative_path: PathBuf::from("general.md"),
            scope: None,
            content: "Be helpful.".into(),
        };
        assert!(rule.applies_to(Path::new("src")));
        assert!(rule.applies_to(Path::new("backend/api")));
        assert!(rule.applies_to(Path::new("")));
    }

    #[test]
    fn rule_scoped_applies_to_matching_path() {
        let rule = Rule {
            relative_path: PathBuf::from("backend/rust.md"),
            scope: Some(PathBuf::from("backend")),
            content: "Use idiomatic Rust.".into(),
        };
        assert!(rule.applies_to(Path::new("backend")));
        assert!(rule.applies_to(Path::new("backend/api")));
        assert!(!rule.applies_to(Path::new("frontend")));
        assert!(!rule.applies_to(Path::new("src")));
    }

    #[test]
    fn rule_deeply_scoped() {
        let rule = Rule {
            relative_path: PathBuf::from("backend/api/rest.md"),
            scope: Some(PathBuf::from("backend/api")),
            content: "Use REST conventions.".into(),
        };
        assert!(rule.applies_to(Path::new("backend/api")));
        assert!(rule.applies_to(Path::new("backend/api/v1")));
        assert!(!rule.applies_to(Path::new("backend")));
        assert!(!rule.applies_to(Path::new("backend/grpc")));
    }

    #[test]
    fn load_rules_from_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        let rules_dir = dir.path().join(".sakamoto").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        std::fs::write(rules_dir.join("general.md"), "Be concise.").unwrap();
        std::fs::write(rules_dir.join("coding.md"), "Write tests.").unwrap();

        let rules = load_rules(dir.path(), &[".sakamoto/rules/*.md".into()]);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].relative_path, PathBuf::from("coding.md"));
        assert_eq!(rules[1].relative_path, PathBuf::from("general.md"));
        assert!(rules[0].scope.is_none());
        assert!(rules[1].scope.is_none());
    }

    #[test]
    fn load_rules_with_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let rules_dir = dir.path().join(".sakamoto").join("rules");
        let backend_dir = rules_dir.join("backend");
        std::fs::create_dir_all(&backend_dir).unwrap();

        std::fs::write(rules_dir.join("general.md"), "Be helpful.").unwrap();
        std::fs::write(backend_dir.join("rust.md"), "Use Result.").unwrap();

        let rules = load_rules(dir.path(), &[".sakamoto/rules/**/*.md".into()]);
        assert_eq!(rules.len(), 2);

        // Sorted: backend/rust.md comes before general.md
        assert_eq!(rules[0].relative_path, PathBuf::from("backend/rust.md"));
        assert_eq!(rules[0].scope, Some(PathBuf::from("backend")));
        assert_eq!(rules[1].relative_path, PathBuf::from("general.md"));
        assert!(rules[1].scope.is_none());
    }

    #[test]
    fn load_rules_nonexistent_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let rules = load_rules(dir.path(), &["nonexistent/**/*.md".into()]);
        assert!(rules.is_empty());
    }

    #[test]
    fn merge_rules_global_only() {
        let rules = vec![
            Rule {
                relative_path: PathBuf::from("a.md"),
                scope: None,
                content: "Rule A.".into(),
            },
            Rule {
                relative_path: PathBuf::from("b.md"),
                scope: None,
                content: "Rule B.".into(),
            },
        ];

        let prompt = merge_rules_to_prompt(&rules, None).unwrap();
        assert!(prompt.contains("# Rules: a.md"));
        assert!(prompt.contains("Rule A."));
        assert!(prompt.contains("# Rules: b.md"));
        assert!(prompt.contains("Rule B."));
    }

    #[test]
    fn merge_rules_filters_by_scope() {
        let rules = vec![
            Rule {
                relative_path: PathBuf::from("backend/rust.md"),
                scope: Some(PathBuf::from("backend")),
                content: "Rust rules.".into(),
            },
            Rule {
                relative_path: PathBuf::from("general.md"),
                scope: None,
                content: "General rules.".into(),
            },
        ];

        // Working in backend → both rules apply
        let prompt = merge_rules_to_prompt(&rules, Some(Path::new("backend"))).unwrap();
        assert!(prompt.contains("Rust rules."));
        assert!(prompt.contains("General rules."));

        // Working in frontend → only general
        let prompt = merge_rules_to_prompt(&rules, Some(Path::new("frontend"))).unwrap();
        assert!(!prompt.contains("Rust rules."));
        assert!(prompt.contains("General rules."));
    }

    #[test]
    fn merge_rules_empty_returns_none() {
        let result = merge_rules_to_prompt(&[], None);
        assert!(result.is_none());
    }

    #[test]
    fn merge_rules_no_matching_scope_returns_none() {
        let rules = vec![Rule {
            relative_path: PathBuf::from("backend/rust.md"),
            scope: Some(PathBuf::from("backend")),
            content: "Rust only.".into(),
        }];

        // No working dir, only global rules included → none match
        let result = merge_rules_to_prompt(&rules, None);
        assert!(result.is_none());
    }

    #[test]
    fn find_rules_base_dir_simple() {
        let root = Path::new("/project");
        let base = find_rules_base_dir(root, ".sakamoto/rules/*.md");
        assert_eq!(base, PathBuf::from("/project/.sakamoto/rules"));
    }

    #[test]
    fn find_rules_base_dir_recursive() {
        let root = Path::new("/project");
        let base = find_rules_base_dir(root, ".sakamoto/rules/**/*.md");
        assert_eq!(base, PathBuf::from("/project/.sakamoto/rules"));
    }
}
