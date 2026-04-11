use rustybara::pages::PageBoxes;
use rustybara::stream::ContentFilter;

// ── Helpers ─────────────────────────────────────────────────────────

const FIXTURE_PDF: &str = "tests/fixtures/pdf_test_data_print_v2.pdf";

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PDF)
}

fn batch_fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/batch")
}

fn secret_fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/secret_fixture")
}

fn load_document(path: &std::path::Path) -> lopdf::Document {
    let file = std::fs::File::open(path).expect("test PDF not found");
    lopdf::Document::load_from(file).expect("failed to parse PDF")
}

fn try_load_document(path: &std::path::Path) -> Option<lopdf::Document> {
    let file = std::fs::File::open(path).ok()?;
    lopdf::Document::load_from(file).ok()
}

fn obj_to_f64(obj: &lopdf::Object) -> f64 {
    match obj {
        lopdf::Object::Integer(i) => *i as f64,
        lopdf::Object::Real(r) => *r as f64,
        _ => panic!("non-numeric PDF object: {:?}", obj),
    }
}

// ════════════════════════════════════════════════════════════════════
// Pipeline 1: ptrim — Load → read boxes → filter outside trim → save
// ════════════════════════════════════════════════════════════════════

#[test]
fn ptrim_pipeline_single_page() {
    let fixture = fixture_path();
    if !fixture.exists() {
        eprintln!("Skipping: fixture not found at {}", fixture.display());
        return;
    }

    let mut doc = load_document(&fixture);
    let page_id = doc.get_pages()[&1];

    // Read boxes
    let boxes = PageBoxes::read(&doc, page_id).expect("read page boxes");
    let trim = boxes.trim_or_media();
    assert!(trim.width > 0.0 && trim.height > 0.0);

    // Count ops before
    let before = doc
        .get_and_decode_page_content(page_id)
        .unwrap()
        .operations
        .len();

    // Filter
    ContentFilter::filter_page(&mut doc, page_id, &trim).expect("filter_page failed");

    // Count ops after — should be fewer
    let after = doc
        .get_and_decode_page_content(page_id)
        .unwrap()
        .operations
        .len();
    assert!(
        after < before,
        "filter should reduce op count (before={before}, after={after})"
    );

    // Save round-trip
    let out = std::env::temp_dir().join("rustybara_ptrim_pipeline.pdf");
    doc.save(&out).expect("save failed");
    assert!(out.exists());
    assert!(std::fs::metadata(&out).unwrap().len() > 0);
    std::fs::remove_file(&out).ok();
}

#[test]
fn ptrim_pipeline_preserves_boxes() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }

    let original = load_document(&fixture);
    let mut doc = load_document(&fixture);

    ContentFilter::remove_outside_trim(&mut doc).expect("remove_outside_trim failed");

    let orig_page = original.get_pages()[&1];
    let out_page = doc.get_pages()[&1];

    // MediaBox must be unchanged
    let src_media = original.get_dictionary(orig_page).unwrap().get(b"MediaBox").unwrap();
    let out_media = doc.get_dictionary(out_page).unwrap().get(b"MediaBox").unwrap();
    assert_eq!(
        format!("{:?}", src_media),
        format!("{:?}", out_media),
        "MediaBox was changed — cropping is not allowed"
    );

    // TrimBox must be unchanged
    let src_trim = original.get_dictionary(orig_page).unwrap().get(b"TrimBox").unwrap();
    let out_trim = doc.get_dictionary(out_page).unwrap().get(b"TrimBox").unwrap();
    assert_eq!(
        format!("{:?}", src_trim),
        format!("{:?}", out_trim),
        "TrimBox was changed"
    );
}

