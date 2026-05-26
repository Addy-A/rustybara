//! Glyph outline extraction from a PDF page content stream.
//!
//! [`outline_page_text`] walks every BT...ET block on the page, resolves each
//! character code to a `ttf_parser` glyph, collects the bézier outline, and
//! returns all glyphs with their coordinates in **PDF page space** (post-CTM).

use lopdf::{Document, Object, ObjectId};
use std::collections::HashMap;
use ttf_parser::OutlineBuilder;

use super::{
    encoding::{self, FontEncoding},
    font,
};
use crate::geometry::Matrix;
use crate::pages::boxes::object_to_f64;

/// A single path verb, with all coordinates in **PDF page space** (Y-up, points).
#[derive(Clone, Debug)]
pub enum GlyphVerb {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    /// Quadratic bézier: (control_x, control_y, end_x, end_y)
    QuadTo(f64, f64, f64, f64),
    /// Cubic bézier: (cp1_x, cp1_y, cp2_x, cp2_y, end_x, end_y)
    CubicTo(f64, f64, f64, f64, f64, f64),
    Close,
}

/// Outline geometry for a single positioned glyph.
#[derive(Clone, Debug)]
pub struct PositionedGlyph {
    /// Path verbs in PDF page space.
    pub verbs: Vec<GlyphVerb>,
}

