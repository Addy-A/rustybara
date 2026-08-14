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
pub mod extract;
pub mod spread;
pub mod stitch;
pub use boxes::{PageBoxes, set_media_box, set_trim_boxes};
pub use extract::extract_pages;
pub use spread::{SplitAxis, split_pages, split_pages_explicit};
pub use stitch::stitch_pages;
