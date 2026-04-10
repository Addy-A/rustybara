# Rustybara — Project Design & Research Document

**Crate name:** `rustybara`
**Primary binary:** `rbara`
**Viewer binary:** `rbv`
**Status:** Pre-scaffold
**Author:** Addy Alvarado
**License:** MIT OR Apache-2.0
**MSRV:** Rust 1.85 (edition 2024). Compatibility with older Rust versions is in scope if community demand warrants it.

---

## What Rustybara Is

Rustybara (`rustybara`) is a prepress-focused Rust crate and interactive toolset
for graphic designers and prepress operators. It is the convergence of three
standalone open-source CLI tools into a unified library and interactive application,
built on the same primitives those tools proved out in production.

The three foundational tools:

| Tool | Binary | Primitive |
|---|---|---|
| `pdf-mark-removal` | `ptrim` | Content stream filtering, CTM math |
| `resize_to_bleed_or_trim_pdf` | `prsz` | Page box geometry (MediaBox, TrimBox, BleedBox) |
| `pdf-2-image` | `p2i` | PDF rasterization and image encoding |

Rustybara extracts, abstracts, and unifies these primitives into a high-level,
focused crate scoped for print production workflows — not a general-purpose low-level
PDF library. `lopdf` is a low-level primitive; `rustybara` is the opinionated
prepress layer built on top of it.

---

## Goals

- **Free forever.** No subscription, no license server, no internet connection required
  at runtime. A user who has a version of `rbara` has it permanently.
- **Open-accessible.** Ships as a single binary. Operators can put it on a USB drive
  and run it on any compatible machine.
- **Contributor-friendly.** High-quality READMEs, stated MSRV, clear module boundaries,
  and a public API designed so contributors can build standalone tools on top of
  `rustybara` without coupling to internal implementation details.
- **Prepress-first.** Every design decision — DPI defaults, box semantics, color handling —
  reflects the vocabulary and workflows of print production, not general software tooling.

---

## Interface Strategy

### The problem with plain CLI

The standalone tools (`ptrim`, `prsz`, `p2i`) expose a minimal flat CLI. This is
appropriate for scripting and power users but creates a barrier for graphic designers
and prepress operators who are not CLI-native. `rbara` solves this with a TUI layer
without sacrificing the CLI path.

### Decision: Ratatui + Clap + wgpu + winit

| Component | Role |
|---|---|
| `clap` | Flag-based CLI access for scripting, batch pipelines, power users |
| `ratatui` | Interactive TUI for designers and operators who want a guided interface |
| `wgpu` + `winit` | `rbv` — GPU-accelerated PDF page viewer, spawned on demand by `rbara` |

This combination preserves the "no browser, no server, no framework" philosophy of the
standalone tools while providing a genuinely interactive surface that non-CLI users
can navigate.

### Why not Tauri + Svelte

Tauri was considered and rejected. A webview-based UI is the wrong rendering surface
for prepress inspection — CMYK accuracy, zoom-to-registration-mark, and bleed boundary
verification all require a direct GPU render path, not an HTML canvas inside a Chromium
webview. Tauri also introduces a heavier distribution story than the project's
philosophy supports at this stage.

### Why not Leptos

Leptos requires a running server, which breaks the offline, no-subscription guarantee.
File system access from a web app is awkward for batch prepress workflows where operators
process directories of files. The I/O model and infrastructure dependency are not
acceptable tradeoffs for this audience.

### UX model: app-style, not modal

The TUI uses an app-style keyboard model:

- Arrow keys navigate
- Enter selects
- Escape cancels / goes back
- Single-letter shortcuts displayed in a persistent footer bar
- `?` opens an in-TUI keyboard reference at any screen

This is immediately discoverable for designers who have never used a TUI. Vim-style
bindings can be layered on as aliases in a later version. The priority is a low
barrier to first use, not power-user density.

---

## rbv — The PDF Page Viewer

### Architecture

`rbv` is a separate binary distributed alongside `rbara`. It is not a full GUI
application — it is a minimal GPU-accelerated window for PDF page preview, spawned
by `rbara` on demand and closed when the user dismisses it.

