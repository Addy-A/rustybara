//! # rustybara-icc
//!
//! ICC color profile storage and transform engine for the rustybara workspace.
//!
//! This crate provides ICC-based color management capabilities, including loading
//! ICC profiles, performing color space transformations, and converting colors in
//! PDF documents between different color spaces.
//!
//! ## Core Functionality
//!
//! - Load and validate ICC profiles from bytes or bundled profiles
//! - Transform pixel data between color spaces (RGB, CMYK, Gray, Lab)
//! - Convert entire PDF documents from one color space to another
//! - Flatten spot colors in PDF content streams
//!
//! ## Quick Start
//!
//! ```no_run
//! use rustybara_icc::{ColorTransform, RenderingIntent, profiles};
//!
//! # fn main() -> rustybara_icc::Result<()> {
//! // Create a transform between two CMYK profiles
//! let transform = ColorTransform::new(
//!     &profiles::COATED_FOGRA_39,
//!     &profiles::COATED_GRACOL_2006,
//!     RenderingIntent::RelativeColorimetric,
//! )?;
//!
//! // Transform pixel data
//! let cmyk_in = vec![0, 0, 0, 255]; // Pure black
//! let cmyk_out = transform.convert(&cmyk_in);
//! # Ok(())
//! # }
//! ```
//!
//! ## PDF Color Conversion
//!
//! ```no_run
//! use rustybara_icc::{ColorTransform, RenderingIntent, profiles};
//! use rustybara_icc::pdf::PdfColorConverter;
//!
//! # fn main() -> rustybara_icc::Result<()> {
//! let mut doc = lopdf::Document::load("input.pdf").unwrap();
//!
//! let transform = ColorTransform::new(
//!     &profiles::COATED_FOGRA_39,
//!     &profiles::COATED_GRACOL_2006,
//!     RenderingIntent::RelativeColorimetric,
//! )?;
//!
//! let report = PdfColorConverter::new(&mut doc, transform)
//!     .convert_document()?;
//!
//! doc.save("output.pdf").unwrap();
//! # Ok(())
//! # }
//! ```
//!
//! ## Bundled Profiles
//!
//! The [`profiles`] module includes common prepress ICC profiles:
//! - `COATED_FOGRA_39` — Coated FOGRA39 (ISO 12647-2:2004)
//! - `COATED_GRACOL_2006` — Coated GRACoL 2006 (ISO 12647-2:2004)
//! - `UNCOATED_FOGRA_29` — Uncoated FOGRA29 (ISO 12647-2:2004)
//! - `SRGB` — sRGB IEC61966-2.1
//!
//! ## Architecture
//!
//! - [`ColorSpaceKind`] — Enum of supported color spaces
//! - [`ColorTransform`] — Pixel-level color transformation engine
//! - [`RenderingIntent`] — ICC rendering intent (Perceptual, Relative, etc.)
//! - [`PixelFormat`] — Pixel format descriptors (RGB8, CMYK8, etc.)
//! - [`pdf::PdfColorConverter`] — Document-level color space conversion

pub mod color_space;
pub mod error;
pub mod intent;
pub mod pdf;
pub mod pixel_format;
pub mod profiles;
pub mod transform;

pub use color_space::ColorSpaceKind;
pub use error::{IccError, Result};
pub use intent::RenderingIntent;
pub use pixel_format::PixelFormat;
pub use transform::ColorTransform;
