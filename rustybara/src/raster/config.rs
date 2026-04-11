/// Configuration structure for controlling PDF rendering behavior.
///
/// This struct defines the parameters that control how a PDF document is rendered,
/// including display resolution and what elements should be included in the output.
///
/// # Examples
///
/// ```
/// use rustybara::raster::RenderConfig;
/// let config = RenderConfig {
///     dpi: 300,
///     render_annotations: true,
///     render_form_data: false,
/// };
/// ```
pub struct RenderConfig {
    pub dpi: u32,
    pub render_annotations: bool,
    pub render_form_data: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            dpi: 150,
            render_annotations: true,
            render_form_data: true,
        }
    }
}

impl RenderConfig {
    /// Creates a new configuration preset optimized for prepress workflows.
    ///
    /// This preset sets the DPI to 300, which is a standard resolution for high-quality
    /// print production. All other configuration values fall back to their default settings.
    ///
    /// # Returns
    ///
    /// A new instance of the configuration struct with prepress-optimized settings.
    ///
    /// # Example
    ///
    /// ```
    /// use rustybara::raster::RenderConfig;
    /// let config = RenderConfig::prepress();
    /// assert_eq!(config.dpi, 300);
    /// ```
    pub fn prepress() -> Self {
        Self {
            dpi: 300,
            ..Self::default()
        }
    }
}