```
rbara (Ratatui TUI)
  └── on preview action:
        suspend TUI (LeaveAlternateScreen)
        spawn rbv <file_path> <page_index>
        wait for rbv exit
        resume TUI (EnterAlternateScreen)
```

`rbv` receives its input via command-line arguments (file path + page index). It
communicates back to `rbara` only on exit — via exit code or a small temp file if
page-level state needs to be passed (e.g. "user approved this page").

This pattern is the same model `lazygit` uses for spawning `$EDITOR`. It is
well-understood and requires no persistent IPC channel.

### Why wgpu + winit (not egui, iced, or a widget framework)

The viewer UI is trivially simple: a window, a texture, and keyboard navigation
(zoom, pan, next/prev page). A full GUI framework adds compilation weight and API
surface that isn't needed. `wgpu` + `winit` keeps `rbv` lean, fast to start, and
gives a direct GPU render path for the pdfium bitmap with correct color handling.

When annotation overlays are added (TrimBox, BleedBox, mark boundaries rendered as
colored geometry over the preview), `wgpu`'s render pipeline handles this with
additional geometry passes. A widget framework would fight this requirement.

### Graceful GPU fallback

`rbv` must not panic if no GPU is available (headless servers, remote workstations,
CI). Graceful fallback strategy:

1. Attempt hardware-accelerated backend (Vulkan / Metal / DX12)
2. Fall back to `wgpu::Backends::GL` (Mesa software renderer on Linux)
3. If no backend initializes, print a clear error and suggest piping output to
   a system image viewer instead

### Version pinning

`wgpu` and `winit` have frequent breaking API changes between minor versions. Both
must be pinned to explicit versions in `Cargo.toml` from the start, with the pinned
versions documented in `rbv`'s README. Do not rely on semver-compatible updates
without a manual review of changelog entries.

---

## Library Architecture

### Module structure

```
rustybara/
  Cargo.toml
  README.md
  LICENSE-MIT
  LICENSE-APACHE
  src/
    lib.rs
    geometry/
      mod.rs
      rect.rs       -- Rect struct (extracted from all three standalones)
      matrix.rs     -- Matrix struct (CTM math, extracted from ptrim)
    pages/
      mod.rs
      boxes.rs      -- PageBoxes: TrimBox, MediaBox, BleedBox, CropBox reader
    stream/
      mod.rs
      filter.rs     -- ContentFilter: CTM-walking content stream filter (from ptrim)
    raster/
      mod.rs
      render.rs     -- render_page, RenderConfig (from pdf-2-image)
    encode/
      mod.rs
      save.rs       -- encode DynamicImage → JPG / PNG / WebP (from pdf-2-image)
    color/
      mod.rs
      icc.rs        -- ICC profile-aware color conversion via lcms2 (future v1.x)
  rbara/
    Cargo.toml
    src/
      main.rs       -- rbara binary entry point
      cli.rs        -- clap definitions
      tui/
        app.rs      -- Ratatui app state
        ui.rs       -- render functions
        events.rs   -- keyboard event handling
  rbv/
    Cargo.toml
    src/
      main.rs       -- rbv binary entry point
      viewer.rs     -- wgpu + winit render loop
      texture.rs    -- DynamicImage → wgpu texture upload
```

### Public API design principle

`rustybara` is a **high-level, prepress-scoped crate**, not a general PDF manipulation
library. The public API surface should speak in prepress vocabulary:

```rust
// Good — prepress vocabulary
rustybara::pages::PageBoxes::trim_box(&doc, page_id)
rustybara::raster::render_page(&page, &RenderConfig { dpi: 300 })
rustybara::stream::ContentFilter::remove_outside_trim(&mut doc, page_id)

// Avoid exposing — too low-level, belongs inside the implementation
rustybara::geometry::Matrix::concat(&self, other: &Matrix)
```

Internal types (`Rect`, `Matrix`) are `pub(crate)` unless there is a concrete
reason a downstream user needs them. The goal is a focused API that a contributor
building a new standalone tool can learn in an afternoon.

### Renderer trait — future-proofing for GPU rasterization

The raster module exposes rendering behind a trait from the start:

```rust
pub trait PageRenderer {
    fn render(&self, page: &PdfPage, config: &RenderConfig)
        -> Result<DynamicImage, RenderError>;
}

pub struct CpuRenderer;  // pdfium-render, ships today
// pub struct GpuRenderer;  // vello/wgpu bridge, future work
```

