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

/// Stable per-page sampling prevents cache churn as neighbours enter the viewport.
/// Supersampling sharpens fine vector lines; 12 MP fits the existing 64 MiB cache.
/// Neither PDFium's hard allocation limit nor the rendering cache budget is raised.
pub const PAGE_RENDER_PIXELS: f32 = 12_000_000.0;

pub fn render_width(width: f32, height: f32, pixels_per_point: f32) -> u32 {
    let aspect = width / height.max(1.0);
    let cap = (PAGE_RENDER_PIXELS * aspect)
        .sqrt()
        .floor()
        .min(8192.0)
        .min((8192.0 * aspect).floor())
        .max(64.0);
    (width * pixels_per_point * 1.5).round().clamp(64.0, cap) as u32
}

/// Integer pixel rectangle in the rotated full-page raster coordinate system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RasterTile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl RasterTile {
    /// The paintable core excludes the rasterizer gutter on each interior edge.
    pub fn core(self, full_width: u32, full_height: u32) -> Self {
        let x = if self.x == 0 { 0 } else { self.x + 16 };
        let y = if self.y == 0 { 0 } else { self.y + 16 };
        Self {
            x,
            y,
            width: (x + 768).min(full_width) - x,
            height: (y + 768).min(full_height) - y,
        }
    }
}

/// Area sampling preserves thin vector strokes when reducing navigation images.
pub fn thumbnail_rgba(source: &[u8], size: [u32; 2], width: u32) -> (u32, Vec<u8>) {
    let [sw, sh] = size;
    let height =
        ((u64::from(sh) * u64::from(width) + u64::from(sw) / 2) / u64::from(sw)).max(1) as u32;
    let mut output = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        let top = y * sh / height;
        let bottom = ((y + 1) * sh / height).max(top + 1).min(sh);
        for x in 0..width {
            let left = x * sw / width;
            let right = ((x + 1) * sw / width).max(left + 1).min(sw);
            let mut sum = [0u32; 4];
            for sy in top..bottom {
                for sx in left..right {
                    let offset = ((sy * sw + sx) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += u32::from(source[offset + c]);
                    }
                }
            }
            let count = (bottom - top) * (right - left);
            let at = ((y * width + x) * 4) as usize;
            for c in 0..4 {
                output[at + c] = (sum[c] / count) as u8;
            }
        }
    }
    (height, output)
}

/// Populate a small fallback from already-rendered tiles; no second PDF draw is needed.
pub fn stitch_preview(
    preview: &mut [u8],
    size: [u32; 2],
    tile: RasterTile,
    full_size: [u32; 2],
    source: &[u8],
) {
    let [width, height] = size;
    let [full_width, full_height] = full_size;
    let l = tile.x * width / full_width;
    let t = tile.y * height / full_height;
    let r = ((tile.x + tile.width) * width)
        .div_ceil(full_width)
        .min(width);
    let b = ((tile.y + tile.height) * height)
        .div_ceil(full_height)
        .min(height);
    for y in t..b {
        let sy = (y * full_height / height)
            .saturating_sub(tile.y)
            .min(tile.height - 1);
        for x in l..r {
            let sx = (x * full_width / width)
                .saturating_sub(tile.x)
                .min(tile.width - 1);
            let from = ((sy * tile.width + sx) * 4) as usize;
            let to = ((y * width + x) * 4) as usize;
            preview[to..to + 4].copy_from_slice(&source[from..from + 4]);
        }
    }
}

/// Bound the entire visible tile set, including grid overdraw and gutters.
/// This keeps required images resident together within the existing cache budget.
pub fn viewport_raster_density(
    pages: &[PagePlacement],
    viewport: [f32; 4],
    pixels_per_point: f32,
) -> f32 {
    let mut density = pixels_per_point;
    for _ in 0..12 {
        let pixels: u64 = pages
            .iter()
            .flat_map(|p| visible_tiles(p, viewport, density).1)
            .map(|tile| u64::from(tile.width) * u64::from(tile.height))
            .sum();
        if pixels <= 12_000_000 {
            break;
        }
        density *= 0.8;
    }
    density
}

/// Render only visible regions at native display density, independent of page area.
/// Fixed grid coordinates let scrolling reuse all overlapping cached tiles.
pub fn visible_tiles(
    page: &PagePlacement,
    viewport: [f32; 4],
    pixels_per_point: f32,
) -> (u32, Vec<RasterTile>) {
    let [left, top, right, bottom] = viewport;
    let area = ((right - left) * (bottom - top)).max(1.0);
    let density = (pixels_per_point * 1.25).min((10_000_000.0 / area).sqrt());
    let full_width = ((page.width * density / 64.0).ceil() * 64.0).clamp(64.0, 262_144.0) as u32;
    let scale = full_width as f32 / page.width;
    let full_height = (full_width as f64 * page.height as f64 / page.width as f64).round() as u32;
    let x0 = ((left - page.left).max(0.0) * scale).floor() as u32;
    let y0 = ((top - page.top).max(0.0) * scale).floor() as u32;
    let x1 = ((right - page.left).max(0.0) * scale)
        .ceil()
        .min(full_width as f32) as u32;
    let y1 = ((bottom - page.top).max(0.0) * scale)
        .ceil()
        .min(full_height as f32) as u32;
    let mut tiles = Vec::new();
    if x0 < x1 && y0 < y1 {
        for y in (y0 / 768 * 768..y1).step_by(768) {
            for x in (x0 / 768 * 768..x1).step_by(768) {
                // Rasterize a 16-pixel gutter; only paint the core to avoid clipped-glyph edges.
                let l = x.saturating_sub(16);
                let t = y.saturating_sub(16);
                tiles.push(RasterTile {
                    x: l,
                    y: t,
                    width: (x + 784).min(full_width) - l,
                    height: (y + 784).min(full_height) - t,
                });
            }
        }
    }
    (full_width, tiles)
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
    fn thumbnail_area_sampling_keeps_thin_lines_and_page_proportions() {
        let mut source = vec![255; 8 * 4 * 4];
        for y in 0..4 {
            source[(y * 8 + 1) * 4..(y * 8 + 1) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
        let (height, result) = thumbnail_rgba(&source, [8, 4], 2);
        assert_eq!(height, 1);
        assert_eq!(result, vec![191, 191, 191, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn assembled_preview_preserves_old_regions_when_new_tiles_arrive() {
        let mut preview = vec![255; 4 * 4 * 4];
        let red = [255, 0, 0, 255].repeat(4 * 4);
        let blue = [0, 0, 255, 255].repeat(4 * 4);
        stitch_preview(
            &mut preview,
            [4, 4],
            RasterTile {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
            [8, 8],
            &red,
        );
        stitch_preview(
            &mut preview,
            [4, 4],
            RasterTile {
                x: 4,
                y: 4,
                width: 4,
                height: 4,
            },
            [8, 8],
            &blue,
        );
        assert_eq!(&preview[..4], &[255, 0, 0, 255]);
        assert_eq!(&preview[60..64], &[0, 0, 255, 255]);
        assert_eq!(&preview[12..16], &[255, 255, 255, 255]);
    }

    #[test]
    fn visible_tile_set_fits_cache_even_at_grid_boundaries_and_high_dpi() {
        let page = PagePlacement {
            page: 0,
            left: 0.0,
            top: 0.0,
            width: 20000.0,
            height: 20000.0,
        };
        for dpi in [1.0, 1.25, 1.5, 2.0, 4.0] {
            for offset in [0.0, 760.0, 1530.0, 2200.0] {
                let viewport = [offset, offset, offset + 1800.0, offset + 1800.0];
                let density = viewport_raster_density(std::slice::from_ref(&page), viewport, dpi);
                let (_, tiles) = visible_tiles(&page, viewport, density);
                let pixels: u64 = tiles
                    .iter()
                    .map(|t| u64::from(t.width) * u64::from(t.height))
                    .sum();
                assert!(
                    pixels <= 12_000_000,
                    "visible tiles cannot evict one another at dpi {dpi}"
                );
            }
        }
    }

    #[test]
    fn viewport_tiles_keep_full_zoom_resolution_and_reuse_grid_on_scroll() {
        let page = PagePlacement {
            page: 7,
            left: 20.0,
            top: 30.0,
            width: 8000.0,
            height: 5656.0,
        };
        let (width, first) = visible_tiles(&page, [1600.0, 800.0, 2800.0, 1600.0], 1.5);
        assert!(
            width > 8192,
            "zoom detail must not inherit the whole-page raster ceiling"
        );
        assert!(first.len() < 20);
        assert!(first.iter().all(|t| t.width <= 800 && t.height <= 800));
        let (other_width, scrolled) = visible_tiles(&page, [1605.0, 805.0, 2805.0, 1605.0], 1.5);
        assert_eq!(width, other_width);
        assert!(first.iter().filter(|t| scrolled.contains(t)).count() >= first.len() - 1);
        let (_, hidden) = visible_tiles(&page, [9000.0, 0.0, 10000.0, 900.0], 1.0);
        assert!(hidden.is_empty());
        let pixels: u64 = first
            .iter()
            .map(|t| u64::from(t.width) * u64::from(t.height))
            .sum();
        assert!(pixels * 4 < 64 * 1024 * 1024);
    }

    #[test]
    fn dense_vector_pages_rerender_sharply_without_raising_memory_limits() {
        let normal = render_width(1200.0, 848.0, 1.0);
        let zoomed = render_width(3200.0, 2263.0, 1.0);
        assert_eq!(normal, 1800);
        assert!(zoomed > 4000, "must exceed the old 3 MP landscape ceiling");
        assert!(zoomed > normal * 2);
        for (w, h) in [
            (1200.0, 848.0),
            (848.0, 1200.0),
            (4000.0, 8000.0),
            (8000.0, 2000.0),
        ] {
            let width = render_width(w, h, 4.0);
            let height = (width as f32 * h / w).round() as u64;
            assert!(width <= 8192 && height <= 8192);
            assert!(u64::from(width) * height <= crate::pdf::MAX_PIXELS);
            assert!(u64::from(width) * height * 4 < 64 * 1024 * 1024);
        }
    }

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
            assert!((width as f32).powi(2) * p.height / p.width <= PAGE_RENDER_PIXELS);
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
