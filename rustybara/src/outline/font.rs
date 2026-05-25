//! Extract embedded font binary data from a PDF resource dictionary.
//!
//! Navigates: page `/Resources` -> `/Font` -> font name -> `/FontDescriptor` -> `/FontFile2`
//! (TrueType) or `/FontFile3` (CFF / OpenType).
//! Returns the decompressed font bytes, or `None` on any navigation failure.
//!
//! FontFile3 streams with `/Subtype /Type1C` carry raw CFF data that has no
//! OpenType (sfnt) wrapper. `ttf_parser::Face::parse` requires an sfnt container,
//! so we transparently wrap bare CFF in a minimal single-table OTTO header before
//! returning the bytes. The wrapping is a no-op for TrueType / already-wrapped
//! OpenType data.

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
        let raw = s.decompressed_content().ok()?;
        Some(wrap_cff_in_otf(raw))
    } else {
        None
    }
}

// ── CFF → OTF wrapper ─────────────────────────────────────────────────────────

/// Wrap raw CFF (Type1C) bytes in a minimal OpenType container so that
/// `ttf_parser::Face::parse` can handle them.
///
/// FontFile3 streams with `/Subtype /Type1C` hold a standalone CFF binary.
/// CFF v1 starts with major-version byte `0x01`; it has no sfnt signature.
/// We detect this and build a single-table `OTTO` wrapper whose only entry
/// points to a `CFF ` table containing the original bytes verbatim.
///
/// Bytes that already carry a known sfnt signature (TrueType, OpenType,
/// WOFF, WOFF2) are returned unchanged so the function is always safe to
/// call on any font stream.
fn wrap_cff_in_otf(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.len() < 4 {
        return bytes;
    }

    // Detect known sfnt signatures — these fonts need no wrapping.
    let sig = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    match sig {
        0x0001_0000  // TrueType version 1.0
        | 0x4F54_544F  // 'OTTO' — OpenType CFF (already wrapped)
        | 0x7472_7565  // 'true' — Apple TrueType
        | 0x7479_7031  // 'typ1' — old Apple format
        | 0x774F_4646  // 'wOFF' — WOFF
        | 0x774F_4632  // 'wOF2' — WOFF2
        => return bytes,
        _ => {}
    }

    // Raw CFF v1 starts with major version byte 0x01; v2 with 0x02.
    // Anything else is unknown — return as-is rather than corrupt it.
    if bytes[0] != 1 && bytes[0] != 2 {
        return bytes;
    }

    // ── Build minimal OTTO wrapper ────────────────────────────────────────
    //
    //   Offset table  (12 bytes)  — sfnt header
    //   Table record  (16 bytes)  — single "CFF " entry
    //   CFF data      (N bytes)
    //
    let cff_len = bytes.len() as u32;
    let data_offset: u32 = 12 + 16; // header + one 16-byte table record

    let mut otf: Vec<u8> = Vec::with_capacity(data_offset as usize + bytes.len());

    // Offset table ("OTTO" flavour for CFF-based OpenType)
    otf.extend_from_slice(b"OTTO");              // sfVersion
    otf.extend_from_slice(&1u16.to_be_bytes());  // numTables = 1
    otf.extend_from_slice(&16u16.to_be_bytes()); // searchRange  = (2^floor(log2(1))) × 16 = 16
    otf.extend_from_slice(&0u16.to_be_bytes());  // entrySelector = floor(log2(1)) = 0
    otf.extend_from_slice(&0u16.to_be_bytes());  // rangeShift    = 1×16 − 16 = 0

    // Table record for "CFF "
    otf.extend_from_slice(b"CFF ");                    // tableTag
    otf.extend_from_slice(&0u32.to_be_bytes());        // checkSum (ttf_parser does not validate)
    otf.extend_from_slice(&data_offset.to_be_bytes()); // offset from start of file
    otf.extend_from_slice(&cff_len.to_be_bytes());     // length of CFF data

    // Append the original CFF bytes
    otf.append(&mut bytes);
    otf
}
