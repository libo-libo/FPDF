//! Pure reading layout, navigation history, and display color transforms.
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
pub struct NavigationHistory {
    back: Vec<u32>,
    forward: Vec<u32>,
}
impl NavigationHistory {
    pub fn visit(&mut self, from: u32, to: u32) {
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
    pub fn back(&mut self, current: u32) -> Option<u32> {
        let page = self.back.pop()?;
        self.forward.push(current);
        Some(page)
    }
    pub fn forward(&mut self, current: u32) -> Option<u32> {
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
}
