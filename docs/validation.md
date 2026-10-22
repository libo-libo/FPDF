# Validation

## v0.8.5 underlines and embedded directory links — 2026-10-22

- Formatting, all-target Clippy, 37 Rust tests and three AI bridge tests passed. Real PDF coverage adds direct/named internal targets, malformed/external target exclusion, four rotated link rectangles/destination positions, saved Underline subtype/quads, baseline pixel visibility and preservation of existing links.
- Native GUI automation clicks both embedded directory entries, uses the Back button and Alt+Left/Right, and asserts restoration of the original scroll offset. Rapid return-and-click is covered.
- Underline GUI coverage includes selecting a word first, continuous multiline dragging, an independently retained RGB palette, undo/redo, actual saved annotation colors and reopening. Highlighter, tabs, search/AI coexistence, reading and the general navigation/zoom workflow passed again.
- Isolated v0.8.5 installation ran the same directory/underline GUI workflow; all 110 payload hashes matched. Confirming uninstall removed owned files and preserved a user PDF. All six manual pages were rendered and visually reviewed.
- Internal navigation relies on embedded same-document PDF links. Unlinked plain/scanned directory text is not inferred.

## v0.8.4 unified repair — 2026-10-22

- Format and all-target Clippy passed; 37 Rust tests pass, including actual PDF multi-client rendering/search/save isolation, global cache eviction, partial close, last-client runtime release and restart.
- Native tab smoke clicks real labels and close buttons and sends Ctrl+T/W/Tab/Shift+Tab; checks independent page/zoom/rotation/search/AI state, highlight undo after switching while a deliberately slow save is pending, and close/reopen.
- Cached hit testing agrees with native text selection in four rotations. Native highlighter smoke deliberately delays persistence for three seconds while continuous selection, immediate ink, undo/redo and unchanged base texture/document IDs are verified. CPU paint submission is not monitor presentation latency.
- Text editors retain Ctrl+A/C/X/V/Z/Y, Ctrl+Shift+Z and Ctrl+Home/End. Historical v0.8.3 reload behavior below is superseded by independent persistence and a stable UI annotation layer.
- WPS resource and native-wrapper evidence supports in-memory annotation edits, undo/redo actions and tab/subpage concepts. Exact WPS runtime timing, visual tab ordering and complete save implementation remain unobserved.
- Whole-document search/AI coexistence, independent windows, continuous reading, RGB palettes and uninstall cancellation passed again after tabs were integrated. All three AI bridge tests passed.
- The installed v0.8.4 ran the real tab/undo workflow; all 110 installed payload hashes matched distribution. Uninstall removed owned files/settings and preserved a user PDF. The six-page manual was rendered and visually checked; source, installer, PDF and ZIP passed private endpoint/key/token scanning.

## v0.8.3 highlight interaction repair — 2026-10-22

- Format/Clippy passed; 31 Rust tests and 3 AI bridge tests passed. Actual saved annotation pixels have visible contrast inside the selected text region in white, night, green, warm and dark custom RGB themes.
- Native GUI automation exercises select-first and tool-first highlighter input, multiline drag, continued tool state, Escape/button exit, save/reopen, palette clicks, RGB annotation values and cancelling a changed color. The screenshot `fpdf-highlight-preview.png` shows the actual night interface and palette.
- Queued highlight ink is painted in the same frame before persistence finishes; original page textures remain visible until replacement images arrive. This measures UI paint submission, not monitor presentation or disk completion time.
- Search/AI coexistence, independent closes, compact layout, multiple PDF windows, uninstall cancellation, theme RGB controls, scrollbar drag and continuous reading regressions passed.
- WPS comparison reuses command/color/quad evidence and a new REA PE inventory. Exact WPS visual timing/blending is unobserved; the immediate preview and texture replacement are independent implementations.
- Six-page manual updated and visually reviewed. The release packaging retains the author API/key/token exclusion checks.
- Isolated installed v0.8.3 passed the full highlighter/palette GUI workflow, all 110 installed file hashes matched distribution, uninstall preserved a user PDF and removed owned settings/logs. The source, installer, manual and ZIP passed private endpoint/key/token scanning.

## v0.8.2 unified release — 2026-10-22

- Rust formatting and all-target Clippy passed. The suite contains 30 Rust tests, including actual modal button clicks for cancel and confirm.
- Native GUI validation opens whole-document search and AI together, independently closes each pane, retains active AI state while navigating matches, and checks usable non-overlapping controls at 900 × 680.
- Two separate reader processes simultaneously opened the original 3-page and 16-page fixtures using the production new-window command; both rendered and exited successfully.
- The reader's uninstall action now requires explicit confirmation; cancel and Escape return to reading. The Start menu uninstall shortcut opens the standard installer confirmation instead of supplying silent arguments.
- All 3 AI bridge tests passed. Isolated v0.8.2 installation matched 110 payload hashes, rendered a real PDF, and removed owned settings/logs while preserving a user PDF on uninstall.
- The six-page Chinese guide was rendered and visually checked. The installer, PDF, ZIP contents and source passed private endpoint/key/token checks; the portable ZIP matched all 110 distribution files.
- This release incorporates the withdrawn v0.8.0 and v0.8.1 fixes. Their Git tags and source history remain available; their GitHub Releases were withdrawn before this unified publication.

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

Fixture and local validation do not establish compatibility with every complex PDF. There is no OCR or cross-platform support. The installer is unsigned.
