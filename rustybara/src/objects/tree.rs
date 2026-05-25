//! Object tree construction from PDF content streams.
//!
//! Parses the PDF operator sequence for a single page and records every
//! painted element as a [`PageObject`]. All geometry is in **PDF page spec**
//! (origin bottom-left, Y increases upward, unit in points).

use crate::geometry::{Matrix, Rect};
use crate::pages::boxes::object_to_f64;
use lopdf::{Document, Object, ObjectId};

/// Per-object device paint color.
///
/// This is a concrete sampled value on a single object and is distinct from
/// [`crate::pipeline::DocumentColorKind`], which classifies the document as a whole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PdfColor {
    DeviceGray(f64),
    DeviceRgb(f64, f64, f64),
    DeviceCmyk(f64, f64, f64, f64),
}

/// A single path segment stored in local (pre-CTM) coordinates.
#[derive(Clone, Copy, Debug)]
pub enum PathPoint {
    /// `m` — begin a new subpath at `(x, y)`.
    MoveTo(f64, f64),
    /// `l` — append a straight line to `(x, y)`.
    LineTo(f64, f64),
    /// `c` — cubic Bézier: `(x1, y1)` first control, `(x2, y2)` second control, `(x3, y3)`
    /// endpoint
    CurveTo(f64, f64, f64, f64, f64, f64),
    /// `h` — close the subpath with a straight line back to its starting point.
    Close,
}

/// One open or closed subpath within a path object, in local (pre-CTM) coordinates.
#[derive(Clone, Debug, Default)]
pub struct SubPath {
    pub points: Vec<PathPoint>,
}

/// How a [`PageObject`] was rendered.
#[derive(Clone, Debug)]
pub enum ObjectKind {
    Fill,
    Stroke,
    FillStroke,
    Text(String),
    Image,
    FormXObject,
}

/// A single logical painted object on a PDF page.
///
/// All geometry fields are in **PDF page space** (origin bottom-left, Y-up, units in points).
/// `subpaths` are stored in *local* (pre-CTM) coordinates so that callers can apply
/// their own transforms or perform exact geometric tests.
#[derive(Clone, Debug)]
pub struct PageObject {
    /// Axis-aligned bounding box in page space.
    pub bbox: Rect,
    /// Current transformation matrix when this object was painted.
    pub ctm: Matrix,
    pub kind: ObjectKind,
    /// Fill color at paint time. Set for [`ObjectKind::Fill`], [`ObjectKind::FillStroke],
    /// and [`ObjectKind::Text`]; `None` for pure strokes, images, and form XObjects.
    pub fill_color: Option<PdfColor>,
    /// Stroke color at paint time. Set for [`ObjectKind::Stroke`] and [`ObjectKind::FillStroke`];
    /// `None` otherwise.
    pub stroke_color: Option<PdfColor>,
    /// Line width at paint time (in user units, before CTM scaling).
    pub stroke_width: f64,
    /// Subpaths in local (pre-CTM) coordinates.
    /// Empty for [`ObjectKind::Text`], [`ObjectKind::Image`], and [`ObjectKind::FormXObject`].
    pub subpaths: Vec<SubPath>,
}

/// All painted objects on a page, in back-to-front paint order.
pub struct ObjectTree {
    pub objects: Vec<PageObject>,
}

#[derive(Clone)]
struct GraphicsState {
    ctm: Matrix,
    fill_color: PdfColor,
    stroke_color: PdfColor,
    stroke_width: f64,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: Matrix::identity(),
            fill_color: PdfColor::DeviceGray(0.0),
            stroke_color: PdfColor::DeviceGray(0.0),
            stroke_width: 1.0,
        }
    }
}

