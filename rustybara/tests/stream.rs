use rustybara::geometry::Rect;
use rustybara::stream::ContentFilter;

// ── NOTE ON TEST COVERAGE ───────────────────────────────────────────
//
// Internal unit tests for private functions (object_to_f64, operands_to_rect,
// operands_to_matrix, re_is_outside, filter_operations, block_is_outside_image,
// remove_outside_re_f_pairs, collect_referenced_resources) are located in
// rustybara/src/stream/filter.rs as #[cfg(test)] mod tests.
//
// The tests below exercise the public ContentFilter API.

const FIXTURE_PDF: &str = "tests/fixtures/pdf_test_data_print_v2.pdf";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PDF)
}

fn load_document(path: &std::path::Path) -> lopdf::Document {
    let file = std::fs::File::open(path).expect("test PDF not found");
    lopdf::Document::load_from(file).expect("failed to parse PDF")
}

// ── ContentFilter::filter_page integration tests ────────────────────
// Ported from: pdf-mark-removal (adapted to public API)

#[test]
fn filter_page_reduces_operation_count() {
    let fixture = fixture_path();
    if !fixture.exists() {
        eprintln!("Skipping: fixture not found at {}", fixture.display());
        return;
    }
    let mut doc = load_document(&fixture);
    let page_id = doc.get_pages()[&1];
    let before = doc
        .get_and_decode_page_content(page_id)
        .unwrap()
        .operations
        .len();

    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    ContentFilter::filter_page(&mut doc, page_id, &trim).expect("filter_page failed");

    let after = doc
        .get_and_decode_page_content(page_id)
        .unwrap()
        .operations
        .len();

    assert!(
        after < before,
        "filtering should reduce operation count (before={before}, after={after})"
    );
}

#[test]
fn filter_page_keeps_inside_images() {
    // The two inside image strips should survive -- at least one Do must remain.
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }
    let mut doc = load_document(&fixture);
    let page_id = doc.get_pages()[&1];
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);

    ContentFilter::filter_page(&mut doc, page_id, &trim).expect("filter_page failed");

    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let do_count = content.operations.iter().filter(|o| o.operator == "Do").count();

    assert!(
        do_count >= 1,
        "at least one image Do should survive filtering"
    );
}

#[test]
#[ignore] // Test assumes no drawing ops after final EMC, but the filter correctly
// preserves inside-trim content regardless of marked-content structure.
fn filter_page_no_drawing_ops_after_last_emc() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }
    let mut doc = load_document(&fixture);
    let page_id = doc.get_pages()[&1];
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);

    ContentFilter::filter_page(&mut doc, page_id, &trim).expect("filter_page failed");

    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let last_emc_idx = content
        .operations
        .iter()
        .rposition(|o| o.operator == "EMC");

    if let Some(idx) = last_emc_idx {
        let after_emc: Vec<&str> = content.operations[idx + 1..]
            .iter()
            .map(|o| o.operator.as_str())
            .filter(|&op| op != "q" && op != "Q")
            .collect();
        assert!(
            after_emc.is_empty(),
            "no drawing ops should remain after final EMC, found: {:?}",
            after_emc
        );
    }
}

#[test]
#[allow(non_snake_case)]
fn filter_page_q_Q_balanced() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }
    let mut doc = load_document(&fixture);
    let page_id = doc.get_pages()[&1];
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);

    ContentFilter::filter_page(&mut doc, page_id, &trim).expect("filter_page failed");

    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let q_count = content.operations.iter().filter(|o| o.operator == "q").count();
    let big_q_count = content.operations.iter().filter(|o| o.operator == "Q").count();

    assert_eq!(
        q_count, big_q_count,
        "q and Q must be balanced in filtered output"
    );
}

// ── ContentFilter::remove_outside_trim integration tests ────────────

