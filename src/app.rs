use eframe::egui::{self, Color32, Rect, RichText, Vec2};
use fpdf::pdf::{DocumentInfo, PageImage, save_png};
use fpdf::reading::{NavigationHistory, PageLayout, PaperTheme, ReadingMode, render_width};
use fpdf::worker::{self, RenderTarget, ViewportRequest, ViewportResponse};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

const RED: Color32 = Color32::from_rgb(218, 51, 54);
#[derive(Clone, Copy, PartialEq)]
enum Zoom {
    FitWidth,
    FitPage,
    Scale(f32),
}
#[derive(Clone, Copy, PartialEq)]
enum NavigationTab {
    Thumbnails,
    Outline,
}
struct CachedPage {
    target: RenderTarget,
    image: Arc<PageImage>,
    texture: egui::TextureHandle,
}
type LayoutKey = (u32, u32, u8, ReadingMode, u32);
pub enum ExtraSmoke {
    None,
    Ai,
    Grid,
    Search,
    Settings,
    Highlight,
}

pub struct ReaderApp {
    smoke_highlight: bool,
    smoke_highlight_before: Option<Arc<PageImage>>,
    smoke_ink_started: Option<Instant>,
    smoke_ink_seen: bool,
    smoke_windows: Vec<std::process::Child>,
    uninstall_confirm: bool,
    search: crate::search_ui::SearchUi,
    highlight_requests: mpsc::Sender<worker::HighlightRequest>,
    highlight_responses: mpsc::Receiver<worker::HighlightResponse>,
    highlight_id: u64,
    highlight_pending: Option<u64>,
    highlight_color: [u8; 3],
    highlight_tool: bool,
    highlight_preview: Option<(fpdf::pdf::PdfSelection, [u8; 3], u8)>,
    highlight_stale: HashSet<RenderTarget>,
    highlight_palette_open: bool,
    highlight_draft: [u8; 3],
    highlight_before: [u8; 3],
    highlight_palette: crate::color_palette::Palette,
    selection: crate::text_selection::TextSelection,
    selection_requests: mpsc::Sender<worker::SelectionRequest>,
    selection_responses: mpsc::Receiver<worker::SelectionResponse>,
    selection_id: u64,
    selection_committed_id: Option<u64>,
    ai: crate::ai_reader::AiReader,
    collapsed: HashSet<usize>,
    text_requests: mpsc::Sender<worker::TextRequest>,
    text_responses: mpsc::Receiver<worker::TextResponse>,
    outline_editor: Option<crate::outline_editor::OutlineEditor>,
    inline_rename: Option<crate::inline_outline::Rename>,
    last_outline_click: Option<(usize, Instant)>,
    outline_notice: Option<String>,
    custom_rgb: [u8; 3],
    rgb_draft: [u8; 3],
    rgb_window: bool,
    rgb_palette: crate::color_palette::Palette,
    rgb_before: Option<(PaperTheme, [u8; 3])>,
    rgb_changed: bool,
    theme_notice: Option<String>,
    requests: mpsc::Sender<ViewportRequest>,
    responses: mpsc::Receiver<ViewportResponse>,
    path: Option<PathBuf>,
    document_id: u64,
    request_id: u64,
    info: Option<Arc<DocumentInfo>>,
    page: u32,
    page_input: String,
    page_editing: bool,
    zoom: Zoom,
    zoom_input: String,
    smoke_zoom_before: f32,
    smoke_ctrl_wheel: bool,
    smoke_events: Vec<egui::Event>,
    rotation: u8,
    mode: ReadingMode,
    theme: PaperTheme,
    applied_theme: Option<PaperTheme>,
    sidebar: bool,
    navigation_tab: NavigationTab,
    ribbon: u8,
    text_panel: bool,
    cache: VecDeque<CachedPage>,
    errors: HashMap<RenderTarget, String>,
    wanted: Vec<RenderTarget>,
    last_plan: Vec<RenderTarget>,
    thumbnails: Vec<u32>,
    busy: bool,
    error: Option<String>,
    password: String,
    need_password: bool,
    history: NavigationHistory,
    layout: PageLayout,
    layout_key: Option<LayoutKey>,
    jump: Option<(u32, f32)>,
    scroll_y: f32,
    follow_navigation: bool,
    screenshot: Option<PathBuf>,
    screenshot_sent: bool,
    screenshot_prepared: bool,
    settled_frames: u32,
    started: Instant,
    smoke_features: bool,
    smoke_outline: bool,
    smoke_reader: bool,
    smoke_ai: bool,
    smoke_grid: bool,
    smoke_search: bool,
    smoke_settings: bool,
    smoke_double_click: Option<(egui::Pos2, u8)>,
    smoke_drag: Option<(egui::Pos2, egui::Pos2, u8)>,
    visible_missing: usize,
    audit_cached_scroll: bool,
    smoke_texture_ids: Vec<(RenderTarget, egui::TextureId)>,
    smoke_document_id: u64,
    smoke_translation_progress: bool,
    smoke_step: u8,
    smoke_hits: HashMap<String, Rect>,
    smoke_click: Option<egui::Pos2>,
    smoke_scroll: Option<egui::Pos2>,
    reader_center: egui::Pos2,
}

