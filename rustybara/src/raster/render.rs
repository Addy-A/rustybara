use crate::raster::config::RenderConfig;
use image::DynamicImage;
use pdfium_render::prelude::*;

pub trait PageRenderer {
    /// Renders a PDF page to a dynamic image format.
    ///
    /// This function converts the specified PDF page into a `DynamicImage` using the provided
    /// rendering configuration. The resulting image can be in various formats (RGBA, RGB, etc.)
    /// depending on the configuration settings.
    ///
    /// # Arguments
    ///
    /// * `page` - A reference to the `PdfPage` to be rendered
    /// * `config` - A reference to the `RenderConfig` that specifies rendering parameters
    ///   such as scale factor, rotation, and image format
    ///
    /// # Returns
    ///
    /// * `Ok(DynamicImage)` - The rendered page as a dynamic image on success
    /// * `Err(PdfiumError)` - An error if the rendering process fails, such as invalid
    ///   page data, unsupported configuration, or internal PDFium rendering errors
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Assuming `renderer` is a PdfRenderer instance
    /// let config = RenderConfig::new()
    ///     .set_scale_factor(2.0)
    ///     .set_rotate_if_landscape(RotateValue::Degrees90);
    ///
    /// match renderer.render(&page, &config) {
    ///     Ok(image) => {
    ///         // Use the rendered image
    ///         println!("Rendered image dimensions: {}x{}", image.width(), image.height());
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Failed to render page: {}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Note
    ///
    /// The quality and performance of rendering may depend on the PDFium library version
    /// and the complexity of the PDF content being rendered.
    fn render(&self, page: &PdfPage, config: &RenderConfig) -> Result<DynamicImage, PdfiumError>;
}

/// A CPU-based renderer for generating images without GPU acceleration.
///
/// The `CpuRenderer` struct provides software rendering capabilities,
/// allowing for image generation and manipulation using only CPU resources.
/// This renderer is useful for environments where GPU acceleration is
/// unavailable or when consistent, predictable rendering performance is required.
///
/// # Examples
///
/// ```no_test
/// let renderer = CpuRenderer::new();
/// // Use the renderer for software-based image processing
/// ```
pub struct CpuRenderer;
/// A graphics processing unit (GPU) renderer responsible for handling
/// hardware-accelerated rendering operations.
///
/// This struct provides an interface for rendering graphics using GPU
/// capabilities, enabling efficient processing of visual data through
/// parallel computation and specialized hardware functions.
///
/// # Examples
///
/// ```no_test
/// let renderer = GpuRenderer::new();
/// // Use renderer for graphics operations
/// ```
///
/// # Thread Safety
///
/// The thread safety characteristics of this renderer depend on the
/// underlying GPU driver implementation and should be consulted in
/// the specific platform documentation.
#[cfg(feature = "gpu")]
pub struct GpuRenderer; // Stubbed

impl PageRenderer for CpuRenderer {
    fn render(&self, page: &PdfPage, config: &RenderConfig) -> Result<DynamicImage, PdfiumError> {
        let w = (page.width().value * config.dpi as f32 / 72.0) as i32;
        let h = (page.height().value * config.dpi as f32 / 72.0) as i32;

        let render_cfg = PdfRenderConfig::new()
            .set_target_width(w)
            .set_target_height(h)
            .render_annotations(config.render_annotations)
            .render_form_data(config.render_form_data);

        page.render_with_config(&render_cfg)
            .and_then(|bitmap| bitmap.as_image())
    }
}

/// Renders a PDF page to a dynamic image using the CPU renderer.
///
/// This function takes a reference to a `PdfPage` and rendering configuration,
/// and returns a `DynamicImage` containing the rendered page contents. The
/// rendering is performed using the CPU-based renderer implementation.
///
/// # Arguments
///
/// * `page` - A reference to the `PdfPage` to be rendered
/// * `config` - A reference to the `RenderConfig` containing rendering parameters
///   such as scale, rotation, and color settings
///
/// # Returns
///
/// * `Ok(DynamicImage)` - The rendered page as a dynamic image on success
/// * `Err(PdfiumError)` - A `PdfiumError` if rendering fails
///
/// # Example
///
/// ```no_test
/// use pdfium::prelude::*;
///
/// let document = PdfDocument::load("document.pdf")?;
/// let page = document.pages().get(0)?;
/// let config = RenderConfig::new()
///     .set_scale(2.0)
///     .set_rotation(PageRotation::Degrees0);
///
/// let image = render_page(&page, &config)?;
/// ```
pub fn render_page(page: &PdfPage, config: &RenderConfig) -> Result<DynamicImage, PdfiumError> {
    CpuRenderer.render(page, config)
}
