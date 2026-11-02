//! Restricted, inert Markdown for streamed answers. No HTML or URL execution.
use eframe::egui::{self, FontId, TextFormat, text::LayoutJob};

#[derive(Debug, PartialEq)]
enum Block {
    Text(String),
    Heading(String),
    Quote(String),
    Code(String),
}

fn blocks(text: &str) -> Vec<Block> {
    let mut result = Vec::new();
    let mut code = None;
    let mut fence = "";
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(body) = &mut code {
            if trimmed.starts_with(fence) {
                result.push(Block::Code(std::mem::take(body)));
                code = None;
            } else {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = if trimmed.starts_with('`') {
                "```"
            } else {
                "~~~"
            };
            code = Some(String::new());
        } else if let Some(body) = trimmed.strip_prefix("> ") {
            result.push(Block::Quote(body.into()));
        } else if trimmed.starts_with('#')
            && let Some((prefix, body)) = trimmed.split_once(' ')
            && prefix.len() <= 6
            && prefix.chars().all(|c| c == '#')
        {
            result.push(Block::Heading(body.into()));
        } else {
            // Preserve list numbers, tables and unsupported syntax as readable text.
            result.push(Block::Text(line.into()));
        }
    }
    if let Some(body) = code {
        result.push(Block::Code(body));
    }
    result
}

fn inline(job: &mut LayoutJob, text: &str, format: &TextFormat) {
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(escaped) = rest.strip_prefix('\\')
            && let Some(c) = escaped.chars().next()
        {
            job.append(&c.to_string(), 0.0, format.clone());
            rest = &escaped[c.len_utf8()..];
            continue;
        }
        let marker = if rest.starts_with("**") {
            "**"
        } else if rest.starts_with('`') {
            "`"
        } else {
            ""
        };
        if !marker.is_empty()
            && let Some(end) = rest[marker.len()..].find(marker)
        {
            let body = &rest[marker.len()..marker.len() + end];
            let mut styled = format.clone();
            if marker == "`" {
                styled.font_id = FontId::monospace(format.font_id.size - 1.0);
                styled.background = egui::Color32::from_black_alpha(18);
                job.append(body, 0.0, styled);
            } else {
                styled.extra_letter_spacing = 0.35;
                styled.font_id.size += 0.5;
                inline(job, body, &styled);
            }
            rest = &rest[marker.len() * 2 + end..];
        } else {
            let c = rest.chars().next().unwrap();
            job.append(&c.to_string(), 0.0, format.clone());
            rest = &rest[c.len_utf8()..];
        }
    }
}

pub fn show(ui: &mut egui::Ui, text: &str, size: f32) {
    for (index, block) in blocks(text).into_iter().enumerate() {
        let (body, heading, quote) = match block {
            Block::Code(body) => {
                egui::ScrollArea::horizontal()
                    .id_salt(("answer-code", index))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(body).monospace().size(size - 2.0),
                            )
                            .selectable(true),
                        );
                    });
                ui.add_space(8.0);
                continue;
            }
            Block::Heading(body) => (body, true, false),
            Block::Quote(body) => (body, false, true),
            Block::Text(body) => (body, false, false),
        };
        if body.is_empty() {
            ui.add_space(10.0);
            continue;
        }
        let format = TextFormat {
            font_id: FontId::proportional(size + if heading { 1.0 } else { 0.0 }),
            color: if quote {
                ui.visuals().weak_text_color()
            } else {
                ui.visuals().text_color()
            },
            line_height: Some(size * 1.75),
            ..Default::default()
        };
        let mut job = LayoutJob::default();
        inline(&mut job, &body, &format);
        ui.add(egui::Label::new(job).wrap().selectable(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_fence_preserves_code_and_whitespace() {
        assert_eq!(
            blocks("## 结论\n```rust\n  **literal**\n"),
            vec![
                Block::Heading("结论".into()),
                Block::Code("  **literal**\n".into())
            ]
        );
    }

    #[test]
    fn inline_code_and_escapes_are_literal() {
        let mut job = LayoutJob::default();
        inline(
            &mut job,
            "**结论 `a*b`** \\*raw\\* `unfinished",
            &TextFormat::default(),
        );
        assert_eq!(job.text, "结论 a*b *raw* `unfinished");
    }
}
