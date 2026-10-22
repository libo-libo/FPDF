# Architecture and implementation boundaries

fpdf is an independent Rust PDF reader. The egui interface uses public PDFium APIs for parsing, rendering and text geometry. It does not load WPS private libraries.

## Modules

- `src/app.rs`: toolbar, navigation, canvas and interaction.
- `src/reading.rs`: layout, 1/2/4/8 columns, navigation history and themes.
- `src/pdf.rs` / `src/worker.rs`: native handle lifetimes and serialized background PDF operations. Native handles stay on their owning thread.
- `src/outline.rs` / `src/outline_editor.rs`: bookmark changes, document identity checks and save operations.
- `src/ai_reader.rs`: streamed answers, persistent translation and cancellation.
- `src/search_ui.rs`: debounced whole-document search, virtualized right-side previews and temporary match overlays.
- `src/annotation.rs`: standards-compatible Highlight quads/appearance streams and identity-checked atomic persistence.
- `ai-runtime/bridge.mjs`: JSONL communication and pi agent integration, exposing only bounded PDF text reads.

## Reading and editing

Rendering runs on demand, with a backend cache limited to 16 items / 64 MiB. Existing textures remain visible while updated images load. Multi-column layout retains the current page.

Outline page numbers and star leaders are display elements. Editing writes bookmarks to the PDF after identity checks, using a temporary file and page-count validation. Encrypted and digitally signed files are excluded from outline editing.

Text selection uses the PDF text layer and character geometry, supporting words and multiple lines on one page. OCR is not included. AI configuration belongs to the user; the release embeds no author's endpoint or credentials. Cancelled requests cannot append stale chunks to a replacement answer.

Search uses PDFium's native text-find APIs on the same owning thread, yielding between pages so rendering, selection and cancellation remain responsive. Query/document generations reject stale results. Preview offsets identify the actual occurrence even when the context repeats the same word. Results are bounded to 2,000 per page / 20,000 per document. Temporary search overlays are separate from saved PDF annotations.

## Distribution

Application code is MIT licensed. Releases include independent PDFium, Node.js, pi and dependency notices. Proprietary programs, extracted assets, analysis tools, real AI configuration and tokens are excluded.
