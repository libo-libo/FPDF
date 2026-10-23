//! A small, thread-confined binding to the public PDFium C API.
//! No WPS binary is loaded or linked. See runtime/licenses for PDFium notices.
use libloading::Library;
use std::ffi::{CString, c_char, c_int, c_ulong, c_void};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, marker::PhantomData};

mod links;
pub use links::{PdfDestination, PdfLink};

type Handle = *mut c_void;
#[repr(C)]
#[derive(Default)]
struct SizeF {
    width: f32,
    height: f32,
}

#[derive(Clone, Debug)]
pub struct Bookmark {
    pub title: String,
    pub page: Option<u32>,
    pub depth: u8,
}
#[derive(Clone, Debug, Default)]
pub struct DocumentInfo {
    pub sizes: Vec<[f32; 2]>,
    pub bookmarks: Vec<Bookmark>,
}
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 24_000_000;
static ENGINE_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, PartialEq)]
pub struct PdfError {
    pub message: String,
    pub password_required: bool,
}
impl PdfError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            password_required: false,
        }
    }
}
impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for PdfError {}

struct Api {
    _library: Library,
    init: unsafe extern "system" fn(),
    destroy: unsafe extern "system" fn(),
    load: unsafe extern "system" fn(*const c_void, usize, *const c_char) -> Handle,
    close: unsafe extern "system" fn(Handle),
    last_error: unsafe extern "system" fn() -> c_ulong,
    page_count: unsafe extern "system" fn(Handle) -> c_int,
    page_size: unsafe extern "system" fn(Handle, c_int, *mut SizeF) -> c_int,
    bookmark_child: unsafe extern "system" fn(Handle, Handle) -> Handle,
    bookmark_next: unsafe extern "system" fn(Handle, Handle) -> Handle,
    bookmark_title: unsafe extern "system" fn(Handle, *mut c_void, c_ulong) -> c_ulong,
    bookmark_dest: unsafe extern "system" fn(Handle, Handle) -> Handle,
    bookmark_action: unsafe extern "system" fn(Handle) -> Handle,
    action_type: unsafe extern "system" fn(Handle) -> c_ulong,
    action_dest: unsafe extern "system" fn(Handle, Handle) -> Handle,
    link_enumerate: unsafe extern "system" fn(Handle, *mut c_int, *mut Handle) -> c_int,
    link_dest: unsafe extern "system" fn(Handle, Handle) -> Handle,
    link_action: unsafe extern "system" fn(Handle) -> Handle,
    link_rect: unsafe extern "system" fn(Handle, *mut links::RectF) -> c_int,
    dest_view: unsafe extern "system" fn(Handle, *mut c_ulong, *mut f32) -> c_ulong,
    dest_location: unsafe extern "system" fn(
        Handle,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut f32,
        *mut f32,
        *mut f32,
    ) -> c_int,
    dest_page: unsafe extern "system" fn(Handle, Handle) -> c_int,
    load_page: unsafe extern "system" fn(Handle, c_int) -> Handle,
    close_page: unsafe extern "system" fn(Handle),
    page_width: unsafe extern "system" fn(Handle) -> f32,
    page_height: unsafe extern "system" fn(Handle) -> f32,
    bitmap_create: unsafe extern "system" fn(c_int, c_int, c_int, *mut c_void, c_int) -> Handle,
    bitmap_destroy: unsafe extern "system" fn(Handle),
    bitmap_fill: unsafe extern "system" fn(Handle, c_int, c_int, c_int, c_int, c_ulong) -> c_int,
    bitmap_buffer: unsafe extern "system" fn(Handle) -> *mut c_void,
    bitmap_stride: unsafe extern "system" fn(Handle) -> c_int,
    render: unsafe extern "system" fn(Handle, Handle, c_int, c_int, c_int, c_int, c_int, c_int),
    text_load: unsafe extern "system" fn(Handle) -> Handle,
    text_count: unsafe extern "system" fn(Handle) -> c_int,
    text_unicode: unsafe extern "system" fn(Handle, c_int) -> u32,
    text_char_box:
        unsafe extern "system" fn(Handle, c_int, *mut f64, *mut f64, *mut f64, *mut f64) -> c_int,
    text_get: unsafe extern "system" fn(Handle, c_int, c_int, *mut u16) -> c_int,
    text_close: unsafe extern "system" fn(Handle),
    find_start: unsafe extern "system" fn(Handle, *const u16, c_ulong, c_int) -> Handle,
    find_next: unsafe extern "system" fn(Handle) -> c_int,
    find_index: unsafe extern "system" fn(Handle) -> c_int,
    find_count: unsafe extern "system" fn(Handle) -> c_int,
    find_close: unsafe extern "system" fn(Handle),
    text_at: unsafe extern "system" fn(Handle, f64, f64, f64, f64) -> c_int,
    text_rect_count: unsafe extern "system" fn(Handle, c_int, c_int) -> c_int,
    text_rect:
        unsafe extern "system" fn(Handle, c_int, *mut f64, *mut f64, *mut f64, *mut f64) -> c_int,
    device_to_page: unsafe extern "system" fn(
        Handle,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *mut f64,
        *mut f64,
    ) -> c_int,
    page_to_device: unsafe extern "system" fn(
        Handle,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        f64,
        f64,
        *mut c_int,
        *mut c_int,
    ) -> c_int,
}

