//! PDF content stream manipulation utilities.
//!
//! This module provides tools for parsing and modifying PDF content streams,
//! including filtering operations by geometry and remapping color values.
//!
//! ## Key Types
//!
//! - [`ContentFilter`] — Filter content stream operations based on spatial constraints
//! - [`ColorRemap`] — Remap CMYK color values with tolerance-based matching
//!
//! ## Common Use Cases
//!
//! - Removing content outside page boundaries (trim marks, bleed removal)
//! - Substituting specific CMYK values (e.g., converting rich black to process black)

mod color_ops;
mod filter;
mod layout;

pub use color_ops::ColorRemap;
pub use filter::ContentFilter;
pub use layout::page_layout;
