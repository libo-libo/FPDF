# Validation

## v0.8.0 local repair — 2026-10-22

- Format and all-target Clippy checks passed; 28 Rust tests and 2 AI bridge tests passed.
- Whole-document native search covers case-sensitive / whole-word matches, rotated hit geometry, progress, cancellation and replacement queries. Repeated-word and Unicode/multiline snippets retain the exact hit offset.
- Standard Highlight annotations survive save/reopen, render visibly, preserve text and existing annotations, and retain coordinates across viewer rotations. Encrypted, signed and externally changed PDFs are rejected. Failed writes preserve the original.
- Actual GUI checks verify Ctrl+F, right-side snippets, result clicks, next result, full-word setting, saved highlights/cache reload and direct question input before any translation.
- Reading/zoom regression and live pi one-shot / persistent translation regression passed. Questions remain available during translation; sending a question replaces the current task and retains the long-open preference.
- WPS alignment is supported by REA resource evidence and focused offline metadata/wrapper analysis: Ctrl+F, result-list model, progress, whole-word/case options and Ctrl+F2 highlight action. No full native decompilation or WPS runtime panel placement is claimed; right-side placement follows the user's request.
- Windows PE resources contain the multi-size P icon and 0.8.0 version metadata. Desktop/Start menu shortcuts select the embedded icon and share `libolibo.fpdf` with the application.
- Isolated installer validation used a separate AppId and user-data directory. Actual GUI entry saved a new endpoint/key, connected to an authenticated local mock API and completed a cited question without translation; the API editor remained usable while busy.
- One-click uninstall closed only the installed reader and bundled Node process. Configuration, caches, logs and runtime files were removed; user PDFs, an external junction target and a separate portable process survived. The original 0.7.0 registry and 112 installed file hashes were unchanged.
- The six-page Chinese manual was regenerated and visually checked. Installer/ZIP payloads and output files passed privacy checks for the local API endpoint, key and GitHub token.

## v0.7.0 release

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
