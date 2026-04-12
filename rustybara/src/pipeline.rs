use crate::encode::OutputFormat;
use crate::pages::PageBoxes;
use crate::raster::RenderConfig;
use crate::stream::ContentFilter;
use image::DynamicImage;
use lopdf::Document;
use std::path::Path;

/// A pipeline for processing and manipulating PDF documents.
///
/// The `PdfPipeline` struct provides a structured way to work with PDF files,
/// encapsulating a `Document` and offering methods to perform various operations
/// such as reading, modifying, and writing PDF content.
///
/// # Examples
///
/// ```no_test
/// // Create a new PDF pipeline
/// let pipeline = PdfPipeline::new();
///
/// // Load a PDF document
/// let doc = Document::load("example.pdf")?;
/// let pipeline = PdfPipeline::with_document(doc);
/// ```
pub struct PdfPipeline {
    doc: Document,
}

impl PdfPipeline {
    /// Opens a document from the specified file path.
    ///
    /// This function attempts to load a document from the given path and wraps it
    /// in a new instance of the containing struct.
    ///
    /// # Arguments
    ///
    /// * `path` - A path-like object that implements `AsRef<Path>` pointing to the document file
    ///
    /// # Returns
    ///
    /// * `Ok(Self)` - A new instance containing the loaded document
    /// * `Err(crate::Error)` - An error if the document could not be loaded
    ///
    /// # Examples
    ///
    /// ```no_test
    /// let document = MyStruct::open("path/to/document.txt")?;
    /// ```
    pub fn open(path: impl AsRef<Path>) -> crate::Result<Self> {
        let doc = Document::load(path)?;
        Ok(Self { doc })
    }

    /// Removes whitespace from the beginning and end of the document content.
    ///
    /// This method trims excess whitespace characters (spaces, tabs, newlines, etc.)
    /// from the outer boundaries of the document's content. It modifies the document
    /// in-place and returns a mutable reference to self for method chaining.
    ///
    /// # Returns
    ///
    /// Returns `Ok(&mut Self)` containing a mutable reference to the document if
    /// trimming succeeds, or an error if the trimming operation fails.
    ///
    /// # Errors
    ///
    /// Returns an error if the content filtering operation encounters issues
    /// while attempting to remove the outer whitespace.
    ///
    /// # Example
    ///
    /// ```no_test
    /// // Assuming `doc` is a mutable document instance
    /// doc.trim()?;
    /// ```
    pub fn trim(&mut self) -> crate::Result<&mut Self> {
        ContentFilter::remove_outside_trim(&mut self.doc)?;
        Ok(self)
    }

    /// Resizes the document's page boxes by applying bleed margins.
    ///
    /// This method adjusts the MediaBox (and optionally CropBox) of all pages in the document
    /// by expanding them outward by the specified bleed points. Bleed is extra space added
    /// around the edges of a page to ensure proper printing and trimming.
    ///
    /// # Arguments
    ///
    /// * `bleed_pts` - The amount of bleed margin to add in points (1/72 of an inch)
    ///
    /// # Returns
    ///
    /// Returns a mutable reference to self on success, or an error if page box operations fail.
    ///
    /// # Behavior
    ///
    /// For each page in the document:
    /// - Reads the current page boxes (MediaBox, CropBox, etc.)
    /// - Calculates a new media rectangle expanded by the bleed amount
    /// - Updates the MediaBox with the new dimensions
    /// - If the page has a CropBox, updates it to match the new MediaBox dimensions
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to read page boxes from any page
    /// - Failed to access or modify page dictionary objects
    pub fn resize(&mut self, bleed_pts: f64) -> crate::Result<&mut Self> {
        let pages = self.doc.get_pages();
        for &page_id in pages.values() {
            let boxes = PageBoxes::read(&self.doc, page_id)?;
            let new_media = boxes.bleed_rect(bleed_pts).to_pdf_array();
            let page_dict = self.doc.get_dictionary_mut(page_id)?;
            let arr: Vec<lopdf::Object> = new_media.iter().map(|&v| v.into()).collect();
            let has_cropbox = page_dict.has(b"CropBox");
            page_dict.set(b"MediaBox", arr.clone());
            if has_cropbox {
                page_dict.set(b"CropBox", arr);
            }
        }
        Ok(self)
    }

