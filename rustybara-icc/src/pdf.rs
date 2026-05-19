use lopdf::content::{Content, Operation};
use lopdf::{Document, Object};

use crate::ColorSpaceKind;
use crate::transform::ColorTransform;

/// Summary of changes made during a PDF color space conversion.
///
/// This struct tracks the operations performed by [`PdfColorConverter::convert_document`]
/// across all pages of a PDF.
///
/// # Fields
///
/// * `pages_processed` — Number of pages visited by the converter
/// * `images_converted` — Number of image XObjects whose pixel data was ICC-transformed (not yet implemented)
/// * `spot_colors_flattened` — Number of Separation spot color uses flattened to device CMYK
/// * `color_spaces_rewritten` — Number of color space resource dictionary entries rewritten (not yet implemented)
/// * `warnings` — Non-fatal warnings encountered during conversion (e.g., skipped DeviceN color spaces)
pub struct ConversionReport {
    /// Number of pages visited by the converter.
    pub pages_processed: u32,
    /// Number of image XObjects whose pixel data was ICC-transformed (Phase 4c — not yet implemented).
    pub images_converted: u32,
    /// Number of `Separation` spot color uses flattened to device CMYK.
    pub spot_colors_flattened: u32,
    /// Number of color space resource dictionary entries rewritten (Phase 4a — not yet implemented).
    pub color_spaces_rewritten: u32,
    /// Non-fatal warnings encountered during conversion (e.g. skipped DeviceN color spaces).
    pub warnings: Vec<String>,
}

/// Converts PDF documents between color spaces using ICC profiles.
///
/// `PdfColorConverter` walks through a PDF's content streams and applies ICC-based
/// color transformations to device color operators (e.g., `k`, `K`, `rg`, `RG`). It also
/// flattens spot colors (Separation color spaces) to their device equivalents before
/// applying the transform.
///
/// # Example
///
/// ```ignore
/// use rustybara_icc::{ColorTransform, RenderingIntent, profiles};
/// use rustybara_icc::pdf::PdfColorConverter;
///
/// # fn main() -> rustybara_icc::Result<()> {
/// let mut doc = lopdf::Document::load("input.pdf").unwrap();
///
/// let transform = ColorTransform::new(
///     &profiles::COATED_FOGRA_39,
///     &profiles::COATED_GRACOL_2006,
///     RenderingIntent::RelativeColorimetric,
/// )?;
///
/// let report = PdfColorConverter::new(&mut doc, transform)
///     .convert_document()?;
///
/// println!("{} pages, {} spots flattened",
///     report.pages_processed, report.spot_colors_flattened);
///
/// doc.save("output.pdf").unwrap();
/// # Ok(())
/// # }
/// ```
pub struct PdfColorConverter<'a> {
    doc: &'a mut lopdf::Document,
    transform: ColorTransform,
}

impl<'a> PdfColorConverter<'a> {
    /// Creates a new PDF color converter with the given document and ICC transform.
    ///
    /// # Arguments
    ///
    /// * `doc` — Mutable reference to the PDF document to convert
    /// * `transform` — ICC color transform defining source and destination profiles
    pub fn new(doc: &'a mut lopdf::Document, transform: ColorTransform) -> Self {
        PdfColorConverter { doc, transform }
    }

    /// Convert color spaces throughout the entire document and return a summary report.
    ///
    /// Iterates every page returned by the document's page tree, applies the spot-color
    /// pre-pass followed by the ICC color transform to each page's content stream, and
    /// aggregates the results into a [`ConversionReport`].
    ///
    /// **DeviceN limitation:** `DeviceN` color spaces (multi-channel inks, Hexachrome, etc.)
    /// are detected but not correctly flattened in this version. Their `scn`/`SCN` operators
    /// carry one tint value per ink channel, which the current single-channel tint evaluator
    /// does not handle. Full DeviceN support is a post-v1.0 item; for now, documents
    /// containing DeviceN inks may produce incorrect output for those operators.
    ///
    /// # Example
    /// ```ignore
    /// use rustybara_icc::{ColorTransform, RenderingIntent, profiles};
    /// use rustybara_icc::pdf::PdfColorConverter;
    ///
    /// let mut doc = lopdf::Document::load("input.pdf").unwrap();
    /// let transform = ColorTransform::new(
    ///     &profiles::COATED_FOGRA_39,
    ///     &profiles::COATED_GRACOL_2006,
    ///     RenderingIntent::RelativeColorimetric,
    /// ).unwrap();
    /// let report = PdfColorConverter::new(&mut doc, transform)
    ///     .convert_document()
    ///     .unwrap();
    /// println!("{} pages, {} spots flattened", report.pages_processed, report.spot_colors_flattened);
    /// ```
    pub fn convert_document(&mut self) -> crate::Result<ConversionReport> {
        let mut report = ConversionReport {
            pages_processed: 0,
            images_converted: 0,
            spot_colors_flattened: 0,
            color_spaces_rewritten: 0,
            warnings: Vec::new(),
        };
        let page_ids: Vec<lopdf::ObjectId> = self.doc.get_pages().values().copied().collect();

        for page_id in page_ids {
            let spots = self.convert_page(page_id)?;
            report.spot_colors_flattened += spots;
            report.pages_processed += 1;
        }

        Ok(report)
    }