impl Api {
    fn load(path: &Path) -> Result<Self, PdfError> {
        // SAFETY: The caller supplies a trusted PDFium library. All symbols below
        // use the signatures in the accompanying public fpdfview/fpdf_text headers.
        // The Library lives longer than every copied function pointer.
        unsafe {
            let library = Library::new(path)
                .map_err(|e| PdfError::new(format!("无法加载 PDF 引擎 {}：{e}", path.display())))?;
            macro_rules! sym {
                ($name:literal) => { *library.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| PdfError::new(format!("PDF 引擎接口 {} 缺失：{e}", $name)))? };
            }
            Ok(Self {
                init: sym!("FPDF_InitLibrary"),
                destroy: sym!("FPDF_DestroyLibrary"),
                load: sym!("FPDF_LoadMemDocument64"),
                close: sym!("FPDF_CloseDocument"),
                last_error: sym!("FPDF_GetLastError"),
                page_count: sym!("FPDF_GetPageCount"),
                page_size: sym!("FPDF_GetPageSizeByIndexF"),
                bookmark_child: sym!("FPDFBookmark_GetFirstChild"),
                bookmark_next: sym!("FPDFBookmark_GetNextSibling"),
                bookmark_title: sym!("FPDFBookmark_GetTitle"),
                bookmark_dest: sym!("FPDFBookmark_GetDest"),
                bookmark_action: sym!("FPDFBookmark_GetAction"),
                action_type: sym!("FPDFAction_GetType"),
                action_dest: sym!("FPDFAction_GetDest"),
                link_enumerate: sym!("FPDFLink_Enumerate"),
                link_dest: sym!("FPDFLink_GetDest"),
                link_action: sym!("FPDFLink_GetAction"),
                link_rect: sym!("FPDFLink_GetAnnotRect"),
                dest_view: sym!("FPDFDest_GetView"),
                dest_location: sym!("FPDFDest_GetLocationInPage"),
                dest_page: sym!("FPDFDest_GetDestPageIndex"),
                load_page: sym!("FPDF_LoadPage"),
                close_page: sym!("FPDF_ClosePage"),
                page_width: sym!("FPDF_GetPageWidthF"),
                page_height: sym!("FPDF_GetPageHeightF"),
                bitmap_create: sym!("FPDFBitmap_CreateEx"),
                bitmap_destroy: sym!("FPDFBitmap_Destroy"),
                bitmap_fill: sym!("FPDFBitmap_FillRect"),
                bitmap_buffer: sym!("FPDFBitmap_GetBuffer"),
                bitmap_stride: sym!("FPDFBitmap_GetStride"),
                render: sym!("FPDF_RenderPageBitmap"),
                text_load: sym!("FPDFText_LoadPage"),
                text_count: sym!("FPDFText_CountChars"),
                text_unicode: sym!("FPDFText_GetUnicode"),
                text_char_box: sym!("FPDFText_GetCharBox"),
                text_get: sym!("FPDFText_GetText"),
                text_close: sym!("FPDFText_ClosePage"),
                find_start: sym!("FPDFText_FindStart"),
                find_next: sym!("FPDFText_FindNext"),
                find_index: sym!("FPDFText_GetSchResultIndex"),
                find_count: sym!("FPDFText_GetSchCount"),
                find_close: sym!("FPDFText_FindClose"),
                text_at: sym!("FPDFText_GetCharIndexAtPos"),
                text_rect_count: sym!("FPDFText_CountRects"),
                text_rect: sym!("FPDFText_GetRect"),
                device_to_page: sym!("FPDF_DeviceToPage"),
                page_to_device: sym!("FPDF_PageToDevice"),
                _library: library,
            })
        }
    }
}

struct Document {
    handle: Handle,
    // PDFium borrows this allocation until FPDF_CloseDocument.
    _bytes: Arc<Vec<u8>>,
    pages: u32,
}

