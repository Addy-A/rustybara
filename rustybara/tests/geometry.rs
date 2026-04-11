use rustybara::geometry::{Matrix, Rect};

// ── Rect unit tests ─────────────────────────────────────────────────
// Ported from: pdf-2-image, pdf-mark-removal, pdf-trim-or-bleed-resizer

#[test]
fn rect_from_corners_normalizes() {
    // Reversed corners should produce same result
    let r = Rect::from_corners(642.0, 822.0, 30.0, 30.0);
    assert!((r.x - 30.0).abs() < 1e-10);
    assert!((r.y - 30.0).abs() < 1e-10);
    assert!((r.width - 612.0).abs() < 1e-10);
    assert!((r.height - 792.0).abs() < 1e-10);
}

#[test]
fn rect_from_corners_normalises_order() {
    // Passing corners in reversed order should still produce the same rect
    let a = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    let b = Rect::from_corners(642.0, 822.0, 30.0, 30.0);
    assert_eq!(a.x, b.x);
    assert_eq!(a.y, b.y);
    assert_eq!(a.width, b.width);
    assert_eq!(a.height, b.height);
}

#[test]
fn rect_right_and_top() {
    let r = Rect::new(30.0, 30.0, 612.0, 792.0);
    assert!((r.right() - 642.0).abs() < 1e-10);
    assert!((r.top() - 822.0).abs() < 1e-10);
}

#[test]
fn rect_is_outside_all_directions() {
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);

    // Entirely to the right
    assert!(Rect::new(650.0, 100.0, 10.0, 10.0).is_outside(&trim));
    // Entirely to the left
    assert!(Rect::new(0.0, 100.0, 10.0, 10.0).is_outside(&trim));
    // Entirely above
    assert!(Rect::new(100.0, 830.0, 10.0, 10.0).is_outside(&trim));
    // Entirely below
    assert!(Rect::new(100.0, 0.0, 10.0, 10.0).is_outside(&trim));
    // Straddling from inside -- must NOT be outside
    assert!(!Rect::new(635.0, 100.0, 20.0, 10.0).is_outside(&trim));
}

#[test]
fn rect_straddling_is_not_outside() {
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    assert!(!Rect::new(635.0, 100.0, 20.0, 10.0).is_outside(&trim));
}

#[test]
fn rect_fully_inside_is_not_outside() {
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    assert!(!Rect::new(100.0, 100.0, 50.0, 50.0).is_outside(&trim));
}

#[test]
fn rect_expand_zero_is_identity() {
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    let same = trim.expand(0.0);
    assert_eq!(same.x, trim.x);
    assert_eq!(same.y, trim.y);
    assert_eq!(same.width, trim.width);
    assert_eq!(same.height, trim.height);
}

#[test]
fn rect_expand_positive_bleed() {
    // 125 thousandths of an inch = 0.125" = 9.0 points (0.125 * 72)
    let bleed_pts = 9.0;
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    let expanded = trim.expand(bleed_pts);

    assert_eq!(expanded.x, 21.0); // 30 - 9
    assert_eq!(expanded.y, 21.0); // 30 - 9
    assert!((expanded.right() - 651.0).abs() < 0.001); // 642 + 9
    assert!((expanded.top() - 831.0).abs() < 0.001); // 822 + 9
    assert!((expanded.width - 630.0).abs() < 0.001); // 612 + 18
    assert!((expanded.height - 810.0).abs() < 0.001); // 792 + 18
}

#[test]
fn test_to_pdf_array() {
    let r = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    let arr = r.to_pdf_array();
    assert_eq!(arr.len(), 4);
    assert!((arr[0] - 30.0).abs() < 0.01);
    assert!((arr[1] - 30.0).abs() < 0.01);
    assert!((arr[2] - 642.0).abs() < 0.01);
    assert!((arr[3] - 822.0).abs() < 0.01);
}

#[test]
fn test_to_pdf_array_matches_expand() {
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
    let bleed_pts = 9.0;
    let expanded = trim.expand(bleed_pts);
    let arr = expanded.to_pdf_array();
    assert!((arr[0] - 21.0).abs() < 0.01); // 30 - 9
    assert!((arr[1] - 21.0).abs() < 0.01);
    assert!((arr[2] - 651.0).abs() < 0.01); // 642 + 9
    assert!((arr[3] - 831.0).abs() < 0.01); // 822 + 9
}

// ── Matrix unit tests ───────────────────────────────────────────────
// Ported from: pdf-2-image, pdf-mark-removal

#[test]
fn matrix_identity_leaves_point_unchanged() {
    let m = Matrix::identity();
    let (x, y) = m.transform_point(100.0, 200.0);
    assert!((x - 100.0).abs() < 1e-10);
    assert!((y - 200.0).abs() < 1e-10);
}

#[test]
fn matrix_translation_moves_point() {
    // a=1 b=0 c=0 d=1 e=50 f=75 -> pure translation by (50, 75)
    let m = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 50.0, 75.0);
    let (x, y) = m.transform_point(10.0, 20.0);
    assert!((x - 60.0).abs() < 1e-10);
    assert!((y - 95.0).abs() < 1e-10);
}

#[test]
fn matrix_concat_translations_add() {
    // Two pure translations should add together
    let t1 = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 10.0, 20.0);
    let t2 = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 5.0, 3.0);
    let combined = t1.concat(&t2);
    let (x, y) = combined.transform_point(0.0, 0.0);
    assert!((x - 15.0).abs() < 1e-10, "expected x=15, got {x}");
    assert!((y - 23.0).abs() < 1e-10, "expected y=23, got {y}");
}

#[test]
fn matrix_transform_rect_identity() {
    let m = Matrix::identity();
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    let result = m.transform_rect(&r);
    assert!((result.x - 10.0).abs() < 1e-10);
    assert!((result.y - 20.0).abs() < 1e-10);
    assert!((result.width - 100.0).abs() < 1e-10);
    assert!((result.height - 50.0).abs() < 1e-10);
}

#[test]
fn matrix_known_ctm_places_red_rect_outside_trim() {
    // This is the actual CTM from the source PDF's PlacedPDF block.
    // The red rectangle at local coords (298.292, -312.455, 7.879, 60.394)
    // should land outside TrimBox right edge (642) after transformation.
    let ctm = Matrix::from_values(1.02883, 0.0, 0.0, -1.03942, 336.0, 426.0);
    let red_rect = Rect::new(298.292, -312.455, 7.879, 60.394);
    let in_page_space = ctm.transform_rect(&red_rect);
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);

    assert!(
        in_page_space.is_outside(&trim),
        "red rect left edge {:.4} should be outside trim right 642",
        in_page_space.x
    );
}

#[test]
fn matrix_known_ctm_keeps_blue_rect_inside_trim() {
    // The blue square at local (296.95, -205.476, 9.222, 7.853)
    // should land inside TrimBox -- left edge ~641.51 < 642.
    let ctm = Matrix::from_values(1.02883, 0.0, 0.0, -1.03942, 336.0, 426.0);
    let blue_rect = Rect::new(296.95, -205.476, 9.222, 7.853);
    let in_page_space = ctm.transform_rect(&blue_rect);
    let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);

    assert!(
        !in_page_space.is_outside(&trim),
        "blue rect left edge {:.4} should be inside trim right 642",
        in_page_space.x
    );
}

// ── Unit conversion ─────────────────────────────────────────────────
// Ported from: pdf-trim-or-bleed-resizer

#[test]
fn thousandths_to_points_conversion() {
    // The conversion formula: thousandths / 1000.0 * 72.0
    let convert = |thousandths: u32| thousandths as f64 / 1000.0 * 72.0;

    assert!((convert(125) - 9.0).abs() < 0.001); // 1/8"
    assert!((convert(250) - 18.0).abs() < 0.001); // 1/4"
    assert!((convert(500) - 36.0).abs() < 0.001); // 1/2"
    assert!((convert(1000) - 72.0).abs() < 0.001); // 1"
}
