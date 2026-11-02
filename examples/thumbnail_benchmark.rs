//! Read-only local thumbnail benchmark; no network or service configuration.
use fpdf::pdf::{PdfEngine, library_path};
use std::{path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("PDF path required")?);
    let mut pdf = PdfEngine::new(&library_path()?)?;
    for round in 0..3 {
        {
            let width = 180;
            pdf.open(&path, "")?;
            let start = Instant::now();
            let image = pdf.render_thumbnail(7, width, 0)?;
            println!(
                "{}",
                serde_json::json!({"round":round,"width":width,"ms":start.elapsed().as_secs_f64()*1000.0,"bytes":image.rgba.len()})
            );
        }
    }
    Ok(())
}
