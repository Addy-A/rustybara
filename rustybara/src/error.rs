use image::ImageError;
#[cfg(feature = "color")]
use lcms2::Error as Lcms2Error;
use lopdf::Error as LopdfError;
use pdfium_render::prelude::PdfiumError;
use std::fmt;
use std::io::Error as IoError;

pub type Result<T> = std::result::Result<T, Error>;

/// Represents errors that can occur during PDF processing operations.
///
/// This enum consolidates various error types that may arise when working with PDF files,
/// including image processing, file I/O operations, PDF parsing, and rendering errors.
///
/// # Variants
///
/// * `Image(ImageError)` - Errors related to image processing operations
/// * `Io(IoError)` - File system and I/O related errors
/// * `Pdf(LopdfError)` - Errors from the lopdf library when parsing or manipulating PDFs
/// * `Render(PdfiumError)` - Errors from Pdfium when rendering PDF content
///
/// # Examples
///
/// ```no_test
/// use crate::Error;
/// use std::io;
///
/// fn handle_pdf_error(error: Error) {
///     match error {
///         Error::Image(e) => println!("Image error: {:?}", e),
///         Error::Io(e) => println!("IO error: {:?}", e),
///         Error::Pdf(e) => println!("PDF error: {:?}", e),
///         Error::Render(e) => println!("Rendering error: {:?}", e),
///     }
/// }
/// ```
#[derive(Debug)]
pub enum Error {
    Image(ImageError),
    Io(IoError),
    Pdf(LopdfError),
    Render(PdfiumError),
    #[cfg(feature = "color")]
    Color(Lcms2Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Image(e) => write!(f, "image error: {e}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Pdf(e) => write!(f, "PDF error: {e}"),
            Error::Render(e) => write!(f, "Render error: {e}"),
            #[cfg(feature = "color")]
            Error::Color(e) => write!(f, "Color error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Image(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Pdf(e) => Some(e),
            Error::Render(e) => Some(e),
            #[cfg(feature = "color")]
            Error::Color(e) => Some(e),
        }
    }
}

impl From<ImageError> for Error {
    fn from(err: ImageError) -> Error {
        Error::Image(err)
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Error {
        Error::Io(err)
    }
}

impl From<LopdfError> for Error {
    fn from(err: LopdfError) -> Error {
        Error::Pdf(err)
    }
}

impl From<PdfiumError> for Error {
    fn from(err: PdfiumError) -> Self {
        Error::Render(err)
    }
}

#[cfg(feature = "color")]
impl From<Lcms2Error> for Error {
    fn from(err: Lcms2Error) -> Self {
        Error::Color(err)
    }
}