/// All engines share one owner thread. !Send + !Sync keeps PDFium calls,
/// document destruction, and final library teardown on that same thread.
pub struct PdfEngine {
    api: Rc<Api>,
    document: Option<Document>,
    _thread_confined: PhantomData<Rc<()>>,
}

#[derive(Clone, Debug)]
pub struct PageImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub points: [f32; 2],
    pub text: String,
    pub text_truncated: bool,
    pub geometry: Option<Arc<crate::reading::TextGeometry>>,
    pub links: Vec<PdfLink>,
}
impl PageImage {
    pub fn bytes(&self) -> usize {
        self.rgba.len()
            + self.links.len() * std::mem::size_of::<PdfLink>()
            + self.geometry.as_ref().map_or(0, |g| g.bytes())
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PageText {
    pub page: u32,
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Debug)]
pub struct PdfSelection {
    pub page: u32,
    pub text: String,
    pub context: String,
    /// Normalized rectangles in the displayed (rotated) page.
    pub rects: Vec<[f32; 4]>,
    /// Unrotated PDF user-space quad points, retained for persistent annotations.
    pub quads: Vec<[f64; 8]>,
    pub truncated: bool,
}

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub page: u32,
    pub index: usize,
    pub rects: Vec<[f32; 4]>,
    pub snippet: String,
    /// UTF-8 byte boundaries of this exact occurrence inside the preview snippet.
    pub snippet_match: [usize; 2],
    pub matched_text: String,
}

fn search_snippet(before: &str, matched: &str, after: &str) -> (String, [usize; 2], String) {
    let normalize = |text: &str| {
        text.chars()
            .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
            .collect::<String>()
    };
    let mut snippet = normalize(before);
    let matched_text = normalize(matched);
    let start = snippet.len();
    snippet.push_str(&matched_text);
    let end = snippet.len();
    snippet.push_str(&normalize(after));
    (snippet, [start, end], matched_text)
}