    /// Convert a single page identified by its lopdf `ObjectId`.
    ///
    /// Returns the number of spot color uses flattened on this page.
    /// Prefer [`Self::convert_document`] for whole-document conversion.
    pub fn convert_page(&mut self, page_id: lopdf::ObjectId) -> crate::Result<u32> {
        let content = self.doc.get_and_decode_page_content(page_id)?;

        let spot_names: std::collections::HashSet<String> = find_spot_colorspaces(self.doc)
            .into_iter()
            .map(|(alias, _)| alias)
            .collect();
        let (flattened, spots_counts) =
            flatten_spot_ops(&content.operations, &spot_names, self.doc, page_id);

        let rewritten = convert_operations(&flattened, &self.transform);
        let new_content = Content {
            operations: rewritten,
        };
        let bytes = new_content.encode()?;

        let stream_ids = self.doc.get_page_contents(page_id);
        if stream_ids.is_empty() {
            return Ok(spots_counts);
        }
        let stream_id = stream_ids[0];
        if let Ok(Object::Stream(stream)) = self.doc.get_object_mut(stream_id) {
            stream.set_plain_content(bytes);
        }

        if stream_ids.len() > 1 {
            for &extra_id in &stream_ids[1..] {
                if let Ok(Object::Stream(s)) = self.doc.get_object_mut(extra_id) {
                    s.set_plain_content(Vec::new());
                }
            }
            if let Ok(page_obj) = self.doc.get_object_mut(page_id)
                && let Ok(dict) = page_obj.as_dict_mut()
            {
                dict.set("Contents", Object::Reference(stream_id));
            }
        }
        Ok(spots_counts)
    }
}

/// Flattens all `Separation` spot color uses to their device CMYK alternates across the
/// entire document without applying any ICC transform.
///
/// For each page, `cs`/`scn` operator pairs that reference a `Separation` color space are
/// replaced with equivalent `k` (device CMYK) operators using the tint function embedded in
/// the color space definition. The document is modified in-place.
///
/// Returns the total number of spot color operator sequences replaced across all pages.
///
/// **DeviceN limitation:** `DeviceN` color spaces are detected but not correctly flattened —
/// their `scn` operators carry one tint value per ink channel, which the current single-channel
/// evaluator does not handle. Those operators are left unchanged.
pub fn flatten_spot_colors(doc: &mut Document) -> crate::Result<u32> {
    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().values().copied().collect();
    let mut total = 0u32;

    for page_id in page_ids {
        let content = doc.get_and_decode_page_content(page_id)?;
        let spot_names: std::collections::HashSet<String> = find_spot_colorspaces(doc)
            .into_iter()
            .map(|(alias, _)| alias)
            .collect();
        let (flattened, count) = flatten_spot_ops(&content.operations, &spot_names, doc, page_id);
        total += count;

        let new_content = Content { operations: flattened };
        let bytes = new_content.encode()?;

        let stream_ids = doc.get_page_contents(page_id);
        if stream_ids.is_empty() {
            continue;
        }
        let stream_id = stream_ids[0];
        if let Ok(Object::Stream(stream)) = doc.get_object_mut(stream_id) {
            stream.set_plain_content(bytes);
        }
        if stream_ids.len() > 1 {
            for &extra_id in &stream_ids[1..] {
                if let Ok(Object::Stream(s)) = doc.get_object_mut(extra_id) {
                    s.set_plain_content(Vec::new());
                }
            }
            if let Ok(page_obj) = doc.get_object_mut(page_id)
                && let Ok(dict) = page_obj.as_dict_mut()
            {
                dict.set("Contents", Object::Reference(stream_id));
            }
        }
    }
    Ok(total)
}

