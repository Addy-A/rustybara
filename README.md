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
- **Remap CMYK colors** — Substitute specific CMYK values in PDF content streams with
  tolerance-based matching.
- **ICC color management** — Feature-gated (`color`) lcms2 integration for ICC profile
  loading and color space transforms (RGB, CMYK, Gray).
- **Pipeline API** — Chain operations fluently: `open → trim → resize → remap → save`.
- **Batch processing** — Process entire directories of PDFs from CLI or TUI.
- **Interactive TUI** — App-style terminal interface for designers who prefer guided
  workflows over raw CLI flags. Configurable output directory.
- **Prepress vocabulary** — Every API surface speaks in boxes, bleeds, and DPI — not
  generic PDF primitives.

---

## Installation

Pre-built installers for `rbara` (the CLI/TUI binary) are published with each
release. Each installer bundles its own `pdfium` runtime — no system pdfium
needed.

### Windows

Download `rbara-setup-<version>-x64.exe` from the
[Releases page](https://github.com/Addy-A/rustybara/releases) and run it.
This is a per-user Inno Setup installer (no admin required) that installs to
`%LOCALAPPDATA%\Programs\rbara\`, registers an opt-in PATH entry, and adds
an Add/Remove Programs entry. SmartScreen may warn the first time — the
binary is currently unsigned.

### macOS

```sh
# Apple silicon
tar -xzf rbara-<version>-macos-arm64.tar.gz && cd rbara-<version>-macos-arm64
./install.sh        # installs to ~/.local

# Intel
tar -xzf rbara-<version>-macos-x86_64.tar.gz && cd rbara-<version>-macos-x86_64
./install.sh
```

The bundle is unsigned; `install.sh` strips the `com.apple.quarantine`
attribute automatically. To uninstall: `./uninstall.sh`.

### Linux (glibc x86_64)

```sh
tar -xzf rbara-<version>-linux-x64.tar.gz && cd rbara-<version>-linux-x64
./install.sh                       # ~/.local
sudo PREFIX=/usr/local ./install.sh   # system-wide
```

Tested on Ubuntu 22.04+, Debian 12+, Fedora 38+, RHEL 9+, Arch, openSUSE
Tumbleweed. Musl distros (Alpine) need a source build. To uninstall:
`./uninstall.sh`.

### Docker

```sh
docker pull ghcr.io/addy-a/rbara:latest

# CLI usage — bind-mount your working directory
docker run --rm -v "$PWD:/work" ghcr.io/addy-a/rbara:latest \
  trim /work/in.pdf -o /work/out.pdf
```

The image is ~175 MB (debian:bookworm-slim base) and runs as a non-root user.

### Building from source

See the [Contributing](#contributing) section. The maintainer-side installer
scripts live in [`installer/`](installer/) (one subdir per platform, each
with its own README).

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

# Remap a CMYK color (rich black → 60/40/20/100)
rbara remap-color --from 1.0 1.0 1.0 1.0 --to 0.6 0.4 0.2 1.0 input.pdf
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
    color_ops.rs  — ColorRemap: CMYK→CMYK value substitution in content streams
  raster/
    render.rs     — PageRenderer trait, CpuRenderer (pdfium-render)
    config.rs     — RenderConfig (DPI, annotation toggles)
  encode/
    save.rs       — OutputFormat enum, image encoding (JPG/PNG/WebP/TIFF)
  color/            (feature-gated: "color")
    icc.rs        — ColorSpace, IccProfile (ICC profile loading + detection)
    transform.rs  — ColorTransform, RenderingIntent (lcms2 bridge)
```

### Public API

`rustybara` is a high-level, prepress-scoped crate. The public API speaks in
prepress vocabulary:

```rust
// Prepress operations
PdfPipeline::open(path)?
    .trim()?                    // Remove content outside TrimBox
    .resize(bleed_pts)?         // Expand page boxes by bleed margin
    .remap_color(from, to, tolerance)?  // Substitute CMYK values
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
| [`lcms2`](https://docs.rs/lcms2) 6.1 | ICC color management (optional, `color` feature) |

### Runtime Requirement — PDFium

The `render_page` and `save_page_image` functions require the PDFium shared library
at runtime. Place the appropriate binary alongside your executable:

| Platform | File |
|---|---|
| Windows | `pdfium.dll` |
| macOS | `libpdfium.dylib` |
| Linux | `libpdfium.so` |

Pre-built binaries: [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)

> **Note:** End-users of the `rbara` binary do **not** need to do this manually
> — the [pre-built installers](#installation) bundle the matching pdfium for
> each platform. This requirement applies only when consuming `rustybara` as a
> library in your own Rust project.

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
| `t` | Trim print marks |
| `r` | Resize to bleed |
| `x` | Export to image |
| `m` | Remap colors |
| `p` | Preview page |
| `o` | Toggle overwrite mode |
| `/` | Output path |
| `f` | Change files |
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
| sRGB rasterization only | CMYK→sRGB via PDFium. ICC color transforms available via `color` feature for stream-level operations. |
| JPEG quality not configurable | Fixed encoder quality. `--quality` flag planned. |
| Spot color approximation | PDFium renders spot inks as CMYK approximations. |
| No Form XObject ColorSpace pruning | Inherited limitation from content stream filtering. |
| `rbv` requires display server | No headless preview. Graceful error on missing GPU. |

---

## Roadmap

- [x] ICC color management (`color` module via `lcms2`) — v0.1.2
- [x] CMYK→CMYK color remapping in content streams — v0.1.2
- [x] Cross-platform installers (Windows / macOS / Linux / Docker) with bundled pdfium — v0.1.3
- [x] GitHub Actions release pipeline (one tag → all installers + GHCR image) — v0.1.3
- [ ] RGB→CMYK conversion (vector graphics + embedded images)
- [ ] Spot color detection service
- [ ] `rbv` GPU-accelerated page viewer
- [ ] PDF/X validation and preflight reports
- [ ] Configurable JPEG quality (`--quality` flag)

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

### Cutting a release

Releases are fully automated by [`.github/workflows/release.yml`](.github/workflows/release.yml).
To cut a new version:

1. Bump `version` in `rbara/Cargo.toml` (and `rustybara/Cargo.toml` if the lib changed).
2. Commit and push.
3. Tag and push the tag:
   ```sh
   git tag v0.1.4
   git push --tags
   ```
4. The workflow will build the Windows installer, the Linux tarball, both
   macOS tarballs (Apple silicon + Intel), and the Docker image, then create
   a GitHub Release with all artifacts and a `SHA256SUMS.txt` attached.

The pdfium chromium build is pinned via `PDFIUM_CHROMIUM` env var in the
workflow (currently `7776`). Bump it there to refresh pdfium across all
artifacts in lockstep.

---

## License

- **`rustybara` (library):** [LGPL-3.0-only](LICENSE-LGPL-3.0)
- **`rbara` and `rbv` (binaries):** [GPL-3.0-only](LICENSE-GPL-3.0)

The LGPL license on the library allows downstream tools to link against
`rustybara` without copyleft obligations on their own code, while the
binaries remain fully copyleft.

Copyright (c) 2026 Addy Alvarado
