#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_reader;
mod app;
mod color_palette;
mod inline_outline;
mod outline_editor;
mod outline_folding;
mod search_ui;
mod text_selection;
mod theme_settings;
mod window_actions;

use fpdf::pdf::{PdfEngine, library_path, save_png};
use std::path::PathBuf;

fn main() {
    set_windows_app_id();
    if let Err(error) = run() {
        eprintln!("fpdf: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn set_windows_app_id() {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
    }
    let app_id: Vec<u16> = "libolibo.fpdf".encode_utf16().chain(Some(0)).collect();
    // SAFETY: The stable, NUL-terminated identifier remains live for the call.
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }
}

#[cfg(not(windows))]
fn set_windows_app_id() {}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--help" || a == "-h") {
        println!(
            "fpdf [document.pdf]\nfpdf --render document.pdf --output page.png [--page 1] [--width 1200] [--rotate 0]\nfpdf --smoke-ui document.pdf --screenshot window.png [--features] [--screenshot-night]\n\nDesktop: Ctrl+O open, PgUp/PgDn pages, Ctrl+Home/End first/last, Alt+Left/Right history, Ctrl+F find, Ctrl+0 fit width, Ctrl+1 actual size, Ctrl+plus/minus or Ctrl+wheel document zoom.\nPDF engine: adjacent pdfium.dll, or FPDF_PDFIUM_PATH."
        );
        return Ok(());
    }
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if let Some(input) = value("--render") {
        let output = value("--output").ok_or("--output is required")?;
        let page: u32 = value("--page")
            .map(|v| v.to_string_lossy().parse())
            .transpose()?
            .unwrap_or(1);
        if page == 0 {
            return Err("page numbers start at 1".into());
        }
        let width = value("--width")
            .map(|v| v.to_string_lossy().parse())
            .transpose()?
            .unwrap_or(1200);
        let rotate: u32 = value("--rotate")
            .map(|v| v.to_string_lossy().parse())
            .transpose()?
            .unwrap_or(0);
        if ![0, 90, 180, 270].contains(&rotate) {
            return Err("rotation must be 0, 90, 180, or 270".into());
        }
        let mut pdf = PdfEngine::new(&library_path()?)?;
        let count = pdf.open(&PathBuf::from(input), "")?;
        let image = pdf.render(page - 1, width, (rotate / 90) as u8)?;
        save_png(&PathBuf::from(output), &image)?;
        println!(
            "page={page}/{count} width={} height={} text_chars={}",
            image.width,
            image.height,
            image.text.chars().count()
        );
        return Ok(());
    }
    let smoke = value("--smoke-ui").map(PathBuf::from);
    let screenshot = value("--screenshot").map(PathBuf::from);
    if smoke.is_some() != screenshot.is_some() {
        return Err("--smoke-ui requires --screenshot".into());
    }
    let mut input = smoke.clone().or_else(|| {
        args.first()
            .filter(|a| !a.to_string_lossy().starts_with('-'))
            .map(PathBuf::from)
    });
    if input.is_none() && !args.is_empty() {
        return Err("unknown arguments; use --help".into());
    }
    let outline_smoke = args.iter().any(|a| a == "--outline-features");
    let reader_smoke = args.iter().any(|a| a == "--reading-features");
    let ai_smoke = args.iter().any(|a| a == "--ai-features");
    let grid_smoke = args.iter().any(|a| a == "--grid-features");
    let search_smoke = args.iter().any(|a| a == "--search-features");
    let settings_smoke = args.iter().any(|a| a == "--settings-features");
    if settings_smoke {
        if smoke.is_none() {
            return Err("--settings-features requires --smoke-ui".into());
        }
        let expected = std::env::var_os("FPDF_SMOKE_APPDATA")
            .ok_or("FPDF_SMOKE_APPDATA is required for isolated settings smoke")?;
        let actual = std::env::var_os("APPDATA").ok_or("APPDATA is required for settings smoke")?;
        if std::fs::canonicalize(PathBuf::from(&expected))?
            != std::fs::canonicalize(PathBuf::from(&actual))?
        {
            return Err("Settings smoke requires APPDATA to match FPDF_SMOKE_APPDATA".into());
        }
        if PathBuf::from(actual).join("fpdf/agent.txt").exists() {
            return Err(
                "Settings smoke requires a fresh isolated APPDATA without AI config".into(),
            );
        }
        for name in ["FPDF_SMOKE_ENDPOINT", "FPDF_SMOKE_KEY"] {
            if std::env::var(name).is_ok_and(|value| !value.trim().is_empty()) {
                continue;
            }
            return Err(format!("{name} is required for settings smoke").into());
        }
    }
    if search_smoke && smoke.is_none() {
        return Err("--search-features requires --smoke-ui".into());
    }
    if grid_smoke && smoke.is_none() {
        return Err("--grid-features requires --smoke-ui".into());
    }
    if ai_smoke && smoke.is_none() {
        return Err("--ai-features requires --smoke-ui".into());
    }
    let _outline_fixture = if outline_smoke || reader_smoke || search_smoke || settings_smoke {
        if smoke.is_none() {
            return Err("--outline-features requires --smoke-ui".into());
        }
        let dir = tempfile::tempdir()?;
        let copy = dir.path().join("目录编辑测试.pdf");
        std::fs::copy(input.as_ref().ok_or("missing PDF")?, &copy)?;
        input = Some(copy);
        Some(dir)
    } else {
        None
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("fpdf — PDF 阅读器")
            .with_icon(app_icon())
            .with_inner_size([1200.0, 860.0])
            .with_min_inner_size([760.0, 520.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "fpdf",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::ReaderApp::new(
                cc,
                input,
                screenshot,
                args.iter().any(|a| a == "--features")
                    || outline_smoke
                    || reader_smoke
                    || ai_smoke
                    || grid_smoke
                    || search_smoke
                    || settings_smoke,
                outline_smoke,
                reader_smoke,
                if settings_smoke {
                    app::ExtraSmoke::Settings
                } else if search_smoke {
                    app::ExtraSmoke::Search
                } else if ai_smoke {
                    app::ExtraSmoke::Ai
                } else if grid_smoke {
                    app::ExtraSmoke::Grid
                } else {
                    app::ExtraSmoke::None
                },
            )))
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn app_icon() -> eframe::egui::IconData {
    let mut rgba = vec![0; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let white = ((8..12).contains(&x) && (6..27).contains(&y))
                || ((11..23).contains(&x) && ((6..10).contains(&y) || (15..19).contains(&y)))
                || ((21..25).contains(&x) && (9..16).contains(&y));
            let pixel = if white {
                [255, 255, 255, 255]
            } else {
                [218, 51, 54, 255]
            };
            rgba[(y * 32 + x) * 4..(y * 32 + x + 1) * 4].copy_from_slice(&pixel);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: 32,
        height: 32,
    }
}
