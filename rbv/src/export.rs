//! Wireframe PDF export — diagnostic tool for coordinate accuracy debugging.
//!
//! Exports the current page's object tree as a minimal hand-crafted PDF where every
//! path is drawn in **page space** (post-CTM coordinates).  The exported file can
//! then be normalized with `qpdf --qdf` and cross-referenced against the original
//! document to identify Stage-1 (CTM application) vs Stage-2 (screen projection)
//! coordinate bugs.
//!
//! ## Usage
//! Press `Ctrl+Shift+E` in the viewer.  The output path is printed in the debug log.
//!
//! ## PDF structure
//! The file uses the minimal 4-object layout required by the spec:
//! 1. Catalog → Points to Pages
//! 2. Pages   → Contains one Page
//! 3. Page    → MediaBox set to the document's MediaBox; references the content stream
//! 4. Content stream → All objects as thin black outlines using `m/l/c/h/re/S` operators
//!
//! No new crate dependencies are required; the PDF bytes are assembled from scratch.

use rustybara::{
    geometry::Rect as PdfRect,
    objects::{ObjectKind, ObjectTree, PathPoint},
};
use std::{
    io::{self, Write},
    path::Path,
};

// ── Public API ────────────────────────────────────────────────────────────────

/// Export the full page wireframe as a diagnostic PDF.
///
/// Each [`rustybara::objects::PageObject`] is drawn as a thin black outline in
/// page space (post-CTM).  Images and objects with no subpath data are rendered
/// as their bounding-box rectangle (with diagonal cross for images).
///
/// # Errors
/// Returns `io::Error` if the file cannot be created or written.
pub fn export_wireframe(
    tree: &ObjectTree,
    media_box: &PdfRect,
    output_path: &Path,
) -> io::Result<()> {
    // ── Build content stream ──────────────────────────────────────────────────
    let mut content = String::new();

    // Stroke color: black; line width: 0.5 pt.
    content.push_str("0 G\n");    // set stroke color (DeviceGray 0 = black)
    content.push_str("0.5 w\n");  // line width

    for obj in &tree.objects {
        match &obj.kind {
            ObjectKind::Fill | ObjectKind::Stroke | ObjectKind::FillStroke => {
                if obj.subpaths.is_empty() {
                    // No path data — emit the bbox as a rectangle.
                    append_rect(&mut content, &obj.bbox);
                    content.push_str("S\n");
                    continue;
                }

                // Emit path operators; apply CTM to every point.
                for sub in &obj.subpaths {
                    for &pt in &sub.points {
                        match pt {
                            PathPoint::MoveTo(lx, ly) => {
                                let (px, py) = obj.ctm.transform_point(lx, ly);
                                content.push_str(&format!("{:.4} {:.4} m\n", px, py));
                            }
                            PathPoint::LineTo(lx, ly) => {
                                let (px, py) = obj.ctm.transform_point(lx, ly);
                                content.push_str(&format!("{:.4} {:.4} l\n", px, py));
                            }
                            PathPoint::CurveTo(c1x, c1y, c2x, c2y, ex, ey) => {
                                let (s1x, s1y) = obj.ctm.transform_point(c1x, c1y);
                                let (s2x, s2y) = obj.ctm.transform_point(c2x, c2y);
                                let (sex, sey) = obj.ctm.transform_point(ex, ey);
                                content.push_str(&format!(
                                    "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c\n",
                                    s1x, s1y, s2x, s2y, sex, sey
                                ));
                            }
                            PathPoint::Close => {
                                content.push_str("h\n");
                            }
                        }
                    }
                }
                // Stroke only — we want wireframe outlines, not filled shapes.
                content.push_str("S\n");
            }

            ObjectKind::Image => {
                // Acrobat-style image placeholder: bounding rect + X diagonals.
                let b = &obj.bbox;
                let x0 = b.x;
                let y0 = b.y;
                let x1 = b.x + b.width;
                let y1 = b.y + b.height;

                // Bounding rectangle.
                append_rect(&mut content, b);
                // Diagonal top-left → bottom-right.
                content.push_str(&format!("{:.4} {:.4} m\n", x0, y1));
                content.push_str(&format!("{:.4} {:.4} l\n", x1, y0));
                // Diagonal top-right → bottom-left.
                content.push_str(&format!("{:.4} {:.4} m\n", x1, y1));
                content.push_str(&format!("{:.4} {:.4} l\n", x0, y0));
                content.push_str("S\n");
            }

            ObjectKind::Text(_) | ObjectKind::FormXObject => {
                // Text and form XObjects: draw the bounding box only.
                append_rect(&mut content, &obj.bbox);
                content.push_str("S\n");
            }
        }
    }

    // ── Assemble PDF bytes ────────────────────────────────────────────────────
    let content_bytes = content.as_bytes();
    let content_len = content_bytes.len();

    let mut buf: Vec<u8> = Vec::with_capacity(content_len + 512);

    // Header.
    buf.extend_from_slice(b"%PDF-1.4\n");
    buf.extend_from_slice(b"% rbv wireframe diagnostic export\n");

    // Object 1 — Catalog.
    let off1 = buf.len();
    buf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Object 2 — Pages.
    let off2 = buf.len();
    buf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // Object 3 — Page.
    let off3 = buf.len();
    let media_str = format!(
        "[{:.4} {:.4} {:.4} {:.4}]",
        media_box.x,
        media_box.y,
        media_box.x + media_box.width,
        media_box.y + media_box.height
    );
    let page_obj = format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox {} /Contents 4 0 R /Resources << >> >>\nendobj\n",
        media_str
    );
    buf.extend_from_slice(page_obj.as_bytes());

    // Object 4 — Content stream.
    let off4 = buf.len();
    let stream_header = format!(
        "4 0 obj\n<< /Length {} >>\nstream\n",
        content_len
    );
    buf.extend_from_slice(stream_header.as_bytes());
    buf.extend_from_slice(content_bytes);
    buf.extend_from_slice(b"\nendstream\nendobj\n");

    // Cross-reference table.
    let xref_offset = buf.len();
    buf.extend_from_slice(b"xref\n");
    buf.extend_from_slice(b"0 5\n");
    buf.extend_from_slice(b"0000000000 65535 f \n"); // free entry 0
    buf.extend_from_slice(format!("{:010} 00000 n \n", off1).as_bytes());
    buf.extend_from_slice(format!("{:010} 00000 n \n", off2).as_bytes());
    buf.extend_from_slice(format!("{:010} 00000 n \n", off3).as_bytes());
    buf.extend_from_slice(format!("{:010} 00000 n \n", off4).as_bytes());

    // Trailer.
    let trailer = format!(
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        xref_offset
    );
    buf.extend_from_slice(trailer.as_bytes());

    // ── Write file ────────────────────────────────────────────────────────────
    let mut file = std::fs::File::create(output_path)?;
    file.write_all(&buf)?;
    file.flush()?;

    Ok(())
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Append a PDF `re` (rectangle) operator for the given bbox, *without* a paint
/// operator so that the caller can chain additional path segments before stroking.
fn append_rect(content: &mut String, r: &PdfRect) {
    content.push_str(&format!(
        "{:.4} {:.4} {:.4} {:.4} re\n",
        r.x, r.y, r.width, r.height
    ));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::export_wireframe;
    use rustybara::geometry::{Matrix, Rect as PdfRect};
    use rustybara::objects::{ObjectKind, ObjectTree, PageObject, PathPoint, PdfColor, SubPath};
    use std::path::Path;
    use tempfile::NamedTempFile;

    fn identity() -> Matrix {
        Matrix {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn media() -> PdfRect {
        PdfRect::new(0.0, 0.0, 612.0, 792.0)
    }

    fn make_tree(objects: Vec<PageObject>) -> ObjectTree {
        ObjectTree { objects }
    }

    fn make_fill_obj(x: f64, y: f64, w: f64, h: f64) -> PageObject {
        // A simple rectangle subpath.
        PageObject {
            bbox: PdfRect::new(x, y, w, h),
            ctm: identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::DeviceCmyk(0.0, 0.0, 0.0, 1.0)),
            stroke_color: None,
            stroke_width: 1.0,
            subpaths: vec![SubPath {
                points: vec![
                    PathPoint::MoveTo(x, y),
                    PathPoint::LineTo(x + w, y),
                    PathPoint::LineTo(x + w, y + h),
                    PathPoint::LineTo(x, y + h),
                    PathPoint::Close,
                ],
            }],
        }
    }

    fn make_image_obj(x: f64, y: f64, w: f64, h: f64) -> PageObject {
        PageObject {
            bbox: PdfRect::new(x, y, w, h),
            ctm: identity(),
            kind: ObjectKind::Image,
            fill_color: None,
            stroke_color: None,
            stroke_width: 1.0,
            subpaths: vec![],
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn read_pdf_content(path: &Path) -> String {
        String::from_utf8(std::fs::read(path).unwrap()).unwrap()
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// Empty tree → valid minimal PDF (no panic, correct header/trailer).
    #[test]
    fn empty_tree_produces_valid_pdf() {
        let tmp = NamedTempFile::new().unwrap();
        let tree = make_tree(vec![]);
        export_wireframe(&tree, &media(), tmp.path()).unwrap();

        let content = read_pdf_content(tmp.path());
        assert!(content.starts_with("%PDF-1.4"), "missing PDF header");
        assert!(content.contains("%%EOF"), "missing EOF marker");
        assert!(content.contains("/Type /Catalog"), "missing Catalog");
        assert!(content.contains("/Type /Pages"), "missing Pages");
        assert!(content.contains("/Type /Page"), "missing Page");
        assert!(content.contains("startxref"), "missing startxref");
    }

    /// MediaBox in the exported Page object matches the input media_box.
    #[test]
    fn media_box_is_written_correctly() {
        let tmp = NamedTempFile::new().unwrap();
        let tree = make_tree(vec![]);
        // Use a non-zero origin to check all four values.
        let mb = PdfRect::new(10.0, 20.0, 400.0, 500.0);
        export_wireframe(&tree, &mb, tmp.path()).unwrap();

        let content = read_pdf_content(tmp.path());
        // MediaBox should be [x, y, x+w, y+h] = [10, 20, 410, 520]
        assert!(
            content.contains("[10.0000 20.0000 410.0000 520.0000]"),
            "MediaBox mismatch:\n{}",
            &content[content.find("/MediaBox").unwrap_or(0)..]
                .chars()
                .take(80)
                .collect::<String>()
        );
    }

    /// Fill object path operators appear in the content stream.
    #[test]
    fn fill_object_emits_path_operators() {
        let tmp = NamedTempFile::new().unwrap();
        let tree = make_tree(vec![make_fill_obj(10.0, 20.0, 100.0, 200.0)]);
        export_wireframe(&tree, &media(), tmp.path()).unwrap();

        let content = read_pdf_content(tmp.path());
        // Expect m (moveto) and l (lineto) and h (close) and S (stroke) operators
        assert!(content.contains(" m\n"), "missing moveto");
        assert!(content.contains(" l\n"), "missing lineto");
        assert!(content.contains("h\n"), "missing closepath");
        assert!(content.contains("S\n"), "missing stroke");
    }

    /// Image object emits bbox rect + diagonals before S.
    #[test]
    fn image_object_emits_rect_and_diagonals() {
        let tmp = NamedTempFile::new().unwrap();
        let tree = make_tree(vec![make_image_obj(50.0, 60.0, 100.0, 80.0)]);
        export_wireframe(&tree, &media(), tmp.path()).unwrap();

        let content = read_pdf_content(tmp.path());
        // `re` operator for the bounding rect
        assert!(content.contains(" re\n"), "missing rect (re) operator");
        // Two diagonal lines (m ... l pairs)
        let m_count = content.matches(" m\n").count();
        assert!(m_count >= 2, "expected at least 2 moveto for diagonals, got {m_count}");
        assert!(content.contains("S\n"), "missing stroke");
    }

    /// Curve-to in a subpath emits the `c` operator with 6 coordinates.
    #[test]
    fn curveto_emits_c_operator() {
        let tmp = NamedTempFile::new().unwrap();
        let curve_obj = PageObject {
            bbox: PdfRect::new(0.0, 0.0, 100.0, 100.0),
            ctm: identity(),
            kind: ObjectKind::Fill,
            fill_color: None,
            stroke_color: None,
            stroke_width: 1.0,
            subpaths: vec![SubPath {
                points: vec![
                    PathPoint::MoveTo(0.0, 0.0),
                    PathPoint::CurveTo(10.0, 50.0, 90.0, 50.0, 100.0, 0.0),
                    PathPoint::Close,
                ],
            }],
        };
        let tree = make_tree(vec![curve_obj]);
        export_wireframe(&tree, &media(), tmp.path()).unwrap();

        let content = read_pdf_content(tmp.path());
        assert!(content.contains(" c\n"), "missing bezier curve (c) operator");
    }

    /// CTM translation is applied to path points before writing.
    #[test]
    fn ctm_translation_applied_to_path() {
        let tmp = NamedTempFile::new().unwrap();
        // CTM translates by (100, 200)
        let ctm = Matrix {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 100.0,
            f: 200.0,
        };
        let obj = PageObject {
            bbox: PdfRect::new(100.0, 200.0, 50.0, 50.0),
            ctm,
            kind: ObjectKind::Stroke,
            fill_color: None,
            stroke_color: Some(PdfColor::DeviceGray(0.0)),
            stroke_width: 1.0,
            subpaths: vec![SubPath {
                points: vec![
                    PathPoint::MoveTo(0.0, 0.0), // post-CTM: (100, 200)
                    PathPoint::LineTo(50.0, 0.0), // post-CTM: (150, 200)
                ],
            }],
        };
        let tree = make_tree(vec![obj]);
        export_wireframe(&tree, &media(), tmp.path()).unwrap();

        let content = read_pdf_content(tmp.path());
        // The moveto should be at (100, 200) after CTM application.
        assert!(
            content.contains("100.0000 200.0000 m"),
            "CTM translation not applied: expected '100.0000 200.0000 m' in:\n{}",
            content
        );
    }

    /// No-subpath fill object falls back to bbox rect.
    #[test]
    fn no_subpath_falls_back_to_bbox_rect() {
        let tmp = NamedTempFile::new().unwrap();
        let obj = PageObject {
            bbox: PdfRect::new(5.0, 10.0, 200.0, 300.0),
            ctm: identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::DeviceGray(0.5)),
            stroke_color: None,
            stroke_width: 1.0,
            subpaths: vec![], // no subpath data
        };
        let tree = make_tree(vec![obj]);
        export_wireframe(&tree, &media(), tmp.path()).unwrap();

        let content = read_pdf_content(tmp.path());
        // Should emit a `re` rect for the bbox.
        assert!(content.contains("5.0000 10.0000 200.0000 300.0000 re"), "missing bbox rect fallback");
    }

    /// xref table offsets point to valid object positions.
    ///
    /// This is a structural sanity check: each xref entry's byte offset must
    /// correspond to the start of an "N 0 obj" line in the file.
    #[test]
    fn xref_offsets_are_valid() {
        let tmp = NamedTempFile::new().unwrap();
        let tree = make_tree(vec![make_fill_obj(0.0, 0.0, 100.0, 100.0)]);
        export_wireframe(&tree, &media(), tmp.path()).unwrap();

        let bytes = std::fs::read(tmp.path()).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();

        // Extract startxref value.
        let sxref_pos = text.find("startxref\n").unwrap() + "startxref\n".len();
        let sxref_end = text[sxref_pos..].find('\n').unwrap() + sxref_pos;
        let xref_offset: usize = text[sxref_pos..sxref_end].trim().parse().unwrap();

        // The xref keyword must appear at that offset.
        assert_eq!(&text[xref_offset..xref_offset + 4], "xref", "startxref points to wrong offset");

        // Parse the four n-entries and verify each byte offset starts "N 0 obj".
        let xref_section = &text[xref_offset..];
        for obj_num in 1u32..=4 {
            let expected_marker = format!("{} 0 obj", obj_num);
            // Find the xref entry line for this object.
            // Format: "XXXXXXXXXX 00000 n \n"
            let entry_start = xref_section
                .lines()
                .skip(2 + obj_num as usize) // skip "xref\n" + "0 5\n" + free entry + prior entries
                .next()
                .unwrap_or("");
            let offset: usize = entry_start[..10].trim().parse().unwrap_or(usize::MAX);
            let end = (offset + 20).min(bytes.len());
            let slice = std::str::from_utf8(&bytes[offset..end]).unwrap_or("");
            assert!(
                slice.starts_with(&expected_marker),
                "obj {} offset {} → {:?}",
                obj_num, offset, slice
            );
        }
    }
}