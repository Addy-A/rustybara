//! PDF page object tree — logical painted objects extracted from content streams.
//!
//! Provides a structured view of every painted element on a page: filled and stroked
//! paths, placed images, form XObjects, and text runs — in back-to-front paint order.
//!
//! ## Key Types
//!
//! - [`PageObject`] — A singled painted shape, image, or text run with its geometry
//! - [`ObjectTree`] — All objects on a page, in paint order (back to front)
//! - [`ObjectKind`] — Discriminates Fill / Stroke / FillStroke / Text / Image / Form
//! - [`PdfColor`] — Per-object device color value (distinct from document-level classification)
//! - [`CmykChannel`] — Selects a CMYK process ink channel for plate filtering
//!
//! ## Key Functions
//!
//! - [`build_object_tree`] — Parse a page content stream into an [`ObjectTree`]
//! - [`hit_test`] — Find all objects that contain a given page-space coordinate
//! - [`separation::filter_by_ink`] — Filter objects by ink channel or spot color name

pub mod hittest;
pub mod separation;
pub mod tree;

pub use hittest::hit_test;
pub use separation::{filter_by_ink, InkSelector};
pub use tree::{
    CmykChannel, ObjectKind, ObjectTree, OverprintState, PageObject, PathPoint, PdfColor, SubPath,
    build_object_tree,
};