#[test]
fn ptrim_pipeline_no_white_rect_coverups() {
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
                if ops[i].operands.len() == 1 {
                    let v = obj_to_f64(&ops[i].operands[0]);
                    fill_is_white = (v - 1.0).abs() < 1e-6;
                }
            }
            "rg" => {
                if ops[i].operands.len() == 3 {
                    let r = obj_to_f64(&ops[i].operands[0]);
                    let g = obj_to_f64(&ops[i].operands[1]);
                    let b = obj_to_f64(&ops[i].operands[2]);
                    fill_is_white =
                        (r - 1.0).abs() < 1e-6 && (g - 1.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6;
                }
            }
            "f" | "F" | "f*" => {
                if fill_is_white && i > 0 && ops[i - 1].operator == "re" {
                    panic!(
                        "Found white-filled rectangle at ops[{}..{}] — no white rect cover-ups allowed",
                        i - 1, i
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
fn ptrim_pipeline_keeps_inside_images() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }

    let mut doc = load_document(&fixture);
    ContentFilter::remove_outside_trim(&mut doc).expect("remove_outside_trim failed");

    let page_id = doc.get_pages()[&1];
    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let do_count = content.operations.iter().filter(|o| o.operator == "Do").count();
    assert!(do_count >= 1, "at least one image Do should survive filtering");
}

#[test]
#[allow(non_snake_case)]
fn ptrim_pipeline_q_Q_balanced() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }

    let mut doc = load_document(&fixture);
    ContentFilter::remove_outside_trim(&mut doc).expect("remove_outside_trim failed");

    let page_id = doc.get_pages()[&1];
    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let q = content.operations.iter().filter(|o| o.operator == "q").count();
    let big_q = content.operations.iter().filter(|o| o.operator == "Q").count();
    assert_eq!(q, big_q, "q and Q must be balanced in filtered output");
}

// ════════════════════════════════════════════════════════════════════
// Pipeline 2: prsz — Load → read boxes → expand bleed → save
// ════════════════════════════════════════════════════════════════════

#[test]
fn prsz_pipeline_trim_to_trimbox() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }

    let mut doc = load_document(&fixture);
    let pages = doc.get_pages();

    for (_, &page_id) in &pages {
        let boxes = PageBoxes::read(&doc, page_id).expect("read boxes");
        let target = boxes.trim_or_media();
        let arr = target.to_pdf_array();

        // Set MediaBox = CropBox = TrimBox
        let pdf_arr = lopdf::Object::Array(
            arr.iter().map(|&v| lopdf::Object::Real(v as f32)).collect(),
        );
        let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
        page.set("MediaBox", pdf_arr.clone());
        page.set("CropBox", pdf_arr);
    }

    let out = std::env::temp_dir().join("rustybara_prsz_trim.pdf");
    doc.save(&out).expect("save failed");

    // Verify MediaBox matches TrimBox
    let reloaded = load_document(&out);
    for (_, &page_id) in &reloaded.get_pages() {
        let page = reloaded.get_dictionary(page_id).unwrap();
        let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
        let media_vals: Vec<f64> = media.iter().map(obj_to_f64).collect();

        if let Ok(trim) = page.get(b"TrimBox") {
            let trim_vals: Vec<f64> = trim.as_array().unwrap().iter().map(obj_to_f64).collect();
            for i in 0..4 {
                assert!(
                    (media_vals[i] - trim_vals[i]).abs() < 0.5,
                    "MediaBox[{i}]={} != TrimBox[{i}]={}",
                    media_vals[i], trim_vals[i]
                );
            }
        }
    }
    std::fs::remove_file(&out).ok();
}

#[test]
fn prsz_pipeline_expand_bleed() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }

    let bleed_pts = 9.0; // 125 thousandths = 1/8"
    let original = load_document(&fixture);
    let mut doc = load_document(&fixture);
    let pages = doc.get_pages();

    for (page_num, &page_id) in &pages {
        let boxes = PageBoxes::read(&doc, page_id).expect("read boxes");
        let target = boxes.trim_or_media();
        let expanded = target.expand(bleed_pts);
        let arr = expanded.to_pdf_array();

        let pdf_arr = lopdf::Object::Array(
            arr.iter().map(|&v| lopdf::Object::Real(v as f32)).collect(),
        );
        let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
        page.set("MediaBox", pdf_arr.clone());
        page.set("CropBox", pdf_arr);

        // Verify dimensions: should be original trim + 2*bleed on each axis
        let orig_page_id = original.get_pages()[page_num];
        let orig_boxes = PageBoxes::read(&original, orig_page_id).expect("read orig boxes");
        let orig_trim = orig_boxes.trim_or_media();
        assert!(
            (expanded.width - (orig_trim.width + 2.0 * bleed_pts)).abs() < 0.001,
            "expanded width wrong"
        );
        assert!(
            (expanded.height - (orig_trim.height + 2.0 * bleed_pts)).abs() < 0.001,
            "expanded height wrong"
        );
    }

    let out = std::env::temp_dir().join("rustybara_prsz_bleed.pdf");
    doc.save(&out).expect("save failed");
    assert!(out.exists());
    std::fs::remove_file(&out).ok();
}