    /// Returns the total number of pages in the document.
    ///
    /// This method retrieves the current page count by accessing the underlying
    /// document's page collection and returning its length.
    ///
    /// # Returns
    ///
    /// The number of pages as a `usize`. Returns 0 if the document is empty
    /// or contains no pages.
    ///
    /// # Examples
    ///
    /// ```no_test
    /// let doc = Document::new();
    /// assert_eq!(doc.page_count(), 0);
    ///
    /// // Add some pages...
    /// assert_eq!(doc.page_count(), 3);
    /// ```
    pub fn page_count(&self) -> usize {
        self.doc.get_pages().len()
    }

    /// Saves the current document as a PDF file to the specified path.
    ///
    /// This method serializes the document content and writes it to a PDF file
    /// at the given location. If the file already exists, it will be overwritten.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path where the PDF should be saved. Can be any type
    ///   that implements `AsRef<Path>` (e.g., `&str`, `String`, `PathBuf`).
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the PDF was successfully saved, or an error if:
    /// - The document could not be serialized
    /// - There was an I/O error writing to the file
    /// - The path is invalid or inaccessible
    ///
    /// # Examples
    ///
    /// ```no_test
    /// // Save to a string path
    /// document.save_pdf("output.pdf")?;
    ///
    /// // Save to a PathBuf
    /// let path = std::path::PathBuf::from("documents/report.pdf");
    /// document.save_pdf(path)?;
    /// ```
    pub fn save_pdf(&mut self, path: impl AsRef<Path>) -> crate::Result<()> {
        self.doc.save(path)?;
        Ok(())
    }

