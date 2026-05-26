//! Object tree construction from PDF content streams.
//!
//! Parses the PDF operator sequence for a single page and records every
//! painted element as a [`PageObject`]. All geometry is in **PDF page space**
//! (origin bottom-left, Y increases upward, unit in points).
//!
//! ## Form XObject recursion
//!
//! When the content stream invokes a Form XObject (`Do` with `/Form` subtype),
//! this module recurses into the form's own content stream rather than emitting
//! a single bbox placeholder. Every path, image, and text object inside the
//! form — at any nesting depth — appears as a first-class [`PageObject`] with
//! geometry already expressed in page space. Recursion is capped at
//! [`MAX_FORM_DEPTH`] levels; any form beyond that limit is represented by a
//! [`ObjectKind::FormXObject`] bbox placeholder so the footprint is not lost.

use crate::geometry::{Matrix, Rect};
use crate::pages::boxes::object_to_f64;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Maximum Form XObject nesting levels to expand before falling back to a
/// bbox placeholder. Real documents rarely exceed 3–4 levels; 8 prevents
/// infinite loops on malformed PDFs with circular XObject references without
/// sacrificing practical coverage.
const MAX_FORM_DEPTH: u8 = 8;

// ── Public types ──────────────────────────────────────────────────────────────

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
    /// endpoint.
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
    /// A Form XObject that could not be expanded — either because the recursion
    /// depth limit ([`MAX_FORM_DEPTH`]) was reached, or the stream could not be
    /// decoded. The `bbox` field reflects the form's declared `/BBox` transformed
    /// by the CTM at the call site.
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
    /// Fill color at paint time. Set for [`ObjectKind::Fill`], [`ObjectKind::FillStroke`],
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

// ── Graphics state ────────────────────────────────────────────────────────────

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

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse the content stream of `page_id` and return all painted objects in back-to-front order.
///
/// Form XObjects encountered in the content stream are expanded recursively up to
/// [`MAX_FORM_DEPTH`] nesting levels: every path, image, and text object inside a
/// placed form appears as a first-class [`PageObject`] with geometry already in page
/// space. Any form that exceeds the depth cap is emitted as a
/// [`ObjectKind::FormXObject`] bbox placeholder instead.
///
/// # Operator coverage
///
/// | Operator(s)                               | Effect                                |
/// |-------------------------------------------|---------------------------------------|
/// | `q` / `Q`                                 | Graphics state push / pop             |
/// | `cm`                                      | CTM concatenation                     |
/// | `w`                                       | Stroke line width                     |
/// | `g` / `G`                                 | DeviceGray fill / stroke              |
/// | `rg` / `RG`                               | DeviceRGB fill / stroke               |
/// | `k` / `K`                                 | DeviceCMYK fill / stroke              |
/// | `m` `l` `c` `v` `y` `h` `re`             | Path construction                     |
/// | `S` `s` `f` `f*` `F` `B` `B*` `b` `b*` `n` | Path painting                     |
/// | `Do` (Image)                              | Unit-square bbox placeholder          |
/// | `Do` (Form)                               | Recurse into form content stream      |
/// | `BT` … `ET`                               | Text block (one object per block)     |
///
/// Unknown operators and clipping paths (`W` / `W*`) are silently skipped.
pub fn build_object_tree(doc: &Document, page_id: ObjectId) -> crate::Result<ObjectTree> {
    let content = doc.get_and_decode_page_content(page_id)?;
    let mut objects = Vec::new();
    parse_content(
        doc,
        &content.operations,
        &GraphicsState::default(),
        page_id,
        &mut objects,
        MAX_FORM_DEPTH,
    );
    Ok(ObjectTree { objects })
}

// ── Core interpreter ──────────────────────────────────────────────────────────