#[test]
fn prsz_pipeline_trimbox_preserved() {
    let fixture = fixture_path();
    if !fixture.exists() {
        return;
    }

    let original = load_document(&fixture);
    let mut doc = load_document(&fixture);

    for (_, &page_id) in &doc.get_pages() {
        let boxes = PageBoxes::read(&doc, page_id).expect("read boxes");
        let expanded = boxes.trim_or_media().expand(9.0);
        let arr = expanded.to_pdf_array();
        let pdf_arr = lopdf::Object::Array(
            arr.iter().map(|&v| lopdf::Object::Real(v as f32)).collect(),
        );
        let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
        page.set("MediaBox", pdf_arr.clone());
        page.set("CropBox", pdf_arr);
        // TrimBox NOT modified
    }

    // TrimBox should be unchanged
    let orig_page_id = original.get_pages()[&1];
    let out_page_id = doc.get_pages()[&1];
    let orig_trim = original.get_dictionary(orig_page_id).unwrap().get(b"TrimBox").unwrap();
    let out_trim = doc.get_dictionary(out_page_id).unwrap().get(b"TrimBox").unwrap();
    assert_eq!(
        format!("{:?}", orig_trim),
        format!("{:?}", out_trim),
        "TrimBox was modified"
    );
}

// ════════════════════════════════════════════════════════════════════
// Pipeline 3: p2i — Load → render page → encode
// (PDFium integration — requires native library, kept as #[ignore])
// ════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // requires PDFium native library
fn p2i_pipeline_render_and_encode() {
    // This test verifies the full render→encode pipeline.
    // Requires PDFium native library (pdfium.dll / libpdfium.so).
    //
    // When PDFium is available, remove #[ignore] and uncomment:
    //   let fixture = fixture_path();
    //   let pdfium = pdfium_render::prelude::Pdfium::default();
    //   let document = pdfium.load_pdf_from_file(&fixture, None).unwrap();
    //   let page = document.pages().get(0).unwrap();
    //   let config = rustybara::raster::RenderConfig::default();
    //   let image = rustybara::raster::render_page(&page, &config).unwrap();
    //   assert!(image.width() > 0 && image.height() > 0);
    //
    //   let out = std::env::temp_dir().join("rustybara_p2i_test.png");
    //   rustybara::encode::save(&image, &out, &rustybara::encode::OutputFormat::Png).unwrap();
    //   assert!(out.exists());
    //   std::fs::remove_file(&out).ok();
    todo!("requires PDFium native library setup");
}

// ════════════════════════════════════════════════════════════════════
// Batch tests — process multiple fixtures in sequence
// ════════════════════════════════════════════════════════════════════

#[test]
fn batch_ptrim_all_fixtures() {
    let batch_dir = batch_fixture_dir();
    if !batch_dir.exists() {
        eprintln!("Skipping: batch fixture dir not found at {}", batch_dir.display());
        return;
    }

    let pdfs: Vec<_> = std::fs::read_dir(&batch_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("pdf"))
        .collect();

    assert!(!pdfs.is_empty(), "no PDFs found in batch fixture dir");

    for entry in &pdfs {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let mut doc = match try_load_document(&path) {
            Some(d) => d,
            None => {
                eprintln!("Skipping unparseable PDF: {name}");
                continue;
            }
        };

        let pages = doc.get_pages();
        for (_, &page_id) in &pages {
            let boxes = PageBoxes::read(&doc, page_id).expect(&format!("{name}: read boxes"));
            let trim = boxes.trim_or_media();
            ContentFilter::filter_page(&mut doc, page_id, &trim)
                .expect(&format!("{name}: filter_page"));
        }

        // Verify balanced q/Q after filtering
        for (_, &page_id) in &doc.get_pages() {
            let content = doc.get_and_decode_page_content(page_id).unwrap();
            let q = content.operations.iter().filter(|o| o.operator == "q").count();
            let big_q = content.operations.iter().filter(|o| o.operator == "Q").count();
            assert_eq!(q, big_q, "{name}: q/Q imbalanced after filtering");
        }

        // Save and verify round-trip
        let out = std::env::temp_dir().join(format!("rustybara_batch_{name}"));
        doc.save(&out).expect(&format!("{name}: save failed"));
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        std::fs::remove_file(&out).ok();
    }
}