    /// Renders a specific page from the PDF document as an image.
    ///
    /// This function takes a page number and rendering configuration, then generates
    /// a rasterized image of that page. The rendering is performed using Pdfium (the
    /// same engine used by Chrome for PDF rendering), which provides high-quality
    /// and accurate PDF rendering.
    ///
    /// # Arguments
    ///
    /// * `page_num` - The zero-based index of the page to render
    /// * `config` - A reference to `RenderConfig` containing rendering parameters
    ///   such as scale factor, rotation, and color options
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing either:
    /// * `Ok(DynamicImage)` - The rendered page as a dynamic image that can be
    ///   further processed or saved to various formats
    /// * `Err(crate::Error)` - An error if the rendering fails, which could be due
    ///   to invalid page numbers, PDF loading issues, or rendering problems
    ///
    /// # Platform Support
    ///
    /// The function automatically detects the operating system and loads the
    /// appropriate Pdfium library:
    /// * Windows: `pdfium.dll`
    /// * macOS: `libpdfium.dylib`  
    /// * Linux: `libpdfium.so`
    ///
    /// # Example
    ///
    /// ```no_test
    /// let config = RenderConfig::default();
    /// let image = pdf_renderer.render_page(0, &config)?;
    /// image.save("page_1.png")?;
    /// ```
    ///
    /// # Notes
    ///
    /// * The page numbering is zero-based (first page = 0)
    /// * The function clones the internal PDF document for rendering to avoid
    ///   borrowing conflicts
    /// * Pdfium library must be available at runtime in the same directory as
    ///   the executable
    pub fn render_page(&self, page_num: u32, config: &RenderConfig) -> crate::Result<DynamicImage> {
        use pdfium_render::prelude::*;

        let mut doc_clone = self.doc.clone();
        let mut buf = Vec::new();
        doc_clone.save_to(&mut buf).map_err(crate::Error::Io)?;

        let dylib_name = if cfg!(target_os = "windows") {
            "pdfium.dll"
        } else if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else {
            "libpdfium.so" // Linux
        };

        let bindings_result = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join(dylib_name)))
            .and_then(|lib| Pdfium::bind_to_library(lib).ok())
            .map_or_else(|| Pdfium::bind_to_system_library(), Ok);

        let pdfium = match bindings_result {
            Ok(bindings) => Pdfium::new(bindings),
            Err(_) => Pdfium,
        };

        let pdf_doc = pdfium.load_pdf_from_byte_vec(buf, None)?;
        let page = pdf_doc.pages().get(page_num as PdfPageIndex)?;
        crate::raster::render_page(&page, config)
    }

    /// Saves a rendered page as an image file.
    ///
    /// This method renders a specific page from the document and saves it to the specified
    /// file path in the desired output format.
    ///
    /// # Arguments
    ///
    /// * `page_num` - The page number to render and save (0-indexed)
    /// * `path` - The file path where the image should be saved
    /// * `format` - The output format for the saved image (PNG, JPEG, etc.)
    /// * `config` - Rendering configuration specifying quality, resolution, and other parameters
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful save, or an error if rendering or encoding fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The page number is invalid
    /// - Rendering the page fails
    /// - Encoding or saving the image fails
    /// - File system operations fail
    ///
    /// # Example
    ///
    /// ```no_test
    /// use document_renderer::{RenderConfig, OutputFormat};
    ///
    /// let renderer = DocumentRenderer::new();
    /// let config = RenderConfig::default();
    /// let format = OutputFormat::Png;
    ///
    /// renderer.save_page_image(0, "output/page_1.png", &format, &config)?;
    /// ```
    pub fn save_page_image(
        &self,
        page_num: u32,
        path: impl AsRef<Path>,
        format: &OutputFormat,
        config: &RenderConfig,
    ) -> crate::Result<()> {
        let image = self.render_page(page_num, config)?;
        crate::encode::save(&image, path.as_ref(), format)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::PageBoxes;

    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf_test_data_print_v2.pdf")
    }

    #[test]
    fn open_and_page_count() {
        let p = PdfPipeline::open(fixture()).unwrap();
        assert!(p.page_count() > 0);
    }

    #[test]
    fn open_nonexistent_fails() {
        let err = PdfPipeline::open("no_such_file.pdf");
        assert!(err.is_err());
    }

    #[test]
    fn trim_succeeds() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        p.trim().unwrap();
    }

    #[test]
    fn trim_is_chainable() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let out = std::env::temp_dir().join("rustybara_pipeline_trim_chain.pdf");
        p.trim().unwrap().save_pdf(&out).unwrap();
        assert!(out.exists());
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn resize_expands_mediabox() {
        let bleed = 9.0;
        let mut p = PdfPipeline::open(fixture()).unwrap();

        // Grab original trim dimensions for comparison
        let orig_doc = Document::load(fixture()).unwrap();
        let orig_pages = orig_doc.get_pages();
        let first_id = *orig_pages.values().next().unwrap();
        let orig_boxes = PageBoxes::read(&orig_doc, first_id).unwrap();
        let orig_trim = orig_boxes.trim_or_media();

        p.resize(bleed).unwrap();

        // Read back from the mutated doc
        let pages = p.doc.get_pages();
        let page_id = *pages.values().next().unwrap();
        let boxes = PageBoxes::read(&p.doc, page_id).unwrap();
        let media = boxes.media_box;

        assert!(
            (media.width - (orig_trim.width + 2.0 * bleed)).abs() < 0.01,
            "media width should be trim + 2*bleed"
        );
        assert!(
            (media.height - (orig_trim.height + 2.0 * bleed)).abs() < 0.01,
            "media height should be trim + 2*bleed"
        );
    }

    #[test]
    fn save_roundtrip() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let original_count = p.page_count();
        let out = std::env::temp_dir().join("rustybara_pipeline_roundtrip.pdf");

        p.trim().unwrap().save_pdf(&out).unwrap();

        let reopened = PdfPipeline::open(&out).unwrap();
        assert_eq!(reopened.page_count(), original_count);
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn resize_then_save() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let out = std::env::temp_dir().join("rustybara_pipeline_resize_save.pdf");
        p.resize(9.0).unwrap().save_pdf(&out).unwrap();
        assert!(out.exists());

        // Verify the saved file is loadable
        let reopened = PdfPipeline::open(&out).unwrap();
        assert!(reopened.page_count() > 0);
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn trim_then_resize_pipeline() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let out = std::env::temp_dir().join("rustybara_pipeline_trim_resize.pdf");
        p.trim()
            .unwrap()
            .resize(9.0)
            .unwrap()
            .save_pdf(&out)
            .unwrap();
        assert!(out.exists());
        std::fs::remove_file(&out).ok();
    }

    #[test]
    #[ignore = "requires pdfium runtime library"]
    fn render_page_produces_image() {
        let p = PdfPipeline::open(fixture()).unwrap();
        let config = RenderConfig::default();
        let img = p.render_page(0, &config).unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }
}