/// Parse the content stream of `page_id` and return all painted objects in back to front order.
///
/// # Operator coverage
///
/// | Operator(s)           | Effect                                   |
/// |-----------------------|------------------------------------------|
/// | `q` / `Q`             | Graphics state push / pop                |
/// | `cm`                  | CTM concatenation                        |
/// | `w`                   | Stroke line width                        |
/// | `g`/`G`               | DeviceGray fill / stroke                 |
/// | `rg`/`RG`             | DeviceRGB fill / stroke                  |
/// | `k`/`K`               | DeviceCMYK fill / stroke                 |
/// | `m` `l` `c` `v` `y` `h` `re` | Path construction               |
/// | `S` `s` `f` `f*` `F` `B` `B*` `b` `b*` `n` | Path painting  |
/// | `Do`                  | Image / Form XObject placement           |
/// | `BT` … `ET`           | Text block (one object per block)        |
///
/// Unknown operators and clipping paths (`W`/`W*`) are silently skipped.
pub fn build_object_tree(doc: &Document, page_id: ObjectId) -> crate::Result<ObjectTree> {
    let content = doc.get_and_decode_page_content(page_id)?;
    let mut objects: Vec<PageObject> = Vec::new();

    let mut gs_stack: Vec<GraphicsState> = Vec::new();
    let mut gs = GraphicsState::default();

    let mut subpaths: Vec<SubPath> = Vec::new();
    let mut current_sub = SubPath::default();

    let mut in_text = false;
    let mut text_buf = String::new();
    let mut text_origin: Option<Matrix> = None;
    let mut tm = Matrix::identity(); // text matrix
    let mut lm = Matrix::identity(); // line matrix
    let mut font_size: f64 = 12.0;
    let mut leading: f64 = 0.0;

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => gs_stack.push(gs.clone()),
            "Q" => {
                if let Some(prev) = gs_stack.pop() {
                    gs = prev;
                }
                // Note: the current path is NOT part of the graphics state in the
                // PDF spec and is intentionally left intact across q/Q.
            }
            "cm" if op.operands.len() >= 6 => {
                let m = ops_to_matrix(&op.operands);
                gs.ctm = gs.ctm.concat(&m);
            }
            "w" if !op.operands.is_empty() => {
                gs.stroke_width = object_to_f64(&op.operands[0]);
            }
            "G" if !op.operands.is_empty() => {
                gs.stroke_color = PdfColor::DeviceGray(object_to_f64(&op.operands[0]));
            }
            "RG" if op.operands.len() >= 3 => {
                gs.stroke_color = PdfColor::DeviceRgb(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                    object_to_f64(&op.operands[2]),
                )
            }
            "K" if op.operands.len() >= 4 => {
                gs.stroke_color = PdfColor::DeviceCmyk(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                    object_to_f64(&op.operands[2]),
                    object_to_f64(&op.operands[3]),
                );
            }
            "g" if !op.operands.is_empty() => {
                gs.fill_color = PdfColor::DeviceGray(object_to_f64(&op.operands[0]));
            }
            "rg" if op.operands.len() >= 3 => {
                gs.fill_color = PdfColor::DeviceRgb(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                    object_to_f64(&op.operands[2]),
                );
            }
            "k" if op.operands.len() >= 4 => {
                gs.fill_color = PdfColor::DeviceCmyk(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                    object_to_f64(&op.operands[2]),
                    object_to_f64(&op.operands[3]),
                );
            }
            "m" if op.operands.len() >= 2 => {
                if !current_sub.points.is_empty() {
                    subpaths.push(std::mem::take(&mut current_sub));
                }
                current_sub.points.push(PathPoint::MoveTo(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                ));
            }
            "l" if op.operands.len() >= 2 => {
                current_sub.points.push(PathPoint::LineTo(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                ));
            }
            "c" if op.operands.len() >= 6 => {
                current_sub.points.push(PathPoint::CurveTo(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                    object_to_f64(&op.operands[2]),
                    object_to_f64(&op.operands[3]),
                    object_to_f64(&op.operands[4]),
                    object_to_f64(&op.operands[5]),
                ));
            }
            // `v`: current point is implicit first control; operands = x2 y2 x3 y3.
            // Both control points are set to (x2, y2) as a conservative bbox approximation.
            "v" if op.operands.len() >= 4 => {
                let x2 = object_to_f64(&op.operands[0]);
                let y2 = object_to_f64(&op.operands[1]);
                let x3 = object_to_f64(&op.operands[2]);
                let y3 = object_to_f64(&op.operands[3]);
                current_sub
                    .points
                    .push(PathPoint::CurveTo(x2, y2, x2, y2, x3, y3));
            }
            // `y`: operands = x1 y1 x3 y3 (x3, y3 is both the second control point and endpoint).
            "y" if op.operands.len() >= 4 => {
                let x1 = object_to_f64(&op.operands[0]);
                let y1 = object_to_f64(&op.operands[1]);
                let x3 = object_to_f64(&op.operands[2]);
                let y3 = object_to_f64(&op.operands[3]);
                current_sub
                    .points
                    .push(PathPoint::CurveTo(x1, y1, x3, y3, x3, y3));
            }
            "h" => {
                current_sub.points.push(PathPoint::Close);
            }
            "re" if op.operands.len() >= 4 => {
                if !current_sub.points.is_empty() {
                    subpaths.push(std::mem::take(&mut current_sub));
                }
                let x = object_to_f64(&op.operands[0]);
                let y = object_to_f64(&op.operands[1]);
                let w = object_to_f64(&op.operands[2]);
                let h = object_to_f64(&op.operands[3]);
                current_sub.points.push(PathPoint::MoveTo(x, y));
                current_sub.points.push(PathPoint::LineTo(x + w, y));
                current_sub.points.push(PathPoint::LineTo(x + w, y + h));
                current_sub.points.push(PathPoint::LineTo(x, y + h));
                current_sub.points.push(PathPoint::Close);
            }
            "S" => {
                commit_paint(
                    &mut objects,
                    &mut subpaths,
                    &mut current_sub,
                    &gs,
                    ObjectKind::Stroke,
                );
            }
            "s" => {
                // close-and-stroke (equivalent to h S)
                current_sub.points.push(PathPoint::Close);
                commit_paint(
                    &mut objects,
                    &mut subpaths,
                    &mut current_sub,
                    &gs,
                    ObjectKind::Stroke,
                );
            }
            "f" | "f*" | "F" => {
                commit_paint(
                    &mut objects,
                    &mut subpaths,
                    &mut current_sub,
                    &gs,
                    ObjectKind::Fill,
                );
            }
            "B" | "B*" => {
                commit_paint(
                    &mut objects,
                    &mut subpaths,
                    &mut current_sub,
                    &gs,
                    ObjectKind::FillStroke,
                );
            }
            "b" | "b*" => {
                // close-and-fill + stroke
                current_sub.points.push(PathPoint::Close);
                commit_paint(
                    &mut objects,
                    &mut subpaths,
                    &mut current_sub,
                    &gs,
                    ObjectKind::FillStroke,
                );
            }
            "n" => {
                // Discard path (e.g., used after W/W* clipping paths)
                subpaths.clear();
                current_sub = SubPath::default();
            }
            "Do" if !op.operands.is_empty() => {
                let kind = xobject_kind(doc, page_id, &op.operands[0]);
                let local_bbox = match &kind {
                    ObjectKind::FormXObject => {
                        // Form XObjects define their extent via /BBox in their stream dict.
                        // Falling back to the unit square produces a 1x1 bbox – wrong.
                        read_form_bbox(doc, page_id, &op.operands[0]).unwrap_or(Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 1.0,
                            height: 1.0,
                        })
                    }
                    // Image XObjects are defined on [0,1]^2 by PDF spec – unit square is correct.
                    _ => Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                };
                let bbox = gs.ctm.transform_rect(&local_bbox);
                objects.push(PageObject {
                    bbox,
                    ctm: gs.ctm,
                    kind,
                    fill_color: None,
                    stroke_color: None,
                    stroke_width: 0.0,
                    subpaths: vec![],
                });
            }
            "BT" => {
                in_text = true;
                text_buf.clear();
                text_origin = None;
                tm = Matrix::identity();
                lm = Matrix::identity();
                // Note: font_size and leading intentionally persist across BT/ET pairs.
                // PDF spec: BT resets only the text matrix (Tm/Tlm), not the text state.
                leading = 0.0;
            }
            "ET" => {
                if in_text && !text_buf.is_empty() {
                    let origin = text_origin.unwrap_or(tm);
                    let char_count = text_buf.chars().count() as f64;

                    // Estimate text extent in text space.
                    // 0.5 em per character is a reasonable average for proportional fonts.
                    let text_w = char_count * 0.5 * font_size;
                    // Standard ascent/descent proportions relative to the em square.
                    let ascent = 0.8 * font_size;
                    let descent = -0.2 * font_size;

                    // Compute axis-aligned bounding box by transforming all four corners
                    // of the text rectangle through the text matrix (origin) then the CTM.
                    // This correctly handles scaled, rotated, and sheared text matrices.
                    let corners: [(f64, f64); 4] = [
                        (0.0, descent),
                        (text_w, descent),
                        (0.0, ascent),
                        (text_w, ascent),
                    ];
                    let mut px_arr = [0.0_f64; 4];
                    let mut py_arr = [0.0_f64; 4];
                    for (i, &(lx, ly)) in corners.iter().enumerate() {
                        let tx = origin.a * lx + origin.c * ly + origin.e;
                        let ty = origin.b * lx + origin.d * ly + origin.f;
                        let (cx, cy) = gs.ctm.transform_point(tx, ty);
                        px_arr[i] = cx;
                        py_arr[i] = cy;
                    }
                    let min_x = px_arr.iter().copied().fold(f64::INFINITY, f64::min);
                    let max_x = px_arr.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    let min_y = py_arr.iter().copied().fold(f64::INFINITY, f64::min);
                    let max_y = py_arr.iter().copied().fold(f64::NEG_INFINITY, f64::max);

                    objects.push(PageObject {
                        bbox: Rect {
                            x: min_x,
                            y: min_y,
                            width: (max_x - min_x).max(0.1),
                            height: (max_y - min_y).max(0.1),
                        },
                        ctm: gs.ctm,
                        kind: ObjectKind::Text(std::mem::take(&mut text_buf)),
                        fill_color: Some(gs.fill_color),
                        stroke_color: None,
                        stroke_width: 0.0,
                        subpaths: vec![],
                    });
                }
                in_text = false;
                text_buf.clear();
                text_origin = None;
            }
            "Tf" if in_text && op.operands.len() >= 2 => {
                font_size = object_to_f64(&op.operands[1]).abs().max(0.1);
            }
            "TL" if in_text && !op.operands.is_empty() => {
                leading = object_to_f64(&op.operands[0]);
            }
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
                // Apply the offset in the current line matrix's coordinate system.
                // PDF spec: Tlm = [1 0 0 1 tx ty] × Tlm
                // Equivalent to: new translation = lm.transform_point(tx, ty)
                let (new_e, new_f) = lm.transform_point(tx, ty);
                lm = Matrix::from_values(lm.a, lm.b, lm.c, lm.d, new_e, new_f);
                tm = lm;
            }
            "T*" if in_text => {
                let (new_e, new_f) = lm.transform_point(0.0, -leading);
                lm = Matrix::from_values(lm.a, lm.b, lm.c, lm.d, new_e, new_f);
                tm = lm;
            }
            "Tj" | "'" if in_text && !op.operands.is_empty() => {
                if let Object::String(bytes, _) = &op.operands[0] {
                    if text_origin.is_none() {
                        text_origin = Some(tm);
                    }
                    text_buf.push_str(&loss_bytes(bytes));
                }
            }
            "\"" if in_text && op.operands.len() >= 3 => {
                if let Object::String(bytes, _) = &op.operands[2] {
                    if text_origin.is_none() {
                        text_origin = Some(tm);
                    }
                    text_buf.push_str(&loss_bytes(bytes));
                }
            }
            "TJ" if in_text && !op.operands.is_empty() => {
                if let Object::Array(arr) = &op.operands[0] {
                    for item in arr {
                        if let Object::String(bytes, _) = item {
                            if text_origin.is_none() {
                                text_origin = Some(tm);
                            }
                            text_buf.push_str(&loss_bytes(bytes));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(ObjectTree { objects })
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Convert six operands to a [`Matrix`].
///
/// Mirrors `operands_to_matrix` from `stream::filter`, which is private to that module.
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

/// Flush `current_sub` and all accumulated `subpaths` into `objects` as a single
/// painted object, then clear both buffers. Does nothing if all buffers are emtpy.
fn commit_paint(
    objects: &mut Vec<PageObject>,
    subpaths: &mut Vec<SubPath>,
    current_sub: &mut SubPath,
    gs: &GraphicsState,
    kind: ObjectKind,
) {
    if !current_sub.points.is_empty() {
        subpaths.push(std::mem::take(current_sub));
    }
    if subpaths.is_empty() {
        return;
    }
    let bbox = path_bbox(subpaths, &gs.ctm);
    let (fill_color, stroke_color) = match &kind {
        ObjectKind::Fill => (Some(gs.fill_color), None),
        ObjectKind::Stroke => (None, Some(gs.stroke_color)),
        ObjectKind::FillStroke => (Some(gs.fill_color), Some(gs.stroke_color)),
        _ => (None, None),
    };
    objects.push(PageObject {
        bbox,
        ctm: gs.ctm,
        kind,
        fill_color,
        stroke_color,
        stroke_width: gs.stroke_width,
        subpaths: std::mem::take(subpaths),
    });
}

/// Compute the axis-aligned bounding box of all subpaths points transformed by `ctm`.
///
/// For cubic Bézier segments the control points are included in the bbox –
/// this is conservative but avoids the cost of curve falttening.
fn path_bbox(subpaths: &[SubPath], ctm: &Matrix) -> Rect {
    let mut xmin = f64::INFINITY;
    let mut xmax = f64::NEG_INFINITY;
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;

    for sub in subpaths {
        for pt in &sub.points {
            match *pt {
                PathPoint::MoveTo(x, y) | PathPoint::LineTo(x, y) => {
                    let (px, py) = ctm.transform_point(x, y);
                    xmin = xmin.min(px);
                    xmax = xmax.max(px);
                    ymin = ymin.min(py);
                    ymax = ymax.max(py);
                }
                PathPoint::CurveTo(x1, y1, x2, y2, x3, y3) => {
                    for (x, y) in [(x1, y1), (x2, y2), (x3, y3)] {
                        let (px, py) = ctm.transform_point(x, y);
                        xmin = xmin.min(px);
                        xmax = xmax.max(px);
                        ymin = ymin.min(py);
                        ymax = ymax.max(py);
                    }
                }
                PathPoint::Close => {}
            }
        }
    }

    if xmin.is_finite() {
        Rect::from_corners(xmin, ymin, xmax, ymax)
    } else {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

fn loss_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Classify a named XObject as [`ObjectKind::Image`] or [`ObjectKind::FormXObject`].
///
/// Navigates: page dict -> Resources -> XObject -> name -> stream Subtype.
/// Falls back to [`ObjectKind::Image`] on any access failure.
fn xobject_kind(doc: &Document, page_id: ObjectId, name_obj: &Object) -> ObjectKind {
    let name = match name_obj {
        Object::Name(n) => n.as_slice(),
        _ => return ObjectKind::Image,
    };

    let subtype: Option<Vec<u8>> = (|| {
        let page_obj = doc.get_object(page_id).ok()?;
        let page_dict = page_obj.as_dict().ok()?;
        let res_val = page_dict.get(b"Resources").ok()?;
        let res_obj = deref(doc, res_val);
        let res_dict = res_obj.as_dict().ok()?;
        let xo_val = res_dict.get(b"XObject").ok()?;
        let xo_obj = deref(doc, xo_val);
        let xo_dict = xo_obj.as_dict().ok()?;
        let xref = xo_dict.get(name).ok()?;
        let xobj_id = if let Object::Reference(id) = xref {
            *id
        } else {
            return None;
        };
        let xobj = doc.get_object(xobj_id).ok()?;
        let stream = if let Object::Stream(s) = xobj {
            s
        } else {
            return None;
        };
        match stream.dict.get(b"Subtype").ok()? {
            Object::Name(n) => Some(n.clone()),
            _ => None,
        }
    })();

    match subtype.as_deref() {
        Some(n) if n == b"Image" => ObjectKind::Image,
        Some(n) if n == b"Form" => ObjectKind::FormXObject,
        _ => ObjectKind::Image,
    }
}

/// Dereference a single indirect object; return the object unchanged if it is not a reference.
pub fn deref<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

pub fn ref_id(obj: &Object) -> Option<ObjectId> {
    if let Object::Reference(id) = obj {
        Some(*id)
    } else {
        None
    }
}

fn read_form_bbox(doc: &Document, page_id: ObjectId, name_obj: &Object) -> Option<Rect> {
    let name = match name_obj {
        Object::Name(n) => n.as_slice(),
        _ => return None,
    };
    let page_obj = doc.get_object(page_id).ok()?;
    let page_dict = page_obj.as_dict().ok()?;
    let res_val = page_dict.get(b"Resources").ok()?;
    let res_obj = deref(doc, res_val);
    let res_dict = res_obj.as_dict().ok()?;
    let xo_val = res_dict.get(b"XObject").ok()?;
    let xo_obj = deref(doc, xo_val);
    let xo_dict = xo_obj.as_dict().ok()?;
    let xref = xo_dict.get(name).ok()?;
    let xobj_id = if let Object::Reference(id) = xref {
        *id
    } else {
        return None;
    };
    let xobj = doc.get_object(xobj_id).ok()?;
    let stream = if let Object::Stream(s) = xobj {
        s
    } else {
        return None;
    };
    let bbox_arr = stream.dict.get(b"BBox").ok()?.as_array().ok()?;
    if bbox_arr.len() < 4 {
        return None;
    }
    Some(Rect::from_corners(
        object_to_f64(&bbox_arr[0]),
        object_to_f64(&bbox_arr[1]),
        object_to_f64(&bbox_arr[2]),
        object_to_f64(&bbox_arr[3]),
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── PdfColor ─────────────────────────────────────────────────────────────

    #[test]
    fn pdf_color_eq_gray() {
        assert_eq!(PdfColor::DeviceGray(0.5), PdfColor::DeviceGray(0.5));
        assert_ne!(PdfColor::DeviceGray(0.0), PdfColor::DeviceGray(1.0));
    }

    #[test]
    fn pdf_color_eq_rgb() {
        assert_eq!(
            PdfColor::DeviceRgb(1.0, 0.0, 0.5),
            PdfColor::DeviceRgb(1.0, 0.0, 0.5)
        );
    }

    #[test]
    fn pdf_color_eq_cmyk() {
        assert_eq!(
            PdfColor::DeviceCmyk(0.0, 0.0, 0.0, 1.0),
            PdfColor::DeviceCmyk(0.0, 0.0, 0.0, 1.0),
        );
    }

    // ── path_bbox ─────────────────────────────────────────────────────────────

    #[test]
    fn path_bbox_empty_returns_zero() {
        let r = path_bbox(&[], &Matrix::identity());
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.width, 0.0);
        assert_eq!(r.height, 0.0);
    }

    #[test]
    fn path_bbox_axis_aligned_rect_identity_ctm() {
        let mut sub = SubPath::default();
        sub.points.push(PathPoint::MoveTo(10.0, 20.0));
        sub.points.push(PathPoint::LineTo(50.0, 20.0));
        sub.points.push(PathPoint::LineTo(50.0, 60.0));
        sub.points.push(PathPoint::LineTo(10.0, 60.0));
        sub.points.push(PathPoint::Close);
        let r = path_bbox(&[sub], &Matrix::identity());
        assert!((r.x - 10.0).abs() < 0.01, "x={}", r.x);
        assert!((r.y - 20.0).abs() < 0.01, "y={}", r.y);
        assert!((r.width - 40.0).abs() < 0.01, "w={}", r.width);
        assert!((r.height - 40.0).abs() < 0.01, "h={}", r.height);
    }

    #[test]
    fn path_bbox_translate_ctm() {
        let mut sub = SubPath::default();
        sub.points.push(PathPoint::MoveTo(0.0, 0.0));
        sub.points.push(PathPoint::LineTo(10.0, 10.0));
        let ctm = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 100.0, 200.0);
        let r = path_bbox(&[sub], &ctm);
        assert!((r.x - 100.0).abs() < 0.01);
        assert!((r.y - 200.0).abs() < 0.01);
        assert!((r.width - 10.0).abs() < 0.01);
        assert!((r.height - 10.0).abs() < 0.01);
    }

    #[test]
    fn path_bbox_curve_includes_control_points() {
        let mut sub = SubPath::default();
        sub.points.push(PathPoint::MoveTo(0.0, 0.0));
        // Control points at (100, 0) and (100, 100); endpoint at (50, 50)
        sub.points
            .push(PathPoint::CurveTo(100.0, 0.0, 100.0, 100.0, 50.0, 50.0));
        let r = path_bbox(&[sub], &Matrix::identity());
        assert!(
            r.width >= 100.0,
            "bbox must span control points: w={}",
            r.width
        );
        assert!(
            r.height >= 100.0,
            "bbox must span control points: h={}",
            r.height
        );
    }

    // ── ops_to_matrix ─────────────────────────────────────────────────────────

    #[test]
    fn ops_to_matrix_identity_passthrough() {
        let ops = vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ];
        let m = ops_to_matrix(&ops);
        let (x, y) = m.transform_point(3.0, 7.0);
        assert!((x - 3.0).abs() < 1e-10);
        assert!((y - 7.0).abs() < 1e-10);
    }

    #[test]
    fn ops_to_matrix_translation() {
        let ops = vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(50.0),
            Object::Real(75.0),
        ];
        let m = ops_to_matrix(&ops);
        let (x, y) = m.transform_point(0.0, 0.0);
        assert!((x - 50.0).abs() < 1e-10);
        assert!((y - 75.0).abs() < 1e-10);
    }

    // ── build_object_tree (integration, requires fixture) ─────────────────────

    fn fixture() -> Option<(lopdf::Document, lopdf::ObjectId)> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf_test_data_print_v2.pdf");
        if !path.exists() {
            return None;
        }
        let doc = lopdf::Document::load(&path).ok()?;
        let page_id = doc.get_pages()[&1];
        Some((doc, page_id))
    }

    #[test]
    fn build_object_tree_produces_objects() {
        let Some((doc, page_id)) = fixture() else {
            return;
        };
        let tree = build_object_tree(&doc, page_id).unwrap();
        assert!(
            !tree.objects.is_empty(),
            "expected at least one painted object"
        );
    }

    #[test]
    fn build_object_tree_all_bboxes_finite() {
        let Some((doc, page_id)) = fixture() else {
            return;
        };
        let tree = build_object_tree(&doc, page_id).unwrap();
        for (i, obj) in tree.objects.iter().enumerate() {
            assert!(obj.bbox.x.is_finite(), "object {i} bbox.x not finite");
            assert!(obj.bbox.y.is_finite(), "object {i} bbox.y not finite");
            assert!(
                obj.bbox.width.is_finite(),
                "object {i} bbox.width not finite"
            );
            assert!(
                obj.bbox.height.is_finite(),
                "object {i} bbox.height not finite"
            );
        }
    }

    #[test]
    fn build_object_tree_has_image_xobject() {
        let Some((doc, page_id)) = fixture() else {
            return;
        };
        let tree = build_object_tree(&doc, page_id).unwrap();
        let has_img = tree
            .objects
            .iter()
            .any(|o| matches!(o.kind, ObjectKind::Image));
        assert!(has_img, "expected at least one Image XObject");
    }

    #[test]
    fn build_object_tree_fill_objects_have_fill_color() {
        let Some((doc, page_id)) = fixture() else {
            return;
        };
        let tree = build_object_tree(&doc, page_id).unwrap();
        for obj in &tree.objects {
            if matches!(obj.kind, ObjectKind::Fill | ObjectKind::FillStroke) {
                assert!(
                    obj.fill_color.is_some(),
                    "fill/fillstroke object missing fill_color"
                );
            }
        }
    }
}
