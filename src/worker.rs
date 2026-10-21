use crate::pdf::{
    DocumentInfo, PageImage, PageText, PdfEngine, PdfError, PdfSelection, library_path,
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
    std::thread::Builder::new()
        .name("fpdf-render".into())
        .spawn(move || {
            let mut session = Session::new();
            let mut pending = None;
            loop {
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
                    match rx.recv_timeout(std::time::Duration::from_millis(20)) {
                        Ok(r) => r,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                };
                for next in rx.try_iter() {
                    r = next;
                }
                let prepared = session.prepare(r.document_id, &r.path, &r.password);
                for target in &r.targets {
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
    }
}