fn convert_operations(operations: &[Operation], transform: &ColorTransform) -> Vec<Operation> {
    operations
        .iter()
        .map(|op| convert_operation(op, transform))
        .collect()
}

fn convert_operation(op: &Operation, transform: &ColorTransform) -> Operation {
    let (fill_op, stroke_op, channel_count) = match transform.input_color_space() {
        crate::ColorSpaceKind::Rgb => ("rg", "RG", 3usize),
        crate::ColorSpaceKind::Cmyk => ("k", "K", 4usize),
        crate::ColorSpaceKind::Gray => ("g", "G", 1usize),
        _ => return op.clone(),
    };

    let is_fill = (op.operator == fill_op || op.operator == "sc")
        && op.operands.len() == channel_count;
    let is_stroke = (op.operator == stroke_op || op.operator == "SC")
        && op.operands.len() == channel_count;

    if !(is_fill || is_stroke) {
        return op.clone();
    }

    let input_u8: Vec<u8> = op
        .operands
        .iter()
        .map(|obj| (object_to_f64(obj) * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect();

    let output_u8 = transform.convert(&input_u8);

    let out_op = if is_fill {
        dst_fill_op(transform.output_color_space())
    } else {
        dst_stroke_op(transform.output_color_space())
    };

    Operation {
        operator: out_op.to_string(),
        operands: output_u8
            .iter()
            .map(|&v| Object::Real(v as f32 / 255.0))
            .collect(),
    }
}

pub fn find_spot_colorspaces(doc: &Document) -> Vec<(String, String)> {
    let mut spots = Vec::new();

    for (_, page_id) in doc.get_pages() {
        let Ok((Some(res_dict), _)) = doc.get_page_resources(page_id) else {
            continue;
        };
        let Ok(cs_dict) = res_dict.get(b"ColorSpace") else {
            continue;
        };
        let Ok(cs_map) = cs_dict.as_dict() else {
            continue;
        };

        for (name, obj) in cs_map.iter() {
            let Ok((_, Object::Array(arr))) = doc.dereference(obj) else {
                continue;
            };
            let first = arr.first().and_then(|o| o.as_name().ok());
            if first != Some(b"Separation") && first != Some(b"DeviceN") {
                continue;
            }
            let ink_name = arr
                .get(1)
                .and_then(|o| o.as_name().ok())
                .map(|n| String::from_utf8_lossy(n).to_string())
                .unwrap_or_default();
            if ink_name.is_empty() || ink_name == "All" || ink_name == "None" {
                continue;
            }
            spots.push((String::from_utf8_lossy(name).to_string(), ink_name));
        }
    }
    spots
}

fn flatten_spot_ops(
    ops: &[Operation],
    spot_names: &std::collections::HashSet<String>,
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> (Vec<Operation>, u32) {
    let mut out = Vec::with_capacity(ops.len());
    let mut current_fill_cs: Option<String> = None;
    let mut current_stroke_cs: Option<String> = None;
    let mut count = 0u32;

    for op in ops {
        match op.operator.as_str() {
            "cs" => {
                if let Some(lopdf::Object::Name(n)) = op.operands.first() {
                    let name = String::from_utf8_lossy(n).to_string();
                    if spot_names.contains(&name) {
                        current_fill_cs = Some(name);
                        continue;
                    }
                }
                current_fill_cs = None;
                out.push(op.clone());
            }
            "CS" => {
                if let Some(lopdf::Object::Name(n)) = op.operands.first() {
                    let name = String::from_utf8_lossy(n).to_string();
                    if spot_names.contains(&name) {
                        current_stroke_cs = Some(name);
                        continue;
                    }
                }
                current_stroke_cs = None;
                out.push(op.clone());
            }
            "scn" | "sc" if current_fill_cs.is_some() && op.operands.len() == 1 => {
                let tint = op
                    .operands
                    .first()
                    .map(object_to_f64)
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0) as f32;
                let cmyk =
                    spot_tint_to_cmyk(current_fill_cs.as_deref().unwrap(), tint, doc, page_id);
                out.push(Operation {
                    operator: "k".to_string(),
                    operands: cmyk.iter().map(|&v| lopdf::Object::Real(v)).collect(),
                });
                count += 1;
            }
            "SCN" | "SC" if current_stroke_cs.is_some() && op.operands.len() == 1 => {
                let tint = op
                    .operands
                    .first()
                    .map(object_to_f64)
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0) as f32;
                let cmyk =
                    spot_tint_to_cmyk(current_stroke_cs.as_deref().unwrap(), tint, doc, page_id);
                out.push(Operation {
                    operator: "K".to_string(),
                    operands: cmyk.iter().map(|&v| lopdf::Object::Real(v)).collect(),
                });
                count += 1;
            }
            _ => out.push(op.clone()),
        }
    }
    (out, count)
}

fn spot_tint_to_cmyk(
    alias: &str,
    tint: f32,
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> [f32; 4] {
    let fallback = [0.0, 0.0, 0.0, tint];

    let resources = doc.get_page_resources(page_id);
    let Ok((Some(res_dict), _)) = resources else {
        return fallback;
    };
    let Ok(cs_obj) = res_dict.get(b"ColorSpace") else {
        return fallback;
    };
    let Ok(cs_map) = cs_obj.as_dict() else {
        return fallback;
    };
    let Ok(alias_obj) = cs_map.get(alias.as_bytes()) else {
        return fallback;
    };
    let Ok((_, lopdf::Object::Array(arr))) = doc.dereference(alias_obj) else {
        return fallback;
    };

    if arr.len() < 4 {
        return fallback;
    }

    let alternate = arr
        .get(2)
        .and_then(|o| o.as_name().ok().map(|n| n.to_vec()));
    let is_cmyk = alternate.as_deref() == Some(b"DeviceCMYK");
    if !is_cmyk {
        return fallback;
    }

    let Ok((_, lopdf::Object::Dictionary(fn_dict))) = doc.dereference(&arr[3]) else {
        return fallback;
    };
    let Ok(lopdf::Object::Array(c0)) = fn_dict.get(b"C0") else {
        return fallback;
    };
    let Ok(lopdf::Object::Array(c1)) = fn_dict.get(b"C1") else {
        return fallback;
    };

    if c0.len() < 4 || c1.len() < 4 {
        return fallback;
    }

    let lerp = |a: &lopdf::Object, b: &lopdf::Object| -> f32 {
        let av = object_to_f64(a) as f32;
        let bv = object_to_f64(b) as f32;
        av + tint * (bv - av)
    };

    [
        lerp(&c0[0], &c1[0]),
        lerp(&c0[1], &c1[1]),
        lerp(&c0[2], &c1[2]),
        lerp(&c0[3], &c1[3]),
    ]
}

fn dst_fill_op(cs: &ColorSpaceKind) -> &'static str {
    match cs {
        ColorSpaceKind::Rgb => "rg",
        ColorSpaceKind::Cmyk => "k",
        ColorSpaceKind::Gray => "g",
        _ => "k",
    }
}

fn dst_stroke_op(cs: &ColorSpaceKind) -> &'static str {
    match cs {
        ColorSpaceKind::Rgb => "RG",
        ColorSpaceKind::Cmyk => "K",
        ColorSpaceKind::Gray => "G",
        _ => "K",
    }
}

