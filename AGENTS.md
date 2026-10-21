# Repository Guidelines

## Project Structure & Module Organization

`fpdf` is a Windows desktop PDF reader written in Rust. `src/app.rs` contains the egui interface; `src/reading.rs` implements page layout, navigation history, and display themes. `src/pdf.rs` owns PDFium bindings and native resource lifetimes; `src/worker.rs` schedules background rendering. `src/main.rs` provides desktop and command-line entry points. Integration tests and original PDF samples live in `tests/`. Build helpers live in `scripts/`; user documentation is in `docs/`. `analysis/` contains research evidence and local tools, while `dist/fpdf/` contains packaged output.

## Build, Test, and Development Commands

In PowerShell, load the local toolchain with `. ./scripts/dev-env.ps1`. If execution policy blocks this, use `Set-ExecutionPolicy -Scope Process Bypass -Force` for the current process.

- `cargo run -- tests/fixtures/reading-demo.pdf`: launch the reader.
- `cargo fmt --check`: verify Rust formatting.
- `cargo clippy --locked --all-targets -- -D warnings`: check all targets.
- `cargo test --locked`: run layout and actual PDF integration tests.
- `./scripts/build.ps1`: test, build release output, and collect runtime licenses.

## Coding Style & Naming Conventions

Use rustfmt defaults and four-space indentation. Use `snake_case` for functions/modules and `PascalCase` for types. Keep native handles confined to the rendering thread. Put geometry and theme transformations in `reading.rs`; keep interface labels concise and in Chinese. Preserve document identity checks and rendering memory limits.

## Testing Guidelines

Use Rust's built-in test framework. No coverage percentage is prescribed. Add meaningful tests for changed geometry, navigation, or native resource behavior. Use descriptive test names. Interactive changes should also run `cargo run -- --smoke-ui tests/fixtures/reading-demo.pdf --screenshot window.png --features`; inspect the resulting screenshot. Regenerate original fixtures with `node scripts/make-fixtures.cjs`.

## Commit & Pull Request Guidelines

This workspace has no Git history establishing a commit convention. Use concise imperative messages describing the change. Pull requests should describe user-visible behavior, list validation commands, and include screenshots for interface changes. Link relevant issues when available.

## Runtime & Distribution

Use the independent PDFium runtime in `runtime/`. Preserve third-party notices when packaging. Do not distribute the WPS installer, extracted proprietary DLLs, or local build tools.
