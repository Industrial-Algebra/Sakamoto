//! Context reference parsers — pure functions that extract
//! [`ContextRef`] variants from task description text.
//!
//! All parsers are Wasm-safe (no I/O). They operate on string slices
//! and return vectors of `ContextRef` from `sakamoto-types`.

use regex::Regex;
use sakamoto_types::ContextRef;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Compiled regexes (compiled once, reused across calls)
// ---------------------------------------------------------------------------

static FILE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches absolute, relative, or bare file paths ending in a known extension.
    Regex::new(
        r#"(?:^|[\s,;({\[`"'])(?P<path>(?:[./]|\.\./|~/)?[\w./-]+\.(?:rs|toml|json|yaml|yml|md|txt|lock|ts|js|py|go|c|h|cpp|hpp|java|kt|rb|sh|bash|zsh|css|html|xml|sql|proto|graphql|wasm|wat))\b"#
    )
    .expect("file path regex is valid")
});

static GITHUB_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"https://github\.com/(?P<owner>[A-Za-z0-9._-]+)/(?P<repo>[A-Za-z0-9._-]+)/(?P<kind>issues|pull)/(?P<number>\d+)"
    )
    .expect("github url regex is valid")
});

static GITHUB_SHORTHAND_RE: LazyLock<Regex> = LazyLock::new(|| {
    // owner/repo#123
    Regex::new(
        r"(?:^|[\s,({\[`])(?P<owner>[A-Za-z0-9._-]+)/(?P<repo>[A-Za-z0-9._-]+)#(?P<number>\d+)",
    )
    .expect("github shorthand regex is valid")
});

static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://[^\s)\]}>`,]+").expect("url regex is valid"));

static SYMBOL_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Rust-style qualified paths: `foo::bar::Baz` (at least one `::`)
    Regex::new(r"(?:^|[\s,;({\[`])(?P<sym>[a-zA-Z_]\w*(?:::[a-zA-Z_]\w*)+)")
        .expect("symbol path regex is valid")
});

static PASCAL_CASE_RE: LazyLock<Regex> = LazyLock::new(|| {
    // PascalCase identifiers with at least two "humps".
    Regex::new(r"(?:^|[\s,;({\[`])(?P<sym>[A-Z][a-z]+(?:[A-Z][a-z0-9]*)+)")
        .expect("pascal case regex is valid")
});

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse file path references from text.
///
/// Matches patterns like `src/main.rs`, `/home/user/file.rs`, `./relative/path.rs`.
pub fn parse_file_paths(text: &str) -> Vec<ContextRef> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for caps in FILE_PATH_RE.captures_iter(text) {
        if let Some(m) = caps.name("path") {
            let path_str = m.as_str();
            if seen.insert(path_str.to_string()) {
                results.push(ContextRef::FilePath {
                    path: PathBuf::from(path_str),
                });
            }
        }
    }

    results
}

/// Parse GitHub issue/PR references from text.
///
/// Matches:
/// - `owner/repo#123` (shorthand — parsed as issue since kind is ambiguous)
/// - `https://github.com/owner/repo/issues/123`
/// - `https://github.com/owner/repo/pull/456`
pub fn parse_github_refs(text: &str) -> Vec<ContextRef> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    // Full GitHub URLs — we know the kind from the URL path
    for caps in GITHUB_URL_RE.captures_iter(text) {
        let owner = caps["owner"].to_string();
        let repo = caps["repo"].to_string();
        let number: u64 = caps["number"].parse().unwrap_or(0);
        let kind = &caps["kind"];
        let key = (owner.clone(), repo.clone(), number);
        if number > 0 && seen.insert(key) {
            match kind {
                "pull" => results.push(ContextRef::GitHubPr {
                    owner,
                    repo,
                    number,
                }),
                _ => results.push(ContextRef::GitHubIssue {
                    owner,
                    repo,
                    number,
                }),
            }
        }
    }

    // Shorthand owner/repo#123 — default to issue (kind is ambiguous)
    for caps in GITHUB_SHORTHAND_RE.captures_iter(text) {
        let owner = caps["owner"].to_string();
        let repo = caps["repo"].to_string();
        let number: u64 = caps["number"].parse().unwrap_or(0);
        let key = (owner.clone(), repo.clone(), number);
        if number > 0 && seen.insert(key) {
            results.push(ContextRef::GitHubIssue {
                owner,
                repo,
                number,
            });
        }
    }

    results
}

/// Parse generic URL references from text, excluding GitHub issue/PR URLs
/// (which are handled by [`parse_github_refs`]).
pub fn parse_urls(text: &str) -> Vec<ContextRef> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for m in URL_RE.find_iter(text) {
        let url = m.as_str();
        // Strip trailing punctuation that may have been captured
        let url = url.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let url = url.to_string();

        // Skip GitHub issue/PR URLs — those are captured by parse_github_refs
        if GITHUB_URL_RE.is_match(&url) {
            continue;
        }

        if seen.insert(url.clone()) {
            results.push(ContextRef::Url { url });
        }
    }

    results
}

