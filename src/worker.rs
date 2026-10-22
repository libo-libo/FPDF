use crate::pdf::{
    DocumentInfo, PageImage, PageText, PdfEngine, PdfError, PdfSelection, SearchMatch, library_path,
};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

#[derive(Clone)]
pub struct RenderRequest {
    pub id: u64,
    pub document_id: u64,
    pub path: PathBuf,
    pub password: String,
    pub page: u32,
    pub width: u32,
    pub rotation: u8,
}
pub struct RenderResponse {
    pub id: u64,
    pub pages: u32,
    pub result: Result<Arc<PageImage>, PdfError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderTarget {
    pub page: u32,
    pub width: u32,
    pub rotation: u8,
    pub thumbnail: bool,
}
#[derive(Clone)]
pub struct ViewportRequest {
    pub id: u64,
    pub document_id: u64,
    pub path: PathBuf,
    pub password: String,
    /// Visible full-size pages first, followed by visible navigation thumbnails.
    pub targets: Vec<RenderTarget>,
}
pub struct ViewportResponse {
    pub id: u64,
    pub document_id: u64,
    pub target: RenderTarget,
    pub info: Option<Arc<DocumentInfo>>,
    pub result: Result<Arc<PageImage>, PdfError>,
}

pub struct TextRequest {
    pub id: u64,
    pub document_id: u64,
    pub path: PathBuf,
    pub password: String,
    pub pages: Vec<u32>,
}
pub struct TextResponse {
    pub id: u64,
    pub document_id: u64,
    pub result: Result<Vec<PageText>, PdfError>,
}
pub struct ReaderChannels {
    pub requests: mpsc::Sender<ViewportRequest>,
    pub responses: mpsc::Receiver<ViewportResponse>,
    pub text_requests: mpsc::Sender<TextRequest>,
    pub text_responses: mpsc::Receiver<TextResponse>,
    pub selection_requests: mpsc::Sender<SelectionRequest>,
    pub selection_responses: mpsc::Receiver<SelectionResponse>,
    pub search_requests: mpsc::Sender<SearchRequest>,
    pub search_responses: mpsc::Receiver<SearchResponse>,
    pub highlight_requests: mpsc::Sender<HighlightRequest>,
    pub highlight_responses: mpsc::Receiver<HighlightResponse>,
}

pub struct SelectionRequest {
    pub id: u64,
    pub document_id: u64,
    pub path: PathBuf,
    pub password: String,
    pub page: u32,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub rotation: u8,
    pub word: bool,
}
pub struct SelectionResponse {
    pub id: u64,
    pub document_id: u64,
    pub result: Result<PdfSelection, PdfError>,
}

pub struct SearchRequest {
    pub id: u64,
    pub document_id: u64,
    pub path: PathBuf,
    pub password: String,
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub rotation: u8,
}
pub struct SearchResponse {
    pub id: u64,
    pub document_id: u64,
    pub scanned_pages: u32,
    pub total_pages: u32,
    pub matches: Vec<SearchMatch>,
    pub done: bool,
    pub truncated: bool,
    pub error: Option<String>,
}
pub struct HighlightRequest {
    pub id: u64,
    pub document_id: u64,
    pub path: PathBuf,
    pub password: String,
    pub selection: PdfSelection,
    pub rotation: u8,
    pub color: [u8; 3],
    pub output: PathBuf,
}
pub struct HighlightResponse {
    pub id: u64,
    pub document_id: u64,
    pub output: PathBuf,
    pub result: Result<(), PdfError>,
}

struct Session {
    engine: Result<PdfEngine, PdfError>,
    document_id: Option<u64>,
    info: Arc<DocumentInfo>,
    cache: VecDeque<((u32, u32, u8), Arc<PageImage>)>,
}
impl Session {
    fn new() -> Self {
        Self {
            engine: library_path().and_then(|p| PdfEngine::new(&p)),
            document_id: None,
            info: Arc::default(),
            cache: VecDeque::new(),
        }
    }
    fn prepare(&mut self, id: u64, path: &Path, password: &str) -> Result<(), PdfError> {
        let pdf = self.engine.as_mut().map_err(|e| e.clone())?;
        if self.document_id != Some(id) {
            pdf.open(path, password)?;
            self.info = Arc::new(pdf.document_info()?);
            self.document_id = Some(id);
            self.cache.clear();
        }
        Ok(())
    }
    fn render(&mut self, page: u32, width: u32, rotation: u8) -> Result<Arc<PageImage>, PdfError> {
        let key = (page, width, rotation);
        if let Some(i) = self.cache.iter().position(|(k, _)| *k == key) {
            let entry = self.cache.remove(i).unwrap();
            let image = entry.1.clone();
            self.cache.push_back(entry);
            return Ok(image);
        }
        let image = Arc::new(
            self.engine
                .as_ref()
                .map_err(Clone::clone)?
                .render(page, width, rotation)?,
        );
        while self.cache.len() >= 16
            || self.cache.iter().map(|(_, p)| p.rgba.len()).sum::<usize>() + image.rgba.len()
                > 64 * 1024 * 1024
        {
            if self.cache.pop_front().is_none() {
                break;
            }
        }
        if image.rgba.len() <= 64 * 1024 * 1024 {
            self.cache.push_back((key, image.clone()));
        }
        Ok(image)
    }
}

/// Single-page API retained for headless clients and integration tests.
pub fn spawn(
    wake: impl Fn() + Send + 'static,
) -> (mpsc::Sender<RenderRequest>, mpsc::Receiver<RenderResponse>) {
    let (tx, rx) = mpsc::channel::<RenderRequest>();
    let (out, result) = mpsc::channel();
    std::thread::spawn(move || {
        let mut session = Session::new();
        while let Ok(mut r) = rx.recv() {
            for next in rx.try_iter() {
                r = next;
            }
            let value = session
                .prepare(r.document_id, &r.path, &r.password)
                .and_then(|()| session.render(r.page, r.width, r.rotation));
            let pages = if session.document_id == Some(r.document_id) {
                session.info.sizes.len() as u32
            } else {
                0
            };
            if out
                .send(RenderResponse {
                    id: r.id,
                    pages,
                    result: value,
                })
                .is_err()
            {
                break;
            }
            wake();
        }
    });
    (tx, result)
}

/// Render an entire viewport plan without losing pages to single-request
/// coalescing. A newer plan interrupts between pages, never inside a native call.
pub fn spawn_viewport(
    wake: impl Fn() + Send + 'static,
) -> (
    mpsc::Sender<ViewportRequest>,
    mpsc::Receiver<ViewportResponse>,
) {
    let channels = spawn_reader(wake);
    (channels.requests, channels.responses)
}

pub fn spawn_reader(wake: impl Fn() + Send + 'static) -> ReaderChannels {
    let (tx, rx) = mpsc::channel::<ViewportRequest>();
    let (out, result) = mpsc::channel();
    let (text_tx, text_rx) = mpsc::channel::<TextRequest>();
    let (text_out, text_result) = mpsc::channel();
    let (selection_tx, selection_rx) = mpsc::channel::<SelectionRequest>();
    let (selection_out, selection_result) = mpsc::channel();
    let (search_tx, search_rx) = mpsc::channel::<SearchRequest>();
    let (search_out, search_result) = mpsc::channel();
    let (highlight_tx, highlight_rx) = mpsc::channel::<HighlightRequest>();
    let (highlight_out, highlight_result) = mpsc::channel();
    std::thread::Builder::new()
        .name("fpdf-render".into())
        .spawn(move || {
            let mut session = Session::new();
            let mut pending = None;
            let mut searching: Option<(SearchRequest, u32, usize, bool)> = None;
            loop {
                if let Ok(highlight) = highlight_rx.try_recv() {
                    let value = (|| -> Result<(), String> {
                        if session.document_id != Some(highlight.document_id) {
                            return Err("文档已切换，请重新选择文字。".into());
                        }
                        let engine = session.engine.as_ref().map_err(|e| e.message.clone())?;
                        let mut document =
                            crate::annotation::HighlightDocument::load(&highlight.path)?;
                        if !document
                            .matches_source(engine.source_bytes().map_err(|e| e.message.clone())?)
                        {
                            return Err("原 PDF 已被其他程序修改，请重新打开后再标注。".into());
                        }
                        document.add_selection(
                            &highlight.selection,
                            highlight.rotation,
                            highlight.color,
                        )?;
                        document.save(&highlight.output)?;
                        session.document_id = None;
                        session.cache.clear();
                        searching = None;
                        Ok(())
                    })()
                    .map_err(|message| PdfError {
                        message,
                        password_required: false,
                    });
                    let _ = highlight_out.send(HighlightResponse {
                        id: highlight.id,
                        document_id: highlight.document_id,
                        output: highlight.output,
                        result: value,
                    });
                    wake();
                }
                if let Some(request) = search_rx.try_iter().last() {
                    searching = if request.query.trim().is_empty() {
                        None
                    } else {
                        Some((request, 0, 0, false))
                    };
                }
                if let Some((request, page, seen, was_limited)) = searching.take() {
                    let stale = session
                        .document_id
                        .is_some_and(|id| id > request.document_id);
                    let prepared = if stale {
                        Err(PdfError {
                            message: "文档已切换。".into(),
                            password_required: false,
                        })
                    } else {
                        session.prepare(request.document_id, &request.path, &request.password)
                    };
                    let total = session.info.sizes.len() as u32;
                    let result = prepared.and_then(|()| {
                        session.engine.as_ref().map_err(Clone::clone)?.search_page(
                            page,
                            &request.query,
                            request.case_sensitive,
                            request.whole_word,
                            request.rotation,
                        )
                    });
                    let (mut matches, error) = match result {
                        Ok(matches) => (matches, None),
                        Err(e) => (Vec::new(), Some(e.message)),
                    };
                    let limited =
                        was_limited || matches.len() >= 2000 || seen + matches.len() >= 20000;
                    matches.truncate(20000usize.saturating_sub(seen));
                    let scanned = (page + 1).min(total);
                    let done = error.is_some() || scanned >= total || seen + matches.len() >= 20000;
                    let next_seen = seen + matches.len();
                    let _ = search_out.send(SearchResponse {
                        id: request.id,
                        document_id: request.document_id,
                        scanned_pages: scanned,
                        total_pages: total,
                        matches,
                        done,
                        truncated: limited,
                        error,
                    });
                    wake();
                    if !done {
                        searching = Some((request, page + 1, next_seen, limited));
                    }
                }
                if let Some(selection) = selection_rx.try_iter().last() {
                    let value = if session
                        .document_id
                        .is_some_and(|id| id > selection.document_id)
                    {
                        Err(PdfError {
                            message: "文档已切换。".into(),
                            password_required: false,
                        })
                    } else {
                        session
                            .prepare(selection.document_id, &selection.path, &selection.password)
                            .and_then(|()| {
                                let engine = session.engine.as_ref().map_err(Clone::clone)?;
                                if selection.word {
                                    engine.select_word(
                                        selection.page,
                                        selection.start,
                                        selection.rotation,
                                    )
                                } else {
                                    engine.select_text(
                                        selection.page,
                                        selection.start,
                                        selection.end,
                                        selection.rotation,
                                    )
                                }
                            })
                    };
                    let _ = selection_out.send(SelectionResponse {
                        id: selection.id,
                        document_id: selection.document_id,
                        result: value,
                    });
                    wake();
                }
                if let Ok(text) = text_rx.try_recv() {
                    let value = if text.pages.is_empty()
                        || text.pages.len() > 5
                        || session
                            .document_id
                            .is_some_and(|current| current > text.document_id)
                    {
                        Err(PdfError {
                            message: "每次读取 1–5 页。".into(),
                            password_required: false,
                        })
                    } else {
                        session
                            .prepare(text.document_id, &text.path, &text.password)
                            .and_then(|()| {
                                let pdf = session.engine.as_ref().map_err(Clone::clone)?;
                                text.pages.iter().map(|&p| pdf.read_text(p)).collect()
                            })
                    };
                    let _ = text_out.send(TextResponse {
                        id: text.id,
                        document_id: text.document_id,
                        result: value,
                    });
                    wake();
                }
                let mut r = if let Some(r) = pending.take() {
                    r
                } else {
                    match rx.recv_timeout(std::time::Duration::from_millis(
                        if searching.is_some() { 0 } else { 20 },
                    )) {
                        Ok(r) => r,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                };
                for next in rx.try_iter() {
                    r = next;
                }
                let prepared = session.prepare(r.document_id, &r.path, &r.password);
                for (target_index, target) in r.targets.iter().enumerate() {
                    let value = prepared
                        .clone()
                        .and_then(|()| session.render(target.page, target.width, target.rotation));
                    let info =
                        (session.document_id == Some(r.document_id)).then(|| session.info.clone());
                    if out
                        .send(ViewportResponse {
                            id: r.id,
                            document_id: r.document_id,
                            target: *target,
                            info,
                            result: value,
                        })
                        .is_err()
                    {
                        return;
                    }
                    wake();
                    // Errors opening a document are reported once, not once per target.
                    if prepared.is_err() {
                        break;
                    }
                    for next in rx.try_iter() {
                        pending = Some(next);
                    }
                    if pending.is_none() && target_index + 1 < r.targets.len() {
                        // Yield the native owner after each page so a large viewport
                        // cannot starve cancellation, selection, or annotation saves.
                        let mut remainder = r.clone();
                        remainder.targets.drain(..=target_index);
                        pending = Some(remainder);
                    }
                    if pending.is_some() {
                        break;
                    }
                }
            }
        })
        .expect("create PDF render worker");
    ReaderChannels {
        requests: tx,
        responses: result,
        text_requests: text_tx,
        text_responses: text_result,
        selection_requests: selection_tx,
        selection_responses: selection_result,
        search_requests: search_tx,
        search_responses: search_result,
        highlight_requests: highlight_tx,
        highlight_responses: highlight_result,
    }
}
