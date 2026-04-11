use rustybara::geometry::Rect;
use rustybara::pages::PageBoxes;

// ── PageBoxes unit tests (constructed directly, no PDF needed) ──────
// Ported from: pdf-trim-or-bleed-resizer, pdf-mark-removal

#[test]
fn trim_or_media_returns_trim_when_present() {
    let boxes = PageBoxes {
        media_box: Rect::from_corners(0.0, 0.0, 792.0, 612.0),
        trim_box: Some(Rect::from_corners(30.0, 30.0, 642.0, 822.0)),
        bleed_box: None,
        crop_box: None,
    };
    let result = boxes.trim_or_media();
    assert!((result.x - 30.0).abs() < 1e-10);
    assert!((result.y - 30.0).abs() < 1e-10);
    assert!((result.right() - 642.0).abs() < 1e-10);
    assert!((result.top() - 822.0).abs() < 1e-10);
}

#[test]
fn trim_or_media_falls_back_to_media() {
    let boxes = PageBoxes {
        media_box: Rect::from_corners(0.0, 0.0, 792.0, 612.0),
        trim_box: None,
        bleed_box: None,
        crop_box: None,
    };
    let result = boxes.trim_or_media();
    assert!((result.x - 0.0).abs() < 1e-10);
    assert!((result.y - 0.0).abs() < 1e-10);
}

#[test]
fn bleed_rect_expands_trim() {
    let boxes = PageBoxes {
        media_box: Rect::from_corners(0.0, 0.0, 792.0, 612.0),
        trim_box: Some(Rect::from_corners(30.0, 30.0, 642.0, 822.0)),
        bleed_box: None,
        crop_box: None,
    };
    let bleed = boxes.bleed_rect(9.0);
    // expand(9.0) should extend 9pt on each side:
    //   width = 612 + 18 = 630, height = 792 + 18 = 810
    assert!((bleed.width - 630.0).abs() < 0.001);
    assert!((bleed.height - 810.0).abs() < 0.001);
}

// ── PageBoxes::read integration tests (require PDF fixture) ─────────
// Ported from: pdf-mark-removal (TrimBox checkpoint)

const FIXTURE_PDF: &str = "tests/fixtures/pdf_test_data_print_v2.pdf";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PDF)
}

#[test]
fn page_boxes_read_trim_box_values() {
    let fixture = fixture_path();
    if !fixture.exists() {
        eprintln!("Skipping: fixture not found at {}", fixture.display());
        return;
    }
    let file = std::fs::File::open(&fixture).expect("open fixture");
    let doc = lopdf::Document::load_from(file).expect("parse PDF");
    let page_id = doc.get_pages()[&1];

    let boxes = PageBoxes::read(&doc, page_id).expect("read page boxes");

    // From analysis: TrimBox = [30, 30, 642, 822]
    let trim = boxes.trim_box.expect("TrimBox should exist");
    assert!((trim.x - 30.0).abs() < 0.01, "TrimBox x should be 30");
    assert!((trim.y - 30.0).abs() < 0.01, "TrimBox y should be 30");
    assert!(
        (trim.right() - 642.0).abs() < 0.01,
        "TrimBox right should be 642"
    );
    assert!(
        (trim.top() - 822.0).abs() < 0.01,
        "TrimBox top should be 822"
    );
}

#[test]
fn page_boxes_read_has_media_box() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }
    let file = std::fs::File::open(&fixture).expect("open fixture");
    let doc = lopdf::Document::load_from(file).expect("parse PDF");
    let page_id = doc.get_pages()[&1];

    let boxes = PageBoxes::read(&doc, page_id).expect("read page boxes");

    // MediaBox should always exist
    assert!(boxes.media_box.width > 0.0, "MediaBox should have positive width");
    assert!(
        boxes.media_box.height > 0.0,
        "MediaBox should have positive height"
    );
}
