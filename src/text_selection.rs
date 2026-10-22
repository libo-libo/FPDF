use eframe::egui::{self, Color32, Rect, Vec2};
use fpdf::pdf::PdfSelection;
use std::time::{Duration, Instant};

pub struct Gesture {
    pub page: u32,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub word: bool,
    pub committed: bool,
}

pub enum TranslationAction {
    Once(bool),
    Auto(Option<bool>),
    Question,
    Highlight([u8; 3]),
}

#[derive(Default)]
pub struct TextSelection {
    pub current: Option<PdfSelection>,
    pub notice: String,
    pub pending: Option<Gesture>,
    pub invalidate: bool,
    pub action: Option<TranslationAction>,
    pub auto_translation: Option<bool>,
    pub highlight_color: Option<[u8; 3]>,
    pub committed: bool,
    start: Option<(u32, [f64; 2])>,
    rotation: u8,
    last_sample: Option<(Instant, [f64; 2])>,
}

impl TextSelection {
    pub fn set_rotation(&mut self, rotation: u8) -> bool {
        if self.rotation == rotation {
            return false;
        }
        self.current = None;
        self.start = None;
        self.pending = None;
        self.rotation = rotation;
        self.last_sample = None;
        true
    }
    pub fn page(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        page: u32,
        rotation: u8,
        geometry: Option<&fpdf::reading::TextGeometry>,
    ) -> egui::Response {
        self.set_rotation(rotation);
        let response = ui.interact(
            rect,
            ui.id().with(("pdf-select", page)),
            egui::Sense::click_and_drag(),
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(if self.highlight_color.is_some() {
                egui::CursorIcon::Crosshair
            } else {
                egui::CursorIcon::Text
            });
        }
        let normalize = |point: egui::Pos2| {
            [
                f64::from((point.x - rect.left()) / rect.width()),
                f64::from((point.y - rect.top()) / rect.height()),
            ]
        };
        if response.double_clicked_by(egui::PointerButton::Primary)
            && let Some(position) = response.interact_pointer_pos()
        {
            let point = normalize(position);
            self.pending = Some(Gesture {
                page,
                start: point,
                end: point,
                word: true,
                committed: true,
            });
            self.notice = "正在选择单词…".into();
        } else if response.clicked_by(egui::PointerButton::Primary) {
            self.current = None;
            self.notice.clear();
            self.invalidate = true;
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(position) = ui.input(|i| i.pointer.press_origin())
        {
            self.current = None;
            self.invalidate = true;
            self.notice = "正在选择文字…".into();
            self.start = Some((page, normalize(position)));
            self.last_sample = None;
        }
        if let Some((selected_page, start)) = self.start
            && selected_page == page
        {
            if let Some(position) = response.interact_pointer_pos() {
                let end = normalize(position);
                let changed = self.last_sample.is_none_or(|(time, previous)| {
                    time.elapsed() >= Duration::from_millis(40) && previous != end
                });
                if response.drag_stopped() || changed {
                    self.pending = Some(Gesture {
                        page,
                        start,
                        end,
                        word: false,
                        committed: response.drag_stopped(),
                    });
                    self.last_sample = Some((Instant::now(), end));
                }
                if response.drag_stopped() {
                    self.start = None;
                    self.notice = "正在读取选区文字…".into();
                }
            } else if response.drag_stopped() {
                self.start = None;
            }
        }
        if let Some(geometry) =
            geometry.filter(|g| g.page == page && g.rotation == rotation && !g.truncated)
            && let Some(gesture) = self.pending.take()
        {
            self.invalidate = true;
            match geometry.select(gesture.start, gesture.end, gesture.word) {
                Ok(selection) => {
                    self.current = Some(selection);
                    self.committed |= gesture.committed;
                    self.notice.clear();
                }
                Err(error) => {
                    self.notice = error;
                }
            }
        }
        if let Some(selection) = &self.current
            && selection.page == page
        {
            for &[left, top, right, bottom] in &selection.rects {
                let highlight = Rect::from_min_max(
                    rect.min + Vec2::new(left * rect.width(), top * rect.height()),
                    rect.min + Vec2::new(right * rect.width(), bottom * rect.height()),
                );
                ui.painter().rect_filled(
                    highlight,
                    0.0,
                    self.highlight_color.map_or(
                        Color32::from_rgba_unmultiplied(65, 135, 245, 75),
                        |[r, g, b]| Color32::from_rgba_unmultiplied(r, g, b, 110),
                    ),
                );
            }
            response.context_menu(|ui| {
                if ui.button("复制选中文字").clicked() {
                    ui.ctx().copy_text(selection.text.clone());
                    ui.close();
                }
                if ui.button("AI：针对选区提问").clicked() {
                    self.action = Some(TranslationAction::Question);
                    ui.close();
                }
                ui.menu_button("高亮标注 · Ctrl+F2", |ui| {
                    for (name, color) in [
                        ("黄色", [255, 230, 70]),
                        ("绿色", [120, 230, 140]),
                        ("蓝色", [100, 190, 255]),
                        ("粉色", [255, 150, 200]),
                    ] {
                        if ui.button(name).clicked() {
                            self.action = Some(TranslationAction::Highlight(color));
                            ui.close();
                        }
                    }
                });
                if ui.button("AI：结合上下文翻译").clicked() {
                    self.action = Some(TranslationAction::Once(false));
                    ui.close();
                }
                if ui.button("AI：结合上下文翻译并详解").clicked() {
                    self.action = Some(TranslationAction::Once(true));
                    ui.close();
                }
                ui.separator();
                for (detailed, label) in [(false, "长期开启翻译"), (true, "长期开启翻译并详解")]
                {
                    let active = self.auto_translation == Some(detailed);
                    if ui.selectable_label(active, label).clicked() {
                        self.action = Some(TranslationAction::Auto(if active {
                            None
                        } else {
                            Some(detailed)
                        }));
                        ui.close();
                    }
                }
            });
            if response.hovered()
                && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::C))
            {
                ui.ctx().copy_text(selection.text.clone());
            }
        }
        response
    }
}