pub fn library_path() -> Result<PathBuf, PdfError> {
    if let Some(path) = std::env::var_os("FPDF_PDFIUM_PATH") {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe().map_err(|e| PdfError::new(e.to_string()))?;
    let file = if cfg!(windows) {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    };
    let adjacent = exe.parent().unwrap_or(Path::new(".")).join(file);
    if adjacent.is_file() {
        return Ok(adjacent);
    }
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    for directory in ["bin", "lib"] {
        let project = runtime.join(directory).join(file);
        if project.is_file() {
            return Ok(project);
        }
    }
    Err(PdfError::new(
        "未找到 PDF 引擎。请运行 scripts/fetch-pdfium.sh，或将 libpdfium.so 放在 fpdf 可执行文件旁边。",
    ))
}

impl PdfEngine {
    pub fn new(path: &Path) -> Result<Self, PdfError> {
        let api = Api::load(path)?;
        if ENGINE_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(PdfError::new(
                "PDF 引擎已在本进程运行。所有文档操作必须通过同一渲染线程。",
            ));
        }
        // SAFETY: This engine is initialized and used only on its owning thread.
        unsafe {
            (api.init)();
        }
        Ok(Self {
            api: Rc::new(api),
            document: None,
            _thread_confined: PhantomData,
        })
    }

    /// An empty document session sharing this thread's initialized runtime.
    /// Rc deliberately prevents transferring either session to another thread.
    pub(crate) fn fork_empty(&self) -> Self {
        Self {
            api: self.api.clone(),
            document: None,
            _thread_confined: PhantomData,
        }
    }

    pub fn open(&mut self, path: &Path, password: &str) -> Result<u32, PdfError> {
        let metadata =
            fs::metadata(path).map_err(|e| PdfError::new(format!("无法读取文件：{e}")))?;
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(PdfError::new("请选择不超过 512 MiB 的 PDF 文件。"));
        }
        // Bound the read itself as well, including files growing after metadata().
        use std::io::Read;
        let file = fs::File::open(path).map_err(|e| PdfError::new(e.to_string()))?;
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| PdfError::new(e.to_string()))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(PdfError::new("PDF 文件过大。"));
        }
        let password = CString::new(password).map_err(|_| PdfError::new("密码不能包含空字符。"))?;
        // SAFETY: bytes stays allocated for the full Document lifetime. CString
        // is valid for this call. A failed replacement leaves the old document alive.
        let handle =
            unsafe { (self.api.load)(bytes.as_ptr().cast(), bytes.len(), password.as_ptr()) };
        if handle.is_null() {
            let code = unsafe { (self.api.last_error)() };
            return Err(PdfError {
                message: match code {
                    4 => "此 PDF 需要密码，或密码不正确。".into(),
                    3 => "文件不是有效的 PDF，或文件已损坏。".into(),
                    _ => format!("无法打开 PDF（错误码 {code}）。"),
                },
                password_required: code == 4,
            });
        }
        let pages = unsafe { (self.api.page_count)(handle) };
        if pages <= 0 || pages > 100_000 {
            unsafe {
                (self.api.close)(handle);
            }
            return Err(PdfError::new("PDF 页数无效或超过 100,000 页。"));
        }
        self.close();
        self.document = Some(Document {
            handle,
            _bytes: Arc::new(bytes),
            pages: pages as u32,
        });
        Ok(pages as u32)
    }

    pub fn page_count(&self) -> u32 {
        self.document.as_ref().map_or(0, |d| d.pages)
    }

    pub fn document_info(&self) -> Result<DocumentInfo, PdfError> {
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| PdfError::new("请先打开 PDF。"))?;
        let mut info = DocumentInfo::default();
        // SAFETY: This document is live on its owning PDFium thread. All buffer
        // capacities follow the public API byte counts; no native handle escapes.
        unsafe {
            for index in 0..doc.pages {
                let mut size = SizeF::default();
                let ok = (self.api.page_size)(doc.handle, index as c_int, &mut size);
                info.sizes.push(
                    if ok != 0
                        && size.width.is_finite()
                        && size.height.is_finite()
                        && size.width > 0.0
                        && size.height > 0.0
                    {
                        [size.width, size.height]
                    } else {
                        [612.0, 792.0]
                    },
                );
            }
            let root = (self.api.bookmark_child)(doc.handle, std::ptr::null_mut());
            let mut stack = vec![(root, 0u8)];
            let mut seen = std::collections::HashSet::new();
            while let Some((bookmark, depth)) = stack.pop() {
                if bookmark.is_null() || !seen.insert(bookmark as usize) || seen.len() > 5000 {
                    continue;
                }
                let bytes = (self.api.bookmark_title)(bookmark, std::ptr::null_mut(), 0);
                let title = if (2..=8192).contains(&bytes) {
                    let mut text = vec![0u16; (bytes as usize).div_ceil(2)];
                    (self.api.bookmark_title)(bookmark, text.as_mut_ptr().cast(), bytes);
                    if let Some(end) = text.iter().position(|c| *c == 0) {
                        text.truncate(end);
                    }
                    String::from_utf16_lossy(&text)
                } else {
                    "未命名章节".into()
                };
                let mut dest = (self.api.bookmark_dest)(doc.handle, bookmark);
                if dest.is_null() {
                    let action = (self.api.bookmark_action)(bookmark);
                    if !action.is_null() && (self.api.action_type)(action) == 1 {
                        dest = (self.api.action_dest)(doc.handle, action);
                    }
                }
                let page = if dest.is_null() {
                    -1
                } else {
                    (self.api.dest_page)(doc.handle, dest)
                };
                info.bookmarks.push(Bookmark {
                    title,
                    page: (page >= 0 && page < doc.pages as i32).then_some(page as u32),
                    depth,
                });
                stack.push(((self.api.bookmark_next)(doc.handle, bookmark), depth));
                if depth < 32 {
                    stack.push(((self.api.bookmark_child)(doc.handle, bookmark), depth + 1));
                }
            }
        }
        Ok(info)
    }

    pub fn close(&mut self) {
        if let Some(doc) = self.document.take() {
            // SAFETY: All page/bitmap/text guards have already dropped.
            unsafe {
                (self.api.close)(doc.handle);
            }
        }
    }

    pub fn render(&self, index: u32, width: u32, rotation: u8) -> Result<PageImage, PdfError> {
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| PdfError::new("请先打开 PDF。"))?;
        if index >= doc.pages {
            return Err(PdfError::new("页码超出文档范围。"));
        }
        if !(64..=8192).contains(&width) || rotation > 3 {
            return Err(PdfError::new("无效的渲染尺寸或旋转角度。"));
        }
        // SAFETY: All handles here belong to this live engine and are used only
        // on its thread. Guards destroy subordinate handles before returning.
        unsafe {
            let page = (self.api.load_page)(doc.handle, index as c_int);
            if page.is_null() {
                return Err(PdfError::new("无法加载此页。"));
            }
            let _page = Guard {
                handle: page,
                close: self.api.close_page,
            };
            let points = [(self.api.page_width)(page), (self.api.page_height)(page)];
            let [pw, ph] = if rotation.is_multiple_of(2) {
                points
            } else {
                [points[1], points[0]]
            };
            if !pw.is_finite() || !ph.is_finite() || pw <= 0.0 || ph <= 0.0 {
                return Err(PdfError::new("PDF 页面尺寸无效。"));
            }
            let height_f = (width as f64 * ph as f64 / pw as f64).round();
            if !(1.0..=32768.0).contains(&height_f) || width as u64 * height_f as u64 > MAX_PIXELS {
                return Err(PdfError::new("当前页面渲染尺寸过大，请降低缩放比例。"));
            }
            let height = height_f as u32;
            // Format 4 = FPDFBitmap_BGRA. Let PDFium allocate a valid stride.
            let bitmap = (self.api.bitmap_create)(
                width as c_int,
                height as c_int,
                4,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() {
                return Err(PdfError::new("无法分配页面位图。"));
            }
            let _bitmap = Guard {
                handle: bitmap,
                close: self.api.bitmap_destroy,
            };
            if (self.api.bitmap_fill)(bitmap, 0, 0, width as c_int, height as c_int, 0xffffffff)
                == 0
            {
                return Err(PdfError::new("无法初始化页面位图。"));
            }
            (self.api.render)(
                bitmap,
                page,
                0,
                0,
                width as c_int,
                height as c_int,
                rotation as c_int,
                3,
            );
            let stride = (self.api.bitmap_stride)(bitmap);
            let buffer = (self.api.bitmap_buffer)(bitmap);
            if buffer.is_null() || stride < width as c_int * 4 {
                return Err(PdfError::new("PDF 引擎返回了无效的位图。"));
            }
            let src =
                std::slice::from_raw_parts(buffer.cast::<u8>(), stride as usize * height as usize);
            let mut rgba = vec![0; width as usize * height as usize * 4];
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let s = y * stride as usize + x * 4;
                    let t = (y * width as usize + x) * 4;
                    rgba[t..t + 4].copy_from_slice(&[src[s + 2], src[s + 1], src[s], src[s + 3]]);
                }
            }
            let (text, text_truncated) = self.page_text(page);
            Ok(PageImage {
                width,
                height,
                rgba,
                points,
                text,
                text_truncated,
                links: self.page_links(doc, page, rotation),
                geometry: (width >= 256)
                    .then(|| self.page_geometry(page, index, rotation, [pw, ph]))
                    .flatten()
                    .map(Arc::new),
            })
        }
    }

    fn page_geometry(
        &self,
        handle: Handle,
        page: u32,
        rotation: u8,
        points: [f32; 2],
    ) -> Option<crate::reading::TextGeometry> {
        // Only owned, numeric geometry crosses to the UI; all PDFium handles
        // remain on this worker and close before the document does.
        unsafe {
            let text = (self.api.text_load)(handle);
            if text.is_null() {
                return None;
            }
            let _text = Guard {
                handle: text,
                close: self.api.text_close,
            };
            let count = (self.api.text_count)(text).max(0);
            let mut glyphs = Vec::with_capacity(count.min(20000) as usize);
            for index in 0..count.min(20000) {
                let character =
                    char::from_u32((self.api.text_unicode)(text, index)).unwrap_or('\u{fffd}');
                let (mut l, mut r, mut b, mut t) = (0.0, 0.0, 0.0, 0.0);
                let mut rect = [0.0; 4];
                if (self.api.text_char_box)(text, index, &mut l, &mut r, &mut b, &mut t) != 0
                    && r > l
                    && t > b
                {
                    let (mut lx, mut ly, mut rx, mut ry) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
                    for (x, y) in [(l, t), (r, t), (l, b), (r, b)] {
                        let (mut dx, mut dy) = (0, 0);
                        if (self.api.page_to_device)(
                            handle,
                            0,
                            0,
                            10000,
                            10000,
                            rotation as c_int,
                            x,
                            y,
                            &mut dx,
                            &mut dy,
                        ) != 0
                        {
                            lx = lx.min(dx);
                            ly = ly.min(dy);
                            rx = rx.max(dx);
                            ry = ry.max(dy);
                        }
                    }
                    if rx >= lx && ry >= ly {
                        rect = [
                            lx as f32 / 10000.0,
                            ly as f32 / 10000.0,
                            rx as f32 / 10000.0,
                            ry as f32 / 10000.0,
                        ];
                    }
                }
                glyphs.push(crate::reading::TextGlyph {
                    character,
                    rect,
                    quad: [l, t, r, t, l, b, r, b],
                });
            }
            let origin = self.page_point(handle, rotation, 0.0, 0.0)?;
            let x = self.page_point(handle, rotation, 10000.0, 0.0)?;
            let y = self.page_point(handle, rotation, 0.0, 10000.0)?;
            Some(crate::reading::TextGeometry {
                page_transform: [0, 1].map(|i| {
                    [
                        (x[i] - origin[i]) / 10000.0,
                        (y[i] - origin[i]) / 10000.0,
                        origin[i],
                    ]
                }),
                page,
                rotation,
                points,
                glyphs,
                truncated: count > 20000,
            })
        }
    }
    pub fn read_text(&self, page: u32) -> Result<PageText, PdfError> {
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| PdfError::new("请先打开 PDF。"))?;
        if page >= doc.pages {
            return Err(PdfError::new("页码超出范围。"));
        }
        // SAFETY: The owning worker holds the live document and closes both
        // page/text handles on this same thread; no native handle escapes.
        unsafe {
            let handle = (self.api.load_page)(doc.handle, page as c_int);
            if handle.is_null() {
                return Err(PdfError::new("无法读取页面文字。"));
            }
            let _page = Guard {
                handle,
                close: self.api.close_page,
            };
            let (text, truncated) = self.page_text(handle);
            Ok(PageText {
                page,
                text,
                truncated,
            })
        }
    }
    pub fn select_text(
        &self,
        page: u32,
        start: [f64; 2],
        end: [f64; 2],
        rotation: u8,
    ) -> Result<PdfSelection, PdfError> {
        self.select_range(page, start, end, rotation, false)
    }

    pub fn select_word(
        &self,
        page: u32,
        point: [f64; 2],
        rotation: u8,
    ) -> Result<PdfSelection, PdfError> {
        self.select_range(page, point, point, rotation, true)
    }

    fn select_range(
        &self,
        page: u32,
        start: [f64; 2],
        end: [f64; 2],
        rotation: u8,
        word: bool,
    ) -> Result<PdfSelection, PdfError> {
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| PdfError::new("请先打开 PDF。"))?;
        if page >= doc.pages || rotation > 3 || start.into_iter().chain(end).any(|v| !v.is_finite())
        {
            return Err(PdfError::new("选区坐标或页码无效。"));
        }
        // SAFETY: All page/text handles stay on the single PDF worker. PDFium's
        // device transforms account for page crop, intrinsic and viewer rotation.
        unsafe {
            let handle = (self.api.load_page)(doc.handle, page as c_int);
            if handle.is_null() {
                return Err(PdfError::new("无法打开选区页面。"));
            }
            let _page = Guard {
                handle,
                close: self.api.close_page,
            };
            let text = (self.api.text_load)(handle);
            if text.is_null() {
                return Err(PdfError::new("本页没有可选取的文本，可能需要 OCR。"));
            }
            let _text = Guard {
                handle: text,
                close: self.api.text_close,
            };
            let at = |point: [f64; 2], snap: bool| {
                let (mut x, mut y) = (0.0, 0.0);
                if (self.api.device_to_page)(
                    handle,
                    0,
                    0,
                    10000,
                    10000,
                    rotation as c_int,
                    (point[0].clamp(0.0, 1.0) * 10000.0).round() as c_int,
                    (point[1].clamp(0.0, 1.0) * 10000.0).round() as c_int,
                    &mut x,
                    &mut y,
                ) == 0
                {
                    return -1;
                }
                let hit = (self.api.text_at)(text, x, y, 12.0, 12.0);
                if hit >= 0 || !snap {
                    hit
                } else {
                    // In the trailing margin, snap to this text line before
                    // considering a nearby line above or below the pointer.
                    let inline = (self.api.text_at)(
                        text,
                        x,
                        y,
                        f64::from((self.api.page_width)(handle)),
                        12.0,
                    );
                    if inline >= 0 {
                        return inline;
                    }
                    (self.api.text_at)(
                        text,
                        x,
                        y,
                        f64::from((self.api.page_width)(handle)),
                        f64::from((self.api.page_height)(handle)),
                    )
                }
            };
            let (mut first, mut last) = (at(start, false), at(end, !word));
            let total = (self.api.text_count)(text).max(0);
            if first < 0 || last < 0 || first >= total || last >= total {
                return Err(PdfError::new(
                    "请从文字开始拖选，并在文字处松开；扫描页需要 OCR。",
                ));
            }
            if word {
                let word_char = |index| {
                    char::from_u32((self.api.text_unicode)(text, index)).is_some_and(|ch| {
                        ch.is_alphanumeric() || ch == '_' || ch == '\'' || ch == '’' || ch == '-'
                    })
                };
                if word_char(first) {
                    while first > 0 && last - first < 7999 && word_char(first - 1) {
                        first -= 1;
                    }
                    while last + 1 < total && last - first < 7999 && word_char(last + 1) {
                        last += 1;
                    }
                }
            }
            let begin = first.min(last);
            let count = (first.max(last) - begin + 1).min(8000).min(total - begin);
            let get = |from, count: i32| {
                let mut buffer = vec![0; count.max(0) as usize + 1];
                let copied =
                    (self.api.text_get)(text, from, count, buffer.as_mut_ptr()).max(0) as usize;
                buffer.truncate(copied.min(buffer.len()));
                if buffer.last() == Some(&0) {
                    buffer.pop();
                }
                String::from_utf16_lossy(&buffer)
            };
            let selected = get(begin, count);
            if selected.trim().is_empty() {
                return Err(PdfError::new("选区没有可提取的文字。"));
            }
            let before = begin.saturating_sub(1500).max(0);
            let context = format!(
                "选区前文：\n{}\n选区后文：\n{}",
                get(before, begin - before),
                get(begin + count, (total - begin - count).min(1500))
            );
            let rect_count = (self.api.text_rect_count)(text, begin, count).clamp(0, 256);
            let mut rects = Vec::new();
            let mut quads = Vec::new();
            for index in 0..rect_count {
                let (mut left, mut top, mut right, mut bottom) = (0.0, 0.0, 0.0, 0.0);
                if (self.api.text_rect)(text, index, &mut left, &mut top, &mut right, &mut bottom)
                    == 0
                {
                    continue;
                }
                let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (10000, 10000, 0, 0);
                for (x, y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
                    let (mut dx, mut dy) = (0, 0);
                    if (self.api.page_to_device)(
                        handle,
                        0,
                        0,
                        10000,
                        10000,
                        rotation as c_int,
                        x,
                        y,
                        &mut dx,
                        &mut dy,
                    ) != 0
                    {
                        lo_x = lo_x.min(dx);
                        lo_y = lo_y.min(dy);
                        hi_x = hi_x.max(dx);
                        hi_y = hi_y.max(dy);
                    }
                }
                if hi_x >= lo_x && hi_y >= lo_y {
                    quads.push([left, top, right, top, left, bottom, right, bottom]);
                    rects.push([
                        lo_x as f32 / 10000.0,
                        lo_y as f32 / 10000.0,
                        hi_x as f32 / 10000.0,
                        hi_y as f32 / 10000.0,
                    ]);
                }
            }
            Ok(PdfSelection {
                page,
                text: selected,
                context,
                rects,
                quads,
                truncated: first.max(last) - begin + 1 > count,
            })
        }
    }

    pub fn source_bytes(&self) -> Result<&[u8], PdfError> {
        self.document
            .as_ref()
            .map(|d| d._bytes.as_slice())
            .ok_or_else(|| PdfError::new("请先打开 PDF。"))
    }

    pub fn source_snapshot(&self) -> Result<Arc<Vec<u8>>, PdfError> {
        self.document
            .as_ref()
            .map(|d| d._bytes.clone())
            .ok_or_else(|| PdfError::new("请先打开 PDF。"))
    }

    /// Searches one page on the PDFium-owning thread; the scheduler yields between pages.
    pub fn search_page(
        &self,
        page: u32,
        query: &str,
        case_sensitive: bool,
        whole_word: bool,
        rotation: u8,
    ) -> Result<Vec<SearchMatch>, PdfError> {
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| PdfError::new("请先打开 PDF。"))?;
        if page >= doc.pages
            || rotation > 3
            || query.encode_utf16().count() > 1024
            || query.contains('\0')
        {
            return Err(PdfError::new("查找词、页码或旋转参数无效。"));
        }
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        // SAFETY: Native handles are scoped in dependency order on the render thread.
        unsafe {
            let handle = (self.api.load_page)(doc.handle, page as c_int);
            if handle.is_null() {
                return Err(PdfError::new("无法打开查找页面。"));
            }
            let _page = Guard {
                handle,
                close: self.api.close_page,
            };
            let text = (self.api.text_load)(handle);
            if text.is_null() {
                return Ok(Vec::new());
            }
            let _text = Guard {
                handle: text,
                close: self.api.text_close,
            };
            let needle: Vec<u16> = query.encode_utf16().chain(Some(0)).collect();
            let flags = u32::from(case_sensitive) | (u32::from(whole_word) << 1);
            let find = (self.api.find_start)(text, needle.as_ptr(), flags as c_ulong, 0);
            if find.is_null() {
                return Err(PdfError::new("无法启动文本查找。"));
            }
            let _find = Guard {
                handle: find,
                close: self.api.find_close,
            };
            let total = (self.api.text_count)(text).max(0);
            let get = |from, count: i32| {
                let mut buf = vec![0u16; count.max(0) as usize + 1];
                let copied =
                    (self.api.text_get)(text, from, count, buf.as_mut_ptr()).max(0) as usize;
                buf.truncate(copied.min(buf.len()));
                if buf.last() == Some(&0) {
                    buf.pop();
                }
                String::from_utf16_lossy(&buf)
            };
            let mut matches = Vec::new();
            while matches.len() < 2000 && (self.api.find_next)(find) != 0 {
                let begin = (self.api.find_index)(find);
                let count = (self.api.find_count)(find);
                if begin < 0 || count <= 0 || begin.saturating_add(count) > total {
                    continue;
                }
                let mut rects = Vec::new();
                let rect_count = (self.api.text_rect_count)(text, begin, count).clamp(0, 256);
                for index in 0..rect_count {
                    let (mut left, mut top, mut right, mut bottom) = (0.0, 0.0, 0.0, 0.0);
                    if (self.api.text_rect)(
                        text,
                        index,
                        &mut left,
                        &mut top,
                        &mut right,
                        &mut bottom,
                    ) == 0
                    {
                        continue;
                    }
                    let (mut lx, mut ly, mut hx, mut hy) = (10000, 10000, 0, 0);
                    for (x, y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
                        let (mut dx, mut dy) = (0, 0);
                        if (self.api.page_to_device)(
                            handle,
                            0,
                            0,
                            10000,
                            10000,
                            rotation as c_int,
                            x,
                            y,
                            &mut dx,
                            &mut dy,
                        ) != 0
                        {
                            lx = lx.min(dx);
                            ly = ly.min(dy);
                            hx = hx.max(dx);
                            hy = hy.max(dy);
                        }
                    }
                    if hx >= lx && hy >= ly {
                        rects.push([
                            lx as f32 / 10000.0,
                            ly as f32 / 10000.0,
                            hx as f32 / 10000.0,
                            hy as f32 / 10000.0,
                        ]);
                    }
                }
                let start = (begin - 65).max(0);
                let end = (begin + count + 100).min(total);
                let (snippet, snippet_match, matched_text) = search_snippet(
                    &get(start, begin - start),
                    &get(begin, count),
                    &get(begin + count, end - begin - count),
                );
                matches.push(SearchMatch {
                    page,
                    index: matches.len(),
                    rects,
                    snippet,
                    snippet_match,
                    matched_text,
                });
            }
            Ok(matches)
        }
    }

    unsafe fn page_text(&self, page: Handle) -> (String, bool) {
        // SAFETY: Caller guarantees a live page on the PDFium-owning thread.
        unsafe {
            let text = (self.api.text_load)(page);
            if text.is_null() {
                return (String::new(), false);
            }
            let _text = Guard {
                handle: text,
                close: self.api.text_close,
            };
            let count = (self.api.text_count)(text).max(0);
            let limit = count.min(100_000);
            let mut utf16 = vec![0; limit as usize + 1];
            let copied = (self.api.text_get)(text, 0, limit, utf16.as_mut_ptr()).max(0) as usize;
            utf16.truncate(copied.min(utf16.len()));
            if utf16.last() == Some(&0) {
                utf16.pop();
            }
            (String::from_utf16_lossy(&utf16), count > limit)
        }
    }
}

