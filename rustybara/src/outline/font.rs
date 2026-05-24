//! Extract embedded font binary data from a PDF resource dictionary.
//!
//! Navigates: page `/Resources` -> `/Font` -> font name -> `/FontDescriptor` -> `/FontFile2`
//! (TrueType) or `/FontFile3` (CFF / OpenType).
//! Returns the decompressed font bytes, or `None` on any navigation failure.

use crate::objects::tree::{deref, ref_id};
use lopdf::{Document, Object, ObjectId};

/// Return the decompressed bytes of the font embedded under `font_name`
/// in the `/Font` resource dictionary of `page_id`.
///
/// Supports `/FontFile2` (TrueType) and `/FontFile3` (CFF / OpenType).
/// Returns `None` for Type 1, Type 3, CIDFont Type0 wrappers, or any
/// font whose descriptor is absent or whose font file stream cannot be read.
pub fn extract_font_bytes(doc: &Document, page_id: ObjectId, font_name: &[u8]) -> Option<Vec<u8>> {
    let page = doc.get_object(page_id).ok()?;
    let page_dict = page.as_dict().ok()?;

    let res_val = page_dict.get(b"Resources").ok()?;
    let res_dict = deref(doc, res_val).as_dict().ok()?;

    let font_val = res_dict.get(b"Font").ok()?;
    let font_dict = deref(doc, font_val).as_dict().ok()?;

    let font_val = font_dict.get(font_name).ok()?;
    let font_id = ref_id(font_val)?;

    let font_obj = doc.get_object(font_id).ok()?;
    let font_inner = font_obj.as_dict().ok()?;

    // Unwrap DescendantFonts for Type0 (CIDFont wrappers).
    // We navigate one level deep; multi-level nesting is not handled.
    let descriptor_dict = if let Ok(Object::Array(arr)) = font_inner.get(b"DescendantFonts") {
        let desc_ref = arr.first()?;
        let desc_id = ref_id(desc_ref)?;
        let desc_obj = doc.get_object(desc_id).ok()?;
        let desc_dict = desc_obj.as_dict().ok()?;
        let fd_val = desc_dict.get(b"FontDescriptor").ok()?;
        let fd_id = ref_id(fd_val)?;
        let fd_obj = doc.get_object(fd_id).ok()?;
        fd_obj.as_dict().ok()?.clone()
    } else {
        let fd_val = font_inner.get(b"FontDescriptor").ok()?;
        let fd_id = ref_id(fd_val)?;
        let fd_obj = doc.get_object(fd_id).ok()?;
        fd_obj.as_dict().ok()?.clone()
    };

    let file_val = descriptor_dict
        .get(b"FontFile2")
        .or_else(|_| descriptor_dict.get(b"FontFile3"))
        .ok()?;
    let file_id = ref_id(file_val)?;

    let file_obj = doc.get_object(file_id).ok()?;
    if let Object::Stream(s) = file_obj {
        s.decompressed_content().ok()
    } else {
        None
    }
}
