use crate::geometry::{Matrix, Rect};
use crate::pages::boxes::object_to_f64;
use super::filter::operands_to_matrix;
use lopdf::{Document, Object, ObjectId};

const MAX_BLOCKS: usize = 500;

/// Extracts approximate text and image bounding boxes from a page's content stream.
///
/// Returns `(text_blocks, image_blocks)` where each block is `[x, y, w, h]` in PDF
/// coordinate space (pts, origin bottom-left). Positions are mid-tolerance estimates
/// suitable for a layout preview — not pixel-perfect rendering.
pub fn page_layout(doc: &Document, page_id: ObjectId) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
    let mut text: Vec<[f32; 4]> = Vec::new();
    let mut images: Vec<[f32; 4]> = Vec::new();

    let content = match doc.get_and_decode_page_content(page_id) {
        Ok(c) => c,
        Err(_) => return (text, images),
    };

    let mut ctm_stack: Vec<Matrix> = Vec::new();
    let mut ctm = Matrix::identity();

    let mut in_text = false;
    let mut tm = Matrix::identity();
    let mut lm = Matrix::identity();
    let mut font_size: f64 = 12.0;
    let mut leading: f64 = 0.0;

    for op in &content.operations {
        if text.len() + images.len() >= MAX_BLOCKS {
            break;
        }
        match op.operator.as_str() {
            "q" => ctm_stack.push(ctm),
            "Q" => {
                if let Some(prev) = ctm_stack.pop() {
                    ctm = prev;
                }
            }
            "cm" if op.operands.len() >= 6 => {
                ctm = ctm.concat(&operands_to_matrix(&op.operands));
            }
            "BT" => {
                in_text = true;
                tm = Matrix::identity();
                lm = Matrix::identity();
            }
            "ET" => {
                in_text = false;
            }
            "Tf" if in_text && op.operands.len() >= 2 => {
                font_size = object_to_f64(&op.operands[1]).abs().max(0.1);
            }
            "TL" if in_text && !op.operands.is_empty() => {
                leading = object_to_f64(&op.operands[0]);
            }
            "Tm" if in_text && op.operands.len() >= 6 => {
                tm = operands_to_matrix(&op.operands);
                lm = tm;
            }
            "Td" | "TD" if in_text && op.operands.len() >= 2 => {
                let tx = object_to_f64(&op.operands[0]);
                let ty = object_to_f64(&op.operands[1]);
                if op.operator == "TD" {
                    leading = -ty;
                }
                let t = Matrix::from_values(1.0, 0.0, 0.0, 1.0, tx, ty);
                lm = lm.concat(&t);
                tm = lm;
            }
            "T*" if in_text => {
                let t = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 0.0, -leading);
                lm = lm.concat(&t);
                tm = lm;
            }
            "Tj" | "'" if in_text && !op.operands.is_empty() => {
                if let Some(b) = text_block_from_obj(&op.operands[0], &tm, &ctm, font_size) {
                    text.push(b);
                }
            }
            "\"" if in_text && op.operands.len() >= 3 => {
                if let Some(b) = text_block_from_obj(&op.operands[2], &tm, &ctm, font_size) {
                    text.push(b);
                }
            }
            "TJ" if in_text && !op.operands.is_empty() => {
                let chars: usize = match &op.operands[0] {
                    Object::Array(arr) => arr
                        .iter()
                        .filter_map(|o| {
                            if let Object::String(s, _) = o {
                                Some(s.len())
                            } else {
                                None
                            }
                        })
                        .sum(),
                    _ => 0,
                };
                if chars > 0 {
                    if let Some(b) = make_text_block(chars, &tm, &ctm, font_size) {
                        text.push(b);
                    }
                }
            }
            "Do" if !op.operands.is_empty() => {
                if is_image_xobject(doc, page_id, &op.operands[0]) {
                    let unit = Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };
                    let r = ctm.transform_rect(&unit);
                    if r.width > 0.5 && r.height > 0.5 {
                        images.push([r.x as f32, r.y as f32, r.width as f32, r.height as f32]);
                    }
                }
            }
            _ => {}
        }
    }

    (text, images)
}

fn text_block_from_obj(
    obj: &Object,
    tm: &Matrix,
    ctm: &Matrix,
    font_size: f64,
) -> Option<[f32; 4]> {
    let chars = match obj {
        Object::String(b, _) => b.len(),
        _ => return None,
    };
    if chars == 0 {
        return None;
    }
    make_text_block(chars, tm, ctm, font_size)
}

fn make_text_block(
    chars: usize,
    tm: &Matrix,
    ctm: &Matrix,
    font_size: f64,
) -> Option<[f32; 4]> {
    let scale = tm.a.abs().max(tm.d.abs());
    let size = if scale > 0.001 { scale * font_size } else { font_size };
    let (px, py) = ctm.transform_point(tm.e, tm.f);
    let w = chars as f64 * size * 0.5;
    let h = size * 1.1;
    if w < 0.5 || h < 0.5 {
        return None;
    }
    Some([px as f32, py as f32, w as f32, h as f32])
}

fn is_image_xobject(doc: &Document, page_id: ObjectId, name_obj: &Object) -> bool {
    let name = match name_obj {
        Object::Name(n) => n.as_slice(),
        _ => return false,
    };

    let page_obj = match doc.get_object(page_id) {
        Ok(o) => o,
        Err(_) => return false,
    };
    let page_dict = match page_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let res_val = match page_dict.get(b"Resources") {
        Ok(v) => v,
        Err(_) => return false,
    };
    let res_obj = match res_val {
        Object::Reference(id) => match doc.get_object(*id) {
            Ok(o) => o,
            Err(_) => return false,
        },
        other => other,
    };
    let res_dict = match res_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let xo_val = match res_dict.get(b"XObject") {
        Ok(v) => v,
        Err(_) => return false,
    };
    let xo_obj = match xo_val {
        Object::Reference(id) => match doc.get_object(*id) {
            Ok(o) => o,
            Err(_) => return false,
        },
        other => other,
    };
    let xo_map = match xo_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let xobj_id = match xo_map.get(name) {
        Ok(Object::Reference(id)) => *id,
        _ => return false,
    };
    let xobj = match doc.get_object(xobj_id) {
        Ok(o) => o,
        Err(_) => return false,
    };
    let stream = match xobj {
        Object::Stream(s) => s,
        _ => return false,
    };
    matches!(
        stream.dict.get(b"Subtype"),
        Ok(Object::Name(n)) if n == b"Image"
    )
}