/// Parse Rust-style symbol references from text.
///
/// Matches:
/// - Qualified paths like `std::collections::HashMap`
/// - PascalCase type names like `ContextRef`, `HashMap`
pub fn parse_symbols(text: &str) -> Vec<ContextRef> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    // Qualified paths (foo::bar::Baz)
    for caps in SYMBOL_PATH_RE.captures_iter(text) {
        if let Some(m) = caps.name("sym") {
            let sym = m.as_str().to_string();
            if seen.insert(sym.clone()) {
                results.push(ContextRef::Symbol { name: sym });
            }
        }
    }

    // PascalCase identifiers
    for caps in PASCAL_CASE_RE.captures_iter(text) {
        if let Some(m) = caps.name("sym") {
            let sym = m.as_str().to_string();
            // Skip common English words that happen to be PascalCase
            if !is_common_word(&sym) && seen.insert(sym.clone()) {
                results.push(ContextRef::Symbol { name: sym });
            }
        }
    }

    results
}

/// Run all parsers on the text and return deduplicated results.
pub fn parse_all(text: &str) -> Vec<ContextRef> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();

    let all_refs = [
        parse_file_paths(text),
        parse_github_refs(text),
        parse_urls(text),
        parse_symbols(text),
    ];

    for refs in all_refs {
        for r in refs {
            if seen.insert(r.clone()) {
                results.push(r);
            }
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Common English words that look like PascalCase but aren't symbols.
fn is_common_word(s: &str) -> bool {
    matches!(
        s,
        "The"
            | "This"
            | "That"
            | "These"
            | "Those"
            | "When"
            | "Where"
            | "Which"
            | "While"
            | "Before"
            | "After"
            | "Because"
            | "However"
            | "Also"
            | "Each"
            | "Every"
            | "Some"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_file_paths tests --

    #[test]
    fn file_path_relative() {
        let refs = parse_file_paths("Look at src/main.rs for details");
        assert_eq!(
            refs,
            vec![ContextRef::FilePath {
                path: PathBuf::from("src/main.rs")
            }]
        );
    }

    #[test]
    fn file_path_absolute() {
        let refs = parse_file_paths("Check /home/user/project/lib.rs");
        assert_eq!(
            refs,
            vec![ContextRef::FilePath {
                path: PathBuf::from("/home/user/project/lib.rs")
            }]
        );
    }

    #[test]
    fn file_path_dot_relative() {
        let refs = parse_file_paths("Edit ./src/parser.rs please");
        assert_eq!(
            refs,
            vec![ContextRef::FilePath {
                path: PathBuf::from("./src/parser.rs")
            }]
        );
    }

    #[test]
    fn file_path_multiple() {
        let refs = parse_file_paths("Compare src/lib.rs and src/main.rs");
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&ContextRef::FilePath {
            path: PathBuf::from("src/lib.rs")
        }));
        assert!(refs.contains(&ContextRef::FilePath {
            path: PathBuf::from("src/main.rs")
        }));
    }

    #[test]
    fn file_path_deduplication() {
        let refs = parse_file_paths("src/lib.rs and again src/lib.rs");
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn file_path_unknown_extension_ignored() {
        let refs = parse_file_paths("Look at file.xyz123");
        assert!(refs.is_empty());
    }

    #[test]
    fn file_path_toml() {
        let refs = parse_file_paths("Edit Cargo.toml");
        assert_eq!(
            refs,
            vec![ContextRef::FilePath {
                path: PathBuf::from("Cargo.toml")
            }]
        );
    }

    #[test]
    fn file_path_nested() {
        let refs = parse_file_paths("in crates/sakamoto-context/src/parser.rs");
        assert_eq!(
            refs,
            vec![ContextRef::FilePath {
                path: PathBuf::from("crates/sakamoto-context/src/parser.rs")
            }]
        );
    }

    // -- parse_github_refs tests --

    #[test]
    fn github_shorthand() {
        let refs = parse_github_refs("Fix Industrial-Algebra/Sakamoto#42");
        assert_eq!(
            refs,
            vec![ContextRef::GitHubIssue {
                owner: "Industrial-Algebra".into(),
                repo: "Sakamoto".into(),
                number: 42,
            }]
        );
    }

    #[test]
    fn github_issue_url() {
        let refs =
            parse_github_refs("See https://github.com/rust-lang/rust/issues/12345 for context");
        assert_eq!(
            refs,
            vec![ContextRef::GitHubIssue {
                owner: "rust-lang".into(),
                repo: "rust".into(),
                number: 12345,
            }]
        );
    }

    #[test]
    fn github_pull_url() {
        let refs = parse_github_refs("Review https://github.com/tokio-rs/tokio/pull/789");
        assert_eq!(
            refs,
            vec![ContextRef::GitHubPr {
                owner: "tokio-rs".into(),
                repo: "tokio".into(),
                number: 789,
            }]
        );
    }

    #[test]
    fn github_multiple_mixed() {
        let text = "Fix owner/repo#1 and see https://github.com/a/b/issues/2";
        let refs = parse_github_refs(text);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn github_deduplication_url_and_shorthand() {
        // Same ref in both URL and shorthand form
        let text = "https://github.com/a/b/issues/1 and a/b#1";
        let refs = parse_github_refs(text);
        // URL is parsed first, shorthand deduped
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn github_no_false_positive_on_plain_slash() {
        let refs = parse_github_refs("Use src/main.rs");
        assert!(refs.is_empty());
    }

    // -- parse_urls tests --

    #[test]
    fn url_basic_https() {
        let refs = parse_urls("Visit https://docs.rs/tokio/latest for docs");
        assert_eq!(
            refs,
            vec![ContextRef::Url {
                url: "https://docs.rs/tokio/latest".into()
            }]
        );
    }

    #[test]
    fn url_http() {
        let refs = parse_urls("Check http://example.com/page");
        assert_eq!(
            refs,
            vec![ContextRef::Url {
                url: "http://example.com/page".into()
            }]
        );
    }

    #[test]
    fn url_excludes_github_issues() {
        let refs = parse_urls("See https://github.com/a/b/issues/1 and https://example.com");
        assert_eq!(
            refs,
            vec![ContextRef::Url {
                url: "https://example.com".into()
            }]
        );
    }

    #[test]
    fn url_excludes_github_pulls() {
        let refs = parse_urls("See https://github.com/a/b/pull/1");
        assert!(refs.is_empty());
    }

    #[test]
    fn url_deduplication() {
        let refs = parse_urls("https://example.com and https://example.com");
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn url_strips_trailing_punctuation() {
        let refs = parse_urls("Visit https://example.com/page.");
        assert_eq!(
            refs,
            vec![ContextRef::Url {
                url: "https://example.com/page".into()
            }]
        );
    }

    // -- parse_symbols tests --

    #[test]
    fn symbol_qualified_path() {
        let refs = parse_symbols("Use std::collections::HashMap for this");
        assert_eq!(
            refs,
            vec![ContextRef::Symbol {
                name: "std::collections::HashMap".into()
            }]
        );
    }

    #[test]
    fn symbol_pascal_case() {
        let refs = parse_symbols("The ContextRef type is defined here");
        assert_eq!(
            refs,
            vec![ContextRef::Symbol {
                name: "ContextRef".into()
            }]
        );
    }

    #[test]
    fn symbol_multiple_pascal() {
        let refs = parse_symbols("Both ContextRef and ContextBundle are needed");
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&ContextRef::Symbol {
            name: "ContextRef".into()
        }));
        assert!(refs.contains(&ContextRef::Symbol {
            name: "ContextBundle".into()
        }));
    }

    #[test]
    fn symbol_excludes_common_words() {
        let refs = parse_symbols("Before and After are not symbols");
        assert!(refs.is_empty());
    }

    #[test]
    fn symbol_deduplication() {
        let refs = parse_symbols("ContextRef and ContextRef again");
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn symbol_two_segment_path() {
        let refs = parse_symbols("Call context::parse_all");
        assert_eq!(
            refs,
            vec![ContextRef::Symbol {
                name: "context::parse_all".into()
            }]
        );
    }

    // -- parse_all tests --

    #[test]
    fn parse_all_combines_all_parsers() {
        let text = "Fix src/main.rs, see Industrial-Algebra/Sakamoto#42, \
                     check https://docs.rs/tokio and use std::io::Result";
        let refs = parse_all(text);

        assert!(
            refs.iter()
                .any(|r| matches!(r, ContextRef::FilePath { .. }))
        );
        assert!(
            refs.iter()
                .any(|r| matches!(r, ContextRef::GitHubIssue { .. }))
        );
        assert!(refs.iter().any(|r| matches!(r, ContextRef::Url { .. })));
        assert!(refs.iter().any(|r| matches!(r, ContextRef::Symbol { .. })));
    }

    #[test]
    fn parse_all_deduplicates() {
        let text = "src/main.rs and src/main.rs";
        let refs = parse_all(text);
        let file_refs: Vec<_> = refs
            .iter()
            .filter(|r| matches!(r, ContextRef::FilePath { .. }))
            .collect();
        assert_eq!(file_refs.len(), 1);
    }

    #[test]
    fn parse_all_empty_input() {
        let refs = parse_all("");
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_all_no_refs() {
        let refs = parse_all("Just a plain sentence with nothing special.");
        assert!(refs.is_empty());
    }
}
