use crate::encode::OutputFormat;
use crate::pages::PageBoxes;
use crate::raster::RenderConfig;
use crate::stream::{ColorRemap, ContentFilter};
use image::DynamicImage;
use lopdf::Document;
use std::path::Path;

/// Describes the overall color operator usage found across a PDF document's content streams.
///
/// Returned by [`PdfPipeline::detect_color_space`]. Distinct from ICC profile classification,
/// which identifies individual color profiles.
///
/// # Variants
///
/// * `PureCMYK` — Only CMYK color operators (`k`, `K`) were found
/// * `PureRGB` — Only RGB color operators (`rg`, `RG`) were found
/// * `Mixed` — Both CMYK and RGB operators are present
/// * `Unknown` — No recognizable color operators were found
pub enum DocumentColorKind {
    PureCMYK,
    PureRGB,
    Mixed,
    Unknown,
}

/// High-level pipeline for PDF preprocessing operations.
///
/// `PdfPipeline` wraps a `lopdf::Document` and provides a chainable API for common
/// prepress operations like trimming marks, resizing pages, remapping colors, and
/// exporting to images.
///
/// # Examples
///
/// ```no_run
/// use rustybara::PdfPipeline;
///
/// # fn main() -> rustybara::Result<()> {
/// // Chain multiple operations
/// PdfPipeline::open("input.pdf")?
///     .trim()?                    // Remove content outside TrimBox
///     .resize(9.0)?               // Add 9pt bleed
///     .save_pdf("output.pdf")?;
/// # Ok(())
/// # }
/// ```
///
/// ```no_run
/// use rustybara::{PdfPipeline, encode::OutputFormat, raster::RenderConfig};
///
/// # fn main() -> rustybara::Result<()> {
/// let pipeline = PdfPipeline::open("document.pdf")?;
/// let config = RenderConfig::prepress(); // 300 DPI
///
/// // Export first page as JPEG
/// pipeline.save_page_image(0, "page_1.jpg", &OutputFormat::Jpg, &config)?;
/// # Ok(())
/// # }
/// ```
pub struct PdfPipeline {
    doc: Document,
}

impl PdfPipeline {
    /// Returns a reference to the underlying `lopdf` document.
    ///
    /// Provides direct read access to the raw PDF document object, which can be used
    /// to inspect or query document internals (e.g., page objects, resources, metadata)
    /// without going through the pipeline abstraction.
    ///
    /// # Returns
    ///
    /// A shared reference to the inner `lopdf::Document`.
    pub fn doc(&self) -> &Document {
        &self.doc
    }

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

    /// Sets a `TrimBox` on every page by insetting the `MediaBox` by `bleed_pts` on all sides.
    ///
    /// Use this when a PDF arrives without a `TrimBox` but the bleed extent is known. The most
    /// common prepress default is `9.0` points (⅛ inch / ~3.175 mm). Any existing `TrimBox` is
    /// overwritten. This is the inverse of [`Self::resize`], which expands the `MediaBox` outward.
    ///
    /// # Errors
    ///
    /// Returns an error if any page dictionary cannot be accessed.
    pub fn add_trim_box(&mut self, bleed_pts: f64) -> crate::Result<&mut Self> {
        crate::pages::set_trim_boxes(&mut self.doc, bleed_pts)?;
        Ok(self)
    }

    /// Extracts a subset of pages into a new [`PdfPipeline`].
    ///
    /// Page numbers are **zero-indexed** — page `0` is the first page, consistent with
    /// [`Self::save_page_image`]. Out-of-range values are silently ignored; output page order
    /// always matches the original document.
    ///
    /// # Errors
    ///
    /// Returns an error if `page_nums` contains no valid indices or if the page tree cannot
    /// be rewritten.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rustybara::PdfPipeline;
    /// let doc = PdfPipeline::open("brochure.pdf").unwrap();
    /// let page_count = doc.page_count() as u32;
    /// let mut cover = doc.extract_pages(&[0]).unwrap();
    /// let mut body  = doc.extract_pages(&(1..page_count).collect::<Vec<_>>()).unwrap();
    /// cover.save_pdf("cover.pdf").unwrap();
    /// body.save_pdf("body.pdf").unwrap();
    /// ```
    pub fn extract_pages(&self, page_nums: &[u32]) -> crate::Result<Self> {
        let doc = crate::pages::extract_pages(&self.doc, page_nums)?;
        Ok(Self { doc })
    }