struct Guard {
    handle: Handle,
    close: unsafe extern "system" fn(Handle),
}
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            (self.close)(self.handle);
        }
    }
}
impl Drop for PdfEngine {
    fn drop(&mut self) {
        self.close();
        if Rc::strong_count(&self.api) == 1 {
            // SAFETY: Every document session has closed on this owner thread.
            unsafe {
                (self.api.destroy)();
            }
            ENGINE_ACTIVE.store(false, Ordering::Release);
        }
    }
}

pub fn save_png(path: &Path, page: &PageImage) -> Result<(), Box<dyn std::error::Error>> {
    let writer = std::io::BufWriter::new(fs::File::create(path)?);
    let mut encoder = png::Encoder::new(writer, page.width, page.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&page.rgba)?;
    Ok(())
}

#[cfg(test)]
mod search_preview_tests {
    use super::search_snippet;

    #[test]
    fn exact_occurrence_preview_offsets_survive_unicode_and_multiline_whitespace() {
        let (snippet, range, matched) = search_snippet(
            "中文 first repeat 前文\r\n",
            "跨行\r\nrepeat",
            "\t后文 repeat",
        );
        assert_eq!(matched, "跨行  repeat");
        assert_eq!(&snippet[range[0]..range[1]], matched);
        assert_eq!(range[0], "中文 first repeat 前文  ".len());
        assert!(snippet.is_char_boundary(range[0]) && snippet.is_char_boundary(range[1]));
        let (snippet, range, matched) = search_snippet("中文 repeat ", "repeat", " repeat");
        assert_ne!(snippet.find(&matched).unwrap(), range[0]);
        assert_eq!(&snippet[range[0]..range[1]], "repeat");
    }
}