`rbara` and downstream tools depend on `PageRenderer`, not `CpuRenderer` directly.
When a GPU backend is added it can be swapped in via feature flag with no public
API breakage.

---

## Dependencies

### Core library (`rustybara`)

| Crate | Version | Role |
|---|---|---|
| `lopdf` | `0.40` | Low-level PDF object graph manipulation |
| `pdfium-render` | `0.8` | PDF rasterization via PDFium engine |
| `image` | `0.25` | Bitmap encoding (JPG, PNG, WebP) |
| `rayon` | `1.10` | Parallel page rendering |
| `lcms2` | latest | ICC profile color management (color module, v1.x) |

### rbara binary

| Crate | Role |
|---|---|
| `ratatui` | TUI framework |
| `crossterm` | Terminal backend for ratatui |
| `clap` | CLI argument parsing |

### rbv binary

| Crate | Role |
|---|---|
| `wgpu` | GPU rendering (pinned) |
| `winit` | Window creation and event loop (pinned) |
| `bytemuck` | Safe cast of bitmap bytes to wgpu buffer |

---

## Distribution

### PDFium native dependency

PDFium is a native shared library required by `pdfium-render`. For the standalone
tools, dynamic linking with a documented fetch script is acceptable. For `rbara`,
it is not — a designer who gets a runtime error about a missing shared library will
not return.

**Decision:** `rbara` and `rbv` link PDFium statically using `pdfium-render`'s
`static` feature flag. Binary size increases (~20MB) but the binary ships as a
single file with zero runtime dependencies. The standalone tools (`ptrim`, `prsz`,
`p2i`) may continue to use dynamic linking.

### Release pipeline: cargo-dist

`cargo-dist` generates platform-specific release archives and installers from CI.
Integrate from the first `rbara` release.

Target platforms:

| Target | Notes |
|---|---|
| `x86_64-unknown-linux-gnu` | Primary Linux |
| `x86_64-pc-windows-msvc` | Windows |
| `x86_64-apple-darwin` | Intel Mac |
| `aarch64-apple-darwin` | Apple Silicon — significant portion of design audience |

---

## Color Management

### The CMYK problem

PDFium renders everything to sRGB internally. The `image` crate operates in sRGB.
Rasterized output from the current pipeline is a CMYK→sRGB conversion performed by
PDFium's internal color management, which may not match a RIP or Acrobat's output.

### Decision

`rustybara` will integrate `lcms2` for ICC profile-aware color conversion as a
feature in the `color` module. This will initially surface in `rbv` as a
soft-proof preview mode (display a CMYK PDF simulated for a target output profile)
and eventually as a color separation output feature in `rbara`.

This is a v1.x milestone. v1.0 documents the sRGB limitation explicitly. The module
stub is scaffolded from the start so the integration point is clear to contributors.

---

## Build Sequencing

The extraction and assembly must happen in dependency order. `rb` cannot be built
against `rustybara` until the library exists. Contributors should understand this
sequence:

```
Phase 1 — Extract rustybara lib
  - geometry module (Rect, Matrix from ptrim + prsz)
  - pages module (PageBoxes from prsz)
  - stream module (ContentFilter from ptrim)
  - raster module (render_page from pdf-2-image)
  - encode module (save from pdf-2-image)

Phase 2 — Port standalones to rustybara
  - ptrim depends on rustybara::geometry + rustybara::stream
  - prsz depends on rustybara::geometry + rustybara::pages
  - p2i depends on rustybara::raster + rustybara::encode

Phase 3 — Build rbara against rustybara
  - rbara depends on rustybara for all PDF operations
  - rbv depends on rustybara::raster for bitmap generation

Phase 4 — Publish
  - rustybara to crates.io
  - rbara + rbv via cargo-dist GitHub releases
```

Standalone tools (`ptrim`, `prsz`, `p2i`) continue to exist as independent
repositories and remain usable without `rustybara` installed. Post-port, they
reference `rustybara` as a dependency but are not subsumed into the monorepo.

---

## rbv — Spawn Protocol

```
rbv <file_path> <page_index> [--dpi <n>]
```