/// Core content-stream interpreter shared by page streams and Form XObject streams.
///
/// Iterates `operations` in the coordinate system established by `initial_gs.ctm`,
/// appending every painted object to `objects` in back-to-front (paint) order.
///
/// **`resource_parent_id`** is the lopdf object ID whose `/Resources/XObject`
/// dictionary is consulted when a `Do` operator names an XObject. For a page stream
/// this is the page object ID; for a Form XObject stream it is the form's own object
/// ID, because each Form carries its own `/Resources` dictionary.
///
/// **`depth`** is the remaining Form XObject recursion budget. When it reaches zero,
/// any Form XObject encountered is recorded as a [`ObjectKind::FormXObject`] bbox
/// placeholder rather than being expanded. All other operators are processed normally
/// regardless of depth.
fn parse_content(
    doc: &Document,
    operations: &[lopdf::content::Operation],
    initial_gs: &GraphicsState,
    resource_parent_id: ObjectId,
    mut objects: &mut Vec<PageObject>,
    depth: u8,
) {
    let mut gs_stack: Vec<GraphicsState> = Vec::new();
    let mut gs = initial_gs.clone();

    let mut subpaths: Vec<SubPath> = Vec::new();
    let mut current_sub = SubPath::default();

    let mut in_text = false;
    let mut text_buf = String::new();
    let mut text_origin: Option<Matrix> = None;
    let mut tm = Matrix::identity(); // text matrix
    let mut lm = Matrix::identity(); // line matrix
    let mut font_size: f64 = 12.0;
    let mut leading: f64 = 0.0;

    for op in operations {
        match op.operator.as_str() {
            // ── Graphics state ─────────────────────────────────────────────────

            "q" => gs_stack.push(gs.clone()),
            "Q" => {
                if let Some(prev) = gs_stack.pop() {
                    gs = prev;
                }
                // The current path is NOT part of the graphics state per the PDF
                // spec and is intentionally left intact across q/Q.
            }
            "cm" if op.operands.len() >= 6 => {
                let m = ops_to_matrix(&op.operands);
                gs.ctm = gs.ctm.concat(&m);
            }
            "w" if !op.operands.is_empty() => {
                gs.stroke_width = object_to_f64(&op.operands[0]);
            }

            // ── Color operators ────────────────────────────────────────────────

            "G" if !op.operands.is_empty() => {
                gs.stroke_color = PdfColor::DeviceGray(object_to_f64(&op.operands[0]));
            }
            "RG" if op.operands.len() >= 3 => {
                gs.stroke_color = PdfColor::DeviceRgb(
                    object_to_f64(&op.operands[0]),
                    object_to_f64(&op.operands[1]),
                    object_to_f64(&op.operands[2]),
                );
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

            // ── Path construction ──────────────────────────────────────────────

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
            // `v`: the current point is the implicit first control point.
            // Operands are x2 y2 x3 y3; we duplicate (x2,y2) as both control
            // points — a conservative bbox approximation.
            "v" if op.operands.len() >= 4 => {
                let x2 = object_to_f64(&op.operands[0]);
                let y2 = object_to_f64(&op.operands[1]);
                let x3 = object_to_f64(&op.operands[2]);
                let y3 = object_to_f64(&op.operands[3]);
                current_sub
                    .points
                    .push(PathPoint::CurveTo(x2, y2, x2, y2, x3, y3));
            }
            // `y`: operands x1 y1 x3 y3; (x3,y3) is both the second control
            // point and the endpoint per the PDF spec.
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

            // ── Paint operators ────────────────────────────────────────────────

            "S" => {
                commit_paint(&mut objects, &mut subpaths, &mut current_sub, &gs, ObjectKind::Stroke);
            }
            "s" => {
                // close-and-stroke (equivalent to h S)
                current_sub.points.push(PathPoint::Close);
                commit_paint(&mut objects, &mut subpaths, &mut current_sub, &gs, ObjectKind::Stroke);
            }
            "f" | "f*" | "F" => {
                commit_paint(&mut objects, &mut subpaths, &mut current_sub, &gs, ObjectKind::Fill);
            }
            "B" | "B*" => {
                commit_paint(&mut objects, &mut subpaths, &mut current_sub, &gs, ObjectKind::FillStroke);
            }
            "b" | "b*" => {
                // close-and-fill+stroke
                current_sub.points.push(PathPoint::Close);
                commit_paint(&mut objects, &mut subpaths, &mut current_sub, &gs, ObjectKind::FillStroke);
            }
            "n" => {
                // Discard path without painting (e.g. after W/W* clipping paths).
                subpaths.clear();
                current_sub = SubPath::default();
            }

            // ── XObject placement ──────────────────────────────────────────────

            "Do" if !op.operands.is_empty() => {
                if let Object::Name(name) = &op.operands[0] {
                    match resolve_xobject(doc, resource_parent_id, name) {
                        Some((form_id, ref subtype)) if subtype == b"Form" => {
                            if depth == 0 {
                                // Recursion budget exhausted — emit the form's declared
                                // /BBox as a placeholder so its footprint isn't lost.
                                let local_bbox = form_bbox_from_id(doc, form_id)
                                    .unwrap_or(Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 });
                                objects.push(PageObject {
                                    bbox: gs.ctm.transform_rect(&local_bbox),
                                    ctm: gs.ctm,
                                    kind: ObjectKind::FormXObject,
                                    fill_color: None,
                                    stroke_color: None,
                                    stroke_width: 0.0,
                                    subpaths: vec![],
                                });
                            } else {
                                // Expand the form: parse its content stream with
                                // depth decremented, collecting all inner objects.
                                parse_form_xobject(doc, form_id, &gs, objects, depth - 1);
                            }
                        }
                        Some((_, ref subtype)) if subtype == b"Image" => {
                            // Image XObjects are defined on the unit square [0,1]²
                            // before the current CTM is applied.
                            let bbox = gs.ctm.transform_rect(&Rect {
                                x: 0.0,
                                y: 0.0,
                                width: 1.0,
                                height: 1.0,
                            });
                            objects.push(PageObject {
                                bbox,
                                ctm: gs.ctm,
                                kind: ObjectKind::Image,
                                fill_color: None,
                                stroke_color: None,
                                stroke_width: 0.0,
                                subpaths: vec![],
                            });
                        }
                        // Unknown subtype or lookup failure — skip silently.
                        _ => {}
                    }
                }
            }

            // ── Text blocks ────────────────────────────────────────────────────

            "BT" => {
                in_text = true;
                text_buf.clear();
                text_origin = None;
                tm = Matrix::identity();
                lm = Matrix::identity();
                // font_size persists across BT/ET per the PDF spec (text state is
                // not reset by BT, only the text matrix is). leading is reset
                // because it is part of the text-block positioning context.
                leading = 0.0;
            }
            "ET" => {
                if in_text && !text_buf.is_empty() {
                    let origin = text_origin.unwrap_or(tm);
                    let char_count = text_buf.chars().count() as f64;

                    // Estimate text extent in text space.
                    // 0.5 em per character is a reasonable average for proportional fonts.
                    let text_w = char_count * 0.5 * font_size;
                    let ascent = 0.8 * font_size;
                    let descent = -0.2 * font_size;

                    // Transform all four corners of the text rectangle through the
                    // text matrix then the CTM to get an axis-aligned bbox in page
                    // space. This correctly handles rotated and sheared text matrices.
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
                // PDF spec: Tlm = [1 0 0 1 tx ty] × Tlm
                // Equivalent to applying the offset in the current line matrix space.
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
}

// ── Form XObject recursion ────────────────────────────────────────────────────

/// Expand a Form XObject by parsing its content stream, collecting all inner
/// painted objects into `objects` with geometry already in page space.
///
/// Two transforms are applied before recursing:
///
/// 1. The form's `/Matrix` entry maps the form's local coordinate space to the
///    parent's user space. When absent the identity matrix is used (no change).
/// 2. The parent's CTM is then concatenated, yielding the effective CTM for
///    every object inside the form:
///    `effective_ctm = parent_gs.ctm × form_matrix`
///
/// The form's object ID is used as the `resource_parent_id` for the inner
/// [`parse_content`] call, because each Form XObject carries its own
/// `/Resources` dictionary (a PDF spec requirement). Any `Do` operators inside
/// the form will therefore look up XObject names in the form's own resources,
/// not the page's.
///
/// `depth` is the recursion budget already decremented by the caller; it is
/// passed through unchanged to [`parse_content`] so that further nesting is
/// correctly accounted for.
///
/// Silently returns without modifying `objects` if the stream cannot be
/// retrieved or decoded (the caller's object list is left intact).
fn parse_form_xobject(
    doc: &Document,
    form_id: ObjectId,
    parent_gs: &GraphicsState,
    objects: &mut Vec<PageObject>,
    depth: u8,
) {
    let Ok(form_obj) = doc.get_object(form_id) else { return };
    let Object::Stream(stream) = form_obj else { return };

    // A form's /Matrix maps its local space into the parent user space.
    // The effective CTM for objects inside is parent_ctm × form_matrix.
    let form_matrix = stream
        .dict
        .get(b"Matrix")
        .ok()
        .and_then(|m| m.as_array().ok())
        .filter(|arr| arr.len() >= 6)
        .map(|arr| ops_to_matrix(arr))
        .unwrap_or_else(Matrix::identity);

    let mut entry_gs = parent_gs.clone();
    entry_gs.ctm = parent_gs.ctm.concat(&form_matrix);

    // Decompress the stream content (FlateDecode for virtually all Form XObjects).
    // Mirror the fallback used in lopdf's own get_page_content: if decompression
    // fails, attempt to parse the raw bytes directly (handles uncompressed forms).
    let bytes = match stream.decompressed_content() {
        Ok(b) => b,
        Err(_) => stream.content.clone(),
    };
    let Ok(content) = lopdf::content::Content::decode(&bytes) else { return };

    // Use the form's own ID as the resource parent so that nested Do operators
    // resolve XObject names from the form's /Resources, not the page's.
    parse_content(doc, &content.operations, &entry_gs, form_id, objects, depth);
}

// ── XObject resolution helpers ────────────────────────────────────────────────

/// Resolve a named XObject and return its document object ID and `/Subtype` bytes.
///
/// Navigates `resource_parent_id → /Resources → /XObject → name` to locate the
/// XObject stream, then reads its `/Subtype`. Both pages (`Object::Dictionary`)
/// and Form XObjects (`Object::Stream`) are supported as the resource parent,
/// because the two differ in where their resource dictionary lives.
///
/// Returns `None` if the name cannot be resolved, the XObject entry is not an
/// indirect reference, or the subtype cannot be read. The caller should skip the
/// `Do` operator silently on `None`.
fn resolve_xobject(
    doc: &Document,
    resource_parent_id: ObjectId,
    name: &[u8],
) -> Option<(ObjectId, Vec<u8>)> {
    let xo_dict = xobject_dict_for(doc, resource_parent_id)?;

    // XObject entries must be indirect references per the PDF spec.
    let xobj_id = match xo_dict.get(name).ok()? {
        Object::Reference(id) => *id,
        _ => return None,
    };

    let xobj = doc.get_object(xobj_id).ok()?;
    let stream = match xobj {
        Object::Stream(s) => s,
        _ => return None,
    };
    let subtype = match stream.dict.get(b"Subtype").ok()? {
        Object::Name(n) => n.clone(),
        _ => return None,
    };
    Some((xobj_id, subtype))
}

/// Retrieve an owned clone of the `/XObject` sub-dictionary reachable from
/// `resource_parent_id`.
///
/// The resource parent may be either:
/// - A **page object** (`Object::Dictionary`): resources live directly in its dict.
/// - A **Form XObject** (`Object::Stream`): resources live in its stream dictionary.
///
/// An owned `Dictionary` is returned (rather than a reference) to avoid lifetime
/// entanglement between the borrowed document object and the caller's borrow of
/// `doc`. For the typical case of a few dozen XObject entries the clone is cheap.
///
/// Returns `None` if the resource chain cannot be navigated at any step.
fn xobject_dict_for(doc: &Document, resource_parent_id: ObjectId) -> Option<Dictionary> {
    let parent_obj = doc.get_object(resource_parent_id).ok()?;

    // Pages store /Resources in their object dictionary.
    // Form XObjects store /Resources in their stream dictionary.
    // Both resolve to the same /Resources value structure from that point onward.
    let resources_obj: &Object = match parent_obj {
        Object::Dictionary(d) => deref(doc, d.get(b"Resources").ok()?),
        Object::Stream(s) => deref(doc, s.dict.get(b"Resources").ok()?),
        _ => return None,
    };

    let res_dict = resources_obj.as_dict().ok()?;
    let xo_obj = deref(doc, res_dict.get(b"XObject").ok()?);
    Some(xo_obj.as_dict().ok()?.clone())
}

/// Read the `/BBox` of a Form XObject directly from its stream dictionary,
/// returning the result as a [`Rect`] in the form's local coordinate space.
///
/// Used as a depth-limit fallback: when the recursion budget is exhausted,
/// the form's declared bounding box (transformed by the current CTM) is
/// recorded as a [`ObjectKind::FormXObject`] placeholder so that the form's
/// footprint is still represented in the object tree.
///
/// Returns `None` if the stream cannot be retrieved or `/BBox` is absent or
/// malformed, in which case a unit-square fallback is appropriate.
fn form_bbox_from_id(doc: &Document, form_id: ObjectId) -> Option<Rect> {
    let form_obj = doc.get_object(form_id).ok()?;
    let stream = match form_obj {
        Object::Stream(s) => s,
        _ => return None,
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
/// painted object, then clear both buffers. Does nothing if all buffers are empty.
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

/// Compute the axis-aligned bounding box of all subpath points transformed by `ctm`.
///
/// For cubic Bézier segments the control points are included in the bbox —
/// this is conservative but avoids the cost of curve flattening.
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
        Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
    }
}

fn loss_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
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

    // ── parse_content depth guard ────────────────────────────────────────────
    //
    // These tests verify the depth-zero path without needing a real document.
    // parse_content with depth=0 must not panic and must process all non-Do
    // operators normally.

    #[test]
    fn parse_content_depth_zero_emits_no_objects_for_empty_ops() {
        let mut objects: Vec<PageObject> = Vec::new();
        // With no operations there is nothing to emit regardless of depth.
        parse_content(
            // We pass a dummy page_id; it is only consulted for Do operators,
            // which are absent here.
            &lopdf::Document::new(),
            &[],
            &GraphicsState::default(),
            (0, 0),
            &mut objects,
            0,
        );
        assert!(objects.is_empty());
    }

    #[test]
    fn parse_content_depth_zero_emits_fill_objects() {
        use lopdf::content::Operation;

        // Build a minimal sequence: re (rectangle) + f (fill).
        let ops = vec![
            Operation::new(
                "re",
                vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(100.0),
                    Object::Real(100.0),
                ],
            ),
            Operation::new("f", vec![]),
        ];

        let mut objects: Vec<PageObject> = Vec::new();
        parse_content(
            &lopdf::Document::new(),
            &ops,
            &GraphicsState::default(),
            (0, 0),
            &mut objects,
            0, // depth=0 must not affect non-Do operators
        );
        assert_eq!(objects.len(), 1, "re+f must emit exactly one Fill object");
        assert!(matches!(objects[0].kind, ObjectKind::Fill));
        assert!((objects[0].bbox.width - 100.0).abs() < 0.1);
        assert!((objects[0].bbox.height - 100.0).abs() < 0.1);
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

    /// Any Form XObject in the fixture should be expanded rather than left as a
    /// placeholder — meaning no `FormXObject` kind should appear in the tree
    /// under normal conditions (depth limit of 8 is never reached in practice).
    #[test]
    fn build_object_tree_forms_are_expanded() {
        let Some((doc, page_id)) = fixture() else {
            return;
        };
        let tree = build_object_tree(&doc, page_id).unwrap();
        let unexpanded = tree
            .objects
            .iter()
            .filter(|o| matches!(o.kind, ObjectKind::FormXObject))
            .count();
        assert_eq!(
            unexpanded, 0,
            "all Form XObjects should be recursed into; found {unexpanded} placeholder(s)"
        );
    }
}