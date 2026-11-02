//! Local, read-only PDF rendering benchmark. Never contacts an API.
use fpdf::pdf::{PageImage, PdfEngine, library_path, save_png};
use fpdf::reading::{PagePlacement, visible_tiles};
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let input = PathBuf::from(args.first().ok_or("PDF path required")?);
    let page = args
        .get(1)
        .map(|s| s.to_string_lossy().parse::<u32>())
        .transpose()?
        .unwrap_or(8)
        - 1;
    let output = PathBuf::from(args.get(2).ok_or("PNG output path required")?);
    let mut pdf = PdfEngine::new(&library_path()?)?;
    pdf.open(&input, "")?;
    let info = pdf.document_info()?;
    let [pw, ph] = info.sizes[page as usize];
    let layout = PagePlacement {
        page,
        left: 0.0,
        top: 0.0,
        width: 8000.0,
        height: 8000.0 * ph / pw,
    };
    let viewport = [2700.0, 500.0, 3900.0, 1300.0];
    let (width, tiles) = visible_tiles(&layout, viewport, 1.0);
    let scale = width as f32 / layout.width;
    let x0 = (viewport[0] * scale).floor() as u32;
    let y0 = (viewport[1] * scale).floor() as u32;
    let w = ((viewport[2] - viewport[0]) * scale).ceil() as u32;
    let h = ((viewport[3] - viewport[1]) * scale).ceil() as u32;
    let mut image = PageImage {
        width: w,
        height: h,
        rgba: vec![255; w as usize * h as usize * 4],
        points: [pw, ph],
        text: String::new(),
        text_truncated: false,
        geometry: None,
        links: Vec::new(),
    };
    let mut results = Vec::new();
    for _ in 0..3 {
        pdf.open(&input, "")?;
        let start = Instant::now();
        let old = pdf.render(page, 4119, 0)?;
        let old_ms = start.elapsed().as_secs_f64() * 1000.0;
        drop(old);
        pdf.open(&input, "")?;
        let start = Instant::now();
        let first_tile = pdf.render_tile(page, width, 0, tiles[0])?;
        let first_tile_ms = start.elapsed().as_secs_f64() * 1000.0;
        drop(first_tile);
        let detail_start = Instant::now();
        for tile in &tiles {
            let piece = pdf.render_tile(page, width, 0, *tile)?;
            let core = tile.core(width, (width as f64 * ph as f64 / pw as f64).round() as u32);
            let l = core.x.max(x0);
            let t = core.y.max(y0);
            let r = (core.x + core.width).min(x0 + w);
            let b = (core.y + core.height).min(y0 + h);
            for y in t..b {
                let source = ((y - tile.y) * tile.width + l - tile.x) as usize * 4;
                let dest = ((y - y0) * w + l - x0) as usize * 4;
                let len = (r - l) as usize * 4;
                image.rgba[dest..dest + len].copy_from_slice(&piece.rgba[source..source + len]);
            }
        }
        let tiles_ms = detail_start.elapsed().as_secs_f64() * 1000.0;
        let total_ms = start.elapsed().as_secs_f64() * 1000.0;
        results.push(
            serde_json::json!({"whole_page_ms": old_ms, "first_tile_ms": first_tile_ms,
            "tiles_ms": tiles_ms, "total_ms": total_ms}),
        );
    }
    save_png(&output, &image)?;
    println!(
        "{}",
        serde_json::json!({"runs":results,"full_width":width,"clip":[x0,y0,w,h],"tile_count":tiles.len(),"tile_pixels":tiles.iter().map(|t| u64::from(t.width)*u64::from(t.height)).sum::<u64>()})
    );
    Ok(())
}
