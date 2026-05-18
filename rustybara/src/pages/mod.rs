//! PDF page manipulation and geometry utilities.
//!
//! This module provides tools for working with PDF page boxes (MediaBox, TrimBox, BleedBox, CropBox),
//! extracting subsets of pages, and splitting documents.
//!
//! ## Key Types
//!
//! - [`PageBoxes`] — Read and manipulate PDF page box geometry
//!
//! ## Key Functions
//!
//! - [`extract_pages`] — Create a new document containing only specified pages
//! - [`set_trim_boxes`] — Add TrimBox entries to all pages by insetting MediaBox

pub mod boxes;
pub mod split;
pub mod spread;
pub mod stitch;
pub use boxes::{set_trim_boxes, PageBoxes};
pub use split::extract_pages;
pub use spread::split_pages;
pub use stitch::stitch_pages;