//! Text outline extraction – vectorize embedded PDF glyphs into path geometry.
//!
//! # Feature gate
//! This module is only compiled when the `outline` Cargo feature is enabled.
//!
//! # Modules
//! - [`font`] – Extract raw font bytes from a PDF's resource dictionary.
//! - [`encoding`] – Resolve character codes to `ttf_parser::GlyphId`.
//! - [`paths`] – Walk a page content stream and produce per-glyph path geometry.
//! - [`writer`] – Serialize glyph paths back to PDF path operator sequences.

pub mod encoding;
pub mod font;
pub mod paths;
pub mod writer;

pub use paths::{outline_page_text, GlyphVerb, PositionedGlyph};
