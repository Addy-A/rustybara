//! Serialize [`PositionedGlyph`] outlines to PDF path operator sequences.
//!
//! Useful for Deliverable B: writing an "Outline Text" version of a PDF where
//! all glyphs have been replaced with `m`/`l`/`c`/`h`/`f` path operators.
//! Quadratic béziers are elevated to cubics since PDF has no `q` operator.

use super::paths::{GlyphVerb, PositionedGlyph};

/// Convert `glyphs` to a self-contained PDF content stream fragment.
///
/// Each glyph becomes a filled path (`f`) wrapped in a `q ... Q` so the graphics
/// state is fully isolated. Coordinates are in **PDF page space** (the same space
/// the glyphs were extracted in), so this fragment is correct only when the output
/// PDF's CTM is identity at the insertion point.
pub fn glyphs_to_content_stream(glyphs: &[PositionedGlyph]) -> String {
    let mut buf = String::with_capacity(glyphs.len() * 64);
    buf.push_str("q\n");

    for glyph in glyphs {
        if glyph.verbs.is_empty() {
            continue;
        }
        write_glyph_path(&mut buf, &glyph.verbs);
        buf.push_str("f\n");
    }

    buf.push_str("Q\n");
    buf
}

fn write_glyph_path(buf: &mut String, verbs: &[GlyphVerb]) {
    let mut cur = (0.0_f64, 0.0_f64);

    for verb in verbs {
        match *verb {
            GlyphVerb::MoveTo(x, y) => {
                buf.push_str(&format!("{x:.4} {y:.4} m\n"));
                cur = (x, y);
            }
            GlyphVerb::LineTo(x, y) => {
                buf.push_str(&format!("{x:.4} {y:.4} l\n"));
                cur = (x, y);
            }
            GlyphVerb::QuadTo(cx, cy, x, y) => {
                // Degree elevation: quadratic → cubic
                let cp1x = cur.0 + 2.0 / 3.0 * (cx - cur.0);
                let cp1y = cur.1 + 2.0 / 3.0 * (cy - cur.1);
                let cp2x = x + 2.0 / 3.0 * (cx - x);
                let cp2y = y + 2.0 / 3.0 * (cy - y);
                buf.push_str(&format!(
                    "{cp1x:.4} {cp1y:.4} {cp2x:.4} {cp2y:.4} {x:.4} {y:.4} c\n"
                ));
                cur = (x, y);
            }
            GlyphVerb::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                buf.push_str(&format!(
                    "{c1x:.4} {c1y:.4} {c2x:.4} {c2y:.4} {x:.4} {y:.4} c\n"
                ));
                cur = (x, y);
            }
            GlyphVerb::Close => {
                buf.push_str("h\n");
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_glyph(verbs: Vec<GlyphVerb>) -> PositionedGlyph {
        PositionedGlyph { verbs }
    }

    // ── glyphs_to_content_stream ──────────────────────────────────────────────

    #[test]
    fn empty_input_produces_save_restore_only() {
        let out = glyphs_to_content_stream(&[]);
        assert_eq!(out, "q\nQ\n");
    }

    #[test]
    fn empty_verb_list_is_skipped() {
        let glyphs = vec![make_glyph(vec![])];
        let out = glyphs_to_content_stream(&glyphs);
        // An empty glyph should produce nothing between q/Q.
        assert_eq!(out, "q\nQ\n");
    }

    #[test]
    fn output_starts_with_q_and_ends_with_capital_q() {
        let glyphs = vec![make_glyph(vec![GlyphVerb::MoveTo(0.0, 0.0), GlyphVerb::Close])];
        let out = glyphs_to_content_stream(&glyphs);
        assert!(out.starts_with("q\n"));
        assert!(out.ends_with("Q\n"));
    }

    #[test]
    fn each_non_empty_glyph_gets_fill_operator() {
        let glyphs = vec![
            make_glyph(vec![GlyphVerb::MoveTo(0.0, 0.0), GlyphVerb::Close]),
            make_glyph(vec![GlyphVerb::MoveTo(1.0, 1.0), GlyphVerb::Close]),
        ];
        let out = glyphs_to_content_stream(&glyphs);
        assert_eq!(out.matches("f\n").count(), 2);
    }

    #[test]
    fn empty_glyph_does_not_emit_fill() {
        let glyphs = vec![
            make_glyph(vec![]),
            make_glyph(vec![GlyphVerb::MoveTo(0.0, 0.0), GlyphVerb::Close]),
        ];
        let out = glyphs_to_content_stream(&glyphs);
        assert_eq!(out.matches("f\n").count(), 1);
    }

    // ── write_glyph_path ──────────────────────────────────────────────────────

    #[test]
    fn moveto_not_duplicated() {
        // Regression: a previous bug emitted two identical `m` lines per MoveTo.
        let verbs = vec![GlyphVerb::MoveTo(10.0, 20.0)];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        let m_count = buf.matches(" m\n").count();
        assert_eq!(m_count, 1, "MoveTo must emit exactly one `m` operator, got:\n{buf}");
    }

    #[test]
    fn moveto_correct_coordinates() {
        let verbs = vec![GlyphVerb::MoveTo(1.5, 2.25)];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        assert_eq!(buf, "1.5000 2.2500 m\n");
    }

    #[test]
    fn lineto_correct_operator() {
        let verbs = vec![GlyphVerb::LineTo(3.0, 4.0)];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        assert_eq!(buf, "3.0000 4.0000 l\n");
    }

    #[test]
    fn close_emits_h() {
        let verbs = vec![GlyphVerb::Close];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        assert_eq!(buf, "h\n");
    }

    #[test]
    fn cubic_emits_c_operator() {
        let verbs = vec![GlyphVerb::CubicTo(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        assert_eq!(buf, "1.0000 2.0000 3.0000 4.0000 5.0000 6.0000 c\n");
    }

    #[test]
    fn quad_produces_c_not_l() {
        // QuadTo must be elevated to a cubic `c`, not kept as a line.
        let verbs = vec![GlyphVerb::QuadTo(3.0, 6.0, 6.0, 0.0)];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        assert!(buf.ends_with(" c\n"), "QuadTo must emit a `c` operator, got:\n{buf}");
        assert!(!buf.contains(" l\n"), "QuadTo must not emit an `l` operator");
    }

    #[test]
    fn quad_elevation_math() {
        // Start = (0,0), ctrl = (3,6), end = (6,0).
        // cp1 = start + 2/3*(ctrl-start) = (0,0) + 2/3*(3,6) = (2,4)
        // cp2 = end   + 2/3*(ctrl-end)   = (6,0) + 2/3*(-3,6) = (4,4)
        let verbs = vec![
            GlyphVerb::MoveTo(0.0, 0.0),
            GlyphVerb::QuadTo(3.0, 6.0, 6.0, 0.0),
        ];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        let expected_cubic = "2.0000 4.0000 4.0000 4.0000 6.0000 0.0000 c\n";
        assert!(
            buf.contains(expected_cubic),
            "Quadratic elevation produced wrong control points.\nGot:\n{buf}\nExpected to contain:\n{expected_cubic}"
        );
    }

    #[test]
    fn quad_uses_current_point_from_preceding_moveto() {
        // If the current point is (10,10) and ctrl=(13,16), end=(16,10):
        // cp1 = (10,10) + 2/3*(3,6) = (12,14)
        // cp2 = (16,10) + 2/3*(-3,6) = (14,14)
        let verbs = vec![
            GlyphVerb::MoveTo(10.0, 10.0),
            GlyphVerb::QuadTo(13.0, 16.0, 16.0, 10.0),
        ];
        let mut buf = String::new();
        write_glyph_path(&mut buf, &verbs);
        let expected_cubic = "12.0000 14.0000 14.0000 14.0000 16.0000 10.0000 c\n";
        assert!(
            buf.contains(expected_cubic),
            "QuadTo with non-zero start gave wrong control points.\nGot:\n{buf}\nExpected to contain:\n{expected_cubic}"
        );
    }
}
