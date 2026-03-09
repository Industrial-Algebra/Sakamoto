# sakamoto-context

Context reference parsing for the Sakamoto orchestrator.

Extracts structured references from free-text task descriptions, enabling context pre-hydration before LLM calls. This reduces token waste and improves first-shot quality.

## Parsers

All parsers return `Vec<ContextRef>` and deduplicate results.

```rust
use sakamoto_context::parser;

let refs = parser::parse_all("Fix src/main.rs and see owner/repo#42");
// → [FilePath { path: "src/main.rs" }, GitHubIssue { owner: "owner", repo: "repo", number: 42 }]
```

### File Paths

Recognizes relative and absolute paths with common source file extensions (`.rs`, `.ts`, `.py`, `.go`, `.js`, `.toml`, `.yaml`, `.json`, `.md`, etc.).

```rust
parser::parse_file_paths("check src/lib.rs and config/settings.toml");
```

### GitHub References

Parses both shorthand (`owner/repo#42`) and full URLs (`https://github.com/owner/repo/issues/42`, `.../pull/42`). Distinguishes issues from PRs.

```rust
parser::parse_github_refs("see Industrial-Algebra/Sakamoto#7");
```

### URLs

Extracts `http://` and `https://` URLs, stripping trailing punctuation. GitHub issue/PR URLs are excluded (handled by the GitHub parser).

```rust
parser::parse_urls("docs at https://docs.rs/tokio");
```

### Symbols

Identifies PascalCase type names and qualified paths (`std::collections::HashMap`). Filters common English words that happen to be PascalCase.

```rust
parser::parse_symbols("refactor the ContextBundle type");
```

## License

MIT
