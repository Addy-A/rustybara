/// Errors that can occur during ICC color management operations.
///
/// # Variants
///
/// * `Transform` — Error during color transformation (e.g., invalid pixel data, incompatible profiles)
/// * `UnsupportedColorSpace` — Attempted operation on an unsupported color space
/// * `Pdf` — Error reading or manipulating PDF objects
/// * `Image` — Error processing image data
/// * `Profile` — Error loading or parsing an ICC profile
#[derive(thiserror::Error, Debug)]
pub enum IccError {
    #[error("lcms2 transform error: {0}")]
    Transform(String),
    #[error("unsupported color space: {0}")]
    UnsupportedColorSpace(String),
    #[error("PDF object error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("image processing error: {0}")]
    Image(String),
    #[error("profile loading error: {0}")]
    Profile(String),
}

pub type Result<T> = std::result::Result<T, IccError>;
