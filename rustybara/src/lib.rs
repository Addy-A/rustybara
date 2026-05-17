//! # Rustybara
//!
//! Prepress-focused PDF manipulation library for graphic designers and print operators.
//!
//! Rustybara provides a high-level API for common prepress operations like trimming print marks,
//! resizing pages with bleed margins, rasterizing to images, and remapping CMYK colors. It speaks
//! in prepress vocabulary (TrimBox, BleedBox, DPI) rather than low-level PDF primitives.
//!
//! ## Core Operations
//!
//! - **Trim print marks** — Remove content outside TrimBox boundaries
//! - **Resize to bleed** — Expand MediaBox by specified bleed margins
//! - **Export to image** — Rasterize pages to JPEG, PNG, WebP, or TIFF
//! - **Remap CMYK colors** — Substitute specific CMYK values with tolerance matching
//! - **ICC color management** — Convert between color spaces using ICC profiles (via `color` feature)
//!
//! ## Quick Start
//!
//! ```no_run
//! use rustybara::PdfPipeline;
//!
//! # fn main() -> rustybara::Result<()> {
//! // Trim marks and resize to 9pt bleed
//! PdfPipeline::open("input.pdf")?
//!     .trim()?
//!     .resize(9.0)?
//!     .save_pdf("output.pdf")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Rasterization Example
//!
//! ```no_run
//! use rustybara::{PdfPipeline, encode::OutputFormat, raster::RenderConfig};
//!
//! # fn main() -> rustybara::Result<()> {
//! let pipeline = PdfPipeline::open("input.pdf")?;
//! let config = RenderConfig::prepress(); // 300 DPI
//!
//! pipeline.save_page_image(0, "page_1.jpg", &OutputFormat::Jpg, &config)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Feature Flags
//!
//! - `color` — Enables ICC color management via the [`rustybara-icc`](https://github.com/Addy-A/rustybara/tree/main/rustybara-icc) crate
//! - `gpu` — Reserved for future GPU-accelerated rendering (not yet implemented)
//!
//! ## Architecture
//!
//! The library is organized around the [`PdfPipeline`] struct, which provides a chainable API
//! for PDF operations. Under the hood:
//!
//! - [`pages`] — Page box geometry and manipulation
//! - [`stream`] — Content stream filtering and color remapping
//! - [`raster`] — PDF rendering via PDFium
//! - [`encode`] — Image encoding to various formats
//! - [`geometry`] — 2D geometry primitives (Rect, Matrix)
//! - `color` — ICC color management (feature-gated, requires `color` feature)

#[cfg(feature = "color")]
pub mod color;
#[cfg(feature = "raster")]
pub mod encode;
pub mod error;
pub mod geometry;
pub mod pages;
#[cfg(feature = "raster")]
pub mod raster;
pub mod stream;
pub mod pipeline;

pub use error::Error;
pub use error::Result;

pub use pipeline::DocumentColorKind;
pub use pipeline::PdfPipeline;