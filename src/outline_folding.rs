use fpdf::pdf::Bookmark;
use std::collections::HashSet;

pub struct OutlineEntry {
    pub response: eframe::egui::Response,
    pub page_rect: Option<eframe::egui::Rect>,
    pub leader_rect: Option<eframe::egui::Rect>,
}

/// Draw leaders independently of the title: page numbers never become part of
/// the editable PDF bookmark, and every row ends at the same panel column.
pub fn outline_entry(
    ui: &mut eframe::egui::Ui,
    title: &str,
    page: Option<u32>,
    selected: bool,
) -> OutlineEntry {
    use eframe::egui::{self, Vec2};
    let width = ui.available_width().max(1.0);
    let padding = 5.0;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let color = ui.visuals().text_color();
    let page_text = page.map(|p| (u64::from(p) + 1).to_string());
    let number = page_text.map(|text| ui.painter().layout_no_wrap(text, font.clone(), color));
    let star = ui.painter().layout_no_wrap("*".into(), font.clone(), color);
    let star_width = star.size().x.max(1.0);
    let reserved = number
        .as_ref()
        .map_or(0.0, |n| n.size().x + star_width * 2.0 + 8.0);
    let mut job = egui::text::LayoutJob::simple(
        title.into(),
        font,
        color,
        (width - padding * 2.0 - reserved).max(1.0),
    );
    job.wrap.max_rows = 4;
    let title = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let height = title
        .size()
        .y
        .max(ui.spacing().interact_size.y - padding * 2.0)
        + padding * 2.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter()
        .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
    let text_origin = rect.min + Vec2::splat(padding);
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.galley(text_origin, title.clone(), color);
    let mut page_rect = None;
    let mut leader_rect = None;
    if let Some(number) = number {
        let last = title.rows.last().map_or(egui::Rect::ZERO, |row| row.rect());
        let y = text_origin.y + last.top();
        let x = rect.right() - padding - number.size().x;
        let number_rect = egui::Rect::from_min_size(egui::pos2(x, y), number.size());
        let start = text_origin.x + last.right() + 4.0;
        let count = ((x - 4.0 - start) / star_width).floor().clamp(0.0, 512.0) as usize;
        if count > 0 {
            let leaders = painter.layout_no_wrap(
                "*".repeat(count),
                egui::TextStyle::Button.resolve(ui.style()),
                color,
            );
            let position = egui::pos2(x - 4.0 - leaders.size().x, y);
            leader_rect = Some(egui::Rect::from_min_size(position, leaders.size()));
            painter.galley(position, leaders, color);
        }
        painter.galley(number_rect.min, number, color);
        page_rect = Some(number_rect);
    }
    OutlineEntry {
        response,
        page_rect,
        leader_rect,
    }
}

pub fn fold_button(ui: &mut eframe::egui::Ui, closed: bool) -> eframe::egui::Response {
    use eframe::egui::{self, Vec2};
    let (rect, response) = ui.allocate_exact_size(Vec2::new(16.0, 22.0), egui::Sense::click());
    let center = rect.center();
    let points = if closed {
        [(-3.0, -4.0), (-3.0, 4.0), (3.0, 0.0)]
    } else {
        [(-4.0, -3.0), (4.0, -3.0), (0.0, 3.0)]
    };
    let color = ui.style().interact(&response).fg_stroke.color;
    ui.painter().add(egui::Shape::convex_polygon(
        points
            .into_iter()
            .map(|(x, y)| center + Vec2::new(x, y))
            .collect(),
        color,
        egui::Stroke::NONE,
    ));
    response.on_hover_text(if closed {
        "展开子目录"
    } else {
        "折叠子目录"
    })
}

/// Keep original bookmark indices so folding never changes rename/jump targets.
pub fn visible_indices(bookmarks: &[Bookmark], collapsed: &HashSet<usize>) -> Vec<usize> {
    let mut hidden_below = None;
    let mut visible = Vec::new();
    for (index, mark) in bookmarks.iter().enumerate() {
        if let Some(depth) = hidden_below {
            if mark.depth > depth {
                continue;
            }
            hidden_below = None;
        }
        visible.push(index);
        if collapsed.contains(&index) {
            hidden_below = Some(mark.depth);
        }
    }
    visible
}

pub fn has_children(bookmarks: &[Bookmark], index: usize) -> bool {
    bookmarks
        .get(index + 1)
        .is_some_and(|next| next.depth > bookmarks[index].depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outline_pages_align_across_indents_and_digit_counts_without_changing_titles() {
        use eframe::egui::{self, Vec2};
        let context = egui::Context::default();
        let mut columns = Vec::new();
        let title = "A long chapter heading that must wrap across multiple lines";
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(360.0, 600.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for (name, page, indent) in [
                        ("Short chapter", Some(0), 0.0),
                        (title, Some(12344), 36.0),
                        ("Group without a destination", None, 0.0),
                    ] {
                        ui.horizontal(|ui| {
                            ui.add_space(indent);
                            let entry = outline_entry(ui, name, page, false);
                            if let Some(page) = entry.page_rect {
                                columns.push(page.right());
                                assert!(entry.leader_rect.is_some());
                                assert!(entry.leader_rect.unwrap().right() < page.left());
                                if name == title {
                                    assert!(page.top() > entry.response.rect.top() + 15.0);
                                }
                            } else {
                                assert!(entry.leader_rect.is_none());
                            }
                        });
                    }
                });
            },
        );
        assert!((columns[0] - columns[1]).abs() < 0.5);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "12345")));
        assert_eq!(
            title,
            "A long chapter heading that must wrap across multiple lines"
        );
    }
    #[test]
    fn nested_folds_preserve_siblings_indices_and_child_state() {
        let marks: Vec<_> = [0, 1, 2, 1, 0, 1]
            .into_iter()
            .map(|depth| Bookmark {
                title: String::new(),
                page: None,
                depth,
            })
            .collect();
        let mut closed = HashSet::from([0, 1]);
        assert_eq!(visible_indices(&marks, &closed), [0, 4, 5]);
        closed.remove(&0);
        assert_eq!(visible_indices(&marks, &closed), [0, 1, 3, 4, 5]);
        assert!(has_children(&marks, 1));
        assert!(!has_children(&marks, 5));
    }
}
