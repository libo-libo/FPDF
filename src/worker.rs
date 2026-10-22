use crate::pdf::{
    DocumentInfo, PageImage, PageText, PdfEngine, PdfError, PdfSelection, SearchMatch, library_path,
};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, mpsc};

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
    pub marks: Vec<Arc<crate::annotation::HighlightMark>>,
    pub output: PathBuf,
}
pub struct HighlightResponse {
    pub id: u64,
    pub document_id: u64,
    pub output: PathBuf,
    pub result: Result<(), PdfError>,
    pub marks: Vec<Arc<crate::annotation::HighlightMark>>,
}

struct CachedPage {
    key: (u32, u32, u8),
    image: Arc<PageImage>,
    last_used: std::time::Instant,
}

struct Session {
    engine: Result<PdfEngine, PdfError>,
    document_id: Option<u64>,
    info: Arc<DocumentInfo>,
    cache: VecDeque<CachedPage>,
}
impl Session {
    fn new(engine: Result<PdfEngine, PdfError>) -> Self {
        Self {
            engine,
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
        if let Some(i) = self.cache.iter().position(|entry| entry.key == key) {
            let mut entry = self.cache.remove(i).unwrap();
            let image = entry.image.clone();
            entry.last_used = std::time::Instant::now();
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
            || self
                .cache
                .iter()
                .map(|entry| entry.image.bytes())
                .sum::<usize>()
                + image.bytes()
                > 64 * 1024 * 1024
        {
            if self.cache.pop_front().is_none() {
                break;
            }
        }
        if image.bytes() <= 64 * 1024 * 1024 {
            self.cache.push_back(CachedPage {
                key,
                image: image.clone(),
                last_used: std::time::Instant::now(),
            });
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
        let channels = spawn_reader(|| {});
        let requests = channels.requests;
        let responses = channels.responses;
        while let Ok(mut request) = rx.recv() {
            for newer in rx.try_iter() {
                request = newer;
            }
            if requests
                .send(ViewportRequest {
                    id: request.id,
                    document_id: request.document_id,
                    path: request.path,
                    password: request.password,
                    targets: vec![RenderTarget {
                        page: request.page,
                        width: request.width,
                        rotation: request.rotation,
                        thumbnail: false,
                    }],
                })
                .is_err()
            {
                break;
            }
            let Ok(response) = responses.recv() else {
                break;
            };
            if out
                .send(RenderResponse {
                    id: response.id,
                    pages: response.info.map_or(0, |info| info.sizes.len() as u32),
                    result: response.result,
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
    let (save_tx, save_rx) = spawn_highlight_saver();
    render_service()
        .send(Registration {
            rx,
            out,
            text_rx,
            text_out,
            selection_rx,
            selection_out,
            search_rx,
            search_out,
            highlight_rx,
            highlight_out,
            save_tx,
            save_rx,
            wake: Box::new(wake),
        })
        .expect("register PDF reader");
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

/// Only owned messages cross this channel; native state is created on its owner.
struct Registration {
    rx: mpsc::Receiver<ViewportRequest>,
    out: mpsc::Sender<ViewportResponse>,
    text_rx: mpsc::Receiver<TextRequest>,
    text_out: mpsc::Sender<TextResponse>,
    selection_rx: mpsc::Receiver<SelectionRequest>,
    selection_out: mpsc::Sender<SelectionResponse>,
    search_rx: mpsc::Receiver<SearchRequest>,
    search_out: mpsc::Sender<SearchResponse>,
    highlight_rx: mpsc::Receiver<HighlightRequest>,
    highlight_out: mpsc::Sender<HighlightResponse>,
    save_tx: mpsc::Sender<SaveJob>,
    save_rx: mpsc::Receiver<HighlightResponse>,
    wake: Box<dyn Fn() + Send>,
}

struct ReaderLoop {
    session: Session,
    pending: Option<ViewportRequest>,
    searching: Option<(SearchRequest, u32, usize, bool)>,
    channels: Registration,
}
impl ReaderLoop {
    /// Process at most one viewport target and one search page before yielding.
    fn tick(&mut self) -> bool {
        let Self {
            session,
            pending,
            searching,
            channels,
        } = self;
        let Registration {
            rx,
            out,
            text_rx,
            text_out,
            selection_rx,
            selection_out,
            search_rx,
            search_out,
            highlight_rx,
            highlight_out,
            save_tx,
            save_rx,
            wake,
        } = channels;

        for completed in save_rx.try_iter() {
            let _ = highlight_out.send(completed);
            wake();
        }
        if let Some(highlight) = highlight_rx.try_iter().last() {
            let value = (|| -> Result<(), String> {
                if session.document_id != Some(highlight.document_id) {
                    return Err("文档已切换，请重新选择文字。".into());
                }
                let engine = session.engine.as_ref().map_err(|e| e.message.clone())?;
                let snapshot = engine.source_snapshot().map_err(|e| e.message)?;
                save_tx
                    .send((
                        HighlightRequest {
                            id: highlight.id,
                            document_id: highlight.document_id,
                            path: highlight.path.clone(),
                            password: highlight.password.clone(),
                            marks: highlight.marks.clone(),
                            output: highlight.output.clone(),
                        },
                        snapshot,
                    ))
                    .map_err(|_| "标注保存线程已退出。".to_string())?;
                Ok(())
            })()
            .map_err(|message| PdfError {
                message,
                password_required: false,
            });
            if let Err(error) = value {
                let _ = highlight_out.send(HighlightResponse {
                    id: highlight.id,
                    document_id: highlight.document_id,
                    output: highlight.output,
                    result: Err(error),
                    marks: highlight.marks,
                });
                wake();
            }
        }
        if let Some(request) = search_rx.try_iter().last() {
            *searching = if request.query.trim().is_empty() {
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
            let limited = was_limited || matches.len() >= 2000 || seen + matches.len() >= 20000;
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
                *searching = Some((request, page + 1, next_seen, limited));
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
                            engine.select_word(selection.page, selection.start, selection.rotation)
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
            match rx.try_recv() {
                Ok(request) => request,
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => return false,
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
            let info = (session.document_id == Some(r.document_id)).then(|| session.info.clone());
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
                return false;
            }
            wake();
            // Errors opening a document are reported once, not once per target.
            if prepared.is_err() {
                break;
            }
            for next in rx.try_iter() {
                *pending = Some(next);
            }
            if pending.is_none() && target_index + 1 < r.targets.len() {
                // Yield the native owner after each page so a large viewport
                // cannot starve cancellation, selection, or annotation saves.
                let mut remainder = r.clone();
                remainder.targets.drain(..=target_index);
                *pending = Some(remainder);
            }
            if pending.is_some() {
                break;
            }
        }

        true
    }
}

const SERVICE_CACHE_BYTES: usize = 64 * 1024 * 1024;

fn trim_service_cache(clients: &mut [ReaderLoop]) {
    let mut bytes = clients
        .iter()
        .flat_map(|client| &client.session.cache)
        .map(|entry| entry.image.bytes())
        .sum::<usize>();
    while bytes > SERVICE_CACHE_BYTES {
        let Some(index) = clients
            .iter()
            .enumerate()
            .filter_map(|(index, client)| {
                client
                    .session
                    .cache
                    .front()
                    .map(|entry| (index, entry.last_used))
            })
            .min_by_key(|(_, last_used)| *last_used)
            .map(|(index, _)| index)
        else {
            break;
        };
        let entry = clients[index].session.cache.pop_front().unwrap();
        bytes = bytes.saturating_sub(entry.image.bytes());
    }
}

fn render_service() -> &'static mpsc::Sender<Registration> {
    static SERVICE: OnceLock<mpsc::Sender<Registration>> = OnceLock::new();
    SERVICE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Registration>();
        std::thread::Builder::new()
            .name("fpdf-render".into())
            .spawn(move || {
                let mut runtime: Option<Result<PdfEngine, PdfError>> = None;
                let mut clients: Vec<ReaderLoop> = Vec::new();
                loop {
                    // Release the runtime as soon as the final client is gone.
                    if clients.is_empty() {
                        drop(runtime.take());
                        let Ok(channels) = rx.recv() else {
                            break;
                        };
                        let owner = library_path().and_then(|path| PdfEngine::new(&path));
                        let engine = owner
                            .as_ref()
                            .map(PdfEngine::fork_empty)
                            .map_err(Clone::clone);
                        runtime = Some(owner);
                        clients.push(ReaderLoop {
                            session: Session::new(engine),
                            pending: None,
                            searching: None,
                            channels,
                        });
                    }
                    for channels in rx.try_iter() {
                        let engine = runtime
                            .as_ref()
                            .unwrap()
                            .as_ref()
                            .map(PdfEngine::fork_empty)
                            .map_err(Clone::clone);
                        clients.push(ReaderLoop {
                            session: Session::new(engine),
                            pending: None,
                            searching: None,
                            channels,
                        });
                    }
                    let mut index = 0;
                    while index < clients.len() {
                        if clients[index].tick() {
                            index += 1;
                        } else {
                            clients.remove(index);
                        }
                        // Enforce the shared cap after every client's render,
                        // rather than temporarily retaining N full-size caches.
                        trim_service_cache(&mut clients);
                    }
                    if !clients.is_empty() {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            })
            .expect("create shared PDF render worker");
        tx
    })
}

type SaveJob = (HighlightRequest, Arc<Vec<u8>>);

fn spawn_highlight_saver() -> (mpsc::Sender<SaveJob>, mpsc::Receiver<HighlightResponse>) {
    let (tx, rx) = mpsc::channel::<SaveJob>();
    let (out, result) = mpsc::channel();
    std::thread::Builder::new()
        .name("fpdf-annotation-save".into())
        .spawn(move || {
            struct Saved {
                id: u64,
                base: crate::annotation::HighlightDocument,
                path: PathBuf,
                expected: Arc<Vec<u8>>,
            }
            let mut saved: Option<Saved> = None;
            // Native smoke deliberately blocks the first write to verify that
            // selection, drawing and undo never wait for disk serialization.
            let mut slow_smoke = std::env::args_os()
                .any(|arg| arg == "--highlight-features" || arg == "--tabs-features");
            while let Ok(mut job) = rx.recv() {
                for newer in rx.try_iter() {
                    job = newer;
                }
                let (request, snapshot) = job;
                if slow_smoke {
                    slow_smoke = false;
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
                let value = (|| -> Result<(), String> {
                    if saved
                        .as_ref()
                        .is_none_or(|state| state.id != request.document_id)
                    {
                        let base = crate::annotation::HighlightDocument::from_snapshot(
                            &request.path,
                            snapshot.clone(),
                        )?;
                        saved = Some(Saved {
                            id: request.document_id,
                            base,
                            path: request.path.clone(),
                            expected: snapshot,
                        });
                    }
                    let state = saved.as_mut().unwrap();
                    let mut document = state.base.clone();
                    for mark in &request.marks {
                        document.add_markup(&mark.selection, mark.color, mark.kind)?;
                    }
                    let bytes =
                        document.save_checked(&request.output, &state.path, &state.expected)?;
                    state.expected = bytes;
                    state.path = request.output.clone();
                    Ok(())
                })()
                .map_err(|message| PdfError {
                    message,
                    password_required: false,
                });
                if out
                    .send(HighlightResponse {
                        id: request.id,
                        document_id: request.document_id,
                        output: request.output,
                        result: value,
                        marks: request.marks,
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .expect("create annotation save worker");
    (tx, result)
}

#[cfg(test)]
mod save_tests {
    use super::*;
    #[test]
    fn background_snapshots_coalesce_undo_and_reject_external_changes() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-demo.pdf");
        let original = Arc::new(std::fs::read(fixture).unwrap());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("editable.pdf");
        std::fs::write(&path, &*original).unwrap();
        let mark = Arc::new(crate::annotation::HighlightMark {
            kind: crate::annotation::MarkKind::Highlight,
            selection: PdfSelection {
                page: 0,
                text: "first".into(),
                context: String::new(),
                rects: vec![[0.1, 0.1, 0.2, 0.2]],
                quads: vec![[30.0, 700.0, 80.0, 700.0, 30.0, 680.0, 80.0, 680.0]],
                truncated: false,
            },
            rotation: 0,
            color: [255, 230, 70],
        });
        let (tx, rx) = spawn_highlight_saver();
        let request = |id, marks| HighlightRequest {
            id,
            document_id: 1,
            path: path.clone(),
            password: String::new(),
            marks,
            output: path.clone(),
        };
        tx.send((request(1, vec![mark.clone()]), original.clone()))
            .unwrap();
        tx.send((request(2, vec![]), original.clone())).unwrap();
        tx.send((request(3, vec![mark.clone()]), original.clone()))
            .unwrap();
        loop {
            let done = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            assert!(done.result.is_ok(), "{:?}", done.result);
            if done.id == 3 {
                break;
            }
        }
        let saved = lopdf::Document::load(&path).unwrap();
        assert_eq!(
            saved
                .get_dictionary(saved.get_pages()[&1])
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        tx.send((request(4, vec![]), original.clone())).unwrap();
        let done = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(done.id, 4);
        assert!(done.result.is_ok());
        let undone = lopdf::Document::load(&path).unwrap();
        assert!(
            !undone
                .get_dictionary(undone.get_pages()[&1])
                .unwrap()
                .has(b"Annots")
        );
        let mut changed = std::fs::read(&path).unwrap();
        changed.extend_from_slice(b"\n% external edit\n");
        std::fs::write(&path, &changed).unwrap();
        tx.send((request(5, vec![mark]), original)).unwrap();
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .result
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), changed);
    }
}