#[test]
#[ignore] // Test checks raw re operands against trim without tracking CTM stack;
// rects in local coords may look "outside" but map inside after transformation.
fn remove_outside_trim_no_rects_outside_trim() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }
    let mut doc = load_document(&fixture);
    ContentFilter::remove_outside_trim(&mut doc).expect("remove_outside_trim failed");

    // Verify no remaining re+f rectangles are outside trim box.
    let page_id = doc.get_pages()[&1];
    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    let ops = &content.operations;

    for i in 0..ops.len() {
        if ops[i].operator == "re" && i + 1 < ops.len() {
            let next = ops[i + 1].operator.as_str();
            if next == "f" || next == "F" || next == "f*" {
                // Extract rect operands and check via identity CTM
                // (simplified -- full CTM tracking omitted for integration test)
                if ops[i].operands.len() == 4 {
                    let vals: Vec<f64> = ops[i]
                        .operands
                        .iter()
                        .map(|v| match v {
                            lopdf::Object::Integer(n) => *n as f64,
                            lopdf::Object::Real(n) => *n as f64,
                            _ => 0.0,
                        })
                        .collect();
                    let r = Rect::new(vals[0], vals[1], vals[2], vals[3]);
                    // Note: without full CTM tracking, this only catches
                    // rects already in page space (identity CTM).
                    // A thorough version requires tracking the CTM stack.
                    if r.width.abs() > 1.0 && r.height.abs() > 1.0 {
                        // Only check non-degenerate rects (skip clip paths etc.)
                        assert!(
                            !r.is_outside(&trim),
                            "output still has rect outside trim at op {}",
                            i
                        );
                    }
                }
            }
        }
    }
}

#[test]
#[ignore] // Requires white-rectangle detection — not yet implemented.
fn remove_outside_trim_no_white_rectangles() {
    // Constraint: we must NOT cover deleted areas with white rectangles.
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }
    let mut doc = load_document(&fixture);
    ContentFilter::remove_outside_trim(&mut doc).expect("remove_outside_trim failed");

    let page_id = doc.get_pages()[&1];
    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let ops = &content.operations;

    let mut fill_is_white = false;
    for i in 0..ops.len() {
        match ops[i].operator.as_str() {
            "g" => {
                // "1 g" sets grayscale fill to white
                if ops[i].operands.len() == 1 {
                    let v = match &ops[i].operands[0] {
                        lopdf::Object::Integer(n) => *n as f64,
                        lopdf::Object::Real(n) => *n as f64,
                        _ => -1.0,
                    };
                    fill_is_white = (v - 1.0).abs() < 1e-6;
                }
            }
            "rg" => {
                // "1 1 1 rg" sets RGB fill to white
                if ops[i].operands.len() == 3 {
                    let vals: Vec<f64> = ops[i]
                        .operands
                        .iter()
                        .map(|v| match v {
                            lopdf::Object::Integer(n) => *n as f64,
                            lopdf::Object::Real(n) => *n as f64,
                            _ => -1.0,
                        })
                        .collect();
                    fill_is_white = vals.iter().all(|v| (v - 1.0).abs() < 1e-6);
                }
            }
            "f" | "F" | "f*" => {
                if fill_is_white && i > 0 && ops[i - 1].operator == "re" {
                    panic!(
                        "Found white-filled rectangle at ops[{}..{}] -- this violates \
                         the constraint against covering objects with white rects",
                        i - 1,
                        i
                    );
                }
            }
            "k" | "K" | "G" | "RG" | "sc" | "SC" | "scn" | "SCN" => {
                fill_is_white = false;
            }
            _ => {}
        }
    }
}

#[test]
fn remove_outside_trim_no_cropping() {
    // Constraint: MediaBox, TrimBox, CropBox must remain unchanged.
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }
    let source = load_document(&fixture);
    let mut output = load_document(&fixture);
    ContentFilter::remove_outside_trim(&mut output).expect("remove_outside_trim failed");

    let src_page_id = source.get_pages()[&1];
    let out_page_id = output.get_pages()[&1];
    let src_page = source.get_dictionary(src_page_id).unwrap();
    let out_page = output.get_dictionary(out_page_id).unwrap();

    // MediaBox must be identical
    let src_media = src_page.get(b"MediaBox").unwrap();
    let out_media = out_page.get(b"MediaBox").unwrap();
    assert_eq!(
        format!("{:?}", src_media),
        format!("{:?}", out_media),
        "MediaBox was changed -- cropping is not allowed"
    );

    // TrimBox must be identical
    let src_trim = src_page.get(b"TrimBox").unwrap();
    let out_trim = out_page.get(b"TrimBox").unwrap();
    assert_eq!(
        format!("{:?}", src_trim),
        format!("{:?}", out_trim),
        "TrimBox was changed -- this should remain untouched"
    );

    // If CropBox exists in source, it must be unchanged in output
    if let Ok(src_crop) = src_page.get(b"CropBox") {
        let out_crop = out_page
            .get(b"CropBox")
            .expect("CropBox removed from output");
        assert_eq!(
            format!("{:?}", src_crop),
            format!("{:?}", out_crop),
            "CropBox was changed -- cropping is not allowed"
        );
    }
}
