use rustybara::raster::RenderConfig;

// ── RenderConfig tests ──────────────────────────────────────────────
// Ported from: pdf-2-image (config construction and DPI math)

#[test]
fn render_config_default_150dpi() {
    let cfg = RenderConfig::default();
    assert_eq!(cfg.dpi, 150);
    assert!(cfg.render_annotations);
    assert!(cfg.render_form_data);
}

#[test]
fn render_config_prepress_300dpi() {
    let cfg = RenderConfig::prepress();
    assert_eq!(cfg.dpi, 300);
    assert!(cfg.render_annotations);
    assert!(cfg.render_form_data);
}

// ── DPI pixel math ──────────────────────────────────────────────────
// Ported from: pdf-2-image
// These verify the formula used in CpuRenderer: (pts / 72.0 * dpi) as i32

#[test]
fn dpi_pixel_math_150() {
    // US Letter: 612 × 792 pt at 150 DPI → 1275 × 1650 px
    let dpi: f32 = 150.0;
    let width_pts: f32 = 612.0;
    let height_pts: f32 = 792.0;

    let px_w = (width_pts / 72.0 * dpi) as i32;
    let px_h = (height_pts / 72.0 * dpi) as i32;

    assert_eq!(px_w, 1275);
    assert_eq!(px_h, 1650);
}

#[test]
fn dpi_pixel_math_300() {
    // US Letter: 612 × 792 pt at 300 DPI → 2550 × 3300 px
    let dpi: f32 = 300.0;
    let width_pts: f32 = 612.0;
    let height_pts: f32 = 792.0;

    let px_w = (width_pts / 72.0 * dpi) as i32;
    let px_h = (height_pts / 72.0 * dpi) as i32;

    assert_eq!(px_w, 2550);
    assert_eq!(px_h, 3300);
}

#[test]
fn dpi_pixel_math_72() {
    // At 72 DPI, pixel dimensions equal point dimensions
    let dpi: f32 = 72.0;
    let width_pts: f32 = 612.0;
    let height_pts: f32 = 792.0;

    let px_w = (width_pts / 72.0 * dpi) as i32;
    let px_h = (height_pts / 72.0 * dpi) as i32;

    assert_eq!(px_w, 612);
    assert_eq!(px_h, 792);
}

// ── render_page integration tests ───────────────────────────────────
// Requires PDFium native library on system.
// Ported from: pdf-2-image (process_pdf test, adapted)

#[test]
#[ignore] // requires PDFium native library
fn render_page_produces_image() {
    // This test requires:
    //   1. PDFium native library available on the system
    //   2. pdf_test_data_print_v2.pdf in tests/fixtures/
    //
    // When both are available, remove #[ignore] to run.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdf_test_data_print_v2.pdf");
    if !fixture.exists() {
        eprintln!("Skipping: fixture not found at {}", fixture.display());
        return;
    }

    // PDFium integration would go here:
    //   let pdfium = Pdfium::default();
    //   let doc = pdfium.load_pdf_from_file(&fixture, None).unwrap();
    //   let page = doc.pages().get(0).unwrap();
    //   let config = RenderConfig::default();
    //   let img = rustybara::raster::render_page(&page, &config).unwrap();
    //   assert!(img.width() > 0);
    //   assert!(img.height() > 0);
    todo!("requires PDFium native library setup");
}
