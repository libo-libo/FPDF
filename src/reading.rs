//! Pure reading layout, navigation history, and display color transforms.

#[derive(Clone, Debug)]
pub struct TextGlyph {
    pub character: char,
    pub rect: [f32; 4],
    pub quad: [f64; 8],
}
pub fn rotate_text_rect(mut rect: [f32; 4], from: u8, to: u8) -> [f32; 4] {
    for _ in 0..((to + 4 - from) % 4) {
        rect = [1.0 - rect[3], rect[0], 1.0 - rect[1], rect[2]];
    }
    rect
}
#[derive(Clone, Debug)]
pub struct TextGeometry {
    pub page_transform: [[f32; 3]; 2],
    pub page: u32,
    pub rotation: u8,
    pub points: [f32; 2],
    pub glyphs: Vec<TextGlyph>,
    pub truncated: bool,
}
impl TextGeometry {
    pub fn project(&self, x: f64, y: f64) -> [f32; 2] {
        self.page_transform
            .map(|[a, b, c]| a * x as f32 + b * y as f32 + c)
    }
    pub fn bytes(&self) -> usize {
        self.glyphs.len() * std::mem::size_of::<TextGlyph>()
    }
    pub fn select(
        &self,
        start: [f64; 2],
        end: [f64; 2],
        word: bool,
    ) -> Result<crate::pdf::PdfSelection, String> {
        if start.into_iter().chain(end).any(|v| !v.is_finite()) {
            return Err("选区坐标无效。".into());
        }
        let at = |point: [f64; 2], snap: bool| -> Option<usize> {
            let mut best: Option<(usize, f64)> = None;
            for (index, glyph) in self.glyphs.iter().enumerate() {
                let [l, t, r, b] = glyph.rect;
                if glyph.character.is_whitespace() || r <= l || b <= t {
                    continue;
                }
                let x = point[0].clamp(0.0, 1.0);
                let y = point[1].clamp(0.0, 1.0);
                let dx = (x - f64::from(l)).min(0.0).abs() + (x - f64::from(r)).max(0.0);
                let dy = (y - f64::from(t)).min(0.0).abs() + (y - f64::from(b)).max(0.0);
                let dx = dx * f64::from(self.points[0]);
                let dy = dy * f64::from(self.points[1]);
                if dy > 12.0 || (!snap && dx > 12.0) {
                    continue;
                }
                let distance = dy * dy * 100.0 + dx * dx;
                if best.is_none_or(|(_, previous)| distance < previous) {
                    best = Some((index, distance));
                }
            }
            best.map(|(index, _)| index)
        };
        let Some(mut first) = at(start, false) else {
            return Err("请从文字处开始选取；扫描页需要 OCR。".into());
        };
        let Some(mut last) = at(if word { start } else { end }, true) else {
            return Err("选区没有文字。".into());
        };
        let is_word = |index: usize| {
            let ch = self.glyphs[index].character;
            ch.is_alphanumeric() || matches!(ch, '_' | '\'' | '’' | '-')
        };
        if word && is_word(first) {
            while first > 0 && last - first < 7999 && is_word(first - 1) {
                first -= 1;
            }
            while last + 1 < self.glyphs.len() && last - first < 7999 && is_word(last + 1) {
                last += 1;
            }
        }
        let begin = first.min(last);
        let finish = (first.max(last) + 1).min(begin + 8000);
        let text: String = self.glyphs[begin..finish]
            .iter()
            .map(|g| g.character)
            .collect();
        if text.trim().is_empty() {
            return Err("选区没有文字。".into());
        }
        let mut rects: Vec<[f32; 4]> = Vec::new();
        let mut quads: Vec<[f64; 8]> = Vec::new();
        for glyph in &self.glyphs[begin..finish] {
            if glyph.character.is_whitespace()
                || glyph.rect[2] <= glyph.rect[0]
                || glyph.rect[3] <= glyph.rect[1]
            {
                continue;
            }
            let q = glyph.quad;
            let merge = quads.last().is_some_and(|previous| {
                let overlap = previous[1].min(q[1]) - previous[5].max(q[5]);
                overlap > 0.4 * (previous[1] - previous[5]).min(q[1] - q[5])
                    && q[0] <= previous[2] + 18.0
                    && q[2] >= previous[0] - 18.0
            });
            if merge {
                let previous = quads.last_mut().unwrap();
                let l = previous[0].min(q[0]);
                let r = previous[2].max(q[2]);
                let t = previous[1].max(q[1]);
                let b = previous[5].min(q[5]);
                *previous = [l, t, r, t, l, b, r, b];
                let rect = rects.last_mut().unwrap();
                *rect = [
                    rect[0].min(glyph.rect[0]),
                    rect[1].min(glyph.rect[1]),
                    rect[2].max(glyph.rect[2]),
                    rect[3].max(glyph.rect[3]),
                ];
            } else {
                quads.push(q);
                rects.push(glyph.rect);
            }
        }
        if quads.len() > 256 {
            return Err("单次选区跨越的文本行过多，请分段标注。".into());
        }
        let before: String = self.glyphs[begin.saturating_sub(1500)..begin]
            .iter()
            .map(|g| g.character)
            .collect();
        let after: String = self.glyphs[finish..(finish + 1500).min(self.glyphs.len())]
            .iter()
            .map(|g| g.character)
            .collect();
        Ok(crate::pdf::PdfSelection {
            page: self.page,
            text,
            context: format!("选区前文：\n{before}\n选区后文：\n{after}"),
            rects,
            quads,
            truncated: finish < first.max(last) + 1 || self.truncated,
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReadingMode {
    Single,
    #[default]
    Continuous,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaperTheme {
    #[default]
    White,
    Green,
    Warm,
    Night,
    Custom([u8; 3]),
}
impl PaperTheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::White => "默认",
            Self::Green => "护眼",
            Self::Warm => "羊皮纸",
            Self::Night => "夜间",
            Self::Custom(_) => "自定义 RGB",
        }
    }
    pub fn pixel(self, pixel: [u8; 4]) -> [u8; 4] {
        let (paper, ink) = match self {
            Self::White => return pixel,
            Self::Green => ([218, 234, 214], [0, 0, 0]),
            Self::Warm => ([246, 235, 212], [0, 0, 0]),
            Self::Night => ([39, 43, 50], [220, 224, 230]),
            Self::Custom(rgb) => (
                rgb.map(u32::from),
                if self.is_dark() {
                    [235, 238, 242]
                } else {
                    [0, 0, 0]
                },
            ),
        };
        let mut out = pixel;
        // Preserve colored content on dark paper; per-channel inversion turns
        // pale yellow highlighter ink blue and almost indistinguishable from paper.
        if self.is_dark() {
            let high = u32::from(*pixel[..3].iter().max().unwrap());
            let low = u32::from(*pixel[..3].iter().min().unwrap());
            let chroma = high - low;
            if chroma >= 24 {
                let strength = (chroma * 255).isqrt();
                for c in 0..3 {
                    let hue = (u32::from(pixel[c]) - low) * 255 / chroma;
                    out[c] = ((paper[c] * (255 - strength) + hue * strength) / 255) as u8;
                }
                return out;
            }
        }
        for c in 0..3 {
            out[c] = ((ink[c] * (255 - pixel[c] as u32) + paper[c] * pixel[c] as u32) / 255) as u8;
        }
        out
    }
    pub fn is_dark(self) -> bool {
        match self {
            Self::Night => true,
            Self::Custom([r, g, b]) => {
                u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 < 140_000
            }
            _ => false,
        }
    }
    pub fn rgba(self, input: &[u8]) -> Vec<u8> {
        input
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| self.pixel(*p))
            .collect()
    }
}

/// A page keeps the same bitmap size when its neighbours enter the viewport.
/// This caps each bitmap at 3 MP without coupling quality to scroll position.
pub fn render_width(width: f32, height: f32, pixels_per_point: f32) -> u32 {
    let cap = (3_000_000.0 * width / height.max(1.0))
        .sqrt()
        .floor()
        .clamp(64.0, 4096.0);
    (width * pixels_per_point).round().clamp(64.0, cap) as u32
}

#[derive(Clone, Debug)]
pub struct PagePlacement {
    pub page: u32,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}
impl PagePlacement {
    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

#[derive(Clone, Debug, Default)]
pub struct PageLayout {
    pub pages: Vec<PagePlacement>,
    pub columns: usize,
    pub width: f32,
    pub height: f32,
    rows: Vec<PageRow>,
}
#[derive(Clone, Debug)]
struct PageRow {
    top: f32,
    bottom: f32,
    start: usize,
    end: usize,
}
impl PageLayout {
    pub fn new(
        sizes: &[[f32; 2]],
        available: f32,
        scale: Option<f32>,
        rotation: u8,
        single: Option<u32>,
    ) -> Self {
        let mut result = Self::default();
        for (index, size) in sizes.iter().enumerate() {
            if single.is_some_and(|p| p as usize != index) {
                continue;
            }
            let [pw, ph] = if rotation.is_multiple_of(2) {
                *size
            } else {
                [size[1], size[0]]
            };
            let (pw, ph) = if pw.is_finite() && ph.is_finite() && pw > 0.0 && ph > 0.0 {
                (pw, ph)
            } else {
                (612.0, 792.0)
            };
            let width = scale.map_or((available - 64.0).max(64.0), |z| pw * (96.0 / 72.0) * z);
            // Bound pathological placeholder geometry while preserving normal sizes.
            let height = (width * ph / pw).clamp(1.0, 100_000.0);
            result.width = result.width.max(width + 48.0);
            result.pages.push(PagePlacement {
                page: index as u32,
                left: 0.0,
                top: 0.0,
                width,
                height,
            });
        }
        let cell = result.pages.iter().map(|p| p.width).fold(0.0_f32, f32::max);
        result.columns = if single.is_none() && scale.is_some() {
            [8, 4, 2]
                .into_iter()
                .find(|&n| cell * n as f32 + 18.0 * (n - 1) as f32 + 48.0 <= available)
                .unwrap_or(1)
        } else {
            1
        };
        result.width = result.width.max(available);
        let mut top = 24.0;
        for (row, chunk) in result.pages.chunks_mut(result.columns).enumerate() {
            let height = chunk.iter().map(|p| p.height).fold(0.0_f32, f32::max);
            let row_width = cell * chunk.len() as f32 + 18.0 * (chunk.len() - 1) as f32;
            for (column, page) in chunk.iter_mut().enumerate() {
                page.top = top;
                page.left = (result.width - row_width) / 2.0
                    + column as f32 * (cell + 18.0)
                    + (cell - page.width) / 2.0;
            }
            result.rows.push(PageRow {
                top,
                bottom: top + height,
                start: row * result.columns,
                end: row * result.columns + chunk.len(),
            });
            top += height + 24.0;
        }
        result.height = top;
        result
    }
    pub fn visible(&self, start: f32, end: f32) -> std::ops::Range<usize> {
        let first = self.rows.partition_point(|r| r.bottom < start);
        let last = self.rows.partition_point(|r| r.top <= end);
        let begin = self.rows.get(first).map_or(self.pages.len(), |r| r.start);
        let finish = last
            .checked_sub(1)
            .and_then(|i| self.rows.get(i))
            .map_or(0, |r| r.end);
        begin.min(finish)..finish
    }
    pub fn page_at(&self, y: f32) -> u32 {
        self.page_at_with_hint(y, 0)
    }
    /// Keep a clicked/bookmarked page active when it shares a row with others.
    pub fn page_at_with_hint(&self, y: f32, current: u32) -> u32 {
        let row = self.rows.partition_point(|r| r.top <= y).saturating_sub(1);
        let Some(row) = self.rows.get(row) else {
            return 0;
        };
        if self.pages[row.start..row.end]
            .iter()
            .any(|p| p.page == current)
        {
            return current;
        }
        let column = current as usize % self.columns.max(1);
        self.pages
            .get((row.start + column).min(row.end - 1))
            .map_or(0, |p| p.page)
    }
    pub fn placement(&self, page: u32) -> Option<&PagePlacement> {
        self.pages.iter().find(|p| p.page == page)
    }
}

#[derive(Default)]
pub struct NavigationHistory<T = u32> {
    back: Vec<T>,
    forward: Vec<T>,
}
impl<T: Copy + PartialEq> NavigationHistory<T> {
    pub fn visit(&mut self, from: T, to: T) {
        if from == to {
            return;
        }
        if self.back.last() != Some(&from) {
            self.back.push(from);
        }
        if self.back.len() > 100 {
            self.back.remove(0);
        }
        self.forward.clear();
    }
    pub fn back(&mut self, current: T) -> Option<T> {
        let page = self.back.pop()?;
        self.forward.push(current);
        Some(page)
    }
    pub fn forward(&mut self, current: T) -> Option<T> {
        let page = self.forward.pop()?;
        self.back.push(current);
        Some(page)
    }
    pub fn has_back(&self) -> bool {
        !self.back.is_empty()
    }
    pub fn has_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_out_packs_two_four_eight_pages_without_overlap() {
        let sizes = [[612.0, 792.0]; 16];
        for (zoom, columns) in [(1.0, 1), (0.5, 2), (0.25, 4), (0.125, 8)] {
            let layout = PageLayout::new(&sizes, 1000.0, Some(zoom), 0, None);
            assert_eq!(layout.columns, columns);
            assert_eq!(layout.visible(24.0, layout.pages[0].bottom()), 0..columns);
            for pair in layout.pages[..columns].windows(2) {
                assert_eq!(pair[0].top, pair[1].top);
                assert!(pair[0].left + pair[0].width < pair[1].left);
            }
            assert_eq!(
                layout.page_at_with_hint(25.0, (columns - 1) as u32),
                (columns - 1) as u32
            );
            assert_eq!(
                layout.page_at_with_hint(layout.pages[columns].top, (columns - 1) as u32),
                (columns * 2 - 1) as u32
            );
            assert_eq!(
                PageLayout::new(&sizes, 1000.0, Some(zoom), 0, Some(9)).columns,
                1
            );
        }
        let mixed = PageLayout::new(
            &[[612.0, 792.0], [792.0, 612.0], [612.0, 792.0]],
            1000.0,
            Some(0.4),
            0,
            None,
        );
        assert_eq!(mixed.columns, 2);
        assert_eq!(
            mixed.visible(mixed.pages[1].bottom() + 2.0, mixed.pages[0].bottom()),
            0..2
        );
        let rotated = PageLayout::new(&sizes, 1100.0, Some(0.1), 1, None);
        assert_eq!(rotated.columns, 8);
    }
    #[test]
    fn custom_rgb_controls_paper_and_contrasting_ink() {
        let light = PaperTheme::Custom([205, 225, 245]);
        assert_eq!(light.pixel([255, 255, 255, 127]), [205, 225, 245, 127]);
        assert_eq!(light.pixel([0, 0, 0, 255]), [0, 0, 0, 255]);
        let dark = PaperTheme::Custom([15, 25, 35]);
        assert!(dark.is_dark());
        assert_eq!(dark.pixel([255, 255, 255, 255]), [15, 25, 35, 255]);
        assert!(dark.pixel([0, 0, 0, 255])[0] > 200);
    }
    #[test]
    fn continuous_scrolling_keeps_bitmap_width_at_viewport_boundaries() {
        let layout = PageLayout::new(&[[612.0, 792.0]; 5], 900.0, Some(2.2), 0, None);
        let first = &layout.pages[0];
        assert_ne!(
            layout.visible(0.0, 700.0).len(),
            layout
                .visible(first.bottom() - 100.0, first.bottom() + 600.0)
                .len()
        );
        let width = render_width(first.width, first.height, 2.0);
        for index in layout.visible(first.bottom() - 100.0, first.bottom() + 600.0) {
            let p = &layout.pages[index];
            assert_eq!(render_width(p.width, p.height, 2.0), width);
            assert!((width as f32).powi(2) * p.height / p.width <= 3_000_000.0);
        }
    }
    #[test]
    fn mixed_pages_continuous_scroll_and_jump_geometry() {
        let sizes = [[612.0, 792.0], [792.0, 612.0], [612.0, 792.0]];
        let l = PageLayout::new(&sizes, 800.0, None, 0, None);
        assert_eq!(l.pages.len(), 3);
        assert!(l.pages[1].height < l.pages[0].height);
        assert_eq!(l.page_at(l.pages[1].top + 10.0), 1);
        assert_eq!(l.page_at(l.pages[2].top), 2);
        assert_eq!(l.visible(l.pages[1].top, l.pages[1].bottom()), 1..2);
        let rotated = PageLayout::new(&sizes, 800.0, Some(1.5), 1, None);
        assert_eq!(rotated.page_at(rotated.pages[2].top), 2);
        let single = PageLayout::new(&sizes, 800.0, None, 0, Some(2));
        assert_eq!(single.pages.len(), 1);
        assert_eq!(single.pages[0].page, 2);
    }
    #[test]
    fn history_supports_back_forward_and_new_branch() {
        let mut h = NavigationHistory::default();
        h.visit(0, 2);
        h.visit(2, 8);
        assert_eq!(h.back(8), Some(2));
        assert_eq!(h.back(2), Some(0));
        assert_eq!(h.forward(0), Some(2));
        h.visit(2, 4);
        assert!(!h.has_forward());
        assert_eq!(h.back(4), Some(2));
    }
    #[test]
    fn eye_modes_recolor_paper_and_preserve_alpha() {
        assert_eq!(
            PaperTheme::Green.pixel([255, 255, 255, 255]),
            [218, 234, 214, 255]
        );
        assert_eq!(
            PaperTheme::Warm.pixel([255, 255, 255, 255]),
            [246, 235, 212, 255]
        );
        assert_eq!(
            PaperTheme::Night.pixel([0, 0, 0, 123]),
            [220, 224, 230, 123]
        );
        assert_eq!(PaperTheme::Green.pixel([0, 0, 0, 255]), [0, 0, 0, 255]);
    }

    #[test]
    fn dark_paper_preserves_highlight_hues_and_visible_contrast() {
        for theme in [PaperTheme::Night, PaperTheme::Custom([20, 24, 28])] {
            let paper = theme.pixel([255, 255, 255, 255]);
            for (input, channel) in [
                ([255, 246, 190, 123], 0),
                ([208, 246, 215, 123], 1),
                ([201, 232, 255, 123], 2),
                ([255, 218, 236, 123], 0),
            ] {
                let marked = theme.pixel(input);
                assert_eq!(marked[3], 123);
                assert!(marked[channel] > paper[channel] + 45);
                if channel == 0 {
                    assert!(marked[0] > marked[2] + 30);
                }
            }
        }
    }
}