- `rbara` calls `LeaveAlternateScreen` before spawning
- `rbv` opens a native OS window, renders the page, handles keyboard navigation
- On ESC or window close, `rbv` exits with code 0 (or 1 on error)
- `rbara` calls `EnterAlternateScreen` after `rbv` exits and redraws the TUI

Page navigation within `rbv` (next/prev page) is handled inside `rbv` itself —
`rbara` does not need to be aware of which page the user ended on unless an
explicit "approve this page" workflow requires it, in which case a temp file
carries the result.

---

## Keyboard Reference (rbara)

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate file list / menu |
| `Enter` | Select / confirm |
| `Esc` | Cancel / go back |
| `p` | Preview current page (spawns rbv) |
| `m` | Remove print marks (ptrim operation) |
| `r` | Resize to bleed or trim (prsz operation) |
| `x` | Export to image (p2i operation) |
| `b` | Batch mode |
| `?` | Show keyboard reference overlay |
| `q` | Quit |

Single-letter shortcuts are shown in the persistent footer bar on every screen.

---

## Known Limitations (v1.0 scope)

| Limitation | Notes |
|---|---|
| sRGB output only | CMYK→sRGB via PDFium. ICC-accurate output is v1.x. |
| No TrimBox-aware rasterization | Crop to TrimBox with `prsz -t` before `rbara` export. |
| JPEG quality not configurable | Fixed encoder quality in v1.0. `--quality` flag is v1.1. |
| Spot color rendering | PDFium renders spot inks as CMYK approximations. RIP required for separation accuracy. |
| No Form XObject ColorSpace pruning | Inherited from `ptrim`. Documented in that repo. |
| rbv requires display server | No headless preview. Graceful error on missing GPU/display. |

---

## Positioning

Rustybara occupies a gap that currently has no direct open-source occupant: a
**free, offline, prepress-native PDF manipulation toolkit** with both a scriptable
CLI and an interactive TUI, distributed as a single binary with no subscription or
license requirement.

The closest analogues are:

- **Ghostscript** — powerful but hostile UX, not prepress-vocabulary-aware
- **QPDF** — excellent for structural manipulation, no rendering, no TUI
- **Adobe Acrobat** — the professional standard, expensive, closed, subscription
- **Enfocus PitStop** — prepress-focused, expensive, plugin-only

The Blender parallel is instructive: Blender did not win by being easier than Cinema 4D.
It won by being free, powerful, and having a community that wrote tutorials until
the UX barrier disappeared. `rbara`'s positioning is the same — a tool that takes
designers seriously enough to give them something professional-grade at no cost, with
documentation and tutorials that meet them where they are.

The "open-accessibility" value proposition — free forever, ships as a binary, runs
offline, no subscription — is the marketing foundation. It is also simply the correct
way to build tools for an audience that has been over-monetized by the Adobe ecosystem.

---

## Contributing

- Run `cargo test` before submitting a pull request
- MSRV is Rust 1.85 (edition 2024). PRs must not raise this floor without discussion
- The TrimBox is always the source-of-truth reference box. It is never modified by
  any operation in `rustybara`
- Public API additions require documentation and at least one integration test
- The app-style keyboard model is the UX baseline for `rbara`. Modal or vim-style
  additions are opt-in aliases only, never the primary path

---

## References

### PDF specification
- [PDF Reference 1.7](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf)
  — Chapters 7, 8, 9, 14.6, 14.11.2

### Crate documentation
- [ratatui](https://docs.rs/ratatui)
- [clap](https://docs.rs/clap)
- [wgpu](https://docs.rs/wgpu)
- [winit](https://docs.rs/winit)
- [pdfium-render](https://docs.rs/pdfium-render)
- [image](https://docs.rs/image)
- [rayon](https://docs.rs/rayon)
- [lcms2](https://docs.rs/lcms2)
- [lopdf](https://docs.rs/lopdf)

### Distribution
- [cargo-dist](https://opensource.axo.dev/cargo-dist/)
- [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)

### Tooling
- [qpdf](https://qpdf.sourceforge.io/)
- [Poppler utilities](https://poppler.freedesktop.org/)

---

Copyright (c) 2026 Addy Alvarado
Licensed under MIT OR Apache-2.0
