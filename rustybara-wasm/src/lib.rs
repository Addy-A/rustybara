use serde::Serialize;
use wasm_bindgen::prelude::*;
use rustybara::{DocumentColorKind, PdfPipeline};

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Compute `"sha256:<hex>"` of raw bytes. Call this on the original PDF bytes
/// *before* constructing a `PipelineHandle` so the hash reflects unmodified data.
#[wasm_bindgen]
pub fn hash_bytes(bytes: &[u8]) -> String {
    rustybara::xmp::hash_bytes(bytes)
}

#[derive(Serialize)]
struct LayoutHint {
    text: Vec<[f32; 4]>,
    images: Vec<[f32; 4]>,
}

#[derive(Serialize)]
struct XmpBlockJs {
    uuid: String,
    version: String,
    timestamp: String,
    source_hash: String,
    parent_id: String,
    ops: Vec<String>,
}

/// In-browser PDF pipeline handle.
#[wasm_bindgen]
pub struct PipelineHandle {
    inner: PdfPipeline,
}

#[wasm_bindgen]
impl PipelineHandle {
    /// Construct from raw PDF bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<PipelineHandle, JsValue> {
        let inner = PdfPipeline::from_bytes(bytes)
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(PipelineHandle { inner })
    }

    /// Return the number of pages in the document (does not consume the handle).
    pub fn page_count(&self) -> usize {
        self.inner.page_count()
    }

    /// Strip content outside the TrimBox. Consumes the handle and returns a new one.
    pub fn trim(mut self) -> Result<PipelineHandle, JsValue> {
        self.inner
            .trim()
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(self)
    }

    /// Expand all page boxes by `bleed_pts` PDF points. Consumes the handle and returns a new one.
    pub fn resize(mut self, bleed_pts: f64) -> Result<PipelineHandle, JsValue> {
        self.inner
            .resize(bleed_pts)
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(self)
    }

    /// Set a TrimBox on every page by insetting the MediaBox by `bleed_pts` on all sides.
    /// Use this when a PDF has no TrimBox but the bleed extent is known (typically 9 pts / ⅛ in).
    /// Consumes the handle and returns a new one.
    pub fn add_trim_box(mut self, bleed_pts: f64) -> Result<PipelineHandle, JsValue> {
        self.inner
            .add_trim_box(bleed_pts)
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(self)
    }

    /// Substitute a CMYK color throughout content streams.
    /// All channel values are in the 0.0–1.0 range. Consumes the handle and returns a new one.
    pub fn remap_color(
        mut self,
        from_c: f64, from_m: f64, from_y: f64, from_k: f64,
        to_c: f64,   to_m: f64,   to_y: f64,   to_k: f64,
        tolerance: f64,
    ) -> Result<PipelineHandle, JsValue> {
        self.inner
            .remap_color(
                [from_c, from_m, from_y, from_k],
                [to_c,   to_m,   to_y,   to_k],
                tolerance,
            )
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(self)
    }

    /// Extract a subset of pages into a new handle. Page indices are zero-based.
    /// Out-of-range indices are silently ignored. Does not consume this handle.
    pub fn extract_pages(&self, page_nums: Vec<u32>) -> Result<PipelineHandle, JsValue> {
        let inner = self.inner
            .extract_pages(&page_nums)
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(PipelineHandle { inner })
    }

    /// Split each wide page at `panel_width_pts` into left/right halves.
    /// Does not consume this handle; returns a new one containing the split pages.
    pub fn split_pages(&self, panel_width_pts: f64) -> Result<PipelineHandle, JsValue> {
        let inner = self.inner
            .split_pages(panel_width_pts)
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(PipelineHandle { inner })
    }

    /// Stitch adjacent page pairs into spreads of `spread_width_pts`.
    /// Does not consume this handle; returns a new one containing the stitched spreads.
    pub fn stitch_pages(&self, spread_width_pts: f64) -> Result<PipelineHandle, JsValue> {
        let inner = self.inner
            .stitch_pages(spread_width_pts)
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(PipelineHandle { inner })
    }

    /// Convert all text to outlined vector paths, removing the dependency on embedded fonts.
    /// Consumes the handle and returns a new one.
    pub fn outline_text(mut self) -> Result<PipelineHandle, JsValue> {
        self.inner
            .outline_text()
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(self)
    }

    /// Classify color usage across all pages.
    /// Returns one of `"CMYK"`, `"RGB"`, `"Mixed"`, or `"Unknown"`.
    pub fn detect_color_space(&self) -> String {
        match PdfPipeline::detect_color_space(self.inner.doc()) {
            DocumentColorKind::PureCMYK => "CMYK".to_string(),
            DocumentColorKind::PureRGB  => "RGB".to_string(),
            DocumentColorKind::Mixed    => "Mixed".to_string(),
            DocumentColorKind::Unknown  => "Unknown".to_string(),
        }
    }

    /// Return approximate text and image bounding boxes for a page as
    /// `{ text: [[x,y,w,h], …], images: [[x,y,w,h], …] }` (pts, origin bottom-left).
    /// `page_idx` is zero-based. Returns an object with empty arrays when the page
    /// does not exist or has no parseable content.
    pub fn page_layout_hint(&self, page_idx: u32) -> JsValue {
        let (text, images) = self.inner.page_layout_hint(page_idx);
        let hint = LayoutHint { text, images };
        serde_wasm_bindgen::to_value(&hint).unwrap_or(JsValue::NULL)
    }

    /// Read the `rbara:` XMP block embedded by a previous rustybara run.
    /// Returns `null` for files that have never been processed by rustybara,
    /// otherwise an object with `uuid`, `version`, `timestamp`, `source_hash`,
    /// `parent_id`, and `ops` (string array) fields.
    pub fn read_xmp_block(&self) -> JsValue {
        match self.inner.read_xmp_block() {
            None => JsValue::NULL,
            Some(b) => {
                let block = XmpBlockJs {
                    uuid: b.uuid,
                    version: b.version,
                    timestamp: b.timestamp,
                    source_hash: b.source_hash,
                    parent_id: b.parent_id,
                    ops: b.ops,
                };
                serde_wasm_bindgen::to_value(&block).unwrap_or(JsValue::NULL)
            }
        }
    }

    /// Embed rustybara processing metadata into the document's XMP stream.
    ///
    /// `source_hash` should be the result of calling `hash_bytes()` on the original
    /// unmodified PDF bytes. `timestamp` is an ISO 8601 string from the caller.
    /// `op_names` and `op_params` are parallel arrays of operation names and their
    /// parameter strings (use an empty string for ops with no parameters).
    /// Consumes the handle and returns a new one.
    pub fn embed_metadata(
        mut self,
        source_hash: &str,
        timestamp: &str,
        op_names: Vec<String>,
        op_params: Vec<String>,
    ) -> Result<PipelineHandle, JsValue> {
        let pairs: Vec<(String, String)> = op_names
            .into_iter()
            .zip(op_params.into_iter())
            .collect();
        let ops: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_str()))
            .collect();
        self.inner
            .embed_metadata(source_hash, timestamp, &ops)
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))?;
        Ok(self)
    }

    /// Serialize the result to PDF bytes for download. Consumes the handle.
    pub fn to_pdf_bytes(mut self) -> Result<Vec<u8>, JsValue> {
        self.inner
            .to_bytes()
            .map_err(|e| JsValue::from(js_sys::Error::new(&e.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PDF: &[u8] =
        include_bytes!("../../rustybara/tests/fixtures/pdf_test_data_print_v2.pdf");

    #[test]
    fn new_from_valid_bytes() {
        assert!(PipelineHandle::new(PDF).is_ok());
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn new_from_invalid_bytes_returns_err() {
        assert!(PipelineHandle::new(b"not a pdf").is_err());
    }

    #[test]
    fn page_count_returns_nonzero() {
        let handle = PipelineHandle::new(PDF).unwrap();
        assert!(handle.page_count() > 0);
    }

    #[test]
    fn page_count_does_not_consume() {
        let handle = PipelineHandle::new(PDF).unwrap();
        let _ = handle.page_count();
        assert!(handle.to_pdf_bytes().is_ok());
    }

    #[test]
    fn trim_succeeds() {
        let handle = PipelineHandle::new(PDF).unwrap();
        assert!(handle.trim().is_ok());
    }

    #[test]
    fn resize_succeeds() {
        let handle = PipelineHandle::new(PDF).unwrap();
        assert!(handle.resize(8.504).is_ok());
    }

    #[test]
    fn resize_zero_bleed_succeeds() {
        let handle = PipelineHandle::new(PDF).unwrap();
        assert!(handle.resize(0.0).is_ok());
    }

    #[test]
    fn remap_color_succeeds() {
        let handle = PipelineHandle::new(PDF).unwrap();
        assert!(handle
            .remap_color(1.0, 1.0, 1.0, 1.0, 0.6, 0.4, 0.2, 1.0, 0.05)
            .is_ok());
    }

    #[test]
    fn to_pdf_bytes_produces_valid_pdf() {
        let bytes = PipelineHandle::new(PDF).unwrap().to_pdf_bytes().unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn pipeline_chain_roundtrip() {
        let original_count = PipelineHandle::new(PDF).unwrap().page_count();
        let result = PipelineHandle::new(PDF)
            .unwrap()
            .trim()
            .unwrap()
            .resize(8.504)
            .unwrap()
            .to_pdf_bytes()
            .unwrap();
        assert!(result.starts_with(b"%PDF-"));
        let roundtripped = PipelineHandle::new(&result).unwrap();
        assert_eq!(roundtripped.page_count(), original_count);
    }

    #[test]
    fn add_trim_box_succeeds() {
        let handle = PipelineHandle::new(PDF).unwrap();
        assert!(handle.add_trim_box(9.0).is_ok());
    }

    #[test]
    fn extract_pages_returns_subset() {
        let handle = PipelineHandle::new(PDF).unwrap();
        let total = handle.page_count();
        if total > 0 {
            let extracted = handle.extract_pages(vec![0]).unwrap();
            assert_eq!(extracted.page_count(), 1);
        }
    }

    #[test]
    fn detect_color_space_returns_string() {
        let handle = PipelineHandle::new(PDF).unwrap();
        let cs = handle.detect_color_space();
        assert!(["CMYK", "RGB", "Mixed", "Unknown"].contains(&cs.as_str()));
    }

    #[test]
    fn hash_bytes_produces_sha256_prefix() {
        let h = hash_bytes(PDF);
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), 7 + 64);
    }

    #[test]
    fn embed_metadata_roundtrip() {
        let hash = hash_bytes(PDF);
        let ts = "2025-01-01T00:00:00Z";
        let bytes = PipelineHandle::new(PDF)
            .unwrap()
            .embed_metadata(&hash, ts, vec!["trim".into()], vec!["".into()])
            .unwrap()
            .to_pdf_bytes()
            .unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let handle2 = PipelineHandle::new(&bytes).unwrap();
        assert!(handle2.inner.read_xmp_block().is_some());
    }

    #[test]
    fn outline_text_succeeds() {
        let handle = PipelineHandle::new(PDF).unwrap();
        assert!(handle.outline_text().is_ok());
    }
}
