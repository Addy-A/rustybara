//! Image encoding for rasterized PDF pages.
//!
//! This module provides utilities for encoding rendered PDF pages to various
//! image formats (JPEG, PNG, WebP, TIFF).
//!
//! ## Key Types
//!
//! - [`OutputFormat`] — Enum of supported output image formats
//!
//! ## Key Functions
//!
//! - [`save`] — Save a DynamicImage to disk in the specified format

pub mod save;
pub use save::{OutputFormat, save};