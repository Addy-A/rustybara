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
