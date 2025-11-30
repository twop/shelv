# AGENTS.md

Quick reference for AI coding agents working in this repository.

## Build/Test Commands
- `cargo run` - Run the application
- `cargo check` - Check code for errors (preferred for development)
- `cargo test` - Run all tests
- `cargo test test_name` - Run a single test by name (e.g., `cargo test toggle_heading`)
- `cargo test --package shelv` - Run tests for main package only

## Code Style
- **Edition**: Rust 2024 (see rust-toolchain.toml)
- **Imports**: Group std, external crates, then crate modules; use alphabetical order within groups
- **Naming**: snake_case for functions/variables, PascalCase for types/enums, SCREAMING_SNAKE for constants
- **Types**: Prefer explicit types in function signatures; use type inference for locals
- **Error Handling**: Use `Result` for fallible operations; `.unwrap()` is acceptable for infallible operations or tests
- **Formatting**: Standard rustfmt conventions
- **Documentation**: Add doc comments for public APIs; inline comments for complex logic

## Architecture Notes
- Command-action pattern: user inputs → commands → actions → state mutations
- Central state in `app_state.rs`, UI rendering in `app_ui.rs`, I/O in `app_io.rs`
- Text processing uses `text_structure.rs` for markdown-like content parsing