#[test]
fn batch_prsz_all_fixtures() {
    let batch_dir = batch_fixture_dir();
    if !batch_dir.exists() {
        return;
    }

    let pdfs: Vec<_> = std::fs::read_dir(&batch_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("pdf"))
        .collect();

    let bleed_pts = 9.0;
    for entry in &pdfs {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let mut doc = match try_load_document(&path) {
            Some(d) => d,
            None => {
                eprintln!("Skipping unparseable PDF: {name}");
                continue;
            }
        };

        for (_, &page_id) in &doc.get_pages() {
            let boxes = PageBoxes::read(&doc, page_id).expect(&format!("{name}: read boxes"));
            let target = boxes.trim_or_media();
            let expanded = target.expand(bleed_pts);
            let arr = expanded.to_pdf_array();

            assert!(
                expanded.width > target.width,
                "{name}: expanded width should be larger"
            );

            let pdf_arr = lopdf::Object::Array(
                arr.iter().map(|&v| lopdf::Object::Real(v as f32)).collect(),
            );
            let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
            page.set("MediaBox", pdf_arr.clone());
            page.set("CropBox", pdf_arr);
        }

        let out = std::env::temp_dir().join(format!("rustybara_batch_rsz_{name}"));
        doc.save(&out).expect(&format!("{name}: save failed"));
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        std::fs::remove_file(&out).ok();
    }
}

// ════════════════════════════════════════════════════════════════════
// Secret fixture tests — production PDFs
// ════════════════════════════════════════════════════════════════════

#[test]
fn secret_fixtures_ptrim_pipeline() {
    let dir = secret_fixture_dir();
    if !dir.exists() {
        eprintln!("Skipping: secret fixtures not found at {}", dir.display());
        return;
    }

    let pdfs: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("pdf"))
        .collect();

    for entry in &pdfs {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let mut doc = load_document(&path);

        // Record pre-filter q/Q balance per page
        let pages = doc.get_pages();
        let mut pre_balance: std::collections::HashMap<lopdf::ObjectId, (usize, usize)> =
            std::collections::HashMap::new();
        for (_, &page_id) in &pages {
            if let Ok(content) = doc.get_and_decode_page_content(page_id) {
                let q = content.operations.iter().filter(|o| o.operator == "q").count();
                let big_q = content.operations.iter().filter(|o| o.operator == "Q").count();
                pre_balance.insert(page_id, (q, big_q));
            }
        }

        for (_, &page_id) in &pages {
            let boxes = PageBoxes::read(&doc, page_id).expect(&format!("{name}: read boxes"));
            let trim = boxes.trim_or_media();
            assert!(trim.width > 0.0 && trim.height > 0.0, "{name}: invalid trim box");

            ContentFilter::filter_page(&mut doc, page_id, &trim)
                .expect(&format!("{name}: filter_page"));
        }

        // Verify q/Q balance is not worsened by filtering
        for (_, &page_id) in &doc.get_pages() {
            let content = doc.get_and_decode_page_content(page_id).unwrap();
            let q = content.operations.iter().filter(|o| o.operator == "q").count();
            let big_q = content.operations.iter().filter(|o| o.operator == "Q").count();
            let (pre_q, pre_bq) = pre_balance.get(&page_id).copied().unwrap_or((0, 0));
            let pre_diff = (pre_q as i64 - pre_bq as i64).abs();
            let post_diff = (q as i64 - big_q as i64).abs();
            assert!(
                post_diff <= pre_diff,
                "{name}: q/Q imbalance worsened by filter (pre: {pre_q}/{pre_bq}, post: {q}/{big_q})"
            );
        }

        let out = std::env::temp_dir().join(format!("rustybara_secret_{}", name.replace(' ', "_")));
        doc.save(&out).expect(&format!("{name}: save failed"));
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        std::fs::remove_file(&out).ok();
    }
}

#[test]
fn secret_fixtures_prsz_pipeline() {
    let dir = secret_fixture_dir();
    if !dir.exists() {
        return;
    }

    let pdfs: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("pdf"))
        .collect();

    let bleed_pts = 9.0;
    for entry in &pdfs {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy();
        let mut doc = load_document(&path);

        for (_, &page_id) in &doc.get_pages() {
            let boxes = PageBoxes::read(&doc, page_id).expect(&format!("{name}: read boxes"));
            let trim = boxes.trim_or_media();
            let expanded = trim.expand(bleed_pts);

            assert!(
                (expanded.width - (trim.width + 18.0)).abs() < 0.001,
                "{name}: expanded width wrong"
            );

            let arr = expanded.to_pdf_array();
            let pdf_arr = lopdf::Object::Array(
                arr.iter().map(|&v| lopdf::Object::Real(v as f32)).collect(),
            );
            let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
            page.set("MediaBox", pdf_arr.clone());
            page.set("CropBox", pdf_arr);
        }

        let out = std::env::temp_dir().join(format!("rustybara_secret_rsz_{}", name.replace(' ', "_")));
        doc.save(&out).expect(&format!("{name}: save failed"));
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        std::fs::remove_file(&out).ok();
    }
}
