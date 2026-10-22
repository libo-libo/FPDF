# Architecture and implementation boundaries

fpdf is an independent Rust PDF reader. The egui interface uses public PDFium APIs for parsing, rendering and text geometry. It does not load WPS private libraries.

## Modules

- `src/app.rs`: toolbar, navigation, canvas and interaction.
- `src/app/tabs.rs`: window-level document tabs, per-tab reader/egui memory, close guards and native tab interaction smoke.
- `src/keyboard.rs`: editor-focus routing for common Ctrl shortcuts.
- `src/reading.rs`: layout, 1/2/4/8 columns, navigation history and themes.
- `src/pdf.rs` / `src/worker.rs`: native handle lifetimes and serialized background PDF operations. Native handles stay on their owning thread.
- `src/outline.rs` / `src/outline_editor.rs`: bookmark changes, document identity checks and save operations.
- `src/ai_reader.rs`: streamed answers, persistent translation and cancellation.
- `src/search_ui.rs`: debounced whole-document search, virtualized right-side previews and temporary match overlays.
- `src/annotation.rs`: standards-compatible Highlight quads/appearance streams and identity-checked atomic persistence.
- `ai-runtime/bridge.mjs`: JSONL communication and pi agent integration, exposing only bounded PDF text reads.

## Reading and editing

Rendering runs on demand. All document clients share one native owner thread and independent document/session channels. Each cache has a 16-item / 64-MiB limit; a global LRU additionally caps aggregate native image/geometry caches at 64 MiB. Inactive tabs retain only their current UI page texture. Existing textures remain visible while updated images load. Multi-column layout retains the current page.

Outline page numbers and star leaders are display elements. Editing writes bookmarks to the PDF after identity checks, using a temporary file and page-count validation. Encrypted and digitally signed files are excluded from outline editing.

Text selection uses the PDF text layer and character geometry, supporting words and multiple lines on one page. OCR is not included. AI configuration belongs to the user; the release embeds no author's endpoint or credentials. Cancelled requests cannot append stale chunks to a replacement answer.

Search uses PDFium's native text-find APIs on the same owning thread, yielding between pages so rendering, selection and cancellation remain responsive. Query/document generations reject stale results. Preview offsets identify the actual occurrence even when the context repeats the same word. Results are bounded to 2,000 per page / 20,000 per document. Temporary search overlays are separate from saved PDF annotations.

Tabs retain separate reading, search, AI, selection, undo and egui editor/scroll memory. Switching does not close a reader or interrupt saving. Closing a tab or window checks every affected document for pending saves/dirty outline edits. Ctrl+T/W/Tab/Shift+Tab manage tabs; Ctrl+N and the independent-window open action remain available. WPS resource/native-subpage evidence supports the comparison; exported names alone are not treated as working runtime behavior.

## Distribution

Highlighter input has two entry paths: apply to an existing selection or keep the tool active for subsequent committed selections. Numeric character geometry accompanies cached page images, allowing word/multiline hit testing in the input frame. A UI annotation/history layer paints synchronously over stable base textures. A separate pure-Rust writer serializes the latest history snapshot, coalesces obsolete queued saves and checks source identity before atomic replacement. Editing never reloads PDFium or invalidates the base texture. Undo/redo updates the layer immediately; failed saves restore the last persisted snapshot. Reopening starts a new history over the saved PDF. Dark themes preserve colored content rather than independently inverting RGB channels. The existing RGB palette also supplies annotation colors.

WPS alignment uses shipped highlight commands, Ctrl+F2, color/more-colors resources and quad interfaces documented in local REA research. Static evidence does not establish exact WPS timing, color blending or the complete native save implementation; fpdf's preview/cache strategy is independent.

Application code is MIT licensed. Releases include independent PDFium, Node.js, pi and dependency notices. Proprietary programs, extracted assets, analysis tools, real AI configuration and tokens are excluded.
