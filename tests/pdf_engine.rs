use fpdf::pdf::{PdfEngine, library_path};
use fpdf::worker::{self, RenderRequest};
use std::path::PathBuf;
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn outline_round_trip(pdf: &mut PdfEngine) {
    use fpdf::outline::OutlineDocument;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("原稿.pdf");
    let saved = dir.path().join("已修改.pdf");
    std::fs::copy(fixture("reading-demo.pdf"), &source).unwrap();
    // Existing zoom destinations and bookmark styling must survive a rename.
    let mut styled = lopdf::Document::load(&source).unwrap();
    let root = styled
        .catalog()
        .unwrap()
        .get(b"Outlines")
        .unwrap()
        .as_reference()
        .unwrap();
    let first = styled
        .get_dictionary(root)
        .unwrap()
        .get(b"First")
        .unwrap()
        .as_reference()
        .unwrap();
    let second = styled
        .get_dictionary(first)
        .unwrap()
        .get(b"Next")
        .unwrap()
        .as_reference()
        .unwrap();
    let third = styled
        .get_dictionary(second)
        .unwrap()
        .get(b"Next")
        .unwrap()
        .as_reference()
        .unwrap();
    let last_page = styled.get_pages()[&3];
    let precise = lopdf::Object::Array(vec![
        last_page.into(),
        lopdf::Object::Name(b"XYZ".to_vec()),
        15.into(),
        25.into(),
        lopdf::Object::Real(1.5),
    ]);
    styled
        .get_dictionary_mut(third)
        .unwrap()
        .set("Dest", precise.clone());
    styled.get_dictionary_mut(third).unwrap().set("F", 2);
    styled.save(&source).unwrap();
    let source_bytes = std::fs::read(&source).unwrap();
    pdf.open(&source, "").unwrap();
    let before_pixels = pdf.render(0, 400, 0).unwrap().rgba;
    let info = pdf.document_info().unwrap();
    let editor = OutlineDocument::load(&source, &info.bookmarks).unwrap();
    let mut tree = editor.tree.clone();
    tree.entries[0].title = "第一章：目录修改测试".into();
    tree.entries[0].page = Some(2);
    let child = tree.add(Some(0), true, 1);
    tree.entries[child].title = "子章节".into();
    tree.remove(2); // Remove the original second chapter, retaining the child.
    editor.save(&tree, &saved).unwrap();
    let persisted = lopdf::Document::load(&saved).unwrap();
    assert_eq!(
        persisted
            .get_dictionary(third)
            .unwrap()
            .get(b"Dest")
            .unwrap(),
        &precise
    );
    assert_eq!(
        persisted
            .get_dictionary(third)
            .unwrap()
            .get(b"F")
            .unwrap()
            .as_i64()
            .unwrap(),
        2
    );
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    assert!(
        editor
            .save(&tree, &dir.path().join("missing/subdir.pdf"))
            .is_err()
    );
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    pdf.open(&saved, "").unwrap();
    let reopened = pdf.document_info().unwrap();
    assert_eq!(reopened.sizes, info.sizes);
    assert_eq!(
        reopened
            .bookmarks
            .iter()
            .map(|b| (b.title.as_str(), b.page, b.depth))
            .collect::<Vec<_>>(),
        vec![
            ("第一章：目录修改测试", Some(2), 0),
            ("子章节", Some(1), 1),
            ("03 Final page", Some(2), 0)
        ]
    );
    assert_eq!(pdf.render(0, 400, 0).unwrap().rgba, before_pixels);
    // Save over an existing file, then reopen with an independent parser/renderer.
    editor.save(&tree, &source).unwrap();
    pdf.open(&source, "").unwrap();
    let edited_info = pdf.document_info().unwrap();
    assert_eq!(edited_info.bookmarks[0].title, "第一章：目录修改测试");
    let edited = OutlineDocument::load(&source, &edited_info.bookmarks).unwrap();
    let mut empty = edited.tree.clone();
    empty.entries.clear();
    edited.save(&empty, &source).unwrap();
    pdf.open(&source, "").unwrap();
    assert!(pdf.document_info().unwrap().bookmarks.is_empty());
    // A PDF without an outline can receive its first persistent chapter.
    let blank = OutlineDocument::load(&source, &[]).unwrap();
    let mut fresh = blank.tree.clone();
    fresh.add(None, false, 1);
    blank.save(&fresh, &source).unwrap();
    pdf.open(&source, "").unwrap();
    assert_eq!(pdf.document_info().unwrap().bookmarks[0].page, Some(1));
    // Invalid edits and externally changed originals must remain untouched.
    let snapshot = std::fs::read(&saved).unwrap();
    let mut invalid = tree.clone();
    invalid.entries[0].page = Some(999);
    assert!(editor.save(&invalid, &saved).is_err());
    assert_eq!(std::fs::read(&saved).unwrap(), snapshot);
    assert!(
        editor
            .save(&tree, &saved)
            .unwrap_err()
            .contains("其他程序修改")
    );
    assert_eq!(std::fs::read(&saved).unwrap(), snapshot);
    assert!(OutlineDocument::load(&fixture("password.pdf"), &[]).is_err());
    pdf.open(&fixture("reading-demo.pdf"), "").unwrap();
}