    /// Splits every page into its own [`PdfPipeline`].
    ///
    /// Returns a `Vec` where index `i` holds a single-page pipeline for zero-indexed page `i`.
    /// Convenience wrapper around [`Self::extract_pages`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rustybara::PdfPipeline;
    /// let doc = PdfPipeline::open("spread.pdf").unwrap();
    /// for (i, mut page) in doc.split_pages().unwrap().into_iter().enumerate() {
    ///     page.save_pdf(format!("page_{}.pdf", i + 1)).unwrap();
    /// }
    /// ```
    pub fn split_pages(&self) -> crate::Result<Vec<Self>> {
        (0..self.page_count() as u32)
            .map(|i| self.extract_pages(&[i]))
            .collect()
    }

    /// Analyzes a PDF document and classifies the color spaces used across all pages.
    ///
    /// Iterates through every page's content stream, inspecting PDF paint operators to
    /// determine whether the document uses CMYK (`k`/`K`), RGB (`rg`/`RG`), both, or
    /// neither.
    ///
    /// # Arguments
    ///
    /// * `doc` - A reference to the `lopdf::Document` to inspect.
    ///
    /// # Returns
    ///
    /// A [`DocumentColorKind`] variant describing the overall color usage:
    /// * `DocumentColorKind::PureCMYK`  – only CMYK paint operators were found.
    /// * `DocumentColorKind::PureRGB`   – only RGB paint operators were found.
    /// * `DocumentColorKind::Mixed`     – both CMYK and RGB operators are present.
    /// * `DocumentColorKind::Unknown`   – no recognizable color operators were found.
    ///
    /// # Notes
    ///
    /// Pages whose content stream cannot be decoded are silently skipped.
    pub fn detect_color_space(doc: &Document) -> DocumentColorKind {
        let mut has_cmyk = false;
        let mut has_rgb = false;

        for &page_id in doc.get_pages().values() {
            let Ok(content) = doc.get_and_decode_page_content(page_id) else {
                continue;
            };
            for op in &content.operations {
                match op.operator.as_str() {
                    "k" | "K" => has_cmyk = true,
                    "rg" | "RG" => has_rgb = true,
                    _ => {}
                }
                if has_cmyk && has_rgb {
                    return DocumentColorKind::Mixed;
                }
            }
        }

        match (has_cmyk, has_rgb) {
            (true, false) => DocumentColorKind::PureCMYK,
            (false, true) => DocumentColorKind::PureRGB,
            _ => DocumentColorKind::Unknown,
        }
    }

    /// Remaps a specific CMYK color to another color throughout the document.
    ///
    /// Applies a color substitution rule to every page in the document. Any CMYK paint
    /// command whose channel values are within `tolerance` of the `from` color will have
    /// its operands replaced with the `to` color values. Both fill (`k`) and stroke (`K`)
    /// operators are processed.
    ///
    /// # Arguments
    ///
    /// * `from`      – CMYK source color as `[C, M, Y, K]` with each channel in `0.0..=1.0`.
    /// * `to`        – CMYK target color as `[C, M, Y, K]` with each channel in `0.0..=1.0`.
    /// * `tolerance` – Maximum per-channel absolute difference for a color to be considered
    ///   a match. `0.0` requires an exact match; `1.0` matches any color.
    ///
    /// # Returns
    ///
    /// Returns `Ok(&mut Self)` on success, allowing method chaining, or an error if any
    /// page content stream could not be decoded or re-encoded.
    ///
    /// # Errors
    ///
    /// Returns an error if page content decoding or encoding fails for any page.
    ///
    /// # Example
    ///
    /// ```no_test
    /// // Replace pure black with a warm black within a 5 % tolerance
    /// pipeline.remap_color([0.0, 0.0, 0.0, 1.0], [0.0, 0.06, 0.12, 0.88], 0.05)?;
    /// ```
    pub fn remap_color(
        &mut self,
        from: [f64; 4],
        to: [f64; 4],
        tolerance: f64,
    ) -> crate::Result<&mut Self> {
        let remaps = ColorRemap {
            from,
            to,
            tolerance,
        };
        ColorRemap::apply(&mut self.doc, &[remaps])?;
        Ok(self)
    }

    /// Flattens all `Separation` spot color uses to their device CMYK alternates without
    /// applying any ICC transform.
    ///
    /// This is a lighter alternative to [`Self::convert_color_space`] for documents that have
    /// spot inks but don't need a full profile-to-profile conversion. Each `cs`/`scn`
    /// operator pair referencing a `Separation` color space is replaced with the equivalent
    /// device CMYK `k` operator evaluated from the embedded tint function.
    ///
    /// Returns the total number of spot color operator sequences replaced across all pages.
    ///
    /// # Errors
    ///
    /// Returns an error if any page's content stream cannot be decoded or re-encoded.
    #[cfg(feature = "color")]
    pub fn flatten_spots(&mut self) -> crate::Result<u32> {
        use rustybara_icc::pdf::flatten_spot_colors;
        Ok(flatten_spot_colors(&mut self.doc)?)
    }

    /// Applies an ICC color space conversion to every page in the document.
    ///
    /// Builds a [`rustybara_icc::ColorTransform`] from the named source and destination
    /// profiles, then walks every page's content stream, flattening spot colors and
    /// rewriting CMYK/RGB paint operators through the transform.
    ///
    /// # Arguments
    ///
    /// * `from_profile` – Machine-readable name of the source ICC profile (e.g. `"CoatedFOGRA39"`).
    /// * `to_profile`   – Machine-readable name of the destination ICC profile.
    /// * `intent`       – Rendering intent as a string: `"Perceptual"`, `"Saturation"`,
    ///   `"AbsoluteColorimetric"`, or anything else for `RelativeColorimetric` (the default).
    ///
    /// # Errors
    ///
    /// Returns an error if either profile name is unknown, if the lcms2 transform cannot be
    /// built, or if any page's content stream cannot be decoded or re-encoded.
    #[cfg(feature = "color")]
    pub fn convert_color_space(
        &mut self,
        from_profile: &str,
        to_profile: &str,
        intent: &str,
    ) -> crate::Result<()> {
        use rustybara_icc::pdf::PdfColorConverter;
        use rustybara_icc::{profiles, ColorTransform, IccError, RenderingIntent};

        let from = profiles::by_name(from_profile)
            .ok_or_else(|| IccError::Profile(format!("unknown source profile: {from_profile}")))?;
        let to = profiles::by_name(to_profile).ok_or_else(|| {
            IccError::Profile(format!("unknown destination profile: {to_profile}"))
        })?;
        let ri = match intent {
            "Perceptual" => RenderingIntent::Perceptual,
            "Saturation" => RenderingIntent::Saturation,
            "AbsoluteColorimetric" => RenderingIntent::AbsoluteColorimetric,
            _ => RenderingIntent::RelativeColorimetric,
        };
        let transform = ColorTransform::new(from, to, ri)?;
        PdfColorConverter::new(&mut self.doc, transform).convert_document()?;
        Ok(())
    }

    /// Applies an ICC color space conversion using raw profile bytes.
    ///
    /// Accepts pre-resolved bytes for both profiles, allowing callers to supply
    /// bundled or user-imported profiles without going through name lookup.
    ///
    /// # Errors
    ///
    /// Returns an error if the lcms2 transform cannot be built or if any page's
    /// content stream cannot be decoded or re-encoded.
    #[cfg(feature = "color")]
    pub fn convert_color_space_raw(
        &mut self,
        from_bytes: &[u8],
        to_bytes: &[u8],
        intent: &str,
    ) -> crate::Result<()> {
        use rustybara_icc::pdf::PdfColorConverter;
        use rustybara_icc::{ColorTransform, RenderingIntent};
        let ri = match intent {
            "Perceptual" => RenderingIntent::Perceptual,
            "Saturation" => RenderingIntent::Saturation,
            "AbsoluteColorimetric" => RenderingIntent::AbsoluteColorimetric,
            _ => RenderingIntent::RelativeColorimetric,
        };
        let transform = ColorTransform::from_bytes(from_bytes, to_bytes, ri)?;
        PdfColorConverter::new(&mut self.doc, transform).convert_document()?;
        Ok(())
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
        crate::encode::save(&image, path.as_ref(), format, config.dpi)?;
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
