#[cfg(feature = "color")]
pub mod color;
pub mod encode;
mod error;
pub mod geometry;
pub mod pages;
pub mod raster;
pub mod stream;
pub mod pipeline;

pub use error::Error;
pub use error::Result;

pub use pipeline::PdfPipeline;