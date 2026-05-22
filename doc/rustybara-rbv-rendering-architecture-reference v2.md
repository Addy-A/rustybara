# Rustybara — rbv Rendering Architecture Reference

> **Purpose:** This is a reference document covering the full architectural research
> session for `rbv`, the PDF viewer component of Rustybara.
> All file paths are relative to the workspace root `/home/user/rustybara/`.

---

## Index

1. [Executive Summary](#1-executive-summary)
2. [Workspace Structure](#2-workspace-structure)
3. [Current Rendering Stack](#3-current-rendering-stack)
   - 3.1 [The Two-Layer Model](#31-the-two-layer-model)
   - 3.2 [Startup Sequence (The Problem)](#32-startup-sequence-the-problem)
   - 3.3 [Current Data Flow](#33-current-data-flow)
4. [Decision: Replace wgpu with Skia](#4-decision-replace-wgpu-with-skia)
   - 4.1 [Why wgpu Alone Is Insufficient](#41-why-wgpu-alone-is-insufficient)
   - 4.2 [Why Skia](#42-why-skia)
   - 4.3 [Scope of the Swap](#43-scope-of-the-swap)
5. [Skia Integration Guide](#5-skia-integration-guide)
   - 5.1 [Dependencies](#51-dependencies)
   - 5.2 [Window + Skia Surface Setup](#52-window--skia-surface-setup)
   - 5.3 [Replacing texture.rs](#53-replacing-texturers)
   - 5.4 [Replacing the wgpu Pipeline in viewer.rs](#54-replacing-the-wgpu-pipeline-in-viewerrs)
   - 5.5 [Drawing Overlays with Skia](#55-drawing-overlays-with-skia)
6. [Viewer Invocation Design & Latency](#6-viewer-invocation-design--latency)
   - 6.1 [Design Intent: Explicit Invocation Only](#61-design-intent-explicit-invocation-only)
   - 6.2 [Root Cause of the Stall](#62-root-cause-of-the-stall)
   - 6.3 [Async Rendering Pattern (Within Explicit Invocation)](#63-async-rendering-pattern-within-explicit-invocation)
   - 6.4 [Progressive DPI Pattern](#64-progressive-dpi-pattern)
7. [Interactive Features Architecture](#7-interactive-features-architecture)
   - 7.1 [The Object Tree](#71-the-object-tree)
   - 7.2 [Building from the Existing Content Stream Parser](#72-building-from-the-existing-content-stream-parser)
   - 7.3 [Hit Testing](#73-hit-testing)
   - 7.4 [Hover Color Information](#74-hover-color-information)
   - 7.5 [Wireframe Display](#75-wireframe-display)
   - 7.6 [Color Separation / Plate Preview](#76-color-separation--plate-preview)
8. [GPU-Accelerated PDF Rasterization Research](#8-gpu-accelerated-pdf-rasterization-research)
9. [Feature Reference Table](#9-feature-reference-table)
10. [Key Files Reference](#10-key-files-reference)

---

## 1. Executive Summary

`rbv` is a standalone PDF viewer subprocess launched by `rbara-gui`. Its current architecture
has two problems worth solving before deeper integration:

**Problem 1 — Wrong display technology.** wgpu is being used as a texture blit engine. As `rbv`
grows to support zoom/pan, prepress overlays (TrimBox, BleedBox, trim marks), object wireframes,
and hover color inspection, you would be hand-rolling a 2D graphics library on top of raw wgpu.
**Replace wgpu with Skia** (`skia-safe` crate). Skia is GPU-accelerated, has Vulkan and Metal
backends, and provides `canvas.draw_path()`, `canvas.draw_image()`, `canvas.draw_str()` — the
full 2D toolkit needed for overlay rendering.

**Problem 2 — PDFium blocks the main thread before the window opens.** `viewer::run()` calls
`pipeline.render_page()` synchronously before the winit event loop starts. The viewer is
intentionally invoked only on explicit user request (never automatically), so this cost is
already opt-in. However, once the user requests the viewer, they should see a window
immediately — not stare at a blank screen for 3–5 seconds. **Fix with async rendering**:
open the window on spawn, render PDFium in a background thread, deliver the result via
`EventLoopProxy`. Layer progressive DPI (72 DPI preview first, then full resolution) on top.
This does not change the on-demand invocation model; it only improves the experience within it.

**PDFium stays.** It is the highest-fidelity cross-platform PDF rasterizer available in the Rust
ecosystem. GPU-accelerated alternatives either require a custom PDFium build (complex), are
platform-locked (CoreGraphics/macOS, WinRT/Windows), or sacrifice CMYK/ICC accuracy (Skia's
own PDF parser, MuPDF). For a prepress tool, fidelity outweighs rasterization speed. The async
pattern eliminates perceived latency without touching the rasterizer.

**`rustybara` core stays renderer-agnostic.** Skia belongs in `rbv`, not in the core library.
The stubbed `gpu` feature in `rustybara/Cargo.toml` should remain a stub or be removed.

---

## 2. Workspace Structure

```
rustybara/
├── Cargo.toml                  ← workspace root
├── rbv/                        ← PDF viewer (THIS IS WHERE THE WORK HAPPENS)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             ← CLI entry point (clap)
│       ├── viewer.rs           ← Application logic, wgpu pipeline, winit event loop
│       └── texture.rs          ← GPU texture upload helper
├── rustybara/                  ← Core prepress library (keep renderer-agnostic)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── pipeline.rs         ← PdfPipeline: open, render_page, trim, resize, etc.
│       ├── geometry/
│       │   ├── matrix.rs       ← 2D affine Matrix (CTM tracking)
│       │   └── rect.rs         ← Rect (AABB, overlap detection)
│       ├── pages/
│       │   └── boxes.rs        ← PageBoxes (MediaBox, TrimBox, BleedBox, CropBox)
│       ├── raster/
│       │   ├── mod.rs
│       │   ├── render.rs       ← CpuRenderer (PDFium), GpuRenderer stub
│       │   └── config.rs       ← RenderConfig (dpi, annotations, form data)
│       └── stream/
│           ├── filter.rs       ← Content stream parser: CTM, paths, q/Q, Do
│           ├── color_ops.rs    ← CMYK color remapping (k/K operators)
│           └── layout.rs       ← Text/image layout extraction
├── rbara/                      ← CLI + Ratatui TUI
├── rbara-gui/                  ← Tauri v2 + Svelte desktop app
│   └── src/
│       └── commands.rs         ← open_in_viewer() spawns rbv subprocess
├── rustybara-icc/              ← ICC color profile management
└── rustybara-wasm/             ← WebAssembly bindings
```

**Dependency versions (from `rbv/Cargo.toml`):**

```toml
wgpu      = "29.0.1"       ← to be replaced
winit     = "0.30.13"      ← keep
image     = "0.25.10"      ← keep
pollster  = "0.4.0"        ← keep or remove (was needed for wgpu async init)
bytemuck  = "*"            ← remove (wgpu buffer helper, no longer needed)
notify    = "*"            ← keep (file watcher)
clap      = "*"            ← keep
rustybara = { path = "../rustybara", features = ["raster"] }
```

**How `rbara-gui` launches `rbv`** (`rbara-gui/src/commands.rs`, lines 748–765):

```rust
pub fn open_in_viewer(app: tauri::AppHandle, path: String, page: u32, dpi: u32) {
    let rbv = app.path().resource_dir()
        .ok()
        .join(if cfg!(windows) { "rbv.exe" } else { "rbv" });
    std::process::Command::new(&rbv)
        .arg(&path)
        .arg(page.to_string())
        .args(["--dpi", &dpi.to_string()])
        .spawn()
        .map_err(|e| format!("Failed to launch rbv: {e}"))
}
```

---

## 3. Current Rendering Stack

### 3.1 The Two-Layer Model

```
Layer 1 — PDF Rasterization (CPU)
    pdfium-render crate
    └── PDFium (Google Chrome's PDF renderer)
        └── Outputs: image::DynamicImage (RGBA8 bitmap)

Layer 2 — Display (GPU)
    wgpu 29.0.1
    └── Simple passthrough WGSL shader
        └── Uploads DynamicImage as wgpu::Texture
        └── Draws 6-vertex quad with aspect-ratio uniform
        └── Outputs: swapchain frame
```

The GPU layer is doing almost nothing meaningful. It is a texture blit with aspect-ratio
correction. The WGSL shader (inline in `viewer.rs`, lines 9–55) is:

- Vertex: generates UV-mapped quad, adjusts for image vs. window aspect ratio
- Fragment: samples texture with linear filter, returns color

### 3.2 Startup Sequence (The Problem)

**Current order** (`rbv/src/viewer.rs`, lines 448–492):

```rust
pub fn run(path: PathBuf, page: u32, config: RenderConfig) {
    // Step 1: PDF file opened — lopdf parses structure (~50–200ms)
    let pipeline = rustybara::PdfPipeline::open(&path).unwrap();
    let page_count = pipeline.page_count() as u32;

    // Step 2: PDFium rasterizes the page — BLOCKS MAIN THREAD (~1–5 seconds)
    // Window does not exist yet. User sees nothing.
    let image = pipeline.render_page(page, &config).unwrap();

    // Step 3: Event loop created (instant)
    let event_loop = EventLoop::new().unwrap();

    // Step 4: File watcher spawned on background thread (instant)
    std::thread::spawn(move || { /* notify watcher */ });

    // Step 5: Event loop starts — window appears HERE, after all of the above
    event_loop.run_app(&mut app).unwrap();
}
// Inside resumed() (called as first event):
//   Window is created
//   wgpu adapter/device/surface setup
//   texture::upload() — GPU texture upload (fast)
//   window.request_redraw()
```

**Timing on a 12×18" page at 300 DPI:**
| Step | Duration |
|---|---|
| `PdfPipeline::open()` | 50–200ms |
| `pipeline.render_page()` at 300 DPI | **1,500–5,000ms** ← the stall |
| `EventLoop::new()` + `run_app()` | ~10ms |
| `texture::upload()` (GPU) | ~20ms |

The window is invisible for the entire PDFium duration.

### 3.3 Current Data Flow

```
main()
  └─ viewer::run(path, page, config)
        ├─ PdfPipeline::open(&path)          [lopdf: parse PDF tree]
        │     └─ pipeline.render_page()      [PDFium: CPU rasterize → DynamicImage]
        ├─ EventLoop::new()
        └─ event_loop.run_app(&mut Viewer)
              └─ resumed() [first event]
                    ├─ create_window()
                    ├─ wgpu adapter/device/queue/surface
                    ├─ texture::upload(&device, &queue, &image)
                    │     └─ image.to_rgba8()
                    │     └─ device.create_texture()
                    │     └─ queue.write_texture()
                    └─ window.request_redraw()
```

---

## 4. Decision: Replace wgpu with Skia

### 4.1 Why wgpu Alone Is Insufficient

As `rbv` evolves toward the feature set described in sections 7 and 9 of this document, the
required 2D operations are:

- Draw the rasterized page bitmap (currently done)
- Zoom and pan (affine transform on the bitmap)
- Draw TrimBox, BleedBox, CropBox as dashed rectangles with labels
- Draw path wireframes for selected objects
- Draw selection highlight (filled + stroked rectangle or path outline)
- Render hover tooltip text at cursor position
- Draw ruler overlays

All of these require a **2D canvas API**: `draw_image()`, `draw_path()`, `draw_rect()`,
`draw_str()`. Raw wgpu has none of these. Implementing them on top of wgpu means writing
a tessellator, a text renderer, and a 2D transform stack — i.e., reimplementing Skia.

### 4.2 Why Skia

| Criterion                | Skia                     | Vello        | femtovg        | Stay with wgpu    |
| ------------------------ | ------------------------ | ------------ | -------------- | ----------------- |
| GPU backend              | Vulkan, Metal, GL        | wgpu compute | OpenGL         | Vulkan/Metal/DX12 |
| 2D canvas API            | Full (`Canvas`)          | Full         | Full           | None (hand-roll)  |
| Text rendering           | Production-quality       | Good         | NanoVG quality | None              |
| PDF bitmap display       | `canvas.draw_image()`    | Yes          | Yes            | Texture blit      |
| Path/overlay drawing     | `canvas.draw_path()`     | Yes          | Yes            | Custom shaders    |
| Production readiness     | Chrome, Flutter, Android | Pre-1.0      | Stable         | N/A               |
| Rust binding             | `skia-safe` (mature)     | Native Rust  | Native Rust    | Native Rust       |
| C++ dependency           | Yes (large)              | No           | No             | No                |
| Cross-compile difficulty | High                     | Low          | Low            | Low               |

**Recommendation: Skia.** Production-proven, full 2D canvas, GPU-accelerated. The C++ build
complexity is the real cost; evaluate it in a spike branch before committing.

**Vello** is the all-Rust future. Watch the 0.4 and 1.0 releases. Not ready for a shipping
tool today (pre-1.0, API churn), but revisit in 12–18 months.

### 4.3 Scope of the Swap

**Only `rbv/` changes.** `rustybara` core is untouched.

```
Files changed:
  rbv/Cargo.toml          — remove wgpu/pollster/bytemuck, add skia-safe
  rbv/src/viewer.rs       — replace GpuState (wgpu) with SkiaState
  rbv/src/texture.rs      — replace entirely with skia image helper

Files unchanged:
  rustybara/**            — core library, no renderer dependency
  rbara-gui/**            — Tauri app, spawns rbv as subprocess
  rbara/**                — CLI/TUI
```

The `gpu` feature stub in `rustybara/Cargo.toml` should be **left as-is or removed** — it has
no implementation and Skia does not belong in the core library.

---

## 5. Skia Integration Guide

### 5.1 Dependencies

**`rbv/Cargo.toml`** — after the swap:

```toml
[package]
name = "rbv"
version = "0.1.0"
edition = "2021"

[dependencies]
rustybara = { path = "../rustybara", features = ["raster"] }
winit     = "0.30.13"
image     = { version = "0.25.10", default-features = false, features = ["png", "jpeg"] }
notify    = "6"
clap      = { version = "4", features = ["derive"] }

# Skia replaces wgpu + pollster + bytemuck
skia-safe = { version = "0.75", features = ["gpu", "vulkan"] }
# On macOS, add "metal" feature instead of or in addition to "vulkan":
# skia-safe = { version = "0.75", features = ["gpu", "metal"] }

# For raw-window-handle interop (winit <-> skia surface)
raw-window-handle = "0.6"
```

> **Build note:** `skia-safe` downloads and compiles Skia's C++ source on first build.
> Expect 5–15 minutes on first `cargo build`. Set `SKIA_NINJA_COMMAND` and
> `SKIA_GN_COMMAND` env vars if your system ninja/gn are not on PATH.
> See: https://github.com/rust-skia/rust-skia#building

### 5.2 Window + Skia Surface Setup

Skia needs a GPU context (`DirectContext`) and a surface tied to the window's swapchain.
With winit 0.30, use `raw-window-handle` to get the platform handle.

```rust
// rbv/src/renderer.rs  (new file — replaces texture.rs)

use skia_safe::{
    gpu::{self, backend_render_targets, DirectContext, SurfaceOrigin},
    ColorType, Surface,
};
use winit::window::Window;

pub struct SkiaRenderer {
    pub context: DirectContext,
    pub surface: Surface,
    pub width: i32,
    pub height: i32,
}

impl SkiaRenderer {
    /// Create a Skia GPU surface backed by Vulkan, tied to `window`.
    /// Call this inside winit's `resumed()` after the window exists.
    pub fn new(window: &Window) -> Self {
        let size = window.inner_size();
        let (width, height) = (size.width as i32, size.height as i32);

        // 1. Create Vulkan instance/device via skia's helper
        //    (skia-safe handles VkInstance, VkDevice, VkQueue internally)
        let mut context = unsafe {
            // skia_safe::gpu::vk::BackendContext requires VkInstance, VkPhysicalDevice,
            // VkDevice, VkQueue — skia-safe's `DirectContext::new_vulkan` takes a
            // BackendContext you build from raw window handles.
            // See: https://github.com/rust-skia/rust-skia/tree/master/skia-safe/examples
            DirectContext::new_vulkan(&build_vulkan_backend_context(window), None).unwrap()
        };

        // 2. Create render target from window surface
        let backend_rt = backend_render_targets::make_vk(
            (width, height),
            &gpu::vk::ImageInfo { /* swapchain image info */ ..Default::default() },
        );

        // 3. Wrap in Skia Surface
        let surface = Surface::from_backend_render_target(
            &mut context,
            &backend_rt,
            SurfaceOrigin::BottomLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .unwrap();

        Self { context, surface, width, height }
    }

    /// Resize — recreate surface when window size changes.
    pub fn resize(&mut self, window: &Window) {
        let size = window.inner_size();
        self.width = size.width as i32;
        self.height = size.height as i32;
        // Recreate surface at new size (context stays)
        // ... same as new() but reuse self.context
    }

    pub fn canvas(&mut self) -> &skia_safe::Canvas {
        self.surface.canvas()
    }

    pub fn flush(&mut self) {
        self.context.flush_and_submit();
    }
}
```

> **Practical shortcut during the spike:** Use `skia_safe::surfaces::raster_n32_premul()`
> (CPU-only Skia surface) first to validate the drawing API without the Vulkan setup
> complexity. Swap to GPU surface once drawing is correct.

### 5.3 Replacing texture.rs

**Current `rbv/src/texture.rs`:**

```rust
// Uploads a DynamicImage to a wgpu::Texture
pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, image: &DynamicImage) -> wgpu::Texture {
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    let texture = device.create_texture(&wgpu::TextureDescriptor { ... });
    queue.write_texture(texture.as_image_copy(), rgba.as_raw(), ...);
    texture
}
```

**Replacement — convert `DynamicImage` to `skia_safe::Image`:**

```rust
// rbv/src/renderer.rs  (add to the file above)

use image::DynamicImage;
use skia_safe::{Data, Image, ImageInfo, ColorType, AlphaType};

/// Convert a PDFium-rendered DynamicImage into a Skia Image for canvas drawing.
pub fn image_to_skia(img: &DynamicImage) -> Image {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let info = ImageInfo::new(
        (w as i32, h as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None, // no color space — PDFium output is sRGB
    );
    // Safety: rgba buffer lives for the duration of this call;
    // Skia copies the data into its own allocation.
    let data = Data::new_copy(rgba.as_raw());
    Image::from_raster_data(&info, data, (w * 4) as usize).unwrap()
}
```

### 5.4 Replacing the wgpu Pipeline in viewer.rs

The `GpuState` struct in `viewer.rs` (lines 75–88) currently holds:

```rust
struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    texture_bind_group: wgpu::BindGroup,
    aspect_bind_group: wgpu::BindGroup,
    aspect_buffer: wgpu::Buffer,
}
```

**Replace with:**

```rust
struct SkiaState {
    window: Arc<Window>,
    renderer: SkiaRenderer,        // from renderer.rs above
    page_image: skia_safe::Image,  // the PDFium bitmap as Skia Image
    zoom: f32,                     // zoom level (1.0 = fit to window)
    pan: (f32, f32),               // pan offset in screen pixels
}
```

**The draw call** (replaces the wgpu render pass in `window_event::RedrawRequested`):

```rust
fn draw(&mut self) {
    let canvas = self.renderer.canvas();
    canvas.clear(skia_safe::Color::BLACK);

    // Compute destination rect: fit image to window with zoom/pan
    let (img_w, img_h) = (self.page_image.width() as f32, self.page_image.height() as f32);
    let (win_w, win_h) = (self.renderer.width as f32, self.renderer.height as f32);
    let scale = (win_w / img_w).min(win_h / img_h) * self.zoom;
    let dest_x = (win_w - img_w * scale) / 2.0 + self.pan.0;
    let dest_y = (win_h - img_h * scale) / 2.0 + self.pan.1;

    // Draw the PDF page bitmap
    let mut paint = skia_safe::Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_image_rect(
        &self.page_image,
        None, // src rect = full image
        skia_safe::Rect::from_xywh(dest_x, dest_y, img_w * scale, img_h * scale),
        &paint,
    );

    // Overlay drawing goes here (see section 5.5)

    self.renderer.flush();
}
```

**Update the `Viewer` struct** (lines 65–73 in `viewer.rs`):

```rust
struct Viewer {
    path: PathBuf,
    page: u32,
    page_count: u32,
    config: RenderConfig,
    image: Option<DynamicImage>,   // Option — may not be ready yet (async rendering)
    gpu: Option<SkiaState>,
    digit_buf: String,
    // New fields:
    low_res_image: Option<DynamicImage>,  // 72 DPI preview
}
```

### 5.5 Drawing Overlays with Skia

Once the PDF bitmap is drawn, overlays are trivial `Canvas` calls:

```rust
fn draw_prepress_overlays(&self, canvas: &skia_safe::Canvas, page_rect: skia_safe::Rect) {
    // --- TrimBox overlay ---
    // page_boxes comes from rustybara::PageBoxes (rustybara/src/pages/boxes.rs)
    if let Some(trim) = self.page_boxes.trim_box {
        let r = pdf_rect_to_screen(trim, page_rect);
        let mut paint = skia_safe::Paint::default();
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_color(skia_safe::Color::from_argb(200, 0, 180, 255)); // blue
        paint.set_stroke_width(1.0);
        // Dashed line effect
        let intervals = [4.0f32, 4.0];
        paint.set_path_effect(skia_safe::PathEffect::dash(&intervals, 0.0));
        canvas.draw_rect(r, &paint);
    }

    // --- BleedBox overlay ---
    if let Some(bleed) = self.page_boxes.bleed_box {
        let r = pdf_rect_to_screen(bleed, page_rect);
        let mut paint = skia_safe::Paint::default();
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_color(skia_safe::Color::from_argb(200, 255, 80, 0));  // orange
        paint.set_stroke_width(1.0);
        canvas.draw_rect(r, &paint);
    }

    // --- Wireframe for selected object ---
    if let Some(obj) = &self.selected_object {
        let path = pdf_path_to_skia(&obj.path, page_rect);
        let mut paint = skia_safe::Paint::default();
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_color(skia_safe::Color::from_argb(220, 255, 200, 0)); // yellow
        paint.set_stroke_width(1.5);
        canvas.draw_path(&path, &paint);
    }
}

/// Convert a PDF-space rect (points, Y-up, origin bottom-left)
/// to screen-space Skia rect (pixels, Y-down, origin top-left).
fn pdf_rect_to_screen(pdf_rect: Rect, page_rect: skia_safe::Rect) -> skia_safe::Rect {
    // pdf_rect values are in PDF points (72 pts = 1 inch)
    // page_rect is the destination screen rect for the full page
    let page_height_pts = /* page MediaBox height in pts */ 792.0_f32;
    let scale_x = page_rect.width() / /* page width in pts */ 612.0_f32;
    let scale_y = page_rect.height() / page_height_pts;

    skia_safe::Rect::from_ltrb(
        page_rect.left() + pdf_rect.x as f32 * scale_x,
        page_rect.top() + (page_height_pts - pdf_rect.y as f32 - pdf_rect.height as f32) * scale_y,
        page_rect.left() + (pdf_rect.x as f32 + pdf_rect.width as f32) * scale_x,
        page_rect.top() + (page_height_pts - pdf_rect.y as f32) * scale_y,
    )
}
```

---

## 6. Viewer Invocation Design & Latency

### 6.1 Design Intent: Explicit Invocation Only

**The viewer is intentionally on-demand.** `rbv` is a subprocess spawned only when the user
explicitly requests it via `open_in_viewer()` in `rbara-gui/src/commands.rs`. It is never
opened automatically on document load.

This is a deliberate architectural decision rooted in prepress workflow priorities:

- **Most prepress work is metadata-level:** trim box adjustments, bleed expansion, color
  remapping, page extraction. None of these require rasterization. Forcing a render on
  every document open would impose PDFium's full CPU cost on operations that don't need it.
- **Complex documents are expensive to render:** a large book, a multi-layer spread with
  complex shading, or a document with many spot colors can take 10–30 seconds per page at
  300 DPI. Acrobat and PitStop force this cost on every open; Rustybara does not.
- **Visual inspection is a deliberate act:** the viewer exists to check for visual
  discrepancies that metadata alone cannot reveal — misregistered colors, missing bleeds,
  clipped content. This is a separate workflow step the prepress tech opts into.

**The subprocess model enforces this cleanly.** `rbara-gui` and `rbara` (CLI/TUI) run their
full prepress pipelines with zero renderer overhead. The viewer is isolated; if it crashes
on a pathological document, the core application is unaffected.

**The async fix described below does not change this.** It applies strictly _within_ the
explicit invocation: once the user has consented and `rbv` is spawning, the 3–5 second
black-screen stall before the window appears is eliminated. The user requested the viewer;
they should see a window immediately, not wait in the dark while PDFium works.

### 6.2 Root Cause of the Stall

`pipeline.render_page()` is called on the main thread before `EventLoop::new()`.
The window cannot open until PDFium finishes. This is the **entire** source of the stall.

From `rustybara/src/raster/render.rs` (lines 75–88), `CpuRenderer::render()`:

```rust
fn render(&self, page: &PdfPage, config: &RenderConfig) -> crate::Result<DynamicImage> {
    let w = (page.width().value * config.dpi as f32 / 72.0) as i32;
    let h = (page.height().value * config.dpi as f32 / 72.0) as i32;
    let render_cfg = PdfRenderConfig::new()
        .set_target_size(w, h)
        .render_annotations(config.render_annotations)
        .render_form_data(config.render_form_data);
    // This call blocks. PDFium is CPU-only in pdfium-render.
    Ok(page.render_with_config(&render_cfg)
        .and_then(|bitmap| bitmap.as_image())?)
}
```

PDFium is entirely CPU-bound. There is no GPU path through `pdfium-render`. The fix is
latency-hiding via threading, not acceleration.

**Important implementation detail — `render_page` serializes per call.** Inspecting
`rustybara/src/pipeline.rs` reveals that `PdfPipeline::render_page()` does not hold any
PDFium object internally. Each call clones the `lopdf::Document`, serializes it to a
`Vec<u8>`, then creates a fresh `Pdfium` context and loads the document from those bytes
before rasterizing. All PDFium objects (`Pdfium`, `PdfDocument`, `PdfPage`) are local
variables that are dropped at the end of `render_page`. This has two consequences:

1. The two-phase progressive render (72 DPI preview → full DPI) causes two full
   serialize+reload cycles. Serialization is fast (~5–20ms) relative to rasterization
   (1,500–5,000ms), so this is acceptable.
2. An alternative to `Arc<PdfPipeline>` is to serialize the document once on the main
   thread and pass `Arc<Vec<u8>>` to the background thread (bypassing the redundant
   `doc.clone()` + `save_to()` inside `render_page`). This is a minor optimization and
   not required for correctness.

### 6.3 Async Rendering Pattern (Within Explicit Invocation)

**`PdfPipeline` is confirmed `Send + Sync` — `Arc<PdfPipeline>` is safe.** Verified from
`rustybara/Cargo.toml` and `rustybara/src/pipeline.rs`:

- `PdfPipeline` wraps only `lopdf::Document`, a pure Rust data structure with no raw
  pointers, `Rc`, or `RefCell`. `lopdf::Document` is auto-`Send + Sync`.
- `pdfium-render` is already compiled with `features = ["thread_safe"]` in
  `rustybara/Cargo.toml` (line 16), which applies `unsafe impl Send` and
  `unsafe impl Sync` to the PDFium binding types.
- Even without `thread_safe`, the pattern would be safe: `render_page` keeps all PDFium
  objects as local variables scoped within the call (see §6.2 note above). No PDFium
  state crosses a thread boundary through `PdfPipeline`.

winit's `EventLoopProxy<T>` allows any thread to send a typed event into the event loop.

```rust
// rbv/src/viewer.rs

#[derive(Debug)]
pub enum ViewerEvent {
    PreviewReady { page: u32, image: DynamicImage },   // low-res, fast
    PageReady    { page: u32, image: DynamicImage },   // full-res, slow
    FileChanged,
}

pub fn run(path: PathBuf, page: u32, config: RenderConfig) {
    // Open PDF structure only — fast (lopdf parsing, no rasterization)
    let pipeline = Arc::new(rustybara::PdfPipeline::open(&path).unwrap());
    let page_count = pipeline.page_count() as u32;

    let event_loop: EventLoop<ViewerEvent> = EventLoop::with_user_event().unwrap();
    let proxy = event_loop.create_proxy();

    // Spawn PDFium rasterization OFF the main thread
    let pipeline_clone = Arc::clone(&pipeline);
    let proxy_clone = proxy.clone();
    std::thread::spawn(move || {
        // Step 1: Fast low-res preview (~50ms)
        let preview_config = RenderConfig { dpi: 72, ..config };
        if let Ok(img) = pipeline_clone.render_page(page, &preview_config) {
            let _ = proxy_clone.send_event(ViewerEvent::PreviewReady { page, image: img });
        }

        // Step 2: Full resolution (~1–5s, runs while user sees the preview)
        if let Ok(img) = pipeline_clone.render_page(page, &config) {
            let _ = proxy_clone.send_event(ViewerEvent::PageReady { page, image: img });
        }
    });

    // File watcher (existing pattern, unchanged)
    let proxy_watcher = proxy.clone();
    std::thread::spawn(move || {
        // notify watcher → proxy_watcher.send_event(ViewerEvent::FileChanged)
    });

    let mut app = Viewer {
        path,
        page,
        page_count,
        config,
        image: None,          // not ready yet — window opens before PDFium finishes
        low_res_image: None,
        gpu: None,
        digit_buf: String::new(),
    };

    event_loop.run_app(&mut app).unwrap();
}
```

**Handle the events in the `ApplicationHandler` impl:**

```rust
impl ApplicationHandler<ViewerEvent> for Viewer {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ViewerEvent) {
        match event {
            ViewerEvent::PreviewReady { page, image } if page == self.page => {
                self.low_res_image = Some(image);
                if let Some(gpu) = &mut self.gpu {
                    // Upload low-res texture, show immediately
                    gpu.page_image = image_to_skia(self.low_res_image.as_ref().unwrap());
                    gpu.window.request_redraw();
                }
            }
            ViewerEvent::PageReady { page, image } if page == self.page => {
                self.image = Some(image);
                if let Some(gpu) = &mut self.gpu {
                    // Upgrade to full-res texture
                    gpu.page_image = image_to_skia(self.image.as_ref().unwrap());
                    gpu.window.request_redraw();
                }
            }
            ViewerEvent::FileChanged => {
                // Re-trigger render (existing logic, adapted)
            }
            _ => {}
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Window creation happens immediately — no waiting for PDFium
        let window = Arc::new(event_loop.create_window(
            WindowAttributes::default().with_title("rbv — loading…")
        ).unwrap());

        let renderer = SkiaRenderer::new(&window);

        // If preview is already ready (very fast machines), use it
        // Otherwise page_image will be a 1x1 placeholder
        let page_image = self.low_res_image.as_ref()
            .or(self.image.as_ref())
            .map(image_to_skia)
            .unwrap_or_else(|| placeholder_image());

        self.gpu = Some(SkiaState { window, renderer, page_image, zoom: 1.0, pan: (0.0, 0.0) });
    }
}
```

### 6.4 Progressive DPI Pattern

| Phase        | DPI     | Typical time (12×18" page) | User sees                    |
| ------------ | ------- | -------------------------- | ---------------------------- |
| Window open  | —       | ~100ms                     | Empty window                 |
| PreviewReady | 72      | ~50–100ms                  | Readable page, slightly soft |
| PageReady    | 150/300 | 1,500–5,000ms              | Full resolution              |

The 72 DPI preview is sufficient for navigation and context. The full-res render sharpens
the page without any user action required. This matches Acrobat's behavior on large files.

**DPI constants to define:**

```rust
// rbv/src/main.rs or a constants module
pub const PREVIEW_DPI: u32 = 72;
pub const DEFAULT_DPI:  u32 = 150;   // good balance of speed and quality
pub const PRINT_DPI:    u32 = 300;   // prepress quality
```

---

## 7. Interactive Features Architecture

### 7.1 The Object Tree

A PDF page is a **display list**. Each visible element is defined by:

1. A **graphics state** at draw time (CTM, colorspace, fill/stroke color, line width, clip)
2. A **paint operation** (`f`, `S`, `B`, `Do`, etc.) that terminates the object

PitStop Pro, Acrobat Pro's TouchUp Object tool, and similar tools build an **object tree**
from this display list. This is the foundation for object selection, wireframe display,
and per-object color inspection.

**Proposed data structures** (new module: `rustybara/src/objects/`):

```rust
// rustybara/src/objects/tree.rs

pub struct ObjectTree {
    pub objects: Vec<PageObject>,
}

pub struct PageObject {
    pub kind: ObjectKind,
    pub path: Vec<SubPath>,          // actual path geometry in PDF space
    pub bbox: Rect,                  // AABB — fast pre-filter for hit testing
    pub fill_color: Option<PdfColor>,
    pub stroke_color: Option<PdfColor>,
    pub ctm: Matrix,                 // CTM at paint time (from rustybara/src/geometry/matrix.rs)
    pub clip_depth: usize,           // q/Q nesting depth
}

pub enum ObjectKind {
    Fill,         // f, f*, F
    Stroke,       // S, s
    FillStroke,   // B, B*, b, b*
    Image,        // Do where subtype = /Image
    FormXObject,  // Do where subtype = /Form
    Text,         // BT...ET block
}

pub enum PdfColor {
    DeviceCMYK([f64; 4]),             // k/K operators
    DeviceRGB([f64; 3]),              // rg/RG operators
    DeviceGray(f64),                  // g/G operators
    Separation { name: String, tint: f64 },  // cs + scn with /Separation colorspace
    Pattern(String),                  // cs + scn with /Pattern
}

pub struct SubPath {
    pub points: Vec<PathPoint>,
    pub closed: bool,
}

pub enum PathPoint {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    CurveTo(f64, f64, f64, f64, f64, f64),  // cubic Bézier
}
```

### 7.2 Building from the Existing Content Stream Parser

`rustybara/src/stream/filter.rs` already implements:

- Full CTM stack (`ctm_stack: Vec<Matrix>`) using `rustybara/src/geometry/matrix.rs`
- Path point collection (`current_path: Vec<(f64, f64)>`)
- Bounding box computation per path
- `q`/`Q` operator handling
- `Do` xObject detection

The `build_object_tree()` function reuses this infrastructure but **keeps objects** instead
of discarding them:

```rust
// rustybara/src/objects/tree.rs

pub fn build_object_tree(doc: &lopdf::Document, page_id: ObjectId) -> ObjectTree {
    let content = doc.get_and_decode_page_content(page_id).unwrap();
    let operations = content.operations;

    let mut objects = Vec::new();
    let mut ctm_stack: Vec<Matrix> = vec![Matrix::identity()];
    let mut color_state = ColorState::default(); // tracks current fill/stroke color
    let mut current_path: Vec<SubPath> = Vec::new();
    let mut current_subpath: Option<SubPath> = None;

    for op in &operations {
        match op.operator.as_str() {
            // --- Graphics state ---
            "q" => ctm_stack.push(*ctm_stack.last().unwrap()),
            "Q" => { ctm_stack.pop(); }
            "cm" => {
                // Matrix::concat() — already implemented in rustybara/src/geometry/matrix.rs
                let m = operands_to_matrix(&op.operands);
                let top = ctm_stack.last_mut().unwrap();
                *top = top.concat(&m);
            }

            // --- Color operators (reuse logic from color_ops.rs) ---
            "k"  => color_state.fill  = PdfColor::DeviceCMYK(read_cmyk(&op.operands)),
            "K"  => color_state.stroke = PdfColor::DeviceCMYK(read_cmyk(&op.operands)),
            "rg" => color_state.fill  = PdfColor::DeviceRGB(read_rgb(&op.operands)),
            "RG" => color_state.stroke = PdfColor::DeviceRGB(read_rgb(&op.operands)),
            "g"  => color_state.fill  = PdfColor::DeviceGray(read_f64(&op.operands[0])),
            "G"  => color_state.stroke = PdfColor::DeviceGray(read_f64(&op.operands[0])),

            // --- Path construction ---
            "m" => {
                if let Some(sp) = current_subpath.take() { current_path.push(sp); }
                current_subpath = Some(SubPath {
                    points: vec![PathPoint::MoveTo(read_f64(&op.operands[0]), read_f64(&op.operands[1]))],
                    closed: false,
                });
            }
            "l" => {
                if let Some(sp) = &mut current_subpath {
                    sp.points.push(PathPoint::LineTo(read_f64(&op.operands[0]), read_f64(&op.operands[1])));
                }
            }
            "c" => { /* cubic bezier — similar to "l" */ }
            "h" => { if let Some(sp) = &mut current_subpath { sp.closed = true; } }
            "re" => {
                // Rectangle shorthand — convert to MoveTo + 3× LineTo + close
                let (x, y, w, h) = (read_f64(&op.operands[0]), read_f64(&op.operands[1]),
                                    read_f64(&op.operands[2]), read_f64(&op.operands[3]));
                current_path.push(rect_to_subpath(x, y, w, h));
            }

            // --- Paint operators — each one finalizes an object ---
            "f" | "f*" | "F" => {
                flush_path(&mut objects, &mut current_path, &mut current_subpath,
                           ObjectKind::Fill, &color_state, ctm_stack.last().unwrap());
            }
            "S" | "s" => {
                flush_path(&mut objects, &mut current_path, &mut current_subpath,
                           ObjectKind::Stroke, &color_state, ctm_stack.last().unwrap());
            }
            "B" | "B*" | "b" | "b*" => {
                flush_path(&mut objects, &mut current_path, &mut current_subpath,
                           ObjectKind::FillStroke, &color_state, ctm_stack.last().unwrap());
            }
            "n" => {
                // Clipping path — discard as a visible object, clear path state
                current_path.clear();
                current_subpath = None;
            }
            "Do" => {
                // xObject invocation — add as Image or FormXObject object
                // Check doc.Resources.XObject[name].Subtype
                objects.push(build_xobject_node(&op.operands, doc, page_id,
                                                ctm_stack.last().unwrap()));
            }
            _ => {}
        }
    }

    ObjectTree { objects }
}
```

> **Note:** `read_cmyk()` and the operand helpers already exist in
> `rustybara/src/stream/color_ops.rs` — reuse them rather than reimplementing.
> `Matrix::concat()` and `operands_to_matrix()` exist in
> `rustybara/src/stream/filter.rs` and `rustybara/src/geometry/matrix.rs`.

### 7.3 Hit Testing

Given a cursor position `(px, py)` in PDF coordinate space:

```rust
// rustybara/src/objects/hittest.rs

pub fn hit_test(tree: &ObjectTree, point: (f64, f64)) -> Option<&PageObject> {
    // Walk in reverse paint order — last painted = topmost
    for obj in tree.objects.iter().rev() {
        // Fast AABB pre-filter (Rect from rustybara/src/geometry/rect.rs)
        if !obj.bbox.contains_point(point.0, point.1) {
            continue;
        }
        // Precise test per object kind
        let hit = match obj.kind {
            ObjectKind::Fill | ObjectKind::FillStroke => {
                point_in_path_nonzero(&obj.path, point)
                // or point_in_path_evenodd() for f* operator
            }
            ObjectKind::Stroke => {
                distance_to_path(&obj.path, point) <= obj.stroke_width / 2.0
            }
            ObjectKind::Image | ObjectKind::FormXObject => {
                // The unit square [0,0]–[1,1] transformed by CTM is the image bounds
                // obj.bbox is already that transformed rect
                true // already passed AABB test
            }
            ObjectKind::Text => {
                true // bbox is sufficient for text
            }
        };
        if hit { return Some(obj); }
    }
    None
}

/// Winding number algorithm for point-in-polygon.
pub fn point_in_path_nonzero(path: &[SubPath], point: (f64, f64)) -> bool {
    let mut winding = 0i32;
    for subpath in path {
        winding += winding_number(subpath, point);
    }
    winding != 0
}
```

**Screen-to-PDF space conversion** (needed before calling hit_test):

```rust
fn screen_to_pdf(screen_pos: (f32, f32), page_rect: skia_safe::Rect, page_size_pts: (f64, f64)) -> (f64, f64) {
    let pdf_x = ((screen_pos.0 - page_rect.left()) / page_rect.width()) as f64 * page_size_pts.0;
    // PDF Y-axis is flipped (origin bottom-left, Y increases upward)
    let pdf_y = (1.0 - (screen_pos.1 - page_rect.top()) as f64 / page_rect.height() as f64) * page_size_pts.1;
    (pdf_x, pdf_y)
}
```

### 7.4 Hover Color Information

Two readings displayed simultaneously — both are meaningful for prepress:

```rust
pub struct HoverInfo {
    /// Pixel color from the PDFium bitmap at cursor position.
    /// Represents the rendered result including transparency, blending, overprint.
    pub pixel_rgba: [u8; 4],
    pub pixel_cmyk: Option<[f32; 4]>,   // after ICC conversion via rustybara-icc

    /// Color from the PDF object definition — the actual ink specification.
    /// Spot color names are preserved here (pixel_cmyk would be the alternate).
    pub object_fill_color: Option<PdfColor>,
    pub object_stroke_color: Option<PdfColor>,

    /// PDF operator that defined this color (e.g., "k", "rg", "scn")
    pub color_operator: Option<String>,
}

pub fn hover_info(
    bitmap: &DynamicImage,
    tree: &ObjectTree,
    screen_pos: (f32, f32),
    page_rect: skia_safe::Rect,
    page_size_pts: (f64, f64),
) -> HoverInfo {
    // 1. Sample pixel from bitmap (cheap)
    let bx = ((screen_pos.0 - page_rect.left()) / page_rect.width() * bitmap.width() as f32) as u32;
    let by = ((screen_pos.1 - page_rect.top()) / page_rect.height() * bitmap.height() as f32) as u32;
    let pixel = bitmap.get_pixel(bx.min(bitmap.width()-1), by.min(bitmap.height()-1));
    let pixel_rgba = [pixel[0], pixel[1], pixel[2], pixel[3]];

    // 2. Hit test for object color (more expensive — cache the tree)
    let pdf_pos = screen_to_pdf(screen_pos, page_rect, page_size_pts);
    let obj = hit_test(tree, pdf_pos);

    HoverInfo {
        pixel_rgba,
        pixel_cmyk: None, // TODO: rustybara-icc conversion
        object_fill_color: obj.and_then(|o| o.fill_color.clone()),
        object_stroke_color: obj.and_then(|o| o.stroke_color.clone()),
        color_operator: None, // track from build_object_tree
    }
}
```

The gap between `pixel_cmyk` and `object_fill_color` is often meaningful in prepress:

- **Overprint:** objects on separate plates that combine visually; pixel shows composite
- **ICC profiles:** object defines ink in a profile; pixel shows screen-converted result
- **Spot colors:** object is "PANTONE 485"; pixel is an approximation in screen RGB

### 7.5 Wireframe Display

Wireframe mode replaces the bitmap with path geometry only:

```rust
fn draw_wireframe(&self, canvas: &skia_safe::Canvas, tree: &ObjectTree, page_rect: skia_safe::Rect) {
    canvas.clear(skia_safe::Color::WHITE);

    for obj in &tree.objects {
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(0.5);

        let color = match obj.kind {
            ObjectKind::Fill       => skia_safe::Color::from_rgb(0, 80, 200),
            ObjectKind::Stroke     => skia_safe::Color::from_rgb(200, 40, 0),
            ObjectKind::FillStroke => skia_safe::Color::from_rgb(100, 0, 200),
            ObjectKind::Image      => skia_safe::Color::from_rgb(0, 160, 80),
            ObjectKind::FormXObject => skia_safe::Color::from_rgb(160, 120, 0),
            ObjectKind::Text       => skia_safe::Color::from_rgb(80, 80, 80),
        };
        paint.set_color(color);

        let skia_path = pdf_path_to_skia(&obj.path, &obj.ctm, page_rect);
        canvas.draw_path(&skia_path, &paint);
    }
}

fn pdf_path_to_skia(path: &[SubPath], ctm: &Matrix, page_rect: skia_safe::Rect) -> skia_safe::Path {
    let mut skia_path = skia_safe::Path::new();
    for subpath in path {
        for (i, pt) in subpath.points.iter().enumerate() {
            let (sx, sy) = pdf_point_to_screen(pt.coords(), ctm, page_rect);
            match (i, pt) {
                (0, _) | (_, PathPoint::MoveTo(..)) => skia_path.move_to((sx, sy)),
                (_, PathPoint::LineTo(..))           => skia_path.line_to((sx, sy)),
                (_, PathPoint::CurveTo(..))          => { /* add cubic_to */ }
                _ => {}
            };
        }
        if subpath.closed { skia_path.close(); }
    }
    skia_path
}
```

### 7.6 Color Separation / Plate Preview

Two approaches in order of implementation difficulty:

**Approach A — Post-process the rendered bitmap (easy, CMYK only):**

```rust
/// Zero out all channels except `channel` (0=C, 1=M, 2=Y, 3=K).
/// Requires CMYK bitmap — convert from PDFium's RGBA output via ICC first.
pub fn isolate_channel(img: &mut DynamicImage, channel: usize) {
    // After ICC RGBA→CMYK conversion, zero the other three channels
    // and convert back to RGBA for display (show as grayscale density)
}
```

**Approach B — Filter content stream per colorant (correct, supports spot colors):**

```rust
/// Suppress all operations that don't use `colorant` and re-rasterize with PDFium.
/// Uses the object tree: objects with `fill_color` or `stroke_color` not matching
/// `colorant` get removed from the content stream before rendering.
pub fn render_separation(
    pipeline: &PdfPipeline,
    page: u32,
    colorant: &PdfColor,
    config: &RenderConfig,
) -> Result<DynamicImage> {
    // 1. Clone pipeline
    // 2. Walk content stream, remove operations for non-matching colorants
    //    (reuse filter.rs infrastructure)
    // 3. Re-rasterize modified stream with PDFium
    // 4. Return grayscale DynamicImage representing ink density
}
```

Approach B is the correct one for a prepress tool — it handles spot colors (PANTONE, etc.)
that don't appear in CMYK channels and respects overprint settings.

---

## 8. GPU-Accelerated PDF Rasterization Research

### Summary Table

| Option                       | GPU-Accelerated        | Cross-Platform   | Fidelity  | Rust Support                           | Verdict                                           |
| ---------------------------- | ---------------------- | ---------------- | --------- | -------------------------------------- | ------------------------------------------------- |
| **PDFium (current, CPU)**    | No                     | Yes              | Excellent | `pdfium-render`                        | Keep — best fidelity                              |
| **PDFium + Skia GPU build**  | Yes (via Skia)         | Yes              | Excellent | Custom binary via `pdfium-render` BYOB | Best option if you need GPU raster; complex build |
| **MuPDF + GL device**        | Partial (display only) | Yes              | Good      | `mupdf` crate (GL device not exposed)  | Weaker CMYK/ICC                                   |
| **Skia PDF parser**          | Yes (GPU backend)      | Yes              | Good      | `skia-safe`                            | Less complete than PDFium for edge cases          |
| **CoreGraphics**             | Yes (Metal)            | macOS / iOS only | Excellent | `core-graphics` crate                  | Viable as macOS path                              |
| **Windows.Data.Pdf (WinRT)** | Yes (Direct2D)         | Windows only     | Excellent | `windows` crate                        | Viable as Windows path                            |
| **Ghostscript**              | Partial                | Yes              | Good      | FFI only                               | Complex licensing                                 |

### Key Finding

**PDFium (the Google C++ library) has a GPU path** — it uses Skia as its rendering backend,
and Skia supports Vulkan/Metal. The limitation is `pdfium-render`: it uses prebuilt PDFium
binaries compiled without GPU, and its API only exposes the CPU rendering path.

`pdfium-render` supports **bring-your-own-binary** via `Pdfium::bind_to_library(path)`.
Compiling PDFium with `skia_use_vulkan=true` and linking it through this API would give
GPU-accelerated rasterization while keeping PDFium's fidelity. This is non-trivial
(GN/Ninja build system, Vulkan headers, Skia submodule) but is the highest-value option
if GPU rasterization ever becomes necessary.

### Recommended Path (Chosen)

**Option 3 (Hybrid):** Keep CPU PDFium for rasterization. Fix perceived latency with async
rendering and progressive DPI (section 6). GPU is used only for display compositing (Skia).

This gives good-enough performance for the current use case (prepress preview, not a
high-throughput PDF renderer) and avoids a complex custom PDFium build.

---

## 9. Feature Reference Table

| Feature                             | Status             | Location / Approach                                                     | Notes                                 |
| ----------------------------------- | ------------------ | ----------------------------------------------------------------------- | ------------------------------------- |
| PDF rasterization                   | ✅ Implemented     | `rustybara/src/raster/render.rs` — `CpuRenderer` via PDFium             | Keep as-is                            |
| Page display                        | ✅ Implemented     | `rbv/src/viewer.rs` — wgpu texture blit                                 | Replace with Skia                     |
| Aspect ratio correction             | ✅ Implemented     | `rbv/src/viewer.rs` — `AspectUniform`                                   | Reimplement in Skia draw call         |
| Page navigation (arrows/hjkl)       | ✅ Implemented     | `rbv/src/viewer.rs` — `WindowEvent::KeyboardInput`                      | Keep logic, retrigger render          |
| `g` + digits page jump              | ✅ Implemented     | `rbv/src/viewer.rs` — `digit_buf`                                       | Keep                                  |
| File watching / hot reload          | ✅ Implemented     | `rbv/src/viewer.rs` — `notify` crate                                    | Keep; adapt to `ViewerEvent`          |
| Zoom / pan                          | ❌ Not implemented | `rbv/src/viewer.rs` — add `zoom: f32`, `pan: (f32, f32)` to `SkiaState` | Straightforward with Skia             |
| Async window open (post-invocation) | ❌ Not implemented | `rbv/src/viewer.rs` — `EventLoop<ViewerEvent>` pattern                  | Section 6.3; viewer remains on-demand |
| Progressive DPI preview             | ❌ Not implemented | `rbv/src/viewer.rs` — 72 DPI first, then full DPI                       | Section 6.4                           |
| TrimBox overlay                     | ❌ Not implemented | `rbv/src/viewer.rs` — Skia `draw_rect()` with dashes                    | Needs `PageBoxes` from rustybara      |
| BleedBox overlay                    | ❌ Not implemented | `rbv/src/viewer.rs` — Skia `draw_rect()`                                | Needs `PageBoxes` from rustybara      |
| Object tree build                   | ❌ Not implemented | New: `rustybara/src/objects/tree.rs`                                    | Section 7.2                           |
| Hit testing                         | ❌ Not implemented | New: `rustybara/src/objects/hittest.rs`                                 | Section 7.3                           |
| Hover pixel color                   | ❌ Not implemented | `rbv/src/viewer.rs` — sample `DynamicImage` at cursor                   | Section 7.4                           |
| Hover object color                  | ❌ Not implemented | `rbv/src/viewer.rs` — hit_test → `PdfColor`                             | Section 7.4                           |
| Object selection                    | ❌ Not implemented | `rbv/src/viewer.rs` — `selected_object: Option<PageObject>`             | Sections 7.3, 5.5                     |
| Wireframe display                   | ❌ Not implemented | `rbv/src/viewer.rs` — toggle, draw_wireframe()                          | Section 7.5                           |
| Color separation / plate preview    | ❌ Not implemented | New: `rustybara/src/objects/separation.rs`                              | Section 7.6                           |
| Spot color support                  | ❌ Not implemented | `PdfColor::Separation` variant in object tree                           | Requires colorspace dict parsing      |
| ICC color conversion on hover       | ❌ Not implemented | `rustybara-icc` crate already exists                                    | Wire into `HoverInfo::pixel_cmyk`     |
| Form xObject recursion              | ❌ Not implemented | `build_object_tree()` — recurse on `Do` with `/Form` subtype            | Hard part of object tree              |
| Overprint awareness                 | ❌ Not implemented | Parse `/ExtGState` `OP`/`op`/`OPM` flags                                | Needed for accurate hover color       |
| Persistent rbv process (IPC)        | ❌ Not implemented | Replace `std::process::Command::spawn()` in rbara-gui                   | Longer-term; amortizes PDFium startup |

---

## 10. Key Files Reference

| File                                | Lines | Purpose                              | Key Symbols                                                   |
| ----------------------------------- | ----- | ------------------------------------ | ------------------------------------------------------------- |
| `rbv/src/main.rs`                   | 57    | CLI entry (clap)                     | `Args`, `main()`                                              |
| `rbv/src/viewer.rs`                 | 493   | App logic, event loop, wgpu pipeline | `Viewer`, `GpuState`, `run()`, `resumed()`                    |
| `rbv/src/texture.rs`                | 65    | wgpu texture upload                  | `upload()`                                                    |
| `rustybara/src/pipeline.rs`         | ~600  | High-level PDF API                   | `PdfPipeline`, `render_page()`, `detect_color_space()`        |
| `rustybara/src/raster/render.rs`    | 123   | PDFium rasterization                 | `CpuRenderer`, `PageRenderer` trait, `render_page()`          |
| `rustybara/src/raster/config.rs`    | ~30   | Render configuration                 | `RenderConfig` (dpi, render_annotations, render_form_data)    |
| `rustybara/src/stream/filter.rs`    | ~400  | Content stream parser                | `ContentFilter`, `filter_operations()`, CTM stack logic       |
| `rustybara/src/stream/color_ops.rs` | 133   | CMYK color remapping                 | `ColorRemap`, `read_cmyk()`, `cmyk_matches()`                 |
| `rustybara/src/stream/layout.rs`    | ~200  | Text/image layout extraction         | `extract_layout()`                                            |
| `rustybara/src/geometry/matrix.rs`  | 174   | 2D affine transforms                 | `Matrix`, `concat()`, `transform_point()`, `transform_rect()` |
| `rustybara/src/geometry/rect.rs`    | ~100  | Rectangle / AABB                     | `Rect`, `is_outside()`, `contains_point()`                    |
| `rustybara/src/pages/boxes.rs`      | ~100  | Page box extraction                  | `PageBoxes` (MediaBox, TrimBox, BleedBox, CropBox)            |
| `rbara-gui/src/commands.rs`         | ~800  | Tauri IPC commands                   | `open_in_viewer()` (line 748)                                 |

### Operator Quick Reference (lopdf Operation.operator)

| Operator                    | Meaning                                | Handled in                           |
| --------------------------- | -------------------------------------- | ------------------------------------ |
| `q` / `Q`                   | Save / restore graphics state          | `filter.rs` CTM stack                |
| `cm`                        | Concatenate matrix (CTM update)        | `filter.rs` `operands_to_matrix()`   |
| `m` / `l` / `c` / `v` / `y` | Path construction (move, line, curves) | `filter.rs` path collection          |
| `h`                         | Close subpath                          | `filter.rs`                          |
| `re`                        | Rectangle (shorthand path)             | `filter.rs` `operands_to_rect()`     |
| `f` / `f*` / `F`            | Fill path                              | `filter.rs` paint ops                |
| `S` / `s`                   | Stroke path                            | `filter.rs` paint ops                |
| `B` / `B*` / `b` / `b*`     | Fill and stroke                        | `filter.rs` paint ops                |
| `n`                         | End path (no paint, clipping)          | `filter.rs`                          |
| `W` / `W*`                  | Set clipping path                      | `filter.rs` (preserved, not removed) |
| `Do`                        | Invoke xObject                         | `filter.rs`, `layout.rs`             |
| `k` / `K`                   | Set fill / stroke CMYK color           | `color_ops.rs` `read_cmyk()`         |
| `rg` / `RG`                 | Set fill / stroke RGB color            | `pipeline.rs` `detect_color_space()` |
| `g` / `G`                   | Set fill / stroke gray                 | —                                    |
| `cs` / `CS`                 | Set fill / stroke colorspace           | `filter.rs` resource collection      |
| `scn` / `SCN`               | Set fill / stroke color (general)      | `filter.rs` resource collection      |
| `sh`                        | Paint shading                          | `filter.rs` resource collection      |
| `BT` / `ET`                 | Begin / end text block                 | `layout.rs`                          |
| `Tf`                        | Set font                               | `layout.rs`                          |
| `Tm` / `Td` / `TD`          | Text positioning                       | `layout.rs`                          |
| `Tj` / `TJ` / `'` / `"`     | Show text                              | `layout.rs`                          |
