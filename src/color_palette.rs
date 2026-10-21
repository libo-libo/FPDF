//! A visible color-selection surface. This implements the requested RGB
//! extension; its geometry is not claimed to be recovered WPS source code.
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Vec2};
use egui::ecolor::{Hsva, HsvaGamma};

pub struct Palette {
    hsv: HsvaGamma,
}
impl Palette {
    pub fn from_rgb(rgb: [u8; 3]) -> Self {
        Self {
            hsv: Hsva::from_srgb(rgb).into(),
        }
    }
    fn rgb(&self) -> [u8; 3] {
        Hsva::from(self.hsv).to_srgb()
    }
    fn choose(&mut self, x: f32, y: f32) {
        self.hsv.h = x.clamp(0.0, 1.0);
        self.hsv.s = 1.0 - y.clamp(0.0, 1.0);
    }
    fn brightness(&mut self, y: f32) {
        self.hsv.v = 1.0 - y.clamp(0.0, 1.0);
    }
    pub fn show(&mut self, ui: &mut egui::Ui, rgb: &mut [u8; 3]) -> (Response, Response, bool) {
        if self.rgb() != *rgb {
            *self = Self::from_rgb(*rgb);
        }
        let mut changed = false;
        let (plane, brightness) = ui
            .horizontal(|ui| {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(280.0, 180.0), Sense::click_and_drag());
                if (response.clicked_by(egui::PointerButton::Primary)
                    || response.dragged_by(egui::PointerButton::Primary))
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    self.choose(
                        (pos.x - rect.left()) / rect.width(),
                        (pos.y - rect.top()) / rect.height(),
                    );
                    changed = true;
                }
                let mut mesh = egui::Mesh::default();
                const COLS: u32 = 48;
                const ROWS: u32 = 24;
                for y in 0..=ROWS {
                    for x in 0..=COLS {
                        let fx = x as f32 / COLS as f32;
                        let fy = y as f32 / ROWS as f32;
                        let color: Color32 = HsvaGamma {
                            h: fx,
                            s: 1.0 - fy,
                            v: self.hsv.v,
                            a: 1.0,
                        }
                        .into();
                        mesh.colored_vertex(
                            Pos2::new(
                                rect.left() + fx * rect.width(),
                                rect.top() + fy * rect.height(),
                            ),
                            color,
                        );
                        if x < COLS && y < ROWS {
                            let i = y * (COLS + 1) + x;
                            mesh.add_triangle(i, i + 1, i + COLS + 1);
                            mesh.add_triangle(i + 1, i + COLS + 2, i + COLS + 1);
                        }
                    }
                }
                ui.painter().add(mesh);
                let pos = Pos2::new(
                    rect.left() + self.hsv.h * rect.width(),
                    rect.top() + (1.0 - self.hsv.s) * rect.height(),
                );
                ui.painter()
                    .circle_stroke(pos, 5.0, Stroke::new(2.5_f32, Color32::BLACK));
                ui.painter()
                    .circle_stroke(pos, 5.0, Stroke::new(1.0_f32, Color32::WHITE));
                ui.painter().rect_stroke(
                    rect,
                    0,
                    Stroke::new(1.0_f32, Color32::GRAY),
                    egui::StrokeKind::Outside,
                );
                let response = response
                    .on_hover_cursor(egui::CursorIcon::Crosshair)
                    .on_hover_text("点击或拖动调色盘，设置色相和饱和度");
                let (bar, bar_response) =
                    ui.allocate_exact_size(Vec2::new(22.0, 180.0), Sense::click_and_drag());
                if (bar_response.clicked_by(egui::PointerButton::Primary)
                    || bar_response.dragged_by(egui::PointerButton::Primary))
                    && let Some(pos) = bar_response.interact_pointer_pos()
                {
                    self.brightness((pos.y - bar.top()) / bar.height());
                    changed = true;
                }
                for row in 0..180 {
                    let f = row as f32 / 179.0;
                    let color: Color32 = HsvaGamma {
                        v: 1.0 - f,
                        ..self.hsv
                    }
                    .into();
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            Pos2::new(bar.left(), bar.top() + f * bar.height()),
                            Vec2::new(bar.width(), 2.0),
                        ),
                        0,
                        color,
                    );
                }
                let y = bar.top() + (1.0 - self.hsv.v) * bar.height();
                ui.painter().line_segment(
                    [Pos2::new(bar.left(), y), Pos2::new(bar.right(), y)],
                    Stroke::new(3.0_f32, Color32::BLACK),
                );
                ui.painter().line_segment(
                    [Pos2::new(bar.left(), y), Pos2::new(bar.right(), y)],
                    Stroke::new(1.0_f32, Color32::WHITE),
                );
                ui.painter().rect_stroke(
                    bar,
                    0,
                    Stroke::new(1.0_f32, Color32::GRAY),
                    egui::StrokeKind::Outside,
                );
                (response, bar_response.on_hover_text("点击或拖动亮度条"))
            })
            .inner;
        if changed {
            *rgb = self.rgb();
        }
        (plane, brightness, changed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn click_coordinates_produce_primary_colors_gray_and_black() {
        let mut p = Palette::from_rgb([255, 255, 255]);
        p.choose(0.0, 0.0);
        assert_eq!(p.rgb(), [255, 0, 0]);
        p.choose(1.0 / 3.0, 0.0);
        assert_eq!(p.rgb(), [0, 255, 0]);
        p.choose(2.0 / 3.0, 0.0);
        assert_eq!(p.rgb(), [0, 0, 255]);
        p.choose(0.5, 1.0);
        assert_eq!(p.rgb(), [255, 255, 255]);
        p.brightness(1.0);
        assert_eq!(p.rgb(), [0, 0, 0]);
        p.brightness(0.5);
        assert!(p.rgb().iter().all(|v| v.abs_diff(128) <= 1));
    }
    #[test]
    fn numeric_rgb_round_trips_without_changing_the_selected_color() {
        for rgb in [
            [210, 225, 240],
            [0, 0, 0],
            [255, 255, 255],
            [132, 64, 18],
            [0, 255, 0],
        ] {
            assert_eq!(Palette::from_rgb(rgb).rgb(), rgb);
        }
    }
}