fn object_to_f64(obj: &lopdf::Object) -> f64 {
    match obj {
        lopdf::Object::Integer(i) => *i as f64,
        lopdf::Object::Real(r) => *r as f64,
        _ => 0.0,
    }
}

#[cfg(all(test, feature = "bundled-profiles"))]
mod tests {
    use super::*;
    use crate::RenderingIntent;
    use crate::profiles::{COATED_FOGRA_39, COATED_GRACOL_2006};
    use lopdf::content::{Content, Operation};
    use lopdf::{Dictionary, Object, Stream, dictionary};

    // ── helpers ───────────────────────────────────────────────────────────────

    fn fogra39_to_gracol() -> ColorTransform {
        ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap()
    }

    fn cmyk_op(operator: &str, c: f32, m: f32, y: f32, k: f32) -> Operation {
        Operation {
            operator: operator.to_string(),
            operands: vec![
                Object::Real(c),
                Object::Real(m),
                Object::Real(y),
                Object::Real(k),
            ],
        }
    }

    fn bare_op(operator: &str) -> Operation {
        Operation {
            operator: operator.to_string(),
            operands: vec![],
        }
    }

    fn operand_f32(op: &Operation, idx: usize) -> f32 {
        match op.operands[idx] {
            Object::Real(v) => v,
            Object::Integer(v) => v as f32,
            _ => panic!("expected numeric operand at index {idx}"),
        }
    }

    /// Builds a page-only document suitable for convert_page (no full page tree).
    fn make_minimal_doc(ops: Vec<Operation>) -> (lopdf::Document, lopdf::ObjectId) {
        let bytes = Content { operations: ops }.encode().unwrap();
        let mut doc = lopdf::Document::with_version("1.5");
        let stream_id = doc.add_object(Stream::new(Dictionary::new(), bytes));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Contents" => Object::Reference(stream_id),
        });
        (doc, page_id)
    }

    /// Builds a document with a proper page tree so convert_document can find pages.
    fn make_full_doc(ops: Vec<Operation>) -> (lopdf::Document, lopdf::ObjectId) {
        let bytes = Content { operations: ops }.encode().unwrap();
        let mut doc = lopdf::Document::with_version("1.5");
        let stream_id = doc.add_object(Stream::new(Dictionary::new(), bytes));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![
                Object::Integer(0), Object::Integer(0),
                Object::Integer(612), Object::Integer(792),
            ],
            "Contents" => Object::Reference(stream_id),
        });
        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1_i64,
        });
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));
        (doc, page_id)
    }

    fn decoded_ops(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Vec<Operation> {
        doc.get_and_decode_page_content(page_id).unwrap().operations
    }

    // ── object_to_f64 ─────────────────────────────────────────────────────────

    #[test]
    fn object_to_f64_converts_integer() {
        assert_eq!(object_to_f64(&Object::Integer(200)), 200.0);
    }

    #[test]
    fn object_to_f64_converts_real() {
        assert!((object_to_f64(&Object::Real(0.5_f32)) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn object_to_f64_unknown_type_defaults_to_zero() {
        assert_eq!(object_to_f64(&Object::Boolean(true)), 0.0);
    }

    // ── convert_operation: operator routing ───────────────────────────────────

    #[test]
    fn cmyk_fill_op_output_operator_is_k() {
        let out = convert_operation(&cmyk_op("k", 0.75, 0.68, 0.67, 0.90), &fogra39_to_gracol());
        assert_eq!(out.operator, "k");
    }

    #[test]
    fn cmyk_stroke_op_output_operator_is_uppercase_k() {
        let out = convert_operation(&cmyk_op("K", 0.0, 0.0, 0.0, 1.0), &fogra39_to_gracol());
        assert_eq!(out.operator, "K");
    }

    #[test]
    fn cmyk_fill_op_output_has_four_operands() {
        let out = convert_operation(&cmyk_op("k", 0.5, 0.3, 0.2, 0.1), &fogra39_to_gracol());
        assert_eq!(out.operands.len(), 4);
    }

    #[test]
    fn cmyk_fill_op_output_operands_are_in_zero_to_one_range() {
        let out = convert_operation(&cmyk_op("k", 0.75, 0.68, 0.67, 0.90), &fogra39_to_gracol());
        for i in 0..4 {
            let v = operand_f32(&out, i);
            assert!(
                (0.0..=1.0).contains(&v),
                "operand {i} = {v} is outside 0.0..=1.0"
            );
        }
    }

    #[test]
    fn unrecognized_operator_passes_through_unchanged() {
        let out = convert_operation(&bare_op("BT"), &fogra39_to_gracol());
        assert_eq!(out.operator, "BT");
        assert!(out.operands.is_empty());
    }

    #[test]
    fn q_and_q_save_restore_pass_through_unchanged() {
        let t = fogra39_to_gracol();
        assert_eq!(convert_operation(&bare_op("q"), &t).operator, "q");
        assert_eq!(convert_operation(&bare_op("Q"), &t).operator, "Q");
    }

    #[test]
    fn k_with_wrong_operand_count_passes_through_unchanged() {
        // k expects 4 operands; 3 must not be transformed
        let op = Operation {
            operator: "k".to_string(),
            operands: vec![Object::Real(0.1), Object::Real(0.2), Object::Real(0.3)],
        };
        let out = convert_operation(&op, &fogra39_to_gracol());
        assert_eq!(
            out.operands.len(),
            3,
            "wrong-count op should pass through as-is"
        );
    }

    // ── convert_operations: batch ─────────────────────────────────────────────

    #[test]
    fn convert_operations_preserves_order_and_count() {
        let ops = vec![bare_op("q"), cmyk_op("k", 0.0, 0.0, 0.0, 1.0), bare_op("Q")];
        let out = convert_operations(&ops, &fogra39_to_gracol());
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].operator, "q");
        assert_eq!(out[1].operator, "k");
        assert_eq!(out[2].operator, "Q");
    }

    // ── convert_page: integration ─────────────────────────────────────────────

    #[test]
    fn convert_page_returns_ok_on_cmyk_content() {
        let ops = vec![bare_op("q"), cmyk_op("k", 0.0, 0.0, 0.0, 1.0), bare_op("Q")];
        let (mut doc, page_id) = make_minimal_doc(ops);
        assert!(
            PdfColorConverter::new(&mut doc, fogra39_to_gracol())
                .convert_page(page_id)
                .is_ok()
        );
    }

    #[test]
    fn convert_page_k_ops_remain_k_in_decoded_output() {
        let ops = vec![cmyk_op("k", 0.75, 0.68, 0.67, 0.90)];
        let (mut doc, page_id) = make_minimal_doc(ops);
        PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_page(page_id)
            .unwrap();
        let out_ops = decoded_ops(&doc, page_id);
        assert!(
            out_ops.iter().any(|o| o.operator == "k"),
            "expected a 'k' operator after CMYK→CMYK conversion"
        );
    }

    #[test]
    fn convert_page_non_color_ops_survive_conversion() {
        let ops = vec![bare_op("q"), cmyk_op("k", 0.0, 0.0, 0.0, 1.0), bare_op("Q")];
        let (mut doc, page_id) = make_minimal_doc(ops);
        PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_page(page_id)
            .unwrap();
        let out_ops = decoded_ops(&doc, page_id);
        assert!(
            out_ops.iter().any(|o| o.operator == "q"),
            "'q' must survive"
        );
        assert!(
            out_ops.iter().any(|o| o.operator == "Q"),
            "'Q' must survive"
        );
    }

    #[test]
    fn convert_page_empty_content_stream_is_ok() {
        let (mut doc, page_id) = make_minimal_doc(vec![]);
        assert!(
            PdfColorConverter::new(&mut doc, fogra39_to_gracol())
                .convert_page(page_id)
                .is_ok()
        );
    }

    #[test]
    fn convert_page_output_is_re_decodable() {
        let ops = vec![
            bare_op("q"),
            cmyk_op("k", 0.5, 0.3, 0.2, 0.0),
            cmyk_op("K", 0.0, 0.0, 0.0, 1.0),
            bare_op("Q"),
        ];
        let (mut doc, page_id) = make_minimal_doc(ops);
        PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_page(page_id)
            .unwrap();
        assert!(
            doc.get_and_decode_page_content(page_id).is_ok(),
            "converted content stream must re-decode without error"
        );
    }

    #[test]
    fn convert_page_all_k_output_operands_in_valid_range() {
        let ops = vec![cmyk_op("k", 0.5, 0.3, 0.2, 0.0)];
        let (mut doc, page_id) = make_minimal_doc(ops);
        PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_page(page_id)
            .unwrap();
        let out_ops = decoded_ops(&doc, page_id);
        for op in out_ops.iter().filter(|o| o.operator == "k") {
            for i in 0..op.operands.len() {
                let v = operand_f32(op, i);
                assert!(
                    (0.0..=1.0).contains(&v),
                    "output k operand {i} = {v} is outside 0.0..=1.0"
                );
            }
        }
    }

    // ── convert_document ──────────────────────────────────────────────────────

    #[test]
    fn convert_document_counts_one_page() {
        let (mut doc, _) = make_full_doc(vec![cmyk_op("k", 0.0, 0.0, 0.0, 1.0)]);
        let report = PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_document()
            .unwrap();
        assert_eq!(report.pages_processed, 1);
    }

    #[test]
    fn convert_document_returns_ok_on_valid_doc() {
        let (mut doc, _) = make_full_doc(vec![bare_op("q"), bare_op("Q")]);
        assert!(
            PdfColorConverter::new(&mut doc, fogra39_to_gracol())
                .convert_document()
                .is_ok()
        );
    }

    #[test]
    fn convert_document_images_converted_is_zero_for_plain_doc() {
        // No image XObjects in this test doc — image conversion is Phase 4c.
        let (mut doc, _) = make_full_doc(vec![cmyk_op("k", 0.0, 0.0, 0.0, 1.0)]);
        let report = PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_document()
            .unwrap();
        assert_eq!(report.images_converted, 0);
    }

    // ── test helpers (Phase 5) ────────────────────────────────────────────────

    fn spot_name_op(operator: &str, name: &str) -> Operation {
        Operation {
            operator: operator.to_string(),
            operands: vec![Object::Name(name.as_bytes().to_vec())],
        }
    }

    fn tint_op(operator: &str, tint: f32) -> Operation {
        Operation {
            operator: operator.to_string(),
            operands: vec![Object::Real(tint)],
        }
    }

    fn spot_set(names: &[&str]) -> std::collections::HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    /// Builds a document with a Separation color space (DeviceCMYK alternate, Type 2
    /// tint function: C0=[0,0,0,0], C1=[1,0,0,0]) registered under `spot_alias`.
    fn make_spot_doc(ops: Vec<Operation>, spot_alias: &str) -> (lopdf::Document, lopdf::ObjectId) {
        let tint_fn = dictionary! {
            "FunctionType" => 2_i64,
            "Domain" => vec![Object::Real(0.0_f32), Object::Real(1.0_f32)],
            "C0" => vec![
                Object::Real(0.0_f32), Object::Real(0.0_f32),
                Object::Real(0.0_f32), Object::Real(0.0_f32),
            ],
            "C1" => vec![
                Object::Real(1.0_f32), Object::Real(0.0_f32),
                Object::Real(0.0_f32), Object::Real(0.0_f32),
            ],
            "N" => 1_i64,
        };
        let separation_cs = vec![
            Object::Name(b"Separation".to_vec()),
            Object::Name(b"PANTONE485C".to_vec()),
            Object::Name(b"DeviceCMYK".to_vec()),
            Object::Dictionary(tint_fn),
        ];
        let mut cs_dict = Dictionary::new();
        cs_dict.set(spot_alias.as_bytes(), Object::Array(separation_cs));
        let res_dict = dictionary! {
            "ColorSpace" => Object::Dictionary(cs_dict),
        };
        let bytes = Content { operations: ops }.encode().unwrap();
        let mut doc = lopdf::Document::with_version("1.5");
        let stream_id = doc.add_object(Stream::new(Dictionary::new(), bytes));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Resources" => Object::Dictionary(res_dict),
            "Contents" => Object::Reference(stream_id),
        });
        (doc, page_id)
    }

    // ── flatten_spot_ops ──────────────────────────────────────────────────────

    #[test]
    fn flatten_spot_empty_ops_returns_empty_and_zero() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let (out, count) = flatten_spot_ops(&[], &spot_set(&["CS1"]), &doc, page_id);
        assert!(out.is_empty());
        assert_eq!(count, 0);
    }

    #[test]
    fn flatten_spot_cs_for_non_spot_alias_passes_through() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("cs", "DeviceCMYK")];
        let (out, count) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].operator, "cs");
        assert_eq!(count, 0);
    }

    #[test]
    fn flatten_spot_cs_for_spot_alias_is_dropped() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("cs", "CS1"), tint_op("scn", 0.5)];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert!(
            out.iter().all(|o| o.operator != "cs"),
            "cs op for spot alias must be dropped"
        );
    }

    #[test]
    fn flatten_spot_scn_after_spot_cs_becomes_k_op() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("cs", "CS1"), tint_op("scn", 0.5)];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert!(
            out.iter().any(|o| o.operator == "k"),
            "scn after spot cs must become k"
        );
    }

    #[test]
    fn flatten_spot_k_op_from_scn_has_four_operands() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("cs", "CS1"), tint_op("scn", 0.75)];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        let k_op = out.iter().find(|o| o.operator == "k").unwrap();
        assert_eq!(k_op.operands.len(), 4);
    }

    #[test]
    fn flatten_spot_scn_increments_count() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("cs", "CS1"), tint_op("scn", 0.5)];
        let (_, count) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert_eq!(count, 1);
    }

    #[test]
    fn flatten_spot_two_scn_uses_count_is_two() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![
            spot_name_op("cs", "CS1"),
            tint_op("scn", 0.5),
            tint_op("scn", 1.0),
        ];
        let (_, count) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert_eq!(count, 2);
    }

    #[test]
    fn flatten_spot_scn_without_prior_spot_cs_passes_through() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![tint_op("scn", 0.5)];
        let (out, count) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].operator, "scn");
        assert_eq!(count, 0);
    }

    #[test]
    fn flatten_spot_device_ops_after_spot_section_pass_through() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![
            spot_name_op("cs", "CS1"),
            tint_op("scn", 0.5),
            cmyk_op("k", 0.0, 0.0, 0.0, 1.0),
        ];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        let k_ops: Vec<_> = out.iter().filter(|o| o.operator == "k").collect();
        // both the flattened spot and the original device op produce a k
        assert_eq!(k_ops.len(), 2);
    }

    #[test]
    fn flatten_spot_uppercase_cs_for_spot_is_dropped() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("CS", "CS1"), tint_op("SCN", 0.5)];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert!(
            out.iter().all(|o| o.operator != "CS"),
            "CS op for spot alias must be dropped"
        );
    }

    #[test]
    fn flatten_spot_scn_stroke_emits_uppercase_k() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("CS", "CS1"), tint_op("SCN", 0.5)];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert!(
            out.iter().any(|o| o.operator == "K"),
            "SCN stroke must become K (stroke), not k (fill)"
        );
    }

    // InDesign uses sc/SC (not scn/SCN) for Separation tint operators.

    #[test]
    fn flatten_spot_sc_after_spot_cs_becomes_k_op() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("cs", "CS1"), tint_op("sc", 0.5)];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert!(
            out.iter().any(|o| o.operator == "k"),
            "sc after spot cs must become k"
        );
    }

    #[test]
    fn flatten_spot_sc_increments_count() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("cs", "CS1"), tint_op("sc", 0.8)];
        let (_, count) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert_eq!(count, 1);
    }

    #[test]
    fn flatten_spot_uppercase_sc_after_spot_cs_becomes_uppercase_k() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![spot_name_op("CS", "CS1"), tint_op("SC", 0.5)];
        let (out, _) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert!(
            out.iter().any(|o| o.operator == "K"),
            "SC after spot CS must become K"
        );
    }

    #[test]
    fn flatten_spot_sc_without_prior_spot_cs_passes_through() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let ops = vec![tint_op("sc", 0.5)];
        let (out, count) = flatten_spot_ops(&ops, &spot_set(&["CS1"]), &doc, page_id);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].operator, "sc");
        assert_eq!(count, 0);
    }

    // ── spot_tint_to_cmyk ─────────────────────────────────────────────────────

    #[test]
    fn spot_tint_fallback_when_no_resources() {
        let (doc, page_id) = make_minimal_doc(vec![]);
        let result = spot_tint_to_cmyk("CS1", 0.8, &doc, page_id);
        assert_eq!(result, [0.0, 0.0, 0.0, 0.8]);
    }

    #[test]
    fn spot_tint_unknown_alias_returns_fallback() {
        let (doc, page_id) = make_spot_doc(vec![], "CS1");
        let result = spot_tint_to_cmyk("UNKNOWN", 0.6, &doc, page_id);
        assert_eq!(result, [0.0, 0.0, 0.0, 0.6]);
    }

    #[test]
    fn spot_tint_zero_tint_returns_c0() {
        // C0 = [0, 0, 0, 0]
        let (doc, page_id) = make_spot_doc(vec![], "CS1");
        let result = spot_tint_to_cmyk("CS1", 0.0, &doc, page_id);
        assert_eq!(result, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn spot_tint_full_tint_returns_c1() {
        // C1 = [1, 0, 0, 0]
        let (doc, page_id) = make_spot_doc(vec![], "CS1");
        let result = spot_tint_to_cmyk("CS1", 1.0, &doc, page_id);
        assert!((result[0] - 1.0).abs() < 1e-5, "C = {}", result[0]);
        assert!(result[1].abs() < 1e-5);
        assert!(result[2].abs() < 1e-5);
        assert!(result[3].abs() < 1e-5);
    }

    #[test]
    fn spot_tint_half_tint_interpolates_linearly() {
        // C0=[0,0,0,0], C1=[1,0,0,0], tint=0.5 → [0.5, 0, 0, 0]
        let (doc, page_id) = make_spot_doc(vec![], "CS1");
        let result = spot_tint_to_cmyk("CS1", 0.5, &doc, page_id);
        assert!((result[0] - 0.5).abs() < 1e-5, "C = {}", result[0]);
        assert!(result[1].abs() < 1e-5);
        assert!(result[2].abs() < 1e-5);
        assert!(result[3].abs() < 1e-5);
    }

    // ── convert_page / convert_document with spots ────────────────────────────

    #[test]
    fn convert_page_returns_zero_spot_count_for_plain_cmyk_content() {
        let ops = vec![cmyk_op("k", 0.0, 0.0, 0.0, 1.0)];
        let (mut doc, page_id) = make_minimal_doc(ops);
        let count = PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_page(page_id)
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn convert_document_spot_colors_flattened_is_zero_for_plain_doc() {
        let (mut doc, _) = make_full_doc(vec![cmyk_op("k", 0.0, 0.0, 0.0, 1.0)]);
        let report = PdfColorConverter::new(&mut doc, fogra39_to_gracol())
            .convert_document()
            .unwrap();
        assert_eq!(report.spot_colors_flattened, 0);
    }
}
