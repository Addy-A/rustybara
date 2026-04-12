# Rustybara

**Prepress-focused PDF manipulation toolkit for graphic designers and print operators.**

[![Crates.io](https://img.shields.io/crates/v/rustybara.svg)](https://crates.io/crates/rustybara)
[![Documentation](https://docs.rs/rustybara/badge.svg)](https://docs.rs/rustybara)
[![License](https://img.shields.io/badge/license-LGPL--3.0-blue.svg)](LICENSE-LGPL-3.0)

Rustybara is the convergence of three standalone prepress CLI tools into a
unified Rust library and interactive toolset, built on the same primitives
those tools proved in production:

| Origin Tool | Primitive |
|---|---|
| `pdf-mark-removal` | Content stream filtering, CTM math |
| `resize_to_bleed_or_trim_pdf` | Page box geometry (MediaBox, TrimBox, BleedBox) |
| `pdf-2-image` | PDF rasterization and image encoding |

It ships as a library crate (`rustybara`), a CLI/TUI binary (`rbara`), and a
GPU-accelerated PDF page viewer (`rbv`).

---

## Features

- **Trim print marks** — Strip printer marks, slug content, and anything outside
  the TrimBox from PDF pages.
- **Resize to bleed** — Expand MediaBox (and CropBox) by a specified bleed margin
  to prepare files for print production.
- **Export to image** — Rasterize PDF pages to JPEG, PNG, WebP, or TIFF at any DPI.
- **Pipeline API** — Chain operations fluently: `open → trim → resize → save`.
- **Batch processing** — Process entire directories of PDFs from CLI or TUI.
- **Interactive TUI** — App-style terminal interface for designers who prefer guided
  workflows over raw CLI flags.
- **Prepress vocabulary** — Every API surface speaks in boxes, bleeds, and DPI — not
  generic PDF primitives.

---

## Quick Start

### As a library

Add to your `Cargo.toml`:

```toml
[dependencies]
rustybara = "0.1"
```

```rust
use rustybara::PdfPipeline;

fn main() -> rustybara::Result<()> {
    // Trim marks, resize to 9pt bleed, save
    PdfPipeline::open("input.pdf")?
        .trim()?
        .resize(9.0)?
        .save_pdf("output.pdf")?;

    Ok(())
}
```

### Rasterize a page

```rust
use rustybara::{PdfPipeline, encode::OutputFormat, raster::RenderConfig};

fn main() -> rustybara::Result<()> {
    let pipeline = PdfPipeline::open("input.pdf")?;
    let config = RenderConfig::prepress(); // 300 DPI

    pipeline.save_page_image(0, "page_1.jpg", &OutputFormat::Jpg, &config)?;
    Ok(())
}
```

### CLI

```sh
# Trim print marks
rbara trim input.pdf

# Resize to 9pt bleed
rbara resize --bleed 9.0 input.pdf

# Export pages as 300 DPI PNGs
rbara image --format png --dpi 300 input.pdf
```

### TUI

Launch `rbara` with no arguments to enter the interactive terminal interface:

```sh
rbara
```

Arrow keys navigate, Enter selects, Esc goes back. Single-letter shortcuts are
shown in the footer bar. Press `?` for the full keyboard reference.

---

## Architecture

### Module Map

```
rustybara/src/
  lib.rs          — Public re-exports
  pipeline.rs     — PdfPipeline: high-level chaining API
  error.rs        — Unified error type
  geometry/
    rect.rs       — Rect (position + dimensions, PDF coordinate system)
    matrix.rs     — Matrix (2D affine CTM transformations)
  pages/
    boxes.rs      — PageBoxes: TrimBox, MediaBox, BleedBox, CropBox reader
  stream/
    filter.rs     — ContentFilter: CTM-walking content stream filter
  raster/
    render.rs     — PageRenderer trait, CpuRenderer (pdfium-render)
    config.rs     — RenderConfig (DPI, annotation toggles)
  encode/
    save.rs       — OutputFormat enum, image encoding (JPG/PNG/WebP/TIFF)
  color/
    icc.rs        — ICC profile color management (planned, feature-gated)
```

### Public API

`rustybara` is a high-level, prepress-scoped crate. The public API speaks in
prepress vocabulary:

```rust
// Prepress operations
PdfPipeline::open(path)?
    .trim()?                    // Remove content outside TrimBox
    .resize(bleed_pts)?         // Expand page boxes by bleed margin
    .save_pdf(path)?;           // Write the result

// Rasterization
pipeline.render_page(0, &config)?;                          // → DynamicImage
pipeline.save_page_image(0, path, &format, &config)?;       // → file

// Page inspection
let boxes = PageBoxes::read(&doc, page_id)?;
boxes.trim_or_media()           // TrimBox if present, else MediaBox
boxes.bleed_rect(9.0)           // Expand trim by bleed amount
```

### Renderer Trait

Rendering is behind a trait for future GPU backend support:

```rust
pub trait PageRenderer {
    fn render(&self, page: &PdfPage, config: &RenderConfig)
        -> Result<DynamicImage>;
}

pub struct CpuRenderer;   // pdfium-render — ships today
// pub struct GpuRenderer; // vello/wgpu — future work
```

---

## Dependencies

| Crate | Role |
|---|---|
| [`lopdf`](https://docs.rs/lopdf) 0.40 | PDF object graph manipulation |
| [`pdfium-render`](https://docs.rs/pdfium-render) 0.9 | PDF rasterization via PDFium |
| [`image`](https://docs.rs/image) 0.25 | Bitmap encoding (JPEG, PNG, WebP, TIFF) |
| [`rayon`](https://docs.rs/rayon) 1.11 | Parallel page rendering |

### Runtime Requirement — PDFium

The `render_page` and `save_page_image` functions require the PDFium shared library
at runtime. Place the appropriate binary alongside your executable:

| Platform | File |
|---|---|
| Windows | `pdfium.dll` |
| macOS | `libpdfium.dylib` |
| Linux | `libpdfium.so` |

Pre-built binaries: [pdfium-binaries](https://github.com/nicokoenig/pdfium-binaries)

Operations that do not rasterize (`trim`, `resize`, `save_pdf`, `page_count`,
`PageBoxes::read`) work without PDFium.

---

## rbara — CLI & TUI Binary

`rbara` is the interactive front-end for `rustybara`. It provides both a
flag-based CLI for scripting and a TUI for guided workflows.

### Keyboard Reference (TUI)

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate menu |
| `Enter` | Select / confirm |
| `Esc` | Back / quit |
| `m` | Trim print marks |
| `r` | Resize to bleed |
| `x` | Export to image |
| `p` | Preview page |
| `o` | Toggle overwrite mode |
| `c` | Change files |
| `q` | Quit |
| `?` | Keyboard reference overlay |

### UX Model

The TUI follows an app-style keyboard model — arrow keys, Enter, Esc — designed
for designers who have never used a terminal before. Vim-style bindings may be
layered on as aliases in a future version.

File-first workflow: launch → select file or directory → commands become
available. Directories auto-glob `*.pdf` files.

---

## rbv — PDF Page Viewer

`rbv` is a minimal GPU-accelerated window for PDF page preview, built on
`wgpu` + `winit`. It is spawned by `rbara` on demand and communicates via
command-line arguments and exit codes.

```
rbv <file_path> <page_index> [--dpi <n>]
```

**Status:** Not yet implemented. See the roadmap below.

---

## Known Limitations

| Limitation | Notes |
|---|---|
| sRGB output only | CMYK→sRGB via PDFium. ICC-accurate output planned for v1.x (`color` feature). |
| JPEG quality not configurable | Fixed encoder quality. `--quality` flag planned. |
| Spot color approximation | PDFium renders spot inks as CMYK approximations. |
| No Form XObject ColorSpace pruning | Inherited limitation from content stream filtering. |
| `rbv` requires display server | No headless preview. Graceful error on missing GPU. |

---

## Roadmap

- [ ] ICC color management (`color` module via `lcms2`)
- [ ] `rbv` GPU-accelerated page viewer
- [ ] PDF/X validation and preflight reports
- [ ] Configurable JPEG quality (`--quality` flag)
- [ ] `cargo-dist` release pipeline (Linux, Windows, macOS including Apple Silicon)

---

## Contributing

```sh
cargo test --workspace
```

- MSRV is Rust 1.85 (edition 2024). Do not raise this floor without discussion.
- The TrimBox is always the source-of-truth reference box. It is never modified
  by any operation.
- Public API additions require documentation and at least one integration test.
- The app-style keyboard model is the UX baseline for `rbara`. Modal bindings
  are opt-in aliases only.

---

## License

- **`rustybara` (library):** [LGPL-3.0-only](LICENSE-LGPL-3.0)
- **`rbara` and `rbv` (binaries):** [GPL-3.0-only](LICENSE-GPL-3.0)

The LGPL license on the library allows downstream tools to link against
`rustybara` without copyleft obligations on their own code, while the
binaries remain fully copyleft.

Copyright (c) 2026 Addy Alvarado