/// Extract glyph outlines for every character on`page_id`.
///
/// Returns one [`PositionedGlyph`] per successfully resolved glyph.
/// Glyphs that cannot be resolved (Type 1, CIDFont, missing font) are
/// silently skipped; callers fall back to the bbox placeholder for those
/// text objects.
pub fn outline_page_text(doc: &Document, page_id: ObjectId) -> crate::Result<Vec<PositionedGlyph>> {
    let content = doc.get_and_decode_page_content(page_id)?;
    let mut out: Vec<PositionedGlyph> = Vec::new();

    let mut ctm_stack: Vec<Matrix> = Vec::new();
    let mut ctm = Matrix::identity();

    let mut in_text = false;
    let mut tm = Matrix::identity();
    let mut lm = Matrix::identity();
    let mut font_size: f64 = 12.0;
    let mut horiz_scale: f64 = 1.0;
    let mut char_spacing: f64 = 0.0;
    let mut word_spacing: f64 = 0.0;
    let mut leading: f64 = 0.0;

    let mut font_cache: HashMap<Vec<u8>, Option<(Vec<u8>, FontEncoding)>> = HashMap::new();
    let mut current_font: Vec<u8> = Vec::new();

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => ctm_stack.push(ctm),
            "Q" => {
                if let Some(prev) = ctm_stack.pop() {
                    ctm = prev;
                }
            }
            "cm" if op.operands.len() >= 6 => {
                ctm = ctm.concat(&ops_to_matrix(&op.operands));
            }
            "BT" => {
                in_text = true;
                tm = Matrix::identity();
                lm = Matrix::identity();
            }
            "ET" => {
                in_text = false;
            }

            // ── Text state operators ──────────────────────────────────────────
            "Tf" if in_text && op.operands.len() >= 2 => {
                if let Object::Name(name) = &op.operands[0] {
                    current_font = name.clone();
                    font_cache.entry(name.clone()).or_insert_with(|| {
                        let bytes = font::extract_font_bytes(doc, page_id, name)?;
                        let enc = encoding::build_encoding(doc, page_id, name);
                        Some((bytes, enc))
                    });
                }
                font_size = object_to_f64(&op.operands[1]).abs().max(0.001);
            }
            "Tz" if in_text && !op.operands.is_empty() => {
                horiz_scale = object_to_f64(&op.operands[0]) / 100.0;
            }
            "Tc" if in_text && !op.operands.is_empty() => {
                char_spacing = object_to_f64(&op.operands[0]);
            }
            "Tw" if in_text && !op.operands.is_empty() => {
                word_spacing = object_to_f64(&op.operands[0]);
            }
            "TL" if in_text && !op.operands.is_empty() => {
                leading = object_to_f64(&op.operands[0]);
            }

            // ── Text position operators ───────────────────────────────────────
            "Tm" if in_text && op.operands.len() >= 6 => {
                tm = ops_to_matrix(&op.operands);
                lm = tm;
            }
            "Td" | "TD" if in_text && op.operands.len() >= 2 => {
                let tx = object_to_f64(&op.operands[0]);
                let ty = object_to_f64(&op.operands[1]);
                if op.operator == "TD" {
                    leading = -ty;
                }
                // Correct: apply scale/rotation from lm before updating translation.
                let (new_e, new_f) = lm.transform_point(tx, ty);
                lm = Matrix::from_values(lm.a, lm.b, lm.c, lm.d, new_e, new_f);
                tm = lm;
            }
            "T*" if in_text => {
                let (new_e, new_f) = lm.transform_point(0.0, -leading);
                lm = Matrix::from_values(lm.a, lm.b, lm.c, lm.d, new_e, new_f);
                tm = lm;
            }

            // ── Text show operators ───────────────────────────────────────────
            "Tj" if in_text && !op.operands.is_empty() => {
                if let Object::String(bytes, _) = &op.operands[0] {
                    show_string(
                        bytes,
                        &mut tm,
                        &ctm,
                        font_size,
                        horiz_scale,
                        char_spacing,
                        word_spacing,
                        &current_font,
                        &font_cache,
                        &mut out,
                    );
                }
            }
            // ' is equivalent to T* followed by Tj: move to the next line first.
            "'" if in_text && !op.operands.is_empty() => {
                let (new_e, new_f) = lm.transform_point(0.0, -leading);
                lm = Matrix::from_values(lm.a, lm.b, lm.c, lm.d, new_e, new_f);
                tm = lm;
                if let Object::String(bytes, _) = &op.operands[0] {
                    show_string(
                        bytes,
                        &mut tm,
                        &ctm,
                        font_size,
                        horiz_scale,
                        char_spacing,
                        word_spacing,
                        &current_font,
                        &font_cache,
                        &mut out,
                    );
                }
            }
            // " sets word/char spacing from operands [aw, ac, string] then renders.
            "\"" if in_text && op.operands.len() >= 3 => {
                word_spacing = object_to_f64(&op.operands[0]);
                char_spacing = object_to_f64(&op.operands[1]);
                if let Object::String(bytes, _) = &op.operands[2] {
                    show_string(
                        bytes,
                        &mut tm,
                        &ctm,
                        font_size,
                        horiz_scale,
                        char_spacing,
                        word_spacing,
                        &current_font,
                        &font_cache,
                        &mut out,
                    );
                }
            }
            "TJ" if in_text && !op.operands.is_empty() => {
                if let Object::Array(arr) = &op.operands[0] {
                    for item in arr {
                        match item {
                            Object::String(bytes, _) => {
                                show_string(
                                    bytes,
                                    &mut tm,
                                    &ctm,
                                    font_size,
                                    horiz_scale,
                                    char_spacing,
                                    word_spacing,
                                    &current_font,
                                    &font_cache,
                                    &mut out,
                                );
                            }
                            Object::Integer(n) => {
                                // Negative = move right (tighten); positive = move left.
                                let adj = -(*n as f64) / 1000.0 * font_size * horiz_scale;
                                tm.e += tm.a * adj;
                                tm.f += tm.b * adj;
                            }
                            Object::Real(n) => {
                                let adj = -(*n as f64) / 1000.0 * font_size * horiz_scale;
                                tm.e += tm.a * adj;
                                tm.f += tm.b * adj;
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn show_string(
    bytes: &[u8],
    tm: &mut Matrix,
    ctm: &Matrix,
    font_size: f64,
    horiz_scale: f64,
    char_spacing: f64,
    word_spacing: f64,
    font_name: &[u8],
    cache: &HashMap<Vec<u8>, Option<(Vec<u8>, FontEncoding)>>,
    out: &mut Vec<PositionedGlyph>,
) {
    let Some(Some((font_bytes, encoding))) = cache.get(font_name) else {
        return;
    };
    let Ok(face) = ttf_parser::Face::parse(font_bytes, 0) else {
        return;
    };
    let upem = face.units_per_em() as f64;
    let scale = font_size / upem;

    for &charcode in bytes {
        let glyph_id = match encoding.resolve(charcode, &face) {
            Some(id) => id,
            None => {
                let dx = font_size * horiz_scale;
                tm.e += tm.a * dx;
                tm.f += tm.b * dx;
                continue;
            }
        };

        let mut builder = GlyphBuilder::new(*tm, *ctm, scale);
        face.outline_glyph(glyph_id, &mut builder);
        if !builder.verbs.is_empty() {
            out.push(PositionedGlyph {
                verbs: builder.verbs,
            });
        }

        let raw_adv = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64;
        let glyph_w = raw_adv / upem * font_size;
        let tw = if charcode == 0x20 { word_spacing } else { 0.0 };
        let dx = (glyph_w + char_spacing + tw) * horiz_scale;
        tm.e += tm.a * dx;
        tm.f += tm.b * dx;
    }
}

/// Implements `ttf_parser::OutlineBuilder`, transforming each incoming glyph-space
/// vertex through `CTM * Tm * scale` to produce page-space [`GlyphVerb`]s.
struct GlyphBuilder {
    verbs: Vec<GlyphVerb>,
    tm: Matrix,
    ctm: Matrix,
    scale: f64,
}

impl GlyphBuilder {
    fn new(tm: Matrix, ctm: Matrix, scale: f64) -> Self {
        Self {
            verbs: Vec::new(),
            tm,
            ctm,
            scale,
        }
    }

    /// Transform a glyph-space point to PDF page space.
    #[inline]
    fn to_page(&self, gx: f32, gy: f32) -> (f64, f64) {
        let (tx, ty) = self
            .tm
            .transform_point(gx as f64 * self.scale, gy as f64 * self.scale);
        self.ctm.transform_point(tx, ty)
    }
}

impl OutlineBuilder for GlyphBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let (px, py) = self.to_page(x, y);
        self.verbs.push(GlyphVerb::MoveTo(px, py));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (px, py) = self.to_page(x, y);
        self.verbs.push(GlyphVerb::LineTo(px, py));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (cpx, cpy) = self.to_page(cx, cy);
        let (px, py) = self.to_page(x, y);
        self.verbs.push(GlyphVerb::QuadTo(cpx, cpy, px, py));
    }
    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        let (p1x, p1y) = self.to_page(c1x, c1y);
        let (p2x, p2y) = self.to_page(c2x, c2y);
        let (px, py) = self.to_page(x, y);
        self.verbs
            .push(GlyphVerb::CubicTo(p1x, p1y, p2x, p2y, px, py));
    }
    fn close(&mut self) {
        self.verbs.push(GlyphVerb::Close);
    }
}

fn ops_to_matrix(operands: &[Object]) -> Matrix {
    Matrix::from_values(
        object_to_f64(&operands[0]),
        object_to_f64(&operands[1]),
        object_to_f64(&operands[2]),
        object_to_f64(&operands[3]),
        object_to_f64(&operands[4]),
        object_to_f64(&operands[5]),
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ops_to_matrix ─────────────────────────────────────────────────────────

    fn real(v: f64) -> Object {
        Object::Real(v as f32)
    }

    fn ops(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Vec<Object> {
        vec![real(a), real(b), real(c), real(d), real(e), real(f)]
    }

    #[test]
    fn ops_to_matrix_identity() {
        let m = ops_to_matrix(&ops(1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
        assert!((m.a - 1.0).abs() < 1e-6);
        assert!((m.d - 1.0).abs() < 1e-6);
        assert!((m.e).abs() < 1e-6);
        assert!((m.f).abs() < 1e-6);
    }

    #[test]
    fn ops_to_matrix_translation() {
        let m = ops_to_matrix(&ops(1.0, 0.0, 0.0, 1.0, 72.0, 144.0));
        assert!((m.e - 72.0).abs() < 1e-4);
        assert!((m.f - 144.0).abs() < 1e-4);
    }

    #[test]
    fn ops_to_matrix_scale() {
        let m = ops_to_matrix(&ops(2.0, 0.0, 0.0, 3.0, 0.0, 0.0));
        assert!((m.a - 2.0).abs() < 1e-6);
        assert!((m.d - 3.0).abs() < 1e-6);
    }

    // ── GlyphBuilder ─────────────────────────────────────────────────────────

    /// Construct a GlyphBuilder with identity Tm, identity CTM, and the given scale.
    fn builder(scale: f64) -> GlyphBuilder {
        GlyphBuilder::new(Matrix::identity(), Matrix::identity(), scale)
    }

    /// Construct a GlyphBuilder with the given Tm, identity CTM, scale=1.
    fn builder_tm(tm: Matrix) -> GlyphBuilder {
        GlyphBuilder::new(tm, Matrix::identity(), 1.0)
    }

    /// Construct a GlyphBuilder with the given Tm and CTM, scale=1.
    fn builder_tm_ctm(tm: Matrix, ctm: Matrix) -> GlyphBuilder {
        GlyphBuilder::new(tm, ctm, 1.0)
    }

    #[test]
    fn glyph_builder_identity_move_to() {
        // Identity Tm, CTM, scale=1: glyph coords pass through unchanged.
        let mut b = builder(1.0);
        b.move_to(10.0, 20.0);
        match b.verbs[0] {
            GlyphVerb::MoveTo(x, y) => {
                assert!((x - 10.0).abs() < 1e-6, "x={x}");
                assert!((y - 20.0).abs() < 1e-6, "y={y}");
            }
            _ => panic!("expected MoveTo"),
        }
    }

    #[test]
    fn glyph_builder_scale_applied() {
        // scale = font_size / upem; e.g. 12pt / 1000upem = 0.012.
        // Glyph point (500, 0) → page (500 * 0.012, 0) = (6.0, 0.0).
        let mut b = builder(0.012);
        b.move_to(500.0, 0.0);
        match b.verbs[0] {
            GlyphVerb::MoveTo(x, y) => {
                assert!((x - 6.0).abs() < 1e-4, "x={x}");
                assert!(y.abs() < 1e-6, "y={y}");
            }
            _ => panic!("expected MoveTo"),
        }
    }

    #[test]
    fn glyph_builder_tm_translation() {
        // Tm with translation (736, 80): glyph origin (0,0) → page (736,80).
        let tm = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 736.0, 80.0);
        let mut b = builder_tm(tm);
        b.move_to(0.0, 0.0);
        match b.verbs[0] {
            GlyphVerb::MoveTo(x, y) => {
                assert!((x - 736.0).abs() < 1e-3, "x={x}");
                assert!((y - 80.0).abs() < 1e-3, "y={y}");
            }
            _ => panic!("expected MoveTo"),
        }
    }

    #[test]
    fn glyph_builder_ctm_translation() {
        // CTM offset (-30, -40) shifts the already-computed page point.
        let ctm = Matrix::from_values(1.0, 0.0, 0.0, 1.0, -30.0, -40.0);
        let mut b = builder_tm_ctm(Matrix::identity(), ctm);
        b.move_to(100.0, 200.0);
        match b.verbs[0] {
            GlyphVerb::MoveTo(x, y) => {
                assert!((x - 70.0).abs() < 1e-4, "x={x}");
                assert!((y - 160.0).abs() < 1e-4, "y={y}");
            }
            _ => panic!("expected MoveTo"),
        }
    }

    #[test]
    fn glyph_builder_all_verb_types() {
        let mut b = builder(1.0);
        b.move_to(0.0, 0.0);
        b.line_to(1.0, 0.0);
        b.quad_to(1.5, 1.0, 2.0, 0.0);
        b.curve_to(2.5, 1.0, 3.5, 1.0, 4.0, 0.0);
        b.close();
        assert_eq!(b.verbs.len(), 5);
        assert!(matches!(b.verbs[0], GlyphVerb::MoveTo(..)));
        assert!(matches!(b.verbs[1], GlyphVerb::LineTo(..)));
        assert!(matches!(b.verbs[2], GlyphVerb::QuadTo(..)));
        assert!(matches!(b.verbs[3], GlyphVerb::CubicTo(..)));
        assert!(matches!(b.verbs[4], GlyphVerb::Close));
    }

    #[test]
    fn glyph_builder_rotated_text() {
        // A 15° rotation matrix (approximate): a=cos15≈0.9659, b=sin15≈0.2588,
        // c=-sin15≈-0.2588, d=cos15≈0.9659.
        // Glyph point (1.0, 0.0) should map to approximately (0.9659, 0.2588).
        let a = 15f64.to_radians().cos();
        let b_val = 15f64.to_radians().sin();
        let tm = Matrix::from_values(a, b_val, -b_val, a, 0.0, 0.0);
        let mut b = GlyphBuilder::new(tm, Matrix::identity(), 1.0);
        b.move_to(1.0, 0.0);
        match b.verbs[0] {
            GlyphVerb::MoveTo(x, y) => {
                assert!((x - a).abs() < 1e-4, "x={x} expected≈{a}");
                assert!((y - b_val).abs() < 1e-4, "y={y} expected≈{b_val}");
            }
            _ => panic!("expected MoveTo"),
        }
    }
}
