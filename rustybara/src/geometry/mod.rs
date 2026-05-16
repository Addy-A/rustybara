//! 2D geometry primitives for PDF coordinate system operations.
//!
//! This module provides low-level geometric primitives used throughout rustybara
//! for working with PDF coordinates, transformations, and bounding boxes.
//!
//! ## Key Types
//!
//! - [`Rect`] — A rectangle with position and dimensions in PDF coordinate space
//! - [`Matrix`] — A 2D affine transformation matrix (6-component CTM)
//!
//! ## PDF Coordinate System
//!
//! PDFs use a bottom-left origin coordinate system where:
//! - X increases to the right
//! - Y increases upward
//! - Units are points (1/72 of an inch)
//!
//! This differs from many graphics systems that use top-left origins.

mod rect;
mod matrix;

pub use rect::Rect;
pub use matrix::Matrix;