# Rust Guidelines

- Use idiomatic Rust: prefer `Result` over panics, iterators over manual loops.
- No `unwrap()` or `expect()` in library crates. Use `?` and proper error types.
- Use `thiserror` for library error types and `anyhow` only in binary crates.
- Derive `Debug` on all public types.
- Prefer `&str` over `String` in function parameters when ownership is not needed.
- Run `cargo clippy` and `cargo fmt` before committing.
