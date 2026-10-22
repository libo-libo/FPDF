use eframe::egui::{self, Color32, Rect, RichText, Vec2};
use fpdf::pdf::SearchMatch;
use fpdf::worker::{SearchRequest, SearchResponse};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct SearchUi {
    pub width_limit: Option<f32>,
    pub visible: bool,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub selected: Option<usize>,
    pub busy: bool,
    pub controls: HashMap<&'static str, Rect>,
    case_sensitive: bool,
    whole_word: bool,
    scanned: u32,
    total: u32,
    error: Option<String>,
    truncated: bool,
    submitted: String,
    changed: Option<Instant>,
    focus: bool,
    scroll_selected: bool,
    request_id: u64,
    context: Option<(u64, PathBuf, String, u8)>,
    requests: mpsc::Sender<SearchRequest>,
    responses: mpsc::Receiver<SearchResponse>,
}

impl SearchUi {
    pub fn new(
        requests: mpsc::Sender<SearchRequest>,
        responses: mpsc::Receiver<SearchResponse>,
    ) -> Self {
        Self {
            width_limit: None,
            visible: false,
            query: String::new(),
            matches: vec![],
            selected: None,
            busy: false,
            controls: HashMap::new(),
            case_sensitive: false,
            whole_word: false,
            scanned: 0,
            total: 0,
            error: None,
            truncated: false,
            submitted: String::new(),
            changed: None,
            focus: false,
            scroll_selected: false,
            request_id: 0,
            context: None,
            requests,
            responses,
        }
    }
    pub fn open(&mut self, selection: Option<&str>) {
        self.visible = true;
        self.focus = true;
        if let Some(text) =
            selection.filter(|s| !s.contains(['\n', '\r']) && s.chars().count() <= 120)
        {
            if self.query != text {
                self.send(true);
                self.matches.clear();
                self.selected = None;
                self.error = None;
            }
            self.query = text.into();
            self.changed = Some(Instant::now());
        }
    }
    pub fn close(&mut self) {
        self.visible = false;
        self.send(true);
        self.changed = None;
    }
    fn send(&mut self, cancel: bool) {
        self.request_id += 1;
        if let Some((document_id, path, password, rotation)) = &self.context {
            let query = if cancel {
                String::new()
            } else {
                self.query.trim().to_owned()
            };
            if self
                .requests
                .send(SearchRequest {
                    id: self.request_id,
                    document_id: *document_id,
                    path: path.clone(),
                    password: password.clone(),
                    rotation: *rotation,
                    query: query.clone(),
                    case_sensitive: self.case_sensitive,
                    whole_word: self.whole_word,
                })
                .is_err()
            {
                self.error = Some("查找线程已退出，请重启程序。".into());
            }
            self.busy = !query.is_empty() && self.error.is_none();
            self.submitted = query;
        }
    }
    fn restart(&mut self) {
        self.matches.clear();
        self.selected = None;
        self.scanned = 0;
        self.error = None;
        self.truncated = false;
        self.changed = None;
        self.send(false);
    }
    pub fn poll(
        &mut self,
        document_id: u64,
        path: Option<&Path>,
        password: &str,
        rotation: u8,
        total: u32,
    ) {
        let context = path.map(|p| (document_id, p.to_owned(), password.to_owned(), rotation));
        if context != self.context {
            self.send(true);
            self.context = context;
            self.matches.clear();
            self.selected = None;
            self.error = None;
            self.scanned = 0;
            self.total = total;
            self.busy = false;
            self.submitted.clear();
            self.changed = (self.visible && !self.query.trim().is_empty()).then(Instant::now);
        }
        self.total = total.max(self.total);
        for response in self.responses.try_iter() {
            if response.id != self.request_id || response.document_id != document_id {
                continue;
            }
            self.scanned = response.scanned_pages;
            self.total = response.total_pages;
            self.truncated |= response.truncated;
            self.matches.extend(response.matches);
            self.error = response.error;
            self.busy = !response.done && self.error.is_none();
        }
    }
    fn step(&mut self, backwards: bool) -> Option<SearchMatch> {
        let n = self.matches.len();
        if n == 0 {
            return None;
        }
        let index = match (self.selected, backwards) {
            (Some(i), true) => (i + n - 1) % n,
            (Some(i), false) => (i + 1) % n,
            (None, true) => n - 1,
            (None, false) => 0,
        };
        self.selected = Some(index);
        self.scroll_selected = true;
        Some(self.matches[index].clone())
    }
    pub fn show(&mut self, ctx: &egui::Context) -> Option<SearchMatch> {
        if !self.visible {
            return None;
        }
        let mut jump = None;
        let mut close = false;
        self.controls.clear();
        let maximum = self
            .width_limit
            .unwrap_or((ctx.content_rect().width() * 0.55).max(260.0));
        egui::SidePanel::right("document-search")
            .resizable(true)
            .default_width(330.0_f32.min(maximum))
            .min_width(220.0_f32.min(maximum))
            .max_width(maximum)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("全文搜索");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let button = ui.small_button("关闭 ×");
                        self.controls.insert("search-close", button.rect);
                        close = button.clicked();
                    });
                });
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .desired_width(f32::INFINITY)
                        .char_limit(256)
                        .hint_text("在整个 PDF 中查找 · Ctrl+F"),
                );
                self.controls.insert("search-input", edit.rect);
                if self.focus {
                    edit.request_focus();
                    self.focus = false;
                }
                if edit.changed() {
                    // Cancel immediately so old snippets cannot appear under new input.
                    self.send(true);
                    self.matches.clear();
                    self.selected = None;
                    self.error = None;
                    self.scanned = 0;
                    self.truncated = false;
                    self.changed = Some(Instant::now());
                }
                let enter = edit.has_focus()
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                let previous_key = edit.has_focus()
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter));
                let mut changed = false;
                ui.horizontal_wrapped(|ui| {
                    let case = ui.checkbox(&mut self.case_sensitive, "区分大小写");
                    self.controls.insert("search-case", case.rect);
                    changed |= case.changed();
                    let word = ui.checkbox(&mut self.whole_word, "全词匹配");
                    self.controls.insert("search-whole-word", word.rect);
                    changed |= word.changed();
                });
                let mut search = false;
                ui.horizontal(|ui| {
                    let button = ui.add_enabled(
                        self.context.is_some() && !self.query.trim().is_empty(),
                        egui::Button::new("查找"),
                    );
                    self.controls.insert("search-submit", button.rect);
                    search = button.clicked();
                    let prev =
                        ui.add_enabled(!self.matches.is_empty(), egui::Button::new("上一个"));
                    self.controls.insert("search-previous", prev.rect);
                    if prev.clicked() || previous_key {
                        jump = self.step(true);
                    }
                    let next =
                        ui.add_enabled(!self.matches.is_empty(), egui::Button::new("下一个"));
                    self.controls.insert("search-next", next.rect);
                    if next.clicked() {
                        jump = self.step(false);
                    }
                });
                if changed || search || (enter && self.submitted != self.query.trim()) {
                    self.restart();
                } else if enter {
                    jump = self.step(false);
                }
                if self
                    .changed
                    .is_some_and(|t| t.elapsed() >= Duration::from_millis(260))
                {
                    self.restart();
                }
                if self.changed.is_some() {
                    ctx.request_repaint_after(Duration::from_millis(60));
                }
                if self.busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("正在查找 {}/{} 页", self.scanned, self.total));
                    });
                }
                ui.horizontal(|ui| {
                    ui.label(format!("查找结果：共 {} 条", self.matches.len()));
                    if let Some(i) = self.selected {
                        ui.label(
                            RichText::new(format!("{}/{}", i + 1, self.matches.len()))
                                .small()
                                .weak(),
                        );
                    }
                });
                if self.truncated {
                    ui.colored_label(
                        Color32::from_rgb(215, 140, 40),
                        "匹配过多，已达到显示上限；请缩小查找范围。 ",
                    );
                }
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::from_rgb(218, 51, 54), error);
                }
                if self.context.is_none() {
                    ui.label("请先打开 PDF。");
                } else if !self.busy
                    && !self.submitted.is_empty()
                    && self.matches.is_empty()
                    && self.error.is_none()
                {
                    ui.label("未找到匹配内容。扫描图片需先进行 OCR。");
                }
                ui.separator();
                let scroll_selected = self.scroll_selected;
                self.scroll_selected = false;
                let mut area = egui::ScrollArea::vertical()
                    .id_salt("search-results")
                    .max_height((ui.available_height() - 30.0).max(80.0));
                if scroll_selected && let Some(index) = self.selected {
                    area = area.vertical_scroll_offset(
                        index as f32 * (96.0 + ui.spacing().item_spacing.y),
                    );
                }
                area.show_rows(ui, 96.0, self.matches.len(), |ui, range| {
                    for index in range {
                        let hit = &self.matches[index];
                        let active = self.selected == Some(index);
                        let fill = if active {
                            ui.visuals().selection.bg_fill
                        } else {
                            ui.visuals().faint_bg_color
                        };
                        let card = egui::Frame::new()
                            .fill(fill)
                            .corner_radius(4)
                            .inner_margin(9)
                            .show(ui, |ui| {
                                ui.set_min_width((ui.available_width() - 1.0).max(20.0));
                                ui.set_min_height(78.0);
                                ui.strong(format!("第 {} 页", hit.page + 1));
                                let mut job = preview_job(hit, ui.visuals().text_color());
                                job.wrap.max_rows = 3;
                                job.wrap.overflow_character = Some('…');
                                ui.add(egui::Label::new(job).wrap().selectable(false));
                            });
                        let response = ui.interact(
                            card.response.rect,
                            ui.id().with(("search-hit", index)),
                            egui::Sense::click(),
                        );
                        if index == 0 {
                            self.controls.insert("search-result-first", response.rect);
                        }
                        if response.clicked() {
                            self.selected = Some(index);
                            jump = Some(hit.clone());
                        }
                    }
                });
                ui.label(
                    RichText::new("点击结果预览跳转；黄色为匹配，橙色为当前结果。")
                        .small()
                        .weak(),
                );
            });
        if close || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.close();
        }
        jump
    }
    pub fn paint(&self, ui: &egui::Ui, rect: Rect, page: u32) {
        if !self.visible {
            return;
        }
        for (index, hit) in self
            .matches
            .iter()
            .enumerate()
            .filter(|(_, h)| h.page == page)
        {
            let active = self.selected == Some(index);
            for &[left, top, right, bottom] in &hit.rects {
                let mark = Rect::from_min_max(
                    rect.min + Vec2::new(left * rect.width(), top * rect.height()),
                    rect.min + Vec2::new(right * rect.width(), bottom * rect.height()),
                );
                ui.painter().rect_filled(
                    mark,
                    1,
                    if active {
                        Color32::from_rgba_unmultiplied(255, 145, 35, 125)
                    } else {
                        Color32::from_rgba_unmultiplied(255, 215, 30, 95)
                    },
                );
                if active {
                    ui.painter().rect_stroke(
                        mark,
                        1,
                        egui::Stroke::new(1.0_f32, Color32::from_rgb(245, 135, 20)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
    }
}

fn preview_job(hit: &SearchMatch, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let normal = egui::TextFormat {
        font_id: egui::FontId::proportional(14.0),
        color,
        ..Default::default()
    };
    let [start, end] = hit.snippet_match;
    if start < end && hit.snippet.get(start..end).is_some() {
        job.append(&hit.snippet[..start], 0.0, normal.clone());
        job.append(
            &hit.snippet[start..end],
            0.0,
            egui::TextFormat {
                background: Color32::from_rgb(255, 220, 80),
                color: Color32::BLACK,
                ..normal.clone()
            },
        );
        job.append(&hit.snippet[end..], 0.0, normal);
    } else {
        job.append(&hit.snippet, 0.0, normal);
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn result_navigation_wraps_and_preview_preserves_unicode() {
        let (tx, _) = mpsc::channel();
        let (_, rx) = mpsc::channel();
        let mut ui = SearchUi::new(tx, rx);
        ui.matches = (0..3)
            .map(|page| SearchMatch {
                page,
                index: 0,
                rects: vec![],
                snippet: "上下文 PDF 文本".into(),
                matched_text: "PDF".into(),
                snippet_match: [10, 13],
            })
            .collect();
        assert_eq!(ui.step(false).unwrap().page, 0);
        assert_eq!(ui.step(true).unwrap().page, 2);
        assert_eq!(ui.step(false).unwrap().page, 0);
        let job = preview_job(&ui.matches[0], Color32::WHITE);
        assert_eq!(job.text, "上下文 PDF 文本");
        assert_eq!(job.sections.len(), 3);
    }
}