impl ReaderApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        input: Option<PathBuf>,
        screenshot: Option<PathBuf>,
        smoke_features: bool,
        smoke_outline: bool,
        smoke_reader: bool,
        smoke_extra: ExtraSmoke,
    ) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.button_padding = Vec2::new(11.0, 7.0);
        style.spacing.item_spacing = Vec2::new(7.0, 6.0);
        cc.egui_ctx.set_style(style);
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        for font in ["C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/simhei.ttf"] {
            if let Ok(bytes) = std::fs::read(font) {
                let mut fonts = egui::FontDefinitions::default();
                fonts
                    .font_data
                    .insert("chinese".into(), egui::FontData::from_owned(bytes).into());
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "chinese".into());
                cc.egui_ctx.set_fonts(fonts);
                break;
            }
        }
        let ctx = cc.egui_ctx.clone();
        let channels = worker::spawn_reader(move || ctx.request_repaint());
        let settings = if screenshot.is_some() {
            crate::theme_settings::ThemeSettings::default()
        } else {
            crate::theme_settings::ThemeSettings::load()
        };
        let mut app = Self {
            smoke_highlight: matches!(smoke_extra, ExtraSmoke::Highlight),
            smoke_highlight_before: None,
            smoke_ink_started: None,
            smoke_ink_seen: false,
            smoke_windows: vec![],
            uninstall_confirm: false,
            search: crate::search_ui::SearchUi::new(
                channels.search_requests,
                channels.search_responses,
            ),
            highlight_requests: channels.highlight_requests,
            highlight_responses: channels.highlight_responses,
            highlight_id: 0,
            highlight_pending: None,
            highlight_color: [255, 230, 70],
            highlight_tool: false,
            highlight_preview: None,
            highlight_stale: HashSet::new(),
            highlight_palette_open: false,
            highlight_draft: [255, 230, 70],
            highlight_before: [255, 230, 70],
            highlight_palette: crate::color_palette::Palette::from_rgb([255, 230, 70]),
            selection: crate::text_selection::TextSelection::default(),
            selection_requests: channels.selection_requests,
            selection_responses: channels.selection_responses,
            selection_id: 0,
            selection_committed_id: None,
            ai: crate::ai_reader::AiReader::default(),
            collapsed: HashSet::new(),
            text_requests: channels.text_requests,
            text_responses: channels.text_responses,
            outline_editor: None,
            inline_rename: None,
            last_outline_click: None,
            outline_notice: None,
            custom_rgb: settings.rgb,
            rgb_draft: settings.rgb,
            rgb_window: false,
            rgb_palette: crate::color_palette::Palette::from_rgb(settings.rgb),
            rgb_before: None,
            rgb_changed: false,
            theme_notice: None,
            requests: channels.requests,
            responses: channels.responses,
            path: None,
            document_id: 0,
            request_id: 0,
            info: None,
            page: 0,
            page_input: "1".into(),
            page_editing: false,
            zoom: Zoom::FitWidth,
            zoom_input: "100%".into(),
            smoke_zoom_before: 0.0,
            smoke_ctrl_wheel: false,
            smoke_events: Vec::new(),
            rotation: 0,
            mode: ReadingMode::Continuous,
            theme: settings.theme,
            applied_theme: None,
            sidebar: true,
            navigation_tab: NavigationTab::Thumbnails,
            ribbon: 0,
            text_panel: false,
            cache: VecDeque::new(),
            errors: HashMap::new(),
            wanted: Vec::new(),
            last_plan: Vec::new(),
            thumbnails: Vec::new(),
            busy: false,
            error: None,
            password: String::new(),
            need_password: false,
            history: NavigationHistory::default(),
            layout: PageLayout::default(),
            layout_key: None,
            jump: Some((0, 0.0)),
            scroll_y: 0.0,
            follow_navigation: false,
            screenshot,
            screenshot_sent: false,
            screenshot_prepared: false,
            settled_frames: 0,
            started: Instant::now(),
            smoke_features,
            smoke_outline,
            smoke_reader,
            smoke_ai: matches!(smoke_extra, ExtraSmoke::Ai),
            smoke_grid: matches!(smoke_extra, ExtraSmoke::Grid),
            smoke_search: matches!(smoke_extra, ExtraSmoke::Search),
            smoke_settings: matches!(smoke_extra, ExtraSmoke::Settings),
            smoke_double_click: None,
            smoke_drag: None,
            visible_missing: 0,
            audit_cached_scroll: false,
            smoke_texture_ids: Vec::new(),
            smoke_document_id: 0,
            smoke_translation_progress: false,
            smoke_step: 0,
            smoke_hits: HashMap::new(),
            smoke_click: None,
            smoke_scroll: None,
            reader_center: egui::Pos2::ZERO,
        };
        if let Some(path) = input {
            app.open(path);
        }
        app
    }
    fn pages(&self) -> u32 {
        self.info.as_ref().map_or(0, |i| i.sizes.len() as u32)
    }
    fn hit(&mut self, key: &str, response: egui::Response) -> egui::Response {
        if self.smoke_features {
            self.smoke_hits.insert(key.to_owned(), response.rect);
        }
        response
    }
    fn click_test_control(&mut self, key: &str) {
        self.smoke_click = Some(
            self.smoke_hits
                .get(key)
                .unwrap_or_else(|| panic!("UI control missing: {key}"))
                .center(),
        );
    }
    fn panel(&self) -> Color32 {
        match self.theme {
            PaperTheme::Night => Color32::from_rgb(43, 47, 55),
            PaperTheme::Green => Color32::from_rgb(241, 247, 238),
            PaperTheme::Warm => Color32::from_rgb(251, 247, 237),
            PaperTheme::Custom(rgb) => {
                let rgb = rgb.map(|v| {
                    if self.theme.is_dark() {
                        (u16::from(v) + 22).min(255) as u8
                    } else {
                        ((u16::from(v) + 3 * 255) / 4) as u8
                    }
                });
                Color32::from_rgb(rgb[0], rgb[1], rgb[2])
            }
            _ => Color32::WHITE,
        }
    }
    fn background(&self) -> Color32 {
        match self.theme {
            PaperTheme::Night => Color32::from_rgb(26, 29, 35),
            PaperTheme::Green => Color32::from_rgb(190, 207, 185),
            PaperTheme::Warm => Color32::from_rgb(220, 210, 189),
            PaperTheme::Custom(rgb) => {
                let rgb = rgb.map(|v| (u16::from(v) * 4 / 5) as u8);
                Color32::from_rgb(rgb[0], rgb[1], rgb[2])
            }
            _ => Color32::from_rgb(231, 234, 239),
        }
    }
    fn ink(&self) -> Color32 {
        if self.theme.is_dark() {
            Color32::from_rgb(224, 228, 235)
        } else {
            Color32::from_rgb(46, 51, 61)
        }
    }
    fn muted(&self) -> Color32 {
        if self.theme.is_dark() {
            Color32::from_gray(168)
        } else {
            Color32::from_rgb(121, 127, 138)
        }
    }
    fn selection(&self) -> Color32 {
        match self.theme {
            PaperTheme::Night => Color32::from_rgb(81, 50, 57),
            PaperTheme::Green => Color32::from_rgb(219, 232, 208),
            PaperTheme::Warm => Color32::from_rgb(239, 223, 191),
            PaperTheme::Custom(_) => self.background(),
            _ => Color32::from_rgb(255, 238, 239),
        }
    }
    fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.applied_theme == Some(self.theme) {
            return;
        }
        let mut visuals = if self.theme.is_dark() {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = self.panel();
        visuals.window_fill = self.panel();
        visuals.override_text_color = Some(self.ink());
        visuals.selection.bg_fill = self.selection();
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, RED);
        ctx.set_visuals(visuals);
        for entry in &mut self.cache {
            entry.texture = texture(ctx, self.theme, entry.target, &entry.image);
        }
        self.applied_theme = Some(self.theme);
        if self.screenshot.is_none() {
            self.theme_notice = (crate::theme_settings::ThemeSettings {
                theme: self.theme,
                rgb: self.custom_rgb,
            })
            .save()
            .err()
            .map(|e| format!("主题已应用，但未保存配置：{e}"));
        }
    }
    fn open(&mut self, path: PathBuf) {
        if self
            .inline_rename
            .as_ref()
            .is_some_and(|r| r.changed() || r.result.is_some())
        {
            self.outline_notice =
                Some("请先按 Enter 保存目录名称，或按 Esc 取消后再打开文件。".into());
            return;
        }
        self.inline_rename = None;
        self.last_outline_click = None;
        self.collapsed.clear();
        self.ai.reset();
        self.selection = crate::text_selection::TextSelection::default();
        self.highlight_tool = false;
        self.highlight_preview = None;
        self.highlight_stale.clear();
        self.highlight_palette_open = false;
        self.selection_id += 1;
        self.selection_committed_id = None;
        self.outline_notice = None;
        self.outline_editor = None;
        self.path = Some(path);
        self.document_id += 1;
        self.info = None;
        self.page = 0;
        self.page_input = "1".into();
        self.zoom = Zoom::FitWidth;
        self.rotation = 0;
        self.cache.clear();
        self.errors.clear();
        self.wanted.clear();
        self.last_plan.clear();
        self.layout = PageLayout::default();
        self.layout_key = None;
        self.jump = Some((0, 0.0));
        self.scroll_y = 0.0;
        self.history = NavigationHistory::default();
        self.error = None;
        self.need_password = false;
        self.password.clear();
        self.busy = true;
        self.settled_frames = 0;
        self.follow_navigation = true;
    }
    fn choose_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF 文档", &["pdf"])
            .pick_file()
        {
            self.open(path);
        }
    }
    fn new_window(&mut self, path: Option<PathBuf>) {
        if let Err(error) = crate::window_actions::spawn_window(path.as_deref()) {
            self.outline_notice = Some(format!("无法打开新窗口：{error}"));
        }
    }
    fn uninstall(&mut self, ctx: &egui::Context) {
        if self.highlight_pending.is_some()
            || self.outline_editor.is_some()
            || self.inline_rename.is_some()
        {
            self.outline_notice = Some("请先完成或取消当前编辑，再卸载程序。".into());
            return;
        }
        let Some(uninstaller) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("unins000.exe")))
            .filter(|path| path.is_file())
        else {
            return;
        };
        let mut command = std::process::Command::new(uninstaller);
        command.args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        match command.spawn() {
            Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(error) => self.outline_notice = Some(format!("无法启动卸载程序：{error}")),
        }
    }
    fn open_search(&mut self) {
        let selected = self.selection.current.as_ref().map(|s| s.text.as_str());
        self.search.open(selected);
        self.text_panel = false;
    }
    fn toggle_highlight_tool(&mut self) {
        if self.path.is_none() || self.highlight_pending.is_some() {
            return;
        }
        self.highlight_tool = !self.highlight_tool;
        if self.highlight_tool && self.selection.current.is_some() {
            self.highlight_selection(false);
        } else if self.highlight_tool {
            self.selection.notice = "高亮工具已开启：拖选文字后松手即可标注，按 Esc 退出。".into();
        } else {
            self.selection.notice = "已退出高亮工具。".into();
        }
    }
    fn highlight_selection(&mut self, save_as: bool) {
        if self.highlight_pending.is_some() {
            return;
        }
        if self.outline_editor.is_some() || self.inline_rename.is_some() {
            self.selection.notice = "请先完成目录编辑，再保存高亮标注。".into();
            return;
        }
        let (Some(selection), Some(path)) = (self.selection.current.clone(), self.path.clone())
        else {
            self.selection.notice = "请先双击单词或拖动选择文字，再添加高亮。".into();
            return;
        };
        let output = if save_as {
            let Some(output) = rfd::FileDialog::new()
                .add_filter("PDF 文档", &["pdf"])
                .set_file_name("标注副本.pdf")
                .save_file()
            else {
                return;
            };
            output
        } else {
            path.clone()
        };
        self.highlight_id += 1;
        let id = self.highlight_id;
        if self
            .highlight_requests
            .send(worker::HighlightRequest {
                id,
                document_id: self.document_id,
                path,
                password: self.password.clone(),
                selection: selection.clone(),
                rotation: self.rotation,
                color: self.highlight_color,
                output,
            })
            .is_ok()
        {
            self.highlight_pending = Some(id);
            // Show ink in this frame; disk serialization and PDFium reload are
            // asynchronous and must not gate the visible annotation.
            self.highlight_preview = Some((selection, self.highlight_color, self.rotation));
            if self.smoke_highlight {
                self.smoke_ink_started = Some(Instant::now());
                self.smoke_ink_seen = false;
                assert!(
                    !self.cache.is_empty(),
                    "Original page must remain visible during save"
                );
            }
            self.selection.current = None;
            self.selection.pending = None;
            self.selection_id += 1;
            self.selection_committed_id = None;
            self.selection.notice = "正在保存高亮标注…".into();
        } else {
            self.selection.notice = "保存线程已退出，请重启程序。".into();
        }
    }
    fn poll_highlights(&mut self) {
        while let Ok(response) = self.highlight_responses.try_recv() {
            if self.highlight_pending != Some(response.id) {
                continue;
            }
            self.highlight_pending = None;
            if response.document_id != self.document_id {
                continue;
            }
            match response.result {
                Ok(()) => {
                    self.path = Some(response.output);
                    self.document_id += 1;
                    // Retain the old bitmap beneath the optimistic ink until
                    // its replacement arrives, avoiding a blank/reload flash.
                    self.highlight_stale = self.cache.iter().map(|e| e.target).collect();
                    self.errors.clear();
                    self.wanted.clear();
                    self.last_plan.clear();
                    self.selection.current = None;
                    self.selection_id += 1;
                    self.selection_committed_id = None;
                    self.selection.notice = "高亮标注已保存到 PDF，重新打开仍会显示。".into();
                    self.jump = Some((
                        self.page,
                        self.layout.placement(self.page).map_or(0.0, |p| {
                            ((self.scroll_y - p.top) / p.height).clamp(0.0, 1.0)
                        }),
                    ));
                    self.settled_frames = 0;
                }
                Err(error) => {
                    self.highlight_tool = false;
                    self.highlight_preview = None;
                    self.selection.notice = format!("高亮未保存：{}", error.message);
                    self.outline_notice = Some(self.selection.notice.clone());
                }
            }
        }
    }
    fn set_page(&mut self, page: u32) {
        if self.pages() == 0 {
            return;
        }
        self.page = page.min(self.pages() - 1);
        self.page_input = (self.page + 1).to_string();
        self.jump = Some((self.page, 0.0));
        self.follow_navigation = true;
        self.settled_frames = 0;
    }
    fn goto(&mut self, page: u32) {
        if self.pages() == 0 {
            return;
        }
        let page = page.min(self.pages() - 1);
        self.history.visit(self.page, page);
        self.set_page(page);
    }
    fn back(&mut self) {
        if let Some(p) = self.history.back(self.page) {
            self.set_page(p);
        }
    }
    fn forward(&mut self) {
        if let Some(p) = self.history.forward(self.page) {
            self.set_page(p);
        }
    }
    fn effective_zoom(&self) -> f32 {
        match self.zoom {
            Zoom::Scale(v) => v,
            Zoom::FitWidth | Zoom::FitPage => self.layout.placement(self.page).map_or(1.0, |p| {
                let size = self
                    .info
                    .as_ref()
                    .map_or([612.0, 792.0], |i| i.sizes[self.page as usize]);
                let width = if self.rotation.is_multiple_of(2) {
                    size[0]
                } else {
                    size[1]
                };
                p.width / (width * 96.0 / 72.0)
            }),
        }
    }
    fn scale(&mut self, factor: f32) {
        self.zoom = Zoom::Scale((self.effective_zoom() * factor).clamp(0.1, 5.0));
    }
    fn current_image(&self) -> Option<&PageImage> {
        self.cache
            .iter()
            .rev()
            .find(|e| e.target.page == self.page && !e.target.thumbnail)
            .map(|e| e.image.as_ref())
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(r) = self.responses.try_recv() {
            if r.document_id != self.document_id {
                continue;
            }
            if self.info.is_none()
                && let Some(info) = r.info
            {
                self.info = Some(info);
                self.layout_key = None;
            }
            if !self.wanted.contains(&r.target) && r.result.is_err() {
                continue;
            }
            match r.result {
                Ok(image) => {
                    self.highlight_stale.remove(&r.target);
                    if !r.target.thumbnail
                        && self.highlight_pending.is_none()
                        && self
                            .highlight_preview
                            .as_ref()
                            .is_some_and(|(s, _, rotation)| {
                                s.page == r.target.page && *rotation == r.target.rotation
                            })
                    {
                        self.highlight_preview = None;
                    }
                    self.errors.remove(&r.target);
                    if self
                        .cache
                        .iter()
                        .any(|e| e.target == r.target && Arc::ptr_eq(&e.image, &image))
                    {
                        continue;
                    }
                    self.cache.retain(|e| e.target != r.target);
                    while !self.cache.is_empty()
                        && (self.cache.len() >= 32.max(self.wanted.len() + 2)
                            || self.cache.iter().map(|e| e.image.rgba.len()).sum::<usize>()
                                + image.rgba.len()
                                > 64 * 1024 * 1024)
                    {
                        let i = self
                            .cache
                            .iter()
                            .position(|e| !self.wanted.contains(&e.target))
                            .unwrap_or(0);
                        self.cache.remove(i);
                    }
                    let tex = texture(ctx, self.theme, r.target, &image);
                    self.cache.push_back(CachedPage {
                        target: r.target,
                        image,
                        texture: tex,
                    });
                    self.error = None;
                    self.need_password = false;
                }
                Err(error) => {
                    if self.info.is_none() {
                        self.error = Some(error.message);
                        self.need_password = error.password_required;
                    } else {
                        self.errors.insert(r.target, error.message);
                    }
                }
            }
        }
        self.busy = self.error.is_none()
            && self.wanted.iter().filter(|t| !t.thumbnail).any(|t| {
                !self.cache.iter().any(|e| e.target == *t) && !self.errors.contains_key(t)
            });
    }
    fn submit(&mut self, targets: Vec<RenderTarget>) {
        self.wanted = targets;
        if self.wanted == self.last_plan || self.need_password || self.error.is_some() {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };
        self.request_id += 1;
        self.last_plan = self.wanted.clone();
        self.settled_frames = 0;
        let targets = self
            .wanted
            .iter()
            .copied()
            .filter(|t| !self.errors.contains_key(t))
            .filter(|t| {
                self.highlight_stale.contains(t) || !self.cache.iter().any(|e| e.target == *t)
            })
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return;
        }
        if self
            .requests
            .send(ViewportRequest {
                id: self.request_id,
                document_id: self.document_id,
                path,
                password: self.password.clone(),
                targets,
            })
            .is_err()
        {
            self.error = Some("渲染线程已退出，请重启 fpdf。".into());
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.highlight_tool
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.highlight_tool = false;
            self.selection.notice = "已退出高亮工具。".into();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N)) {
            self.new_window(None);
        }
        let (open, find, fit, prev, next, home, end, back, forward) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::CTRL, egui::Key::O),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::F),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::Num0),
                i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::Home),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::End),
                i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft),
                i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight),
            )
        });
        if open {
            self.choose_file();
        }
        if find {
            self.open_search();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F2)) {
            self.toggle_highlight_tool();
        }
        if fit {
            self.zoom = Zoom::FitWidth;
        }
        if prev {
            self.goto(self.page.saturating_sub(1));
        }
        if next {
            self.goto(self.page + 1);
        }
        if home {
            self.goto(0);
        }
        if end {
            self.goto(self.pages().saturating_sub(1));
        }
        if back {
            self.back();
        }
        if forward {
            self.forward();
        }
        let (larger, smaller, actual) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::CTRL, egui::Key::Plus)
                    | i.consume_key(egui::Modifiers::CTRL, egui::Key::Equals),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::Minus),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::Num1),
            )
        });
        if larger {
            self.scale(1.2);
        }
        if smaller {
            self.scale(1.0 / 1.2);
        }
        if actual {
            self.zoom = Zoom::Scale(1.0);
        }
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone())) {
            self.open(path);
        }
    }
    fn save_inline_name(&mut self, ctx: &egui::Context) {
        if self.inline_rename.as_ref().is_some_and(|r| !r.changed()) {
            self.inline_rename = None;
            return;
        }
        if let (Some(rename), Some(path), Some(info)) =
            (&mut self.inline_rename, self.path.clone(), &self.info)
        {
            rename.save(ctx, path, info.bookmarks.clone());
        }
    }
    fn poll_inline_name(&mut self) {
        let reply = self
            .inline_rename
            .as_ref()
            .and_then(|r| r.result.as_ref())
            .and_then(|rx| match rx.try_recv() {
                Ok(value) => Some(value),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("保存线程意外退出，请重试。".into()))
                }
            });
        if let Some(reply) = reply {
            match reply {
                Ok(()) => {
                    let rename = self.inline_rename.take().unwrap();
                    if let Some(info) = &mut self.info
                        && let Some(mark) = Arc::make_mut(info).bookmarks.get_mut(rename.index)
                    {
                        mark.title = rename.title;
                    }
                    self.outline_notice = Some("目录名称已保存到 PDF".into());
                }
                Err(error) => {
                    let rename = self.inline_rename.as_mut().unwrap();
                    rename.result = None;
                    rename.error = Some(error);
                    rename.focus = true;
                }
            }
        }
    }
    fn theme_choices(&mut self, ui: &mut egui::Ui) {
        // Relative order and names come from WPS 29161 CT_Home.kuip:79–85.
        // Its separate Daylight pixel operation is not recovered; do not invent it.
        for theme in [
            PaperTheme::White,
            PaperTheme::Night,
            PaperTheme::Green,
            PaperTheme::Warm,
        ] {
            if ui
                .selectable_value(&mut self.theme, theme, theme.label())
                .clicked()
            {
                ui.close();
            }
        }
        ui.separator();
        if self
            .hit(
                "custom-theme",
                ui.selectable_label(matches!(self.theme, PaperTheme::Custom(_)), "自定义 RGB…"),
            )
            .clicked()
        {
            self.rgb_draft = self.custom_rgb;
            self.rgb_before = Some((self.theme, self.custom_rgb));
            self.rgb_palette = crate::color_palette::Palette::from_rgb(self.rgb_draft);
            self.rgb_changed = false;
            self.rgb_window = true;
            ui.close();
        }
    }
    fn highlight_color_window(&mut self, ctx: &egui::Context) {
        if !self.highlight_palette_open {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("高亮颜色 · 调色盘")
            .id(egui::Id::new("highlight-color-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("点选颜色后立即用于后续标注；取消恢复原色。");
                let (plane, bar, _) = self.highlight_palette.show(ui, &mut self.highlight_draft);
                self.hit("highlight-palette", plane);
                self.hit("highlight-brightness", bar);
                ui.horizontal(|ui| {
                    for (i, name) in ["R", "G", "B"].into_iter().enumerate() {
                        ui.label(name);
                        let value = ui.add(
                            egui::DragValue::new(&mut self.highlight_draft[i])
                                .range(0..=255)
                                .speed(1.0),
                        );
                        self.hit(&format!("highlight-{name}"), value);
                    }
                });
                let [r, g, b] = self.highlight_draft;
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(44.0, 24.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 3, Color32::from_rgb(r, g, b));
                    ui.label(format!("RGB({r}, {g}, {b})  #{r:02X}{g:02X}{b:02X}"));
                });
                ui.horizontal(|ui| {
                    apply = self
                        .hit("highlight-color-apply", ui.button("确定"))
                        .clicked();
                    cancel = self
                        .hit("highlight-color-cancel", ui.button("取消"))
                        .clicked();
                });
            });
        self.highlight_color = self.highlight_draft;
        if cancel || !open {
            self.highlight_color = self.highlight_before;
            self.highlight_palette_open = false;
        } else if apply {
            self.highlight_palette_open = false;
        }
    }
    fn custom_theme_window(&mut self, ctx: &egui::Context) {
        if !self.rgb_window {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("自定义颜色")
            .id(egui::Id::new("custom-rgb-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(350.0)
            .show(ctx, |ui| {
                ui.label("点击调色盘选色，RGB 数值同步更新。");
                let (plane, brightness, changed) = self.rgb_palette.show(ui, &mut self.rgb_draft);
                self.hit("rgb-palette", plane);
                self.hit("rgb-brightness", brightness);
                self.rgb_changed |= changed;
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    for (i, name) in ["R", "G", "B"].into_iter().enumerate() {
                        ui.label(name);
                        let response = ui.add(
                            egui::DragValue::new(&mut self.rgb_draft[i])
                                .range(0..=255)
                                .speed(1.0),
                        );
                        self.rgb_changed |= response.changed();
                        self.hit(&format!("rgb-{name}"), response);
                    }
                });
                let preview = PaperTheme::Custom(self.rgb_draft);
                let paper = self.rgb_draft;
                let ink = preview.pixel([0, 0, 0, 255]);
                egui::Frame::new()
                    .fill(Color32::from_rgb(paper[0], paper[1], paper[2]))
                    .inner_margin(16.0)
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("阅读预览 Aa 123\n自定义背景，文字自动适配明暗")
                                .color(Color32::from_rgb(ink[0], ink[1], ink[2])),
                        );
                    });
                ui.label(format!(
                    "RGB({}, {}, {})   #{:02X}{:02X}{:02X}",
                    paper[0], paper[1], paper[2], paper[0], paper[1], paper[2]
                ));
                ui.horizontal(|ui| {
                    apply = self.hit("rgb-apply", ui.button("确定")).clicked();
                    cancel = self.hit("rgb-cancel", ui.button("取消")).clicked();
                });
            });
        // Selecting a palette point configures that color directly. During a
        // drag the controls preview continuously; recolor pages on release.
        if (self.rgb_changed && !ctx.input(|i| i.pointer.any_down())) || apply {
            self.custom_rgb = self.rgb_draft;
            self.theme = PaperTheme::Custom(self.custom_rgb);
            self.rgb_changed = false;
            ctx.request_repaint();
        }
        if apply {
            self.rgb_window = false;
            self.rgb_before = None;
        } else if cancel || !open {
            if let Some((theme, rgb)) = self.rgb_before.take() {
                self.theme = theme;
                self.custom_rgb = rgb;
                self.rgb_draft = rgb;
            }
            self.rgb_window = false;
            self.rgb_changed = false;
            ctx.request_repaint();
        }
    }
    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("title")
            .exact_height(49.0)
            .frame(
                egui::Frame::new()
                    .fill(self.panel())
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    egui::Frame::new()
                        .fill(RED)
                        .corner_radius(5)
                        .inner_margin(egui::Margin::symmetric(7, 2))
                        .show(ui, |ui| {
                            ui.label(RichText::new("P").size(21.0).strong().color(Color32::WHITE));
                        });
                    ui.label(RichText::new("fpdf").size(21.0).strong());
                    ui.add_space(18.0);
                    ui.label(RichText::new("PDF 阅读").color(RED));
                    ui.separator();
                    let name = self
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "开始阅读".into());
                    ui.add_sized(
                        [(ui.available_width() - 110.0).max(10.0), 28.0],
                        egui::Label::new(RichText::new(&name).color(self.muted())).truncate(),
                    )
                    .on_hover_text(name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("打开文件").clicked() {
                            self.choose_file();
                        }
                    });
                });
            });
        egui::TopBottomPanel::top("navigation-ribbon")
            .exact_height(34.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.menu_button("文件",|ui| {
                        if ui.button("新窗口 · Ctrl+N").clicked() {self.new_window(None);ui.close();}
                        if ui.button("在新窗口打开 PDF…").clicked() {
                            if let Some(path)=rfd::FileDialog::new().add_filter("PDF 文档", &["pdf"]).pick_file() {self.new_window(Some(path));}
                            ui.close();
                        }
                        if ui.button("打开 PDF…").clicked() {self.choose_file();ui.close();}
                        let installed=std::env::current_exe().ok().and_then(|p|p.parent().map(|d|d.join("unins000.exe"))).is_some_and(|p|p.is_file());
                        if installed {
                            ui.separator();
                            if ui.button("卸载 fpdf（清除全部配置）")
                                .on_hover_text("一键移除安装程序文件、本程序保存的 AI 配置、主题和缓存；保留用户 PDF。")
                                .clicked() {ui.close();self.uninstall_confirm=true;}
                        }
                    });
                    ui.add_space(10.0);
                    ui.selectable_value(&mut self.ribbon, 0, "开始");
                    ui.selectable_value(&mut self.ribbon, 1, "阅读");
                    ui.selectable_value(&mut self.ribbon, 2, "视图");
                });
            });
        egui::TopBottomPanel::top("toolbar")
            .exact_height(108.0)
            .frame(
                egui::Frame::new()
                    .fill(self.panel())
                    .inner_margin(egui::Margin::symmetric(14, 7)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("toolbar-scroll")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    if self
                                        .hit("zoom-out-ribbon", ui.button("− 缩小"))
                                        .on_hover_text("Ctrl+−")
                                        .clicked()
                                    {
                                        self.scale(1.0 / 1.2);
                                    }
                                    if self
                                        .hit("zoom-in-ribbon", ui.button("＋ 放大"))
                                        .on_hover_text("Ctrl++ / Ctrl+滚轮")
                                        .clicked()
                                    {
                                        self.scale(1.2);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    if self
                                        .hit(
                                            "actual-size",
                                            ui.selectable_label(
                                                self.zoom == Zoom::Scale(1.0),
                                                "实际大小",
                                            ),
                                        )
                                        .on_hover_text("Ctrl+1 · 100%")
                                        .clicked()
                                    {
                                        self.zoom = Zoom::Scale(1.0);
                                    }
                                    if self
                                        .hit(
                                            "fit-width",
                                            ui.selectable_label(
                                                self.zoom == Zoom::FitWidth,
                                                "适合宽度",
                                            ),
                                        )
                                        .on_hover_text("Ctrl+0")
                                        .clicked()
                                    {
                                        self.zoom = Zoom::FitWidth;
                                    }
                                    if self
                                        .hit(
                                            "fit-page",
                                            ui.selectable_label(
                                                self.zoom == Zoom::FitPage,
                                                "适合页面",
                                            ),
                                        )
                                        .clicked()
                                    {
                                        self.zoom = Zoom::FitPage;
                                    }
                                });
                                ui.label(RichText::new("显示比例").small().color(self.muted()));
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    if self
                                        .hit(
                                            "back",
                                            ui.add_enabled(
                                                self.history.has_back(),
                                                egui::Button::new("后退"),
                                            ),
                                        )
                                        .on_hover_text("Alt+←")
                                        .clicked()
                                    {
                                        self.back();
                                    }
                                    if self
                                        .hit(
                                            "forward",
                                            ui.add_enabled(
                                                self.history.has_forward(),
                                                egui::Button::new("前进"),
                                            ),
                                        )
                                        .on_hover_text("Alt+→")
                                        .clicked()
                                    {
                                        self.forward();
                                    }
                                    if self
                                        .hit(
                                            "home",
                                            ui.add_enabled(
                                                self.page > 0,
                                                egui::Button::new("首页"),
                                            ),
                                        )
                                        .clicked()
                                    {
                                        self.goto(0);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    if ui.button("上一页").clicked() {
                                        self.goto(self.page.saturating_sub(1));
                                    }
                                    if ui.button("下一页").clicked() {
                                        self.goto(self.page + 1);
                                    }
                                    if ui.button("末页").clicked() {
                                        self.goto(self.pages().saturating_sub(1));
                                    }
                                });
                                ui.label(RichText::new("页面导航").small().color(self.muted()));
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                let r = ui.selectable_value(
                                    &mut self.mode,
                                    ReadingMode::Single,
                                    "单页阅读",
                                );
                                self.hit("single", r);
                                let r = ui.selectable_value(
                                    &mut self.mode,
                                    ReadingMode::Continuous,
                                    "连续阅读",
                                );
                                self.hit("continuous", r);
                                ui.label(RichText::new("阅读方式").small().color(self.muted()));
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                if self
                                    .hit(
                                        "eye",
                                        ui.selectable_label(
                                            self.theme != PaperTheme::White,
                                            "护眼模式",
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.theme = if self.theme == PaperTheme::White {
                                        PaperTheme::Green
                                    } else {
                                        PaperTheme::White
                                    };
                                }
                                let theme_menu = egui::ComboBox::from_id_salt("eye-theme")
                                    .width(83.0)
                                    .selected_text(self.theme.label())
                                    .show_ui(ui, |ui| {
                                        self.theme_choices(ui);
                                    });
                                self.hit("theme-menu", theme_menu.response);
                                if let Some(notice) = &self.theme_notice {
                                    ui.label(
                                        RichText::new("阅读背景 · 配置未保存").small().color(RED),
                                    )
                                    .on_hover_text(notice);
                                } else {
                                    ui.label(RichText::new("阅读背景").small().color(self.muted()));
                                }
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                if self.hit("rotate", ui.button("顺时针旋转")).clicked() {
                                    self.rotation = (self.rotation + 1) % 4;
                                }
                                if self.hit("open-search", ui.button("全文搜索")).clicked() {
                                    self.open_search();
                                }
                                ui.label(RichText::new("阅读工具").small().color(self.muted()));
                            });
                            if self.ribbon != 1 {
                                ui.separator();
                                ui.vertical(|ui| {
                                    ui.toggle_value(&mut self.sidebar, "导航窗格");
                                    ui.toggle_value(&mut self.text_panel, "文本面板");
                                    ui.label(RichText::new("窗口").small().color(self.muted()));
                                });
                            }
                            ui.separator();
                            ui.vertical(|ui| {
                                let can_mark = self.path.is_some()
                                    && self.highlight_pending.is_none();
                                let mark = ui.add_enabled(can_mark, egui::Button::new("高亮标注").selected(self.highlight_tool));
                                if self
                                    .hit("highlight-selection", mark)
                                    .on_hover_text("开启后拖选文字即可连续高亮；也可先选文字再点击。Ctrl+F2 切换，Esc 退出。")
                                    .clicked()
                                {
                                    self.toggle_highlight_tool();
                                }
                                let colors = ui.menu_button("标注颜色", |ui| {
                                    for (name, color) in [
                                        ("黄色", [255, 230, 70]),
                                        ("绿色", [120, 230, 140]),
                                        ("蓝色", [100, 190, 255]),
                                        ("粉色", [255, 150, 200]),
                                    ] {
                                        if ui
                                            .selectable_label(self.highlight_color == color, name)
                                            .clicked()
                                        {
                                            self.highlight_color = color;
                                            ui.close();
                                        }
                                    }
                                    ui.separator();
                                    if self.hit("highlight-custom-color",ui.button("自定义颜色…")).clicked() {
                                        self.highlight_draft = self.highlight_color;
                                        self.highlight_before = self.highlight_color;
                                        self.highlight_palette = crate::color_palette::Palette::from_rgb(self.highlight_draft);
                                        self.highlight_palette_open = true;
                                        ui.close();
                                    }
                                    if ui
                                        .add_enabled(
                                            can_mark && self.selection.current.is_some(),
                                            egui::Button::new("添加高亮并另存为…"),
                                        )
                                        .clicked()
                                    {
                                        self.highlight_selection(true);
                                        ui.close();
                                    }
                                });
                                self.hit("highlight-colors", colors.response);
                                ui.label(RichText::new(if self.highlight_tool {"高亮中 · Esc 退出"} else {"文本标注"}).small().color(if self.highlight_tool {Color32::from_rgb(205,150,30)} else {self.muted()}));
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                let ai = ui.toggle_value(&mut self.ai.visible, "AI 辅助阅读");
                                self.hit("ai-reader", ai);
                                if self
                                    .hit("ask-selection", ui.button("针对选区提问"))
                                    .clicked()
                                {
                                    self.ai.open_question();
                                }
                                ui.label(RichText::new("pi agent").small().color(self.muted()));
                            });
                        });
                    });
            });
    }
    fn navigation(&mut self, ctx: &egui::Context) {
        self.thumbnails.clear();
        if !self.sidebar {
            return;
        }
        egui::SidePanel::left("navigation")
            .resizable(true)
            .default_width(190.0)
            .min_width(155.0)
            .show(ctx, |ui| {
                ui.add_space(7.0);
                ui.horizontal(|ui| {
                    let thumbnails = ui.selectable_value(
                        &mut self.navigation_tab,
                        NavigationTab::Thumbnails,
                        "缩略图",
                    );
                    self.hit("thumbnails-tab", thumbnails);
                    let outline = ui.selectable_value(
                        &mut self.navigation_tab,
                        NavigationTab::Outline,
                        "文档目录",
                    );
                    self.hit("outline-tab", outline);
                });
                ui.separator();
                if self.navigation_tab == NavigationTab::Outline {
                    if self
                        .hit(
                            "edit-outline",
                            ui.add_enabled(
                                self.info.is_some() && self.inline_rename.is_none(),
                                egui::Button::new("编辑目录"),
                            ),
                        )
                        .clicked()
                        && let (Some(path), Some(info)) = (self.path.clone(), self.info.as_ref())
                    {
                        self.outline_editor = Some(crate::outline_editor::OutlineEditor::new(
                            ctx,
                            path,
                            info.bookmarks.clone(),
                        ));
                    }
                    let bookmarks = self
                        .info
                        .as_ref()
                        .map(|i| i.bookmarks.clone())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        if self
                            .hit("outline-expand-all", ui.small_button("展开全部"))
                            .clicked()
                        {
                            self.collapsed.clear();
                        }
                        if self
                            .hit("outline-collapse-all", ui.small_button("折叠全部"))
                            .clicked()
                        {
                            self.collapsed = (0..bookmarks.len())
                                .filter(|&i| crate::outline_folding::has_children(&bookmarks, i))
                                .collect();
                        }
                    });
                    if bookmarks.is_empty() {
                        ui.add_space(16.0);
                        ui.label(RichText::new("该文档没有目录书签").color(self.muted()));
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("outline")
                        .show(ui, |ui| {
                            for index in
                                crate::outline_folding::visible_indices(&bookmarks, &self.collapsed)
                            {
                                let mark = &bookmarks[index];
                                ui.horizontal(|ui| {
                                    ui.add_space((mark.depth as f32 * 12.0).min(72.0));
                                    if crate::outline_folding::has_children(&bookmarks, index) {
                                        let closed = self.collapsed.contains(&index);
                                        if self
                                            .hit(
                                                &format!("outline-fold-{index}"),
                                                crate::outline_folding::fold_button(ui, closed),
                                            )
                                            .clicked()
                                        {
                                            if closed {
                                                self.collapsed.remove(&index);
                                            } else {
                                                self.collapsed.insert(index);
                                            }
                                        }
                                    } else {
                                        ui.add_space(18.0);
                                    }
                                    if self
                                        .inline_rename
                                        .as_ref()
                                        .is_some_and(|r| r.index == index)
                                    {
                                        let rename = self.inline_rename.as_mut().unwrap();
                                        let saving = rename.result.is_some();
                                        let input = ui.add_enabled(
                                            !saving,
                                            egui::TextEdit::singleline(&mut rename.title)
                                                .id_salt(("outline-rename", index))
                                                .desired_width(ui.available_width())
                                                .char_limit(2048),
                                        );
                                        if rename.focus {
                                            input.request_focus();
                                            rename.focus = false;
                                        }
                                        let cancel = (input.has_focus() || input.lost_focus())
                                            && ui.input(|i| i.key_pressed(egui::Key::Escape));
                                        let commit = !saving
                                            && ((input.has_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                                || input.lost_focus());
                                        self.hit("inline-name-input", input);
                                        if cancel {
                                            self.inline_rename = None;
                                            self.outline_notice = None;
                                        } else if commit {
                                            self.save_inline_name(ctx);
                                        }
                                    } else {
                                        let entry = crate::outline_folding::outline_entry(
                                            ui,
                                            &mark.title,
                                            mark.page,
                                            mark.page == Some(self.page),
                                        );
                                        if self.smoke_features {
                                            if let Some(rect) = entry.page_rect {
                                                self.smoke_hits
                                                    .insert(format!("outline-page-{index}"), rect);
                                            }
                                            if let Some(rect) = entry.leader_rect {
                                                self.smoke_hits.insert(
                                                    format!("outline-leader-{index}"),
                                                    rect,
                                                );
                                            }
                                        }
                                        let response = entry.response.on_hover_text(format!(
                                            "{}\n单击跳转；双击名称直接修改，Enter 保存，Esc 取消",
                                            mark.title
                                        ));
                                        self.hit(
                                            &format!("outline-name-{index}"),
                                            response.clone(),
                                        );
                                        if let Some(page) = mark.page {
                                            self.hit(&format!("bookmark-{page}"), response.clone());
                                        }
                                        let same_name = self.last_outline_click.is_some_and(
                                            |(previous, time)| {
                                                previous == index
                                                    && time.elapsed() < Duration::from_millis(500)
                                            },
                                        );
                                        if response.clicked() {
                                            self.last_outline_click = Some((index, Instant::now()));
                                        }
                                        if same_name
                                            && (response.double_clicked()
                                                || response.triple_clicked())
                                            && self.inline_rename.is_none()
                                        {
                                            self.inline_rename =
                                                Some(crate::inline_outline::Rename::new(
                                                    index,
                                                    mark.title.clone(),
                                                ));
                                            self.outline_notice = None;
                                            ctx.request_repaint();
                                        } else if response.clicked()
                                            && let Some(page) = mark.page
                                        {
                                            self.goto(page);
                                        }
                                    }
                                });
                            }
                        });
                    if let Some(rename) = &self.inline_rename {
                        if rename.result.is_some() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("正在保存名称…");
                            });
                        } else {
                            ui.label(RichText::new("Enter 保存 · Esc 取消").small());
                        }
                        if let Some(error) = &rename.error {
                            ui.colored_label(RED, error);
                        }
                    }
                    if let Some(notice) = &self.outline_notice {
                        ui.label(RichText::new(notice).small());
                    }
                } else {
                    let mut scroll =
                        egui::ScrollArea::vertical().id_salt(("thumbnails", self.document_id));
                    if self.follow_navigation {
                        scroll = scroll
                            .vertical_scroll_offset((self.page as f32 * 160.0 - 130.0).max(0.0));
                        self.follow_navigation = false;
                    }
                    scroll.show_rows(ui, 160.0, self.pages() as usize, |ui, range| {
                        for n in range {
                            self.thumbnails.push(n as u32);
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 154.0),
                                egui::Sense::click(),
                            );
                            if n as u32 == self.page {
                                ui.painter().rect_filled(rect, 5.0, self.selection());
                            }
                            let target = RenderTarget {
                                page: n as u32,
                                width: 180,
                                rotation: self.rotation,
                                thumbnail: true,
                            };
                            let size = self.info.as_ref().map_or([612.0, 792.0], |i| i.sizes[n]);
                            let ratio = if self.rotation.is_multiple_of(2) {
                                size[0] / size[1]
                            } else {
                                size[1] / size[0]
                            };
                            let h = (116.0_f32).min(126.0 / ratio.max(0.01));
                            let w = h * ratio;
                            let paper = Rect::from_min_size(
                                egui::pos2(rect.center().x - w / 2.0, rect.top() + 9.0),
                                Vec2::new(w, h),
                            );
                            ui.painter().rect_filled(
                                paper,
                                1.0,
                                Color32::from_rgba_unmultiplied(245, 245, 245, 255),
                            );
                            if let Some(e) = self.cache.iter().find(|e| e.target == target) {
                                ui.painter().image(
                                    e.texture.id(),
                                    paper,
                                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            } else {
                                ui.painter().text(
                                    paper.center(),
                                    egui::Align2::CENTER_CENTER,
                                    (n + 1).to_string(),
                                    egui::FontId::proportional(16.0),
                                    Color32::GRAY,
                                );
                            }
                            ui.painter().rect_stroke(
                                paper,
                                1.0,
                                egui::Stroke::new(
                                    1.0_f32,
                                    if n as u32 == self.page {
                                        RED
                                    } else {
                                        Color32::GRAY
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            ui.painter().text(
                                egui::pos2(rect.center().x, rect.bottom() - 15.0),
                                egui::Align2::CENTER_CENTER,
                                format!("第 {} 页", n + 1),
                                egui::FontId::proportional(12.0),
                                if n as u32 == self.page {
                                    RED
                                } else {
                                    self.ink()
                                },
                            );
                            self.hit(&format!("thumbnail-{n}"), response.clone());
                            if response.clicked() {
                                self.goto(n as u32);
                            }
                        }
                    });
                }
            });
    }
    fn text(&self, ctx: &egui::Context) {
        if !self.text_panel {
            return;
        }
        egui::SidePanel::right("text")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("当前页文本");
                ui.label("可选择并复制");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(p) = self.current_image() {
                        if p.text.is_empty() {
                            ui.label("此页没有可提取的文本。");
                        } else {
                            ui.add(egui::Label::new(&p.text).selectable(true).wrap());
                        }
                        if p.text_truncated {
                            ui.label("仅显示前 100,000 个字符。");
                        }
                    }
                });
            });
    }
    fn content(&mut self, ctx: &egui::Context) {
        self.selection.auto_translation = self.ai.auto_translation;
        if self.selection.set_rotation(self.rotation) {
            self.selection_id += 1;
        }
        self.visible_missing = 0;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.background()))
            .show(ctx, |ui| {
                self.reader_center = ui.max_rect().center();
                if self.smoke_features {
                    self.smoke_hits.insert("reader-area".into(), ui.max_rect());
                }
                if self.path.is_none() {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.25).max(30.0));
                        ui.label(RichText::new("PDF").size(52.0).color(RED));
                        ui.label(RichText::new("用 fpdf，轻松阅读每一页").size(22.0));
                        ui.add_space(15.0);
                        ui.label("拖入 PDF，或打开一个文档");
                        ui.add_space(20.0);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("打开 PDF 文件").color(Color32::WHITE),
                                )
                                .fill(RED)
                                .min_size(Vec2::new(174.0, 42.0)),
                            )
                            .clicked()
                        {
                            self.choose_file();
                        }
                    });
                    return;
                }
                if let Some(error) = self.error.clone() {
                    egui::Frame::new()
                        .fill(self.panel())
                        .inner_margin(16)
                        .show(ui, |ui| {
                            ui.label(RichText::new(error).color(RED));
                            if self.need_password {
                                ui.horizontal(|ui| {
                                    let input = ui.add(
                                        egui::TextEdit::singleline(&mut self.password)
                                            .password(true)
                                            .hint_text("文档密码"),
                                    );
                                    if ui.button("解锁文档").clicked()
                                        || (input.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                    {
                                        self.need_password = false;
                                        self.error = None;
                                        self.document_id += 1;
                                        self.last_plan.clear();
                                    }
                                });
                            } else if ui.button("重新打开").clicked() {
                                self.choose_file();
                            }
                        });
                    return;
                }
                ui.style_mut().spacing.scroll = egui::style::ScrollStyle {
                    bar_width: 14.0,
                    handle_min_length: 36.0,
                    foreground_color: true,
                    ..egui::style::ScrollStyle::solid()
                };
                ui.visuals_mut().widgets.inactive.fg_stroke.color =
                    Color32::from_gray(if self.theme.is_dark() { 175 } else { 130 });
                let available =
                    (ui.available_width() - ui.spacing().scroll.allocated_width()).max(64.0);
                let scale = match self.zoom {
                    Zoom::FitWidth => None,
                    Zoom::FitPage => {
                        let size = self
                            .info
                            .as_ref()
                            .and_then(|i| i.sizes.get(self.page as usize))
                            .copied()
                            .unwrap_or([612.0, 792.0]);
                        let [w, h] = if self.rotation.is_multiple_of(2) {
                            size
                        } else {
                            [size[1], size[0]]
                        };
                        Some(
                            ((available - 64.0) / (w * 96.0 / 72.0))
                                .min((ui.available_height() - 48.0).max(32.0) / (h * 96.0 / 72.0))
                                .clamp(0.01, 5.0),
                        )
                    }
                    Zoom::Scale(v) => Some(v),
                };
                let key = (
                    available.round() as u32,
                    scale.map_or(0, f32::to_bits),
                    self.rotation,
                    self.mode,
                    if self.mode == ReadingMode::Single {
                        self.page
                    } else {
                        0
                    },
                );
                if self.layout_key != Some(key) {
                    if self.jump.is_none()
                        && let Some(p) = self.layout.placement(self.page)
                    {
                        self.jump = Some((
                            self.page,
                            ((self.scroll_y - p.top) / p.height).clamp(0.0, 1.0),
                        ));
                    }
                    let sizes = self
                        .info
                        .as_ref()
                        .map_or(&[[612.0, 792.0]][..], |i| i.sizes.as_slice());
                    self.layout = PageLayout::new(
                        sizes,
                        available,
                        scale,
                        self.rotation,
                        if self.mode == ReadingMode::Single {
                            Some(self.page)
                        } else {
                            None
                        },
                    );
                    self.layout_key = Some(key);
                }
                let mut scroll = egui::ScrollArea::both()
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .id_salt((
                        "document",
                        self.document_id,
                        self.mode == ReadingMode::Continuous,
                    ));
                if let Some((page, fraction)) = self.jump.take()
                    && let Some(p) = self.layout.placement(page)
                {
                    scroll = scroll
                        .vertical_scroll_offset((p.top + fraction * p.height - 12.0).max(0.0));
                }
                let mut targets = Vec::new();
                let output = scroll.show_viewport(ui, |ui, viewport| {
                    let extra = self
                        .layout
                        .pages
                        .last()
                        .map_or(0.0, |p| (viewport.height() - p.height - 24.0).max(0.0));
                    ui.set_min_size(Vec2::new(self.layout.width, self.layout.height + extra));
                    let origin = ui.min_rect().min;
                    if self.mode == ReadingMode::Continuous && self.pages() > 0 {
                        let page = self.layout.page_at_with_hint(
                            viewport.min.y + (viewport.height() * 0.2).min(100.0),
                            self.page,
                        );
                        if page != self.page {
                            self.page = page;
                            if !self.page_editing {
                                self.page_input = (page + 1).to_string();
                            }
                            self.follow_navigation = true;
                        }
                    }
                    let placements = self.layout.pages[self.layout.visible(
                        (viewport.min.y - viewport.height()).max(0.0),
                        viewport.max.y + viewport.height(),
                    )]
                    .to_vec();
                    for p in placements {
                        let target = RenderTarget {
                            page: p.page,
                            width: render_width(p.width, p.height, ctx.pixels_per_point()),
                            rotation: self.rotation,
                            thumbnail: false,
                        };
                        targets.push(target);
                        let rect = Rect::from_min_size(
                            origin + Vec2::new(p.left, p.top),
                            Vec2::new(p.width, p.height),
                        );
                        let paper = self.theme.pixel([255, 255, 255, 255]);
                        ui.painter().rect_filled(
                            rect,
                            1.0,
                            Color32::from_rgba_unmultiplied(paper[0], paper[1], paper[2], 255),
                        );
                        if let Some(entry) = self
                            .cache
                            .iter()
                            .filter(|e| {
                                e.target.page == target.page && e.target.rotation == target.rotation
                            })
                            .min_by_key(|e| {
                                (e.target.thumbnail, e.target.width.abs_diff(target.width))
                            })
                        {
                            ui.painter().image(
                                entry.texture.id(),
                                rect,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else if let Some(error) = self.errors.get(&target).cloned() {
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                error,
                                egui::FontId::proportional(14.0),
                                RED,
                            );
                            if ui
                                .put(
                                    Rect::from_center_size(
                                        rect.center() + Vec2::new(0.0, 40.0),
                                        Vec2::new(90.0, 30.0),
                                    ),
                                    egui::Button::new("重试此页"),
                                )
                                .clicked()
                            {
                                self.errors.remove(&target);
                                self.last_plan.clear();
                            }
                        } else {
                            let visible = Rect::from_min_max(
                                origin + viewport.min.to_vec2(),
                                origin + viewport.max.to_vec2(),
                            );
                            if rect.intersects(visible) {
                                self.visible_missing += 1;
                            }
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("正在加载第 {} 页…", p.page + 1),
                                egui::FontId::proportional(14.0),
                                self.muted(),
                            );
                        }
                        ui.painter().rect_stroke(
                            rect,
                            1.0,
                            egui::Stroke::new(
                                if self.layout.columns > 1 && p.page == self.page {
                                    2.0_f32
                                } else {
                                    1.0
                                },
                                if self.layout.columns > 1 && p.page == self.page {
                                    RED
                                } else {
                                    Color32::from_gray(160)
                                },
                            ),
                            egui::StrokeKind::Outside,
                        );
                        self.search.paint(ui, rect, p.page);
                        if let Some((selection, [r, g, b], rotation)) = &self.highlight_preview
                            && selection.page == p.page
                            && *rotation == self.rotation
                        {
                            if let Some(started) = self.smoke_ink_started.take() {
                                let latency = started.elapsed();
                                assert!(
                                    latency < Duration::from_millis(250),
                                    "Visible ink was delayed: {latency:?}"
                                );
                                self.smoke_ink_seen = true;
                                println!(
                                    "Highlighter ink painted before disk completion: {latency:?}"
                                );
                            }
                            for &[left, top, right, bottom] in &selection.rects {
                                ui.painter().rect_filled(
                                    Rect::from_min_max(
                                        rect.min
                                            + Vec2::new(left * rect.width(), top * rect.height()),
                                        rect.min
                                            + Vec2::new(
                                                right * rect.width(),
                                                bottom * rect.height(),
                                            ),
                                    ),
                                    0.0,
                                    Color32::from_rgba_unmultiplied(*r, *g, *b, 90),
                                );
                            }
                        }
                        let response = if self.highlight_pending.is_some() {
                            ui.interact(
                                rect,
                                ui.id().with(("saving-highlight", p.page)),
                                egui::Sense::hover(),
                            )
                        } else {
                            self.selection.page(ui, rect, p.page, self.rotation)
                        };
                        self.hit(&format!("selection-page-{}", p.page), response.clone());
                        if self.layout.columns > 1 {
                            self.hit(&format!("grid-page-{}", p.page), response.clone());
                            if response.clicked() {
                                self.goto(p.page);
                            }
                            ui.painter().text(
                                rect.center_bottom() + Vec2::new(0.0, 9.0),
                                egui::Align2::CENTER_CENTER,
                                (p.page + 1).to_string(),
                                egui::FontId::proportional(11.0),
                                self.muted(),
                            );
                        }
                    }
                });
                self.scroll_y = output.state.offset.y;
                let track = Rect::from_min_size(
                    egui::pos2(
                        output.inner_rect.right() + ui.spacing().scroll.bar_inner_margin,
                        output.inner_rect.top(),
                    ),
                    Vec2::new(14.0, output.inner_rect.height()),
                );
                let content_height = self.layout.height
                    + self.layout.pages.last().map_or(0.0, |p| {
                        (output.inner_rect.height() - p.height - 24.0).max(0.0)
                    });
                let handle_height = (track.height() * track.height() / content_height.max(1.0))
                    .clamp(36.0_f32.min(track.height()), track.height());
                let fraction = self.scroll_y / (content_height - track.height()).max(1.0);
                let handle = Rect::from_min_size(
                    track.min + Vec2::new(0.0, fraction * (track.height() - handle_height)),
                    Vec2::new(track.width(), handle_height),
                );
                if self.smoke_features {
                    self.smoke_hits.insert("reader-scroll-track".into(), track);
                    self.smoke_hits
                        .insert("reader-scroll-handle".into(), handle);
                }
                if self.audit_cached_scroll {
                    assert_eq!(
                        self.visible_missing, 0,
                        "cached pages must never fall back to loading placeholders while scrolling"
                    );
                }
                targets.sort_by_key(|t| t.page.abs_diff(self.page));
                for &page in &self.thumbnails {
                    targets.push(RenderTarget {
                        page,
                        width: 180,
                        rotation: self.rotation,
                        thumbnail: true,
                    });
                }
                self.submit(targets);
            });
    }
    fn click_editor_control(&mut self, name: &str) {
        self.smoke_click = Some(
            self.outline_editor
                .as_ref()
                .expect("editor open")
                .hits
                .get(name)
                .unwrap_or_else(|| panic!("editor control missing: {name}"))
                .center(),
        );
    }
    fn type_test_title(&mut self, title: &str) {
        self.smoke_events = vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..egui::Modifiers::NONE
                },
            },
            egui::Event::Text(title.into()),
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ];
    }
    fn double_click_test(&mut self, key: &str) {
        self.smoke_double_click = Some((self.smoke_hits[key].center(), 42));
    }
    fn smoke_reading_step(&mut self, ctx: &egui::Context) {
        match self.smoke_step {
            0 => self.click_test_control("outline-tab"),
            1 => {
                self.smoke_document_id = self.document_id;
                self.smoke_texture_ids = self
                    .cache
                    .iter()
                    .filter(|e| !e.target.thumbnail)
                    .map(|e| (e.target, e.texture.id()))
                    .collect();
                self.double_click_test("outline-name-0");
            }
            2 => {
                assert!(
                    self.inline_rename.is_some(),
                    "double click must create an inline text field"
                );
                assert!(
                    self.outline_editor.is_none(),
                    "double click must not open a modal editor"
                );
                self.type_test_title("双击直接修改目录");
            }
            3 => {
                assert!(
                    self.inline_rename.is_none(),
                    "Enter must save the inline name"
                );
                assert_eq!(
                    self.info.as_ref().unwrap().bookmarks[0].title,
                    "双击直接修改目录"
                );
                assert_eq!(
                    self.document_id, self.smoke_document_id,
                    "renaming must not reload the reader"
                );
                for (target, id) in &self.smoke_texture_ids {
                    assert!(
                        self.cache
                            .iter()
                            .any(|e| e.target == *target && e.texture.id() == *id),
                        "rename must retain existing textures"
                    );
                }
                self.double_click_test("outline-name-0");
            }
            4 => {
                self.type_test_title("此修改应取消");
                self.smoke_events.pop();
            }
            5 => {
                assert_eq!(self.inline_rename.as_ref().unwrap().title, "此修改应取消");
                self.smoke_events = vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }];
            }
            6 => {
                assert!(self.inline_rename.is_none());
                assert_eq!(
                    self.info.as_ref().unwrap().bookmarks[0].title,
                    "双击直接修改目录"
                );
                self.click_test_control("theme-menu");
            }
            7 => self.click_test_control("custom-theme"),
            8 => {
                assert!(self.rgb_window);
                self.double_click_test("rgb-R");
            }
            9 => self.type_test_title("210"),
            10 => self.double_click_test("rgb-G"),
            11 => self.type_test_title("225"),
            12 => self.double_click_test("rgb-B"),
            13 => self.type_test_title("240"),
            14 => {
                assert_eq!(
                    self.rgb_draft,
                    [210, 225, 240],
                    "RGB fields must accept keyboard input"
                );
                self.click_test_control("rgb-apply");
            }
            15 => {
                assert_eq!(self.theme, PaperTheme::Custom([210, 225, 240]));
                assert!(!self.rgb_window);
                let handle = self.smoke_hits["reader-scroll-handle"];
                assert!(handle.width() >= 14.0);
                self.smoke_events = vec![
                    egui::Event::PointerMoved(handle.center()),
                    egui::Event::PointerButton {
                        pos: handle.center(),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
            }
            16 => {
                let track = self.smoke_hits["reader-scroll-track"];
                self.smoke_events = vec![egui::Event::PointerMoved(egui::pos2(
                    track.center().x,
                    track.bottom() - 2.0,
                ))];
            }
            17 => {
                assert_eq!(
                    self.page, 2,
                    "dragging the right scrollbar must reach the final page"
                );
                let track = self.smoke_hits["reader-scroll-track"];
                let pos = egui::pos2(track.center().x, track.bottom() - 2.0);
                self.smoke_events = vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }];
            }
            18 => {
                self.zoom = Zoom::Scale(2.2);
                self.goto(0);
            }
            19 => self.goto(1),
            20 => self.goto(2),
            21 => self.goto(0),
            22 => {
                self.audit_cached_scroll = true;
                self.smoke_texture_ids = self
                    .cache
                    .iter()
                    .filter(|e| {
                        !e.target.thumbnail
                            && e.target.width
                                == render_width(
                                    self.layout.placement(e.target.page).unwrap().width,
                                    self.layout.placement(e.target.page).unwrap().height,
                                    ctx.pixels_per_point(),
                                )
                    })
                    .map(|e| (e.target, e.texture.id()))
                    .collect();
                assert_eq!(
                    self.smoke_texture_ids.len(),
                    3,
                    "all three full-size pages must be warm before auditing scroll frames"
                );
                self.goto(1);
            }
            23 => {
                self.smoke_scroll = Some(self.reader_center);
            }
            24 => {
                self.goto(2);
            }
            25 => {
                assert_eq!(self.visible_missing, 0);
                for (target, id) in &self.smoke_texture_ids {
                    assert!(
                        self.cache
                            .iter()
                            .any(|e| e.target == *target && e.texture.id() == *id),
                        "scrolling must not discard and recreate cached page textures"
                    );
                }
                self.audit_cached_scroll = false;
                self.zoom = Zoom::FitWidth;
                self.goto(0);
            }
            26 => {
                self.click_test_control("edit-outline");
            }
            27 => {
                assert_eq!(
                    self.outline_editor.as_ref().unwrap().entries()[0].title,
                    "双击直接修改目录",
                    "inline rename must persist on disk"
                );
                self.click_editor_control("close");
            }
            28 => self.click_test_control("status-background"),
            29 => self.click_test_control("custom-theme"),
            30 => {
                assert!(self.rgb_window);
                let rect = self.smoke_hits["rgb-palette"];
                self.smoke_click = Some(egui::pos2(
                    rect.left() + rect.width() / 3.0,
                    rect.top() + rect.height() * 0.4,
                ));
            }
            31 => {
                let [r, g, b] = self.rgb_draft;
                assert!(
                    u16::from(g) > u16::from(r) + 50 && u16::from(g) > u16::from(b) + 50,
                    "palette click must select the green point"
                );
                assert_eq!(
                    self.theme,
                    PaperTheme::Custom(self.rgb_draft),
                    "palette click must configure page color without pressing Confirm"
                );
                assert_eq!(self.applied_theme, Some(self.theme));
                assert!(self.rgb_window);
                let rect = self.smoke_hits["rgb-brightness"];
                self.smoke_click = Some(rect.center());
            }
            32 => {
                assert!(
                    self.rgb_draft[1].abs_diff(128) <= 1,
                    "brightness selection must synchronize RGB"
                );
                assert_eq!(self.theme, PaperTheme::Custom(self.rgb_draft));
                self.click_test_control("rgb-cancel");
            }
            33 => {
                assert_eq!(
                    self.theme,
                    PaperTheme::Custom([210, 225, 240]),
                    "Cancel must restore the previous reading color"
                );
                assert_eq!(self.custom_rgb, [210, 225, 240]);
                self.click_test_control("status-background");
            }
            34 => self.click_test_control("custom-theme"),
            35 => {
                let rect = self.smoke_hits["rgb-palette"];
                self.smoke_click = Some(egui::pos2(
                    rect.left() + rect.width() * 2.0 / 3.0,
                    rect.top() + rect.height() * 0.3,
                ));
            }
            36 => {
                let [r, g, b] = self.rgb_draft;
                assert!(u16::from(b) > u16::from(r) + 50 && u16::from(b) > u16::from(g) + 50);
                assert_eq!(self.theme, PaperTheme::Custom(self.rgb_draft));
                self.click_test_control("rgb-apply");
            }
            37 => {
                assert!(!self.rgb_window);
                assert_eq!(self.theme, PaperTheme::Custom(self.custom_rgb));
                self.click_test_control("theme-menu");
            }
            38 => self.click_test_control("custom-theme"),
            39 => {
                assert!(self.rgb_window);
                assert_eq!(self.rgb_draft, self.custom_rgb);
                println!(
                    "Reading UI verified: inline rename/cancel/persistence, RGB keyboard input, right scrollbar drag, cached continuous pages without placeholders or texture recreation"
                );
                println!(
                    "Palette UI verified: visible palette click configures reading color immediately and updates RGB; brightness click synchronizes values; Cancel restores old color; Confirm retains selection; top and bottom background entries work"
                );
            }
            _ => {}
        }
    }
    fn smoke_outline_step(&mut self) {
        match self.smoke_step {
            0 => self.click_test_control("outline-tab"),
            1 => self.click_test_control("edit-outline"),
            2 => self.click_editor_control("title"),
            3 => self.type_test_title("已修改目录：第一章"),
            4 => {
                assert_eq!(
                    self.outline_editor.as_ref().unwrap().entries()[0].title,
                    "已修改目录：第一章"
                );
                self.click_editor_control("add-child");
            }
            5 => self.click_editor_control("title"),
            6 => self.type_test_title("新增子章节"),
            7 => {
                assert_eq!(self.outline_editor.as_ref().unwrap().entries()[1].depth, 1);
                self.click_editor_control("outdent");
            }
            8 => {
                assert_eq!(self.outline_editor.as_ref().unwrap().entries()[1].depth, 0);
                self.click_editor_control("down");
            }
            9 => {
                assert_eq!(
                    self.outline_editor.as_ref().unwrap().entries()[2].title,
                    "新增子章节"
                );
                self.click_editor_control("up");
            }
            10 => {
                assert_eq!(
                    self.outline_editor.as_ref().unwrap().entries()[1].title,
                    "新增子章节"
                );
                self.click_editor_control("delete");
            }
            11 => {
                assert_eq!(self.outline_editor.as_ref().unwrap().entries().len(), 3);
                self.click_editor_control("undo");
            }
            12 => {
                assert_eq!(self.outline_editor.as_ref().unwrap().entries().len(), 4);
                self.click_editor_control("save");
            }
            13 => {
                assert!(
                    self.outline_editor.is_none(),
                    "save must complete and refresh PDF"
                );
                let marks = &self.info.as_ref().unwrap().bookmarks;
                assert_eq!(marks.len(), 4);
                assert_eq!(marks[0].title, "已修改目录：第一章");
                assert_eq!(marks[1].title, "新增子章节");
                self.click_test_control("bookmark-1");
            }
            14 => {
                assert_eq!(self.page, 1, "saved outline must navigate");
                self.click_test_control("edit-outline");
            }
            15 => {
                assert_eq!(
                    self.outline_editor.as_ref().unwrap().entries()[0].title,
                    "已修改目录：第一章"
                );
                println!(
                    "Outline UI verified: Chinese rename, child creation, outdent, down/up, delete/undo, atomic save, reload, bookmark navigation, reopen editor; original fixture untouched"
                );
            }
            _ => {}
        }
    }
    fn smoke(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let page = PageImage {
                    width: image.size[0] as u32,
                    height: image.size[1] as u32,
                    rgba: image.pixels.iter().flat_map(|p| p.to_array()).collect(),
                    points: [0.0; 2],
                    text: String::new(),
                    text_truncated: false,
                };
                if let Err(e) = save_png(&path, &page) {
                    eprintln!("screenshot failed: {e}");
                    std::process::exit(1);
                }
                println!("UI screenshot: {}", path.display());
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.started.elapsed() > Duration::from_secs(if self.smoke_ai { 180 } else { 60 }) {
            eprintln!("UI smoke test timed out: {:?}", self.error);
            std::process::exit(1);
        }
        let ready = self.info.is_some()
            && self.error.is_none()
            && self.smoke_double_click.is_none()
            && self.smoke_drag.is_none()
            && self.highlight_pending.is_none()
            && self
                .highlight_stale
                .iter()
                .all(|t| !self.wanted.contains(t))
            && self
                .inline_rename
                .as_ref()
                .is_none_or(|r| r.result.is_none())
            && self.outline_editor.as_ref().is_none_or(|e| e.is_ready())
            && self
                .wanted
                .iter()
                .filter(|t| !t.thumbnail)
                .all(|t| self.cache.iter().any(|e| e.target == *t));
        if ready {
            self.settled_frames += 1;
        } else {
            self.settled_frames = 0;
        }
        if self.settled_frames > 8 {
            if self.smoke_highlight && self.smoke_step < 27 {
                match self.smoke_step {
                    0 => {
                        self.goto(2);
                        self.zoom = Zoom::FitPage;
                    }
                    1 => {
                        let rect = self.smoke_hits["selection-page-2"];
                        let point = rect.min
                            + Vec2::new(150.0 / 612.0 * rect.width(), 72.0 / 792.0 * rect.height());
                        self.smoke_double_click = Some((point, 42));
                    }
                    2 => {
                        if self
                            .selection
                            .current
                            .as_ref()
                            .is_none_or(|s| s.text != "page")
                        {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.smoke_highlight_before = Some(
                            self.cache
                                .iter()
                                .find(|e| e.target.page == 2 && !e.target.thumbnail)
                                .unwrap()
                                .image
                                .clone(),
                        );
                        self.smoke_document_id = self.document_id;
                        self.click_test_control("highlight-selection");
                    }
                    3 => {
                        assert!(
                            self.smoke_ink_seen,
                            "Selected-text highlight must paint immediately"
                        );
                        if self.highlight_pending.is_some() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        assert_eq!(
                            self.document_id,
                            self.smoke_document_id + 1,
                            "{}",
                            self.selection.notice
                        );
                        let image = &self
                            .cache
                            .iter()
                            .find(|e| e.target.page == 2 && !e.target.thumbnail)
                            .unwrap()
                            .image;
                        let before = self.smoke_highlight_before.as_ref().unwrap();
                        assert_eq!(before.width, image.width);
                        assert_ne!(
                            before.rgba, image.rgba,
                            "Saved annotation must change the displayed page bitmap"
                        );
                        save_png(&path.with_file_name("highlight-before.png"), before).unwrap();
                        save_png(&path.with_file_name("highlight-after.png"), image).unwrap();
                        println!(
                            "Selected-text highlight changed actual page pixels after save/cache reload"
                        );
                    }
                    4 => {
                        assert!(self.highlight_tool);
                        self.smoke_document_id = self.document_id;
                        let rect = self.smoke_hits["selection-page-2"];
                        let start = rect.min
                            + Vec2::new(50.0 / 612.0 * rect.width(), 72.0 / 792.0 * rect.height());
                        let end = rect.min
                            + Vec2::new(
                                300.0 / 612.0 * rect.width(),
                                127.0 / 792.0 * rect.height(),
                            );
                        self.smoke_drag = Some((start, end, 0));
                    }
                    5 => {
                        if self.highlight_pending.is_some()
                            || self.document_id == self.smoke_document_id
                        {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        assert!(self.highlight_tool);
                        let saved = lopdf::Document::load(self.path.as_ref().unwrap()).unwrap();
                        let page = saved.get_dictionary(saved.get_pages()[&3]).unwrap();
                        let annotations = page.get(b"Annots").unwrap().as_array().unwrap();
                        assert_eq!(
                            annotations.len(),
                            2,
                            "Both selected-text and tool-first highlights must persist"
                        );
                        let last = saved
                            .get_dictionary(annotations[1].as_reference().unwrap())
                            .unwrap();
                        assert!(
                            last.get(b"QuadPoints").unwrap().as_array().unwrap().len() >= 16,
                            "Multiline drag must annotate multiple lines"
                        );
                        std::fs::copy(
                            self.path.as_ref().unwrap(),
                            path.with_file_name("highlight-saved.pdf"),
                        )
                        .unwrap();
                        self.smoke_events = vec![egui::Event::Key {
                            key: egui::Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }];
                    }
                    6 => {
                        assert!(!self.highlight_tool, "Escape must leave highlighter mode");
                        self.click_test_control("highlight-selection");
                    }
                    7 => {
                        assert!(
                            self.highlight_tool,
                            "Tool must activate before selecting any text"
                        );
                        self.click_test_control("highlight-selection");
                    }
                    8 => {
                        assert!(!self.highlight_tool);
                        let saved = self.path.clone().unwrap();
                        self.open(saved);
                        self.goto(2);
                    }
                    9 => {
                        self.goto(2);
                        self.theme = PaperTheme::Night;
                    }
                    10 => {
                        self.theme = PaperTheme::Green;
                    }
                    11 => {
                        self.theme = PaperTheme::Warm;
                    }
                    12 => {
                        self.theme = PaperTheme::Night;
                        println!(
                            "Highlighter UI verified: select-first, tool-first multiline drag, persistent tool, Escape, button exit, two saved annotations and reopen; night hue/contrast covered by pixel regression"
                        );
                    }
                    13 => self.click_test_control("highlight-colors"),
                    14 => self.click_test_control("highlight-custom-color"),
                    15 => {
                        let rect = self.smoke_hits["highlight-palette"];
                        self.smoke_click =
                            Some(rect.min + Vec2::new(rect.width() / 3.0, rect.height() * 0.06));
                    }
                    16 => {
                        assert!(self.highlight_color[1] > self.highlight_color[0] + 100);
                        assert!(self.highlight_color[1] > self.highlight_color[2] + 100);
                        self.click_test_control("highlight-color-apply");
                    }
                    17 => {
                        assert!(!self.highlight_palette_open);
                        self.smoke_document_id = self.document_id;
                        self.click_test_control("highlight-selection");
                    }
                    18 => {
                        assert!(self.highlight_tool);
                        let rect = self.smoke_hits["selection-page-2"];
                        let point = rect.min
                            + Vec2::new(75.0 / 612.0 * rect.width(), 162.0 / 792.0 * rect.height());
                        self.smoke_double_click = Some((point, 42));
                    }
                    19 => {
                        if self.document_id == self.smoke_document_id
                            || self.highlight_pending.is_some()
                        {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        let saved = lopdf::Document::load(self.path.as_ref().unwrap()).unwrap();
                        let annots = saved
                            .get_dictionary(saved.get_pages()[&3])
                            .unwrap()
                            .get(b"Annots")
                            .unwrap()
                            .as_array()
                            .unwrap();
                        assert_eq!(annots.len(), 3);
                        let color = saved
                            .get_dictionary(annots[2].as_reference().unwrap())
                            .unwrap()
                            .get(b"C")
                            .unwrap()
                            .as_array()
                            .unwrap();
                        for (channel, expected) in color.iter().zip(self.highlight_color) {
                            assert!(
                                (f64::from(channel.as_float().unwrap())
                                    - f64::from(expected) / 255.0)
                                    .abs()
                                    < 0.001
                            );
                        }
                        std::fs::copy(
                            self.path.as_ref().unwrap(),
                            path.with_file_name("highlight-saved.pdf"),
                        )
                        .unwrap();
                        self.click_test_control("highlight-colors");
                    }
                    20 => self.click_test_control("highlight-custom-color"),
                    21 => {
                        assert!(self.highlight_palette_open);
                        let rect = self.smoke_hits["highlight-palette"];
                        self.smoke_click =
                            Some(rect.min + Vec2::new(rect.width() * 0.01, rect.height() * 0.05));
                    }
                    22 => {
                        assert_ne!(self.highlight_color, self.highlight_before);
                        self.click_test_control("highlight-color-cancel");
                    }
                    23 => {
                        assert!(!self.highlight_palette_open);
                        assert_eq!(self.highlight_color, self.highlight_before);
                        println!(
                            "Color palette actual click/apply/cancel and custom RGB annotation persistence verified"
                        );
                        self.click_test_control("highlight-colors");
                    }
                    24 => self.click_test_control("highlight-custom-color"),
                    25 => {
                        assert!(self.highlight_palette_open);
                    }
                    26 => {
                        assert!(self.smoke_hits.contains_key("highlight-R"));
                    }
                    _ => {}
                }
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if self.smoke_settings && self.smoke_step < 12 {
                match self.smoke_step {
                    0 => self.click_test_control("ask-selection"),
                    1 => {
                        assert!(
                            self.ai.controls.contains_key("ai-config-endpoint"),
                            "Missing config must open inline setup immediately"
                        );
                        self.click_test_control("ai-config-endpoint");
                    }
                    2 => self.type_test_title(&std::env::var("FPDF_SMOKE_ENDPOINT").unwrap()),
                    3 => self.click_test_control("ai-config-key"),
                    4 => self.type_test_title(&std::env::var("FPDF_SMOKE_KEY").unwrap()),
                    5 => self.click_test_control("ai-config-save"),
                    6 => {
                        if !self.ai.ready {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        let path = PathBuf::from(std::env::var_os("APPDATA").unwrap())
                            .join("fpdf/agent.txt");
                        let saved = std::fs::read_to_string(path)
                            .expect("Installed settings must persist in isolated APPDATA");
                        let lines: Vec<_> = saved.lines().collect();
                        assert!(
                            lines.first().is_some_and(|line| *line
                                == std::env::var("FPDF_SMOKE_ENDPOINT")
                                    .unwrap()
                                    .trim_end_matches('/')),
                            "Saved endpoint must match UI input"
                        );
                        assert!(
                            lines.get(1).is_some_and(
                                |line| *line == std::env::var("FPDF_SMOKE_KEY").unwrap()
                            ),
                            "Saved key must match masked UI input"
                        );
                        self.ai.busy = true;
                        self.click_test_control("ai-config");
                    }
                    7 => {
                        assert!(
                            self.ai.controls.contains_key("ai-config-endpoint"),
                            "API setup must remain available while AI is busy"
                        );
                        self.ai.busy = false;
                        self.click_test_control("ai-config");
                    }
                    8 => {
                        assert!(
                            !self.ai.controls.contains_key("ai-config-endpoint"),
                            "Close setup before capturing screenshots"
                        );
                        self.click_test_control("ai-question-input");
                    }
                    9 => self.type_test_title("请说明当前页内容，并引用页码。"),
                    10 => self.click_test_control("ai-question-send"),
                    11 => {
                        if self.ai.is_working() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.ai.assert_smoke_answer();
                        assert_eq!(self.ai.translation_run_count(), 0);
                        println!(
                            "Installed settings UI verified: fresh isolated APPDATA, masked API config save, actual connection, setup usable while busy, direct question and cited answer without translation"
                        );
                    }
                    _ => {}
                }
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if self.smoke_search && self.smoke_step < 27 {
                match self.smoke_step {
                    0 => {
                        self.smoke_events = vec![egui::Event::Key {
                            key: egui::Key::F,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::CTRL,
                        }];
                    }
                    1 => {
                        assert!(self.search.visible, "Ctrl+F must open document search");
                        self.click_test_control("search-input");
                    }
                    2 => self.type_test_title("page"),
                    3 => {
                        if self.search.busy || self.search.matches.is_empty() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        assert!(self.search.matches.iter().any(|m| m.page == 0));
                        assert!(
                            self.search.matches.iter().any(|m| m.page == 2),
                            "Search must include pages outside current viewport"
                        );
                        self.click_test_control("search-result-first");
                    }
                    4 => {
                        assert_eq!(self.search.selected, Some(0));
                        assert_eq!(self.page, self.search.matches[0].page);
                        assert!(
                            self.search.matches[0]
                                .rects
                                .iter()
                                .all(|r| r[2] > r[0] && r[3] > r[1])
                        );
                        self.click_test_control("search-next");
                    }
                    5 => {
                        assert_eq!(self.search.selected, Some(1));
                        self.click_test_control("search-whole-word");
                    }
                    6 => {
                        if self.search.busy || self.search.matches.is_empty() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.click_test_control("search-close");
                    }
                    7 => {
                        assert!(!self.search.visible);
                        self.click_test_control("ask-selection");
                    }
                    8 => {
                        assert!(
                            self.ai.visible && self.ai.controls.contains_key("ai-question-input"),
                            "Question input must be directly visible before any translation"
                        );
                        assert_eq!(self.ai.translation_run_count(), 0);
                        self.ai.visible = false;
                        self.goto(2);
                    }
                    9 => {
                        let rect = self.smoke_hits["selection-page-2"];
                        let point = rect.min
                            + Vec2::new(150.0 / 612.0 * rect.width(), 72.0 / 792.0 * rect.height());
                        self.smoke_double_click = Some((point, 42));
                    }
                    10 => {
                        if self
                            .selection
                            .current
                            .as_ref()
                            .is_none_or(|s| s.text != "page")
                        {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.smoke_document_id = self.document_id;
                        self.click_test_control("highlight-selection");
                    }
                    11 => {
                        if self.highlight_pending.is_some() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        assert_eq!(
                            self.document_id,
                            self.smoke_document_id + 1,
                            "Highlight must save and invalidate page cache"
                        );
                        assert!(self.selection.notice.contains("已保存"));
                        self.goto(2);
                    }
                    12 => self.click_test_control("ask-selection"),
                    13 => {
                        assert!(self.ai.controls.contains_key("ai-question-input"));
                        self.ai.visible = false;
                        self.selection.current = None;
                        self.open_search();
                    }
                    14 => self.click_test_control("search-input"),
                    15 => self.type_test_title("page"),
                    16 => {
                        if self.search.busy || self.search.matches.is_empty() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.click_test_control("search-next");
                        println!(
                            "Whole-PDF Ctrl+F search, right snippets, hit geometry, next result, whole-word settings, saved highlight/reload and direct AI question entry verified"
                        );
                    }
                    17 => self.click_test_control("ask-selection"),
                    18 => {
                        assert!(
                            self.search.visible && self.ai.visible,
                            "Search and AI must coexist"
                        );
                        assert!(self.ai.controls.contains_key("ai-question-input"));
                        assert!(self.search.controls.contains_key("search-input"));
                        self.ai.busy = true;
                        self.click_test_control("search-next");
                    }
                    19 => {
                        assert!(
                            self.search.visible && self.ai.visible && self.ai.busy,
                            "Search must preserve the active AI task"
                        );
                        self.ai.busy = false;
                        self.click_test_control("search-close");
                    }
                    20 => {
                        assert!(
                            !self.search.visible && self.ai.visible,
                            "Closing search must retain AI"
                        );
                        self.open_search();
                    }
                    21 => {
                        assert!(
                            self.search.visible && self.ai.visible,
                            "Opening search must retain AI"
                        );
                        self.click_test_control("ai-close");
                    }
                    22 => {
                        assert!(
                            self.search.visible && !self.ai.visible,
                            "Closing AI must retain search"
                        );
                        self.ai.open_question();
                        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                        for (name, shot) in [
                            ("reading-demo.pdf", "multi-window-a.png"),
                            ("grid-demo.pdf", "multi-window-b.png"),
                        ] {
                            let fixture = root.join("tests/fixtures").join(name);
                            let screenshot = self
                                .screenshot
                                .as_ref()
                                .unwrap()
                                .parent()
                                .unwrap()
                                .join(shot);
                            let mut command =
                                crate::window_actions::window_command(Some(&fixture)).unwrap();
                            command
                                .arg("--smoke-ui")
                                .arg(&fixture)
                                .arg("--screenshot")
                                .arg(screenshot);
                            self.smoke_windows.push(command.spawn().unwrap());
                        }
                        assert_ne!(self.smoke_windows[0].id(), self.smoke_windows[1].id());
                        assert!(
                            self.smoke_windows
                                .iter_mut()
                                .all(|c| c.try_wait().unwrap().is_none())
                        );
                    }
                    23 => {
                        for child in &mut self.smoke_windows {
                            let Some(status) = child.try_wait().unwrap() else {
                                ctx.request_repaint_after(Duration::from_millis(100));
                                return;
                            };
                            assert!(
                                status.success(),
                                "Each independent window must render its own PDF"
                            );
                        }
                        self.uninstall_confirm = true;
                    }
                    24 => {
                        assert!(self.uninstall_confirm);
                        self.click_test_control("uninstall-cancel");
                    }
                    25 => {
                        assert!(
                            !self.uninstall_confirm,
                            "Cancel must not uninstall or close the reader"
                        );
                        assert!(self.search.visible && self.ai.visible);
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            900.0, 680.0,
                        )));
                    }
                    26 => {
                        assert!(
                            self.smoke_hits["reader-area"].width() >= 150.0,
                            "Both sidebars must leave a usable reader area"
                        );
                        assert!(
                            !self.search.controls["search-input"]
                                .intersects(self.ai.controls["ai-question-input"])
                        );
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1200.0, 860.0,
                        )));
                        println!(
                            "Search/AI coexistence, independent closes, retained active AI state, compact layout, multiple PDF windows and cancel-safe uninstall confirmed"
                        );
                    }
                    _ => {}
                }
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if self.smoke_grid && self.smoke_step < 16 {
                match self.smoke_step {
                    0 | 3 | 6 | 9 | 12 => self.click_test_control("zoom-percent"),
                    1 => self.type_test_title("50%"),
                    2 => {
                        assert_eq!(self.layout.columns, 2);
                        self.click_test_control("grid-page-1");
                    }
                    4 => {
                        assert_eq!(self.page, 1);
                        self.type_test_title("25%");
                    }
                    5 => {
                        assert_eq!(self.layout.columns, 4);
                        self.click_test_control("grid-page-3");
                    }
                    7 => {
                        assert_eq!(self.page, 3);
                        self.type_test_title("10%");
                    }
                    8 => {
                        assert_eq!(self.layout.columns, 8);
                        self.click_test_control("grid-page-7");
                    }
                    10 => {
                        assert_eq!(self.page, 7);
                        self.type_test_title("100%");
                    }
                    11 => {
                        assert_eq!(self.layout.columns, 1);
                        assert_eq!(self.page, 7);
                    }
                    13 => self.type_test_title("10%"),
                    14 => {
                        assert_eq!(self.layout.columns, 8);
                        assert_eq!(self.page, 7);
                    }
                    15 => println!(
                        "Actual percentage input: 50% two columns, 25% four columns, 10% eight columns, page clicks and zoom restoration verified"
                    ),
                    _ => {}
                }
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if self.smoke_ai && self.smoke_step < 23 {
                if self.ai.translating() && self.ai.controls.contains_key("ai-translating") {
                    self.smoke_translation_progress = true;
                }
                match self.smoke_step {
                    0 => self.click_test_control("outline-tab"),
                    1 => self.click_test_control("outline-fold-0"),
                    2 => {
                        assert_eq!(
                            crate::outline_folding::visible_indices(
                                &self.info.as_ref().unwrap().bookmarks,
                                &self.collapsed
                            ),
                            [0, 3, 4]
                        );
                        self.click_test_control("outline-fold-0");
                    }
                    3 => self.click_test_control("outline-collapse-all"),
                    4 => {
                        assert_eq!(self.collapsed.len(), 2);
                        self.click_test_control("outline-expand-all");
                    }
                    5 => {
                        assert!(self.collapsed.is_empty());
                        self.click_test_control("bookmark-2");
                    }
                    6 => {
                        assert_eq!(self.page, 2);
                        self.click_test_control("ai-reader");
                    }
                    7 => self.click_test_control("ai-connect"),
                    8 => {
                        if !self.ai.ready {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.click_test_control("ai-summary");
                    }
                    9 => {
                        if self.ai.busy {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.ai.assert_smoke_answer();
                        self.goto(0);
                    }
                    10 => self.click_test_control("ai-citation"),
                    11 => {
                        assert_eq!(self.page, 2, "AI citation must jump to the cited page");
                        println!(
                            "Nested outline collapse/expand, child jump, real pi API PDF tool, streaming answer and citation jump verified"
                        );
                    }
                    12 => {
                        self.ai.reset();
                        self.selection.current = None;
                        self.click_test_control("ai-auto-translate");
                    }
                    13 => {
                        assert_eq!(self.ai.auto_translation, Some(false));
                        assert!(
                            !self.ai.is_working(),
                            "Arming without a selection must not send a request"
                        );
                        self.click_test_control("ai-auto-translate");
                    }
                    14 => {
                        assert_eq!(self.ai.auto_translation, None);
                        let rect = self.smoke_hits["selection-page-2"];
                        let point = rect.min
                            + Vec2::new(150.0 / 612.0 * rect.width(), 72.0 / 792.0 * rect.height());
                        self.smoke_double_click = Some((point, 42));
                    }
                    15 => {
                        if self.selection.current.is_none() {
                            assert!(
                                !self.selection.notice.contains("请从文字"),
                                "{}",
                                self.selection.notice
                            );
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        assert_eq!(
                            self.selection.current.as_ref().unwrap().text,
                            "page",
                            "Double click must select one whole word"
                        );
                        assert_eq!(
                            self.ai.translation_run_count(),
                            0,
                            "Disabled mode must not translate selection"
                        );
                        self.click_test_control("ai-translate");
                    }
                    16 => {
                        if self.ai.is_working() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.ai.assert_direct_translation();
                        assert_eq!(
                            self.ai.auto_translation, None,
                            "The original button remains a one-shot operation"
                        );
                        assert_eq!(self.ai.translation_run_count(), 1);
                        self.selection.current = None;
                        self.click_test_control("ai-auto-translate");
                    }
                    17 => {
                        assert_eq!(self.ai.auto_translation, Some(false));
                        assert!(!self.ai.is_working());
                        let rect = self.smoke_hits["selection-page-2"];
                        let start = rect.min
                            + Vec2::new(50.0 / 612.0 * rect.width(), 72.0 / 792.0 * rect.height());
                        let end = rect.min
                            + Vec2::new(
                                300.0 / 612.0 * rect.width(),
                                127.0 / 792.0 * rect.height(),
                            );
                        self.smoke_drag = Some((start, end, 0));
                    }
                    18 => {
                        if self
                            .selection
                            .current
                            .as_ref()
                            .is_none_or(|s| !s.text.contains("third and final"))
                        {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        if self.ai.is_working() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.ai.assert_direct_translation();
                        assert!(self.selection.current.as_ref().unwrap().rects.len() >= 2);
                        assert_eq!(
                            self.ai.translation_run_count(),
                            2,
                            "A second committed selection must translate automatically, once"
                        );
                        self.click_test_control("ai-explain");
                    }
                    19 => {
                        if self.ai.is_working() {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.ai.assert_direct_translation();
                        assert_eq!(
                            self.ai.auto_translation,
                            Some(false),
                            "One-shot detailed translation must not change the armed mode"
                        );
                        assert_eq!(self.ai.translation_run_count(), 3);
                        self.selection.current = None;
                        self.click_test_control("ai-auto-explain");
                    }
                    20 => {
                        assert_eq!(self.ai.auto_translation, Some(true));
                        assert!(!self.ai.is_working());
                        let rect = self.smoke_hits["selection-page-2"];
                        let point = rect.min
                            + Vec2::new(150.0 / 612.0 * rect.width(), 72.0 / 792.0 * rect.height());
                        self.smoke_double_click = Some((point, 42));
                    }
                    21 => {
                        if self
                            .selection
                            .current
                            .as_ref()
                            .is_none_or(|s| s.text != "page")
                            || self.ai.is_working()
                        {
                            ctx.request_repaint_after(Duration::from_millis(100));
                            return;
                        }
                        self.ai.assert_direct_translation();
                        assert_eq!(self.ai.auto_translation, Some(true));
                        assert_eq!(
                            self.ai.translation_run_count(),
                            4,
                            "Detailed mode stays enabled for the next word"
                        );
                        assert!(
                            self.smoke_translation_progress,
                            "The UI must display the translating indicator"
                        );
                    }
                    22 => println!(
                        "Original one-shot translation and explanation preserved; separate persistent toggles verified: arm/disable, auto word/multiline on release only, mutually exclusive modes and translating indicator; GLM-5.3-Flash"
                    ),
                    _ => {}
                }
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if self.smoke_reader && self.smoke_step < 40 {
                self.smoke_reading_step(ctx);
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if self.smoke_outline && self.smoke_step < 16 {
                self.smoke_outline_step();
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if self.smoke_features
                && !self.smoke_outline
                && !self.smoke_reader
                && !self.smoke_ai
                && !self.smoke_grid
                && !self.smoke_search
                && !self.smoke_settings
                && !self.smoke_highlight
                && self.smoke_step < 30
            {
                match self.smoke_step {
                    0 => {
                        assert!(self.pages() >= 3);
                        self.click_test_control("thumbnail-2");
                    }
                    1 => {
                        assert_eq!(self.page, 2);
                        assert_eq!(self.layout.page_at(self.scroll_y + 30.0), 2);
                        self.click_test_control("outline-tab");
                    }
                    2 => {
                        assert!(self.navigation_tab == NavigationTab::Outline);
                        let info = self.info.as_ref().unwrap();
                        let right = self.smoke_hits["outline-page-0"].right();
                        for (index, mark) in info
                            .bookmarks
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| m.page.is_some())
                        {
                            let number = self.smoke_hits[&format!("outline-page-{index}")];
                            assert!(
                                (number.right() - right).abs() < 1.0,
                                "All page numbers must share the same right-aligned column"
                            );
                            if let Some(stars) =
                                self.smoke_hits.get(&format!("outline-leader-{index}"))
                            {
                                assert!(
                                    stars.right() < number.left(),
                                    "Leaders must not overlap the page number"
                                );
                            }
                            assert!(
                                !mark.title.contains('*'),
                                "Display leaders must never be appended to stored titles"
                            );
                        }
                        self.click_test_control("outline-page-1");
                    }
                    3 => {
                        assert_eq!(self.page, 1);
                        self.click_test_control("back");
                    }
                    4 => {
                        assert_eq!(self.page, 2);
                        self.click_test_control("forward");
                    }
                    5 => {
                        assert_eq!(self.page, 1);
                        self.click_test_control("single");
                    }
                    6 => {
                        assert_eq!(self.layout.pages.len(), 1);
                        self.click_test_control("continuous");
                    }
                    7 => {
                        assert_eq!(self.layout.pages.len(), 3);
                        self.click_test_control("eye");
                    }
                    8 => {
                        assert_eq!(self.theme, PaperTheme::Green);
                        self.click_test_control("home");
                    }
                    9 => {
                        assert_eq!(self.page, 0);
                        self.click_test_control("thumbnails-tab");
                    }
                    10 => {
                        self.click_test_control("rotate");
                    }
                    11 => {
                        assert_eq!(self.rotation, 1);
                        self.rotation = 0;
                        self.zoom = Zoom::Scale(0.5);
                    }
                    12 => {
                        self.smoke_scroll = Some(self.reader_center);
                    }
                    13 => {
                        assert!(self.page > 0, "scrolling must update current page");
                        self.click_test_control("home");
                    }
                    14 => {
                        assert_eq!(self.page, 0);
                    }
                    15 => self.click_test_control("actual-size"),
                    16 => {
                        assert!((self.effective_zoom() - 1.0).abs() < 0.01);
                        self.smoke_zoom_before = self.layout.placement(0).unwrap().width;
                        self.click_test_control("zoom-in");
                    }
                    17 => {
                        assert!(
                            self.layout.placement(0).unwrap().width > self.smoke_zoom_before * 1.1,
                            "plus must enlarge actual page geometry"
                        );
                        self.click_test_control("zoom-out");
                    }
                    18 => {
                        assert!(
                            (self.layout.placement(0).unwrap().width - self.smoke_zoom_before)
                                .abs()
                                < 1.0
                        );
                        self.click_test_control("zoom-in-ribbon");
                    }
                    19 => {
                        assert!(self.effective_zoom() > 1.1);
                        self.click_test_control("zoom-out-ribbon");
                    }
                    20 => {
                        assert!((self.effective_zoom() - 1.0).abs() < 0.01);
                        self.smoke_ctrl_wheel = true;
                    }
                    21 => {
                        assert!((self.effective_zoom() - 1.2).abs() < 0.01);
                        assert_eq!(
                            ctx.zoom_factor(),
                            1.0,
                            "document zoom must leave toolbar scale unchanged"
                        );
                        self.click_test_control("zoom-percent");
                    }
                    22 => {
                        self.smoke_events = vec![
                            egui::Event::Key {
                                key: egui::Key::A,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: egui::Modifiers {
                                    ctrl: true,
                                    command: true,
                                    ..egui::Modifiers::NONE
                                },
                            },
                            egui::Event::Text("175%".into()),
                            egui::Event::Key {
                                key: egui::Key::Enter,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ];
                    }
                    23 => {
                        assert!(
                            (self.effective_zoom() - 1.75).abs() < 0.01,
                            "percentage input must apply on Enter"
                        );
                        assert!(
                            (self.layout.placement(0).unwrap().width / self.smoke_zoom_before
                                - 1.75)
                                .abs()
                                < 0.01
                        );
                        let pos = self.smoke_hits["zoom-slider"].center();
                        self.smoke_events = vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ];
                    }
                    24 => {
                        let pos = self.smoke_hits["zoom-slider"].center();
                        self.smoke_events = vec![egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers: egui::Modifiers::NONE,
                        }];
                        assert!(
                            (self.effective_zoom() - 1.75).abs() > 0.1,
                            "slider must update document zoom"
                        );
                    }
                    25 => self.click_test_control("fit-page"),
                    26 => {
                        assert!(self.zoom == Zoom::FitPage);
                        assert!(self.layout.placement(0).unwrap().height < 650.0);
                        self.click_test_control("fit-width-status");
                    }
                    27 => {
                        assert!(self.zoom == Zoom::FitWidth);
                        self.click_test_control("actual-size");
                    }
                    28 => {
                        assert!((self.effective_zoom() - 1.0).abs() < 0.01);
                        self.smoke_events = vec![egui::Event::Key {
                            key: egui::Key::Minus,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::CTRL,
                        }];
                    }
                    29 => {
                        assert!((self.effective_zoom() - 1.0 / 1.2).abs() < 0.01);
                        self.zoom = Zoom::FitWidth;
                        self.navigation_tab = NavigationTab::Outline;
                        println!(
                            "Zoom UI verified: status/ribbon plus and minus change page width, percentage Enter, slider, Ctrl+wheel, Ctrl+minus, actual size, fit width/page; toolbar scale unchanged"
                        );
                    }
                    _ => {}
                }
                self.smoke_step += 1;
                self.settled_frames = 0;
                ctx.request_repaint();
            } else if !self.screenshot_sent {
                if ctx.input(|i| {
                    i.viewport().minimized.unwrap_or(false)
                        || i.viewport()
                            .inner_rect
                            .is_some_and(|rect| rect.width() < 1.0 || rect.height() < 1.0)
                }) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.request_repaint_after(Duration::from_millis(100));
                    return;
                }
                if !self.screenshot_prepared
                    && self.smoke_features
                    && !self.smoke_outline
                    && !self.smoke_reader
                    && !self.smoke_ai
                    && !self.smoke_grid
                    && !self.smoke_search
                    && !self.smoke_settings
                    && !self.smoke_highlight
                {
                    assert_eq!(self.theme, PaperTheme::Green);
                    assert!(self.layout.pages.len() >= 3);
                    println!(
                        "Actual UI clicks and wheel verified: thumbnail/bookmark jumps, back/forward, home, single/continuous, rotation, eye mode, scroll/page synchronization"
                    );
                }
                if !self.screenshot_prepared {
                    self.screenshot_prepared = true;
                    if std::env::args_os().any(|a| a == "--screenshot-night") {
                        self.theme = PaperTheme::Night;
                        self.settled_frames = 0;
                        ctx.request_repaint();
                        return;
                    }
                }
                self.screenshot_sent = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
        ctx.request_repaint_after(Duration::from_millis(60));
    }
}
fn texture(
    ctx: &egui::Context,
    theme: PaperTheme,
    target: RenderTarget,
    image: &PageImage,
) -> egui::TextureHandle {
    let rgba = theme.rgba(&image.rgba);
    ctx.load_texture(
        format!("page-{}-{}-{}", target.page, target.width, target.thumbnail),
        egui::ColorImage::from_rgba_unmultiplied(
            [image.width as usize, image.height as usize],
            &rgba,
        ),
        egui::TextureOptions::LINEAR,
    )
}
impl eframe::App for ReaderApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Some((start, end, phase)) = self.smoke_drag.take() {
            let position = if phase == 0 { start } else { end };
            input.events.push(egui::Event::PointerMoved(position));
            if phase != 1 {
                input.events.push(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: phase == 0,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            if phase < 2 {
                self.smoke_drag = Some((start, end, phase + 1));
            }
        }
        if let Some((pos, remaining)) = self.smoke_double_click.take() {
            if remaining <= 2 {
                self.smoke_click = Some(pos);
            }
            if remaining > 1 {
                self.smoke_double_click = Some((pos, remaining - 1));
            }
        }
        input.events.append(&mut self.smoke_events);
        if self.smoke_ctrl_wheel {
            self.smoke_ctrl_wheel = false;
            input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::new(0.0, 1.0),
                modifiers: egui::Modifiers::CTRL,
            });
        }
        input.events.retain(|event| {
            if let egui::Event::MouseWheel {
                delta, modifiers, ..
            } = event
                && modifiers.ctrl
            {
                if delta.y != 0.0 {
                    self.scale(if delta.y > 0.0 { 1.2 } else { 1.0 / 1.2 });
                }
                false
            } else {
                true
            }
        });
        if let Some(pos) = self.smoke_scroll.take() {
            input.events.push(egui::Event::PointerMoved(pos));
            input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -800.0),
                modifiers: egui::Modifiers::NONE,
            });
        }
        if let Some(pos) = self.smoke_click.take() {
            input.events.push(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.highlight_palette_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.highlight_palette_open = false;
            self.highlight_color = self.highlight_before;
        }
        if self.highlight_pending.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.selection.notice = "正在保存高亮，请稍候再关闭窗口。".into();
        }
        if self.uninstall_confirm
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.uninstall_confirm = false;
        }
        if ctx.input(|i| i.viewport().close_requested())
            && (self
                .outline_editor
                .as_mut()
                .is_some_and(|e| e.guard_close())
                || self
                    .inline_rename
                    .as_ref()
                    .is_some_and(|r| r.changed() || r.result.is_some()))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.outline_notice =
                Some("目录名称有未保存修改，请按 Enter 保存或 Esc 取消后关闭。".into());
        }
        self.poll_inline_name();
        self.poll_highlights();
        self.poll(ctx);
        self.search.poll(
            self.document_id,
            self.path.as_deref(),
            &self.password,
            self.rotation,
            self.pages(),
        );
        let mut mark_committed = false;
        for response in self.selection_responses.try_iter() {
            if response.document_id == self.document_id && response.id == self.selection_id {
                match response.result {
                    Ok(selection) => {
                        if self.selection_committed_id.take() == Some(response.id) {
                            self.ai.selection_committed(&selection);
                            mark_committed = self.highlight_tool;
                        }
                        self.selection.notice = format!(
                            "已选择 {} 字 · 第 {} 页{}",
                            selection.text.chars().count(),
                            selection.page + 1,
                            if selection.truncated {
                                "（选区已截断）"
                            } else {
                                ""
                            }
                        );
                        self.selection.current = Some(selection);
                    }
                    Err(error) => self.selection.notice = error.message,
                }
            }
        }
        if mark_committed {
            self.highlight_selection(false);
        }
        for response in self.text_responses.try_iter() {
            if response.document_id == self.document_id {
                self.ai.pages_result(response.id, response.result);
            }
        }
        for (id, pages) in self.ai.poll() {
            if let Some(path) = self.path.clone() {
                let _ = self.text_requests.send(worker::TextRequest {
                    id,
                    document_id: self.document_id,
                    path,
                    password: self.password.clone(),
                    pages,
                });
            }
        }
        if self.outline_editor.is_none()
            && self.inline_rename.is_none()
            && !self.rgb_window
            && !self.uninstall_confirm
        {
            self.shortcuts(ctx);
        }
        self.apply_theme(ctx);
        self.header(ctx);
        egui::TopBottomPanel::bottom("status")
            .exact_height(38.0)
            .frame(
                egui::Frame::new()
                    .fill(self.panel())
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("页码");
                    let input = ui.add_sized(
                        [42.0, 26.0],
                        egui::TextEdit::singleline(&mut self.page_input)
                            .horizontal_align(egui::Align::Center),
                    );
                    self.page_editing = input.has_focus();
                    if input.lost_focus()
                        || (input.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        if let Ok(p) = self.page_input.trim().parse::<u32>() {
                            self.goto(p.saturating_sub(1));
                        }
                        self.page_input = (self.page + 1).to_string();
                        input.surrender_focus();
                    }
                    ui.label(format!("/ {}", self.pages()));
                    if self.layout.columns > 1 {
                        ui.label(RichText::new(format!("{} 页/行", self.layout.columns)).small());
                    }
                    if !self.selection.notice.is_empty() {
                        ui.add_sized(
                            [135.0, 20.0],
                            egui::Label::new(RichText::new(&self.selection.notice).small())
                                .truncate(),
                        )
                        .on_hover_text(&self.selection.notice);
                    }
                    if self.busy {
                        ui.spinner();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self
                            .hit("zoom-in", ui.button("＋"))
                            .on_hover_text("放大 · Ctrl++")
                            .clicked()
                        {
                            self.scale(1.2);
                        }
                        let mut percent = self.effective_zoom() * 100.0;
                        let slider = ui
                            .scope(|ui| {
                                ui.spacing_mut().slider_width = 130.0;
                                ui.add(
                                    egui::Slider::new(&mut percent, 10.0..=500.0)
                                        .logarithmic(true)
                                        .show_value(false),
                                )
                            })
                            .inner;
                        if self
                            .hit("zoom-slider", slider)
                            .on_hover_text("拖动调整显示比例 · 10%–500%")
                            .changed()
                        {
                            self.zoom = Zoom::Scale(percent / 100.0);
                        }
                        if self
                            .hit("zoom-out", ui.button("−"))
                            .on_hover_text("缩小 · Ctrl+−")
                            .clicked()
                        {
                            self.scale(1.0 / 1.2);
                        }
                        let id = ui.make_persistent_id("zoom-percent");
                        if !ui.memory(|m| m.has_focus(id)) {
                            self.zoom_input = format!("{:.0}%", self.effective_zoom() * 100.0);
                        }
                        let input = ui.add_sized(
                            [58.0, 26.0],
                            egui::TextEdit::singleline(&mut self.zoom_input)
                                .id(id)
                                .horizontal_align(egui::Align::Center),
                        );
                        let input = self
                            .hit("zoom-percent", input)
                            .on_hover_text("输入比例后按 Enter，例如 150%");
                        if input.lost_focus()
                            || (input.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            if let Ok(v) = self
                                .zoom_input
                                .trim()
                                .trim_end_matches('%')
                                .trim()
                                .parse::<f32>()
                                && v.is_finite()
                            {
                                self.zoom = Zoom::Scale((v / 100.0).clamp(0.1, 5.0));
                            }
                            input.surrender_focus();
                            self.zoom_input = format!("{:.0}%", self.effective_zoom() * 100.0);
                        }
                        if self
                            .hit("fit-width-status", ui.small_button("适宽"))
                            .clicked()
                        {
                            self.zoom = Zoom::FitWidth;
                        }
                        if self
                            .hit("fit-page-status", ui.small_button("整页"))
                            .clicked()
                        {
                            self.zoom = Zoom::FitPage;
                        }
                        let background = ui.menu_button("背景", |ui| self.theme_choices(ui));
                        self.hit("status-background", background.response);
                        if self
                            .hit(
                                "status-eye",
                                ui.selectable_label(self.theme == PaperTheme::Green, "护眼"),
                            )
                            .clicked()
                        {
                            self.theme = if self.theme == PaperTheme::Green {
                                PaperTheme::White
                            } else {
                                PaperTheme::Green
                            };
                        }
                    });
                });
            });
        self.navigation(ctx);
        let title = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let shared_limit = (self.search.visible && self.ai.visible)
            .then(|| ((ctx.available_rect().width() - 180.0) / 2.0).max(160.0));
        self.search.width_limit = shared_limit;
        self.ai.width_limit = shared_limit;
        if let Some(hit) = self.search.show(ctx) {
            self.goto(hit.page);
            let y = hit.rects.first().map_or(0.0, |r| (r[1] - 0.12).max(0.0));
            self.jump = Some((hit.page, y));
        }
        if let Some(page) = self.ai.show(
            ctx,
            self.info.as_ref(),
            &title,
            self.page,
            self.selection.current.as_ref(),
        ) {
            self.goto(page);
        }
        if self.smoke_features {
            self.smoke_hits.extend(
                self.search
                    .controls
                    .iter()
                    .map(|(k, r)| (k.to_string(), *r)),
            );
        }
        if self.smoke_ai || self.smoke_settings || self.smoke_search {
            self.smoke_hits.extend(
                self.ai
                    .controls
                    .iter()
                    .map(|(key, rect)| (key.to_string(), *rect)),
            );
        }
        self.text(ctx);
        self.selection.highlight_color = self.highlight_tool.then_some(self.highlight_color);
        self.content(ctx);
        if self.selection.invalidate {
            self.selection_id += 1;
            self.selection_committed_id = None;
            self.selection.invalidate = false;
        }
        if let Some(gesture) = self.selection.pending.take()
            && let Some(path) = self.path.clone()
        {
            let crate::text_selection::Gesture {
                page,
                start,
                end,
                word,
                committed,
            } = gesture;
            if self.smoke_ai {
                println!("Native selection request: page={page} start={start:?} end={end:?}");
            }
            self.selection_id += 1;
            self.selection_committed_id = committed.then_some(self.selection_id);
            let _ = self.selection_requests.send(worker::SelectionRequest {
                id: self.selection_id,
                document_id: self.document_id,
                path,
                password: self.password.clone(),
                page,
                start,
                end,
                rotation: self.rotation,
                word,
            });
        }
        if let Some(mode) = self.selection.action.take() {
            match mode {
                crate::text_selection::TranslationAction::Once(detailed) => {
                    if let Some(selection) = self.selection.current.clone() {
                        self.ai.request_translation(selection, detailed);
                    }
                }
                crate::text_selection::TranslationAction::Auto(mode) => {
                    self.ai
                        .set_auto_translation(mode, self.selection.current.as_ref());
                }
                crate::text_selection::TranslationAction::Question => {
                    self.ai.open_question();
                }
                crate::text_selection::TranslationAction::Highlight(color) => {
                    self.highlight_color = color;
                    self.highlight_selection(false);
                }
            }
        }
        if let Some(editor) = &mut self.outline_editor {
            match editor.show(ctx, self.page) {
                crate::outline_editor::Outcome::Keep => {}
                crate::outline_editor::Outcome::Close => self.outline_editor = None,
                crate::outline_editor::Outcome::Saved(path) => {
                    let page = self.page;
                    let zoom = self.zoom;
                    let rotation = self.rotation;
                    self.open(path);
                    self.zoom = zoom;
                    self.rotation = rotation;
                    self.page = page;
                    self.page_input = (page + 1).to_string();
                    self.jump = Some((page, 0.0));
                    self.navigation_tab = NavigationTab::Outline;
                }
            }
        }
        self.custom_theme_window(ctx);
        self.highlight_color_window(ctx);
        if self.uninstall_confirm {
            let mut controls = HashMap::new();
            match crate::window_actions::confirm_uninstall(ctx, &mut controls) {
                crate::window_actions::UninstallChoice::Keep => {}
                crate::window_actions::UninstallChoice::Cancel => self.uninstall_confirm = false,
                crate::window_actions::UninstallChoice::Confirm => {
                    self.uninstall_confirm = false;
                    self.uninstall(ctx);
                }
            }
            if self.smoke_features {
                self.smoke_hits
                    .extend(controls.into_iter().map(|(k, r)| (k.to_string(), r)));
            }
        }
        self.smoke(ctx);
    }
}