// One end-to-end test deliberately serializes PDFium's process-global runtime.
// Missing runtime is a real failure, never a skipped/false-positive success.
#[test]
fn real_pdf_loading_rendering_and_worker_lifecycle() {
    let mut pdf = PdfEngine::new(&library_path().expect("PDFium runtime installed")).unwrap();
    assert!(PdfEngine::new(&library_path().unwrap()).is_err());
    assert_eq!(pdf.open(&fixture("reading-demo.pdf"), "").unwrap(), 3);
    let info = pdf.document_info().unwrap();
    assert_eq!(
        info.sizes,
        vec![[612.0, 792.0], [792.0, 612.0], [612.0, 792.0]]
    );
    assert_eq!(
        info.bookmarks.iter().map(|b| b.page).collect::<Vec<_>>(),
        vec![Some(0), Some(1), Some(2)]
    );
    assert_eq!(info.bookmarks[1].title, "02 Landscape");
    let portrait = pdf.render(0, 612, 0).unwrap();
    assert_eq!((portrait.width, portrait.height), (612, 792));
    assert!(portrait.text.contains("A clearer way to read."));
    let plain = pdf.read_text(0).unwrap();
    assert_eq!(plain.page, 0);
    assert_eq!(plain.text, portrait.text);
    assert!(!plain.truncated);
    assert!(pdf.read_text(3).is_err());
    let start = [50.0 / 612.0, (792.0 - 672.0) / 792.0];
    let end = [200.0 / 612.0, (792.0 - 672.0) / 792.0];
    let selected = pdf.select_text(0, start, end, 0).unwrap();
    assert!(selected.text.contains("clearer"), "{}", selected.text);
    assert!(!selected.context.is_empty());
    assert!(!selected.rects.is_empty());
    let reversed = pdf.select_text(0, end, start, 0).unwrap();
    assert_eq!(reversed.text, selected.text);
    for rotation in 1..=3 {
        let rotate = |[x, y]: [f64; 2]| match rotation {
            1 => [1.0 - y, x],
            2 => [1.0 - x, 1.0 - y],
            _ => [y, 1.0 - x],
        };
        let rotated = pdf
            .select_text(0, rotate(start), rotate(end), rotation)
            .unwrap();
        assert_eq!(rotated.text, selected.text);
        assert!(
            rotated
                .rects
                .iter()
                .flatten()
                .all(|&value| (-0.01..=1.01).contains(&value))
        );
    }
    assert!(pdf.select_text(0, [0.99, 0.99], [0.98, 0.98], 0).is_err());
    assert!(pdf.select_text(0, [f64::NAN, 0.0], end, 0).is_err());
    let third_selection = pdf
        .select_text(
            2,
            [50.0 / 612.0, 72.0 / 792.0],
            [348.0 / 612.0, 72.0 / 792.0],
            0,
        )
        .unwrap();
    assert!(third_selection.text.contains("Every page"));
    let word = pdf
        .select_word(2, [150.0 / 612.0, 72.0 / 792.0], 0)
        .unwrap();
    assert_eq!(word.text, "page");
    let multi = pdf
        .select_text(
            2,
            [50.0 / 612.0, 72.0 / 792.0],
            [300.0 / 612.0, 127.0 / 792.0],
            0,
        )
        .unwrap();
    assert!(multi.text.contains("Every page"));
    assert!(multi.text.contains("third and final"), "{}", multi.text);
    assert!(multi.rects.len() >= 2);
    assert!(
        portrait
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 170 && p[1] < 100 && p[2] < 100)
    );
    assert!(
        portrait
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] < 120 && p[1] < 140 && p[2] < 150)
    );
    assert!(portrait.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    let landscape = pdf.render(1, 792, 0).unwrap();
    assert_eq!((landscape.width, landscape.height), (792, 612));
    assert!(landscape.text.contains("A different perspective."));
    let rotated = pdf.render(0, 792, 1).unwrap();
    assert_eq!((rotated.width, rotated.height), (792, 612));
    let enlarged = pdf.render(0, 1224, 0).unwrap();
    assert_eq!((enlarged.width, enlarged.height), (1224, 1584));
    outline_round_trip(&mut pdf);
    assert!(pdf.render(3, 612, 0).is_err());
    assert!(pdf.render(0, 0, 0).is_err());
    assert!(pdf.render(0, 8192, 0).is_err()); // pixel budget
    assert!(pdf.render(0, 612, 4).is_err());
    assert!(pdf.open(&fixture("invalid.pdf"), "").is_err());
    assert_eq!(pdf.page_count(), 3); // failed replacement preserves old document
    assert!(pdf.open(&fixture("missing.pdf"), "").is_err());
    assert!(
        pdf.open(&fixture("password.pdf"), "")
            .unwrap_err()
            .password_required
    );
    assert!(
        pdf.open(&fixture("password.pdf"), "wrong")
            .unwrap_err()
            .password_required
    );
    assert_eq!(pdf.open(&fixture("password.pdf"), "readme").unwrap(), 3);
    assert!(pdf.render(0, 612, 0).unwrap().text.contains("essentials"));
    let unicode = std::env::temp_dir().join(format!("fpdf-中文测试-{}.pdf", std::process::id()));
    std::fs::copy(fixture("reading-demo.pdf"), &unicode).unwrap();
    assert_eq!(pdf.open(&unicode, "").unwrap(), 3);
    std::fs::remove_file(unicode).unwrap();
    pdf.close();
    assert!(pdf.render(0, 612, 0).is_err());
    drop(pdf);

    let (tx, rx) = worker::spawn(|| {});
    for id in 1..=3 {
        tx.send(RenderRequest {
            id,
            document_id: 1,
            path: fixture("reading-demo.pdf"),
            password: String::new(),
            page: (id - 1) as u32,
            width: 612,
            rotation: 0,
        })
        .unwrap();
    }
    let result = loop {
        let next = rx.recv_timeout(Duration::from_secs(15)).unwrap();
        if next.id == 3 {
            break next;
        }
    };
    assert_eq!(result.pages, 3);
    let original = result.result.unwrap();
    assert!(original.text.contains("third and final"));
    tx.send(RenderRequest {
        id: 4,
        document_id: 1,
        path: fixture("reading-demo.pdf"),
        password: String::new(),
        page: 2,
        width: 612,
        rotation: 0,
    })
    .unwrap();
    let cached = rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&original, &cached));
    tx.send(RenderRequest {
        id: 5,
        document_id: 2,
        path: fixture("password.pdf"),
        password: String::new(),
        page: 0,
        width: 612,
        rotation: 0,
    })
    .unwrap();
    assert!(
        rx.recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result
            .unwrap_err()
            .password_required
    );
    tx.send(RenderRequest {
        id: 6,
        document_id: 3,
        path: fixture("password.pdf"),
        password: "readme".into(),
        page: 0,
        width: 612,
        rotation: 0,
    })
    .unwrap();
    assert!(
        rx.recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result
            .unwrap()
            .text
            .contains("essentials")
    );
    // Opening succeeds even when the first render exceeds its pixel budget;
    // the response must preserve page count so the UI can navigate elsewhere.
    tx.send(RenderRequest {
        id: 7,
        document_id: 4,
        path: fixture("reading-demo.pdf"),
        password: String::new(),
        page: 0,
        width: 8192,
        rotation: 0,
    })
    .unwrap();
    let oversized = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(oversized.pages, 3);
    assert!(oversized.result.is_err());
    tx.send(RenderRequest {
        id: 8,
        document_id: 4,
        path: fixture("reading-demo.pdf"),
        password: String::new(),
        page: 1,
        width: 792,
        rotation: 0,
    })
    .unwrap();
    assert!(
        rx.recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result
            .unwrap()
            .text
            .contains("perspective")
    );
    drop(tx);
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(10)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    ));

    // A viewport batch must deliver every visible page and thumbnail, rather
    // than coalescing separate pages down to only the final item.
    use worker::{RenderTarget, ViewportRequest};
    let channels = worker::spawn_reader(|| {});
    let (tx, rx) = (channels.requests, channels.responses);
    let targets = (0..3)
        .map(|page| RenderTarget {
            page,
            width: 612,
            rotation: 0,
            thumbnail: false,
        })
        .chain((0..2).map(|page| RenderTarget {
            page,
            width: 180,
            rotation: 0,
            thumbnail: true,
        }))
        .collect::<Vec<_>>();
    tx.send(ViewportRequest {
        id: 100,
        document_id: 10,
        path: fixture("reading-demo.pdf"),
        password: String::new(),
        targets: targets.clone(),
    })
    .unwrap();
    let mut received = Vec::new();
    for _ in 0..targets.len() {
        let r = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(r.id, 100);
        assert_eq!(r.document_id, 10);
        assert_eq!(r.info.as_ref().unwrap().bookmarks.len(), 3);
        assert_eq!(r.result.unwrap().width, r.target.width);
        received.push(r.target);
    }
    assert_eq!(received, targets);
    channels
        .text_requests
        .send(worker::TextRequest {
            id: 8,
            document_id: 10,
            path: fixture("reading-demo.pdf"),
            password: String::new(),
            pages: vec![0, 2],
        })
        .unwrap();
    let text = channels
        .text_responses
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    assert_eq!(text.id, 8);
    let pages = text.result.unwrap();
    assert!(pages[0].text.contains("A clearer way to read."));
    assert!(pages[1].text.contains("Every page has a story."));
    channels
        .selection_requests
        .send(worker::SelectionRequest {
            id: 9,
            document_id: 10,
            path: fixture("reading-demo.pdf"),
            password: String::new(),
            page: 0,
            start,
            end,
            rotation: 0,
            word: false,
        })
        .unwrap();
    let selected = channels
        .selection_responses
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    assert_eq!(selected.id, 9);
    assert!(selected.result.unwrap().text.contains("clearer"));
    let rotated = RenderTarget {
        page: 2,
        width: 792,
        rotation: 1,
        thumbnail: false,
    };
    tx.send(ViewportRequest {
        id: 101,
        document_id: 10,
        path: fixture("reading-demo.pdf"),
        password: String::new(),
        targets: vec![rotated],
    })
    .unwrap();
    let r = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(r.target, rotated);
    assert_eq!(r.result.unwrap().height, 612);
    drop(tx);
}
