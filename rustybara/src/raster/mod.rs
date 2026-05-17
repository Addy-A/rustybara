//! PDF page rasterization and rendering.
//!
//! This module provides functionality for converting PDF pages to raster images
//! using PDFium as the rendering engine.
//!
//! ## Key Types
//!
//! - [`PageRenderer`] — Trait for rendering PDF pages to images
//! - [`CpuRenderer`] — CPU-based renderer using pdfium-render
//! - [`RenderConfig`] — Configuration for DPI, background, and annotation rendering
//!
//! ## Key Functions
//!
//! - [`render_page`] — Convenience function to render a single page
//!
//! ## Runtime Requirements
//!
//! Rendering requires the PDFium shared library at runtime:
//! - Windows: `pdfium.dll`
//! - macOS: `libpdfium.dylib`
//! - Linux: `libpdfium.so`

mod render;
mod config;

pub use render::{PageRenderer, CpuRenderer, render_page};
pub use config::RenderConfig;