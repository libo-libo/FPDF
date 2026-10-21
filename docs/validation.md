# v0.7.0 validation

Validated on Windows x64 using Rust GNU, independent PDFium 158.0.8111.0, Node.js 24 and pi 1.3.0.

## Automated checks

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: 18 passed, including actual PDF rendering and native resource lifecycle, bookmark saving, layout, themes and translation cancellation.
- `npm.cmd test --prefix ai-runtime`: 2 passed, including an authenticated mock API, PDF tool requests and streaming answers.

## Interactive checks

Original PDF fixtures exercise outline navigation and folding, inline renaming, save/reopen, zoom, continuous reading and 2/4/8 columns. Live service validation covers pi questions, page citations, word/multiline selection, single-shot translation, mutually exclusive persistent switches and progress indication.

Published screenshots use the actual night-mode interface and contain no service address or account information. The Chinese guide has six A4 pages; each page was rendered and visually checked, including download instructions, tables and night-mode examples.

The installer was silently installed into an isolated validation directory. All 110 installed payload files matched the portable ZIP and package directory. The installed reader rendered a real PDF successfully; uninstall returned success. Publication files passed checks for the local endpoint, API key and GitHub token, including UTF-16 binary strings, extracted PDF text and ZIP contents.

## Limits

Fixture and local validation do not establish compatibility with every complex PDF. There is no OCR, cross-platform support or whole-document search. The installer is unsigned.
