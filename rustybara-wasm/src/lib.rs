use wasm_bindgen::prelude::*;
use rustybara::PdfPipeline;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
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
}
