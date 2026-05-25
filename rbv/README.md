# rbv — Rasterized-Buffer Viewer Window

**rbv** (marketing name: *rustybara viewer*) is the native desktop PDF viewer component of the rustybara prepress toolkit. It is a prototype viewer designed for prepress quality-control workflows, not a general-purpose PDF reader.

---

## Why rasterized?

The name is deliberate: rbv renders pages as a **single rasterized bitmap** produced by `pdfium-render`, then displays that bitmap via a Skia/OpenGL surface. It does not maintain a live vector scene graph.

This is a conscious tradeoff:

| Concern | Vector renderer (Acrobat, Illustrator) | rbv |
|---|---|---|
| Per-frame CPU cost | High — traverse + paint every object | Low — blit one texture |
| Zoom quality | Perfect at any scale | Pixel degradation past ~200% |
| Startup time | Slower | Near-instant |
| Memory footprint | Scene graph + textures | One bitmap |
| Prepress QC suitability | Overkill | Sufficient |

A prepress technician performing QC is asking **pass/fail questions**:

- Is bleed present?
- Is the document CMYK or RGB?
- Are spot colors declared correctly?
- Are there missing fonts or missing image links?
- Is the layout roughly correct — nothing obviously shifted or missing?

None of these require sub-pixel vector fidelity. A raster preview at 150 DPI answers every one of them in a 10-second glance. The tech is not zooming to 800% to judge anti-aliasing; they are making a go/no-go call before the job hits the RIP. For everything else — hard proof, client approval, actual quality judgment — the workflow already provides the right tools. rbv's job is to be **fast and correct enough to support a decision**.

### Planned quality improvements (future iterations)

The zoom quality limitation is acknowledged and tracked for future work:

- Step-level quality tiers: re-render at a higher DPI when zoom crosses a user-defined threshold
- Low-cost re-render and zoom-distance snapshot: cache renders at multiple zoom levels
- Level-of-detail (LOD) awareness based on viewport position and scale
- Hybrid mode: vector wireframe overlay on raster background for specific zoom ranges

These are planned iterations; the current raster approach is the correct MVP choice given the target workflow.

---

## Architecture overview

```
CLI args (clap)
      │
      ▼
viewer::run()
      │
      ├── PdfPipeline::open()          ← rustybara: lopdf document + pdfium pipeline
      ├── build_object_tree()          ← rustybara: page object list for wireframe + hit-test
      ├── outline_page_text()          ← rustybara: glyph outline paths (experimental)
      ├── PageBoxes::read()            ← rustybara: media/trim/bleed/crop boxes
      │
      ├── EventLoop<ViewerEvent>       ← winit: platform window + input
      ├── spawn render thread          ← preview (72dpi) then full-res render via pdfium
      ├── notify watcher               ← file system watch for live reload on save
      │
      └── Viewer (ApplicationHandler)
            │
            ├── RedrawRequested ──────► SkiaRenderer::draw()   ← skia-safe + glutin/OpenGL
            ├── MouseInput       ──────► handle_selection_click()
            └── KeyboardInput    ──────► zoom / pan / mode toggles
```

The render thread uses `EventLoopProxy<ViewerEvent>` to deliver `PreviewReady` and `PageReady` events back to the main thread without blocking the window.

---

## Source files

| File | Responsibility |
|---|---|
| `src/main.rs` | CLI entry point (`clap`). Parses `file`, `page`, `--dpi`. Calls `viewer::run()`. |
| `src/viewer.rs` | `Viewer` struct, all application state, `ApplicationHandler` implementation, coordinate helpers, ICC transform management. |
| `src/renderer.rs` | `SkiaRenderer` (OpenGL + Skia surface), all per-frame drawing logic, overlay types (`ColorPanel`, `PageWireframe`, `OverlayData`, `DebugOverlay`). |
| `src/export.rs` | Wireframe diagnostic PDF export (`Ctrl+Shift+E`). Assembles a minimal hand-crafted PDF from the object tree for coordinate debugging. |

---

## Dependency stack

```
rbv
├── rustybara          (features: raster, outline)
│   ├── lopdf          — PDF document parsing
│   ├── pdfium-render  — page rasterization (requires pdfium.dll / libpdfium.dylib)
│   └── (outline)      — glyph path extraction via ttf_parser
├── rustybara-icc      (features: bundled-profiles)
│   └── lcms2          — ICC color transforms
├── image              — DynamicImage, pixel sampling
├── winit              — cross-platform window + event loop
├── skia-safe          — 2D GPU canvas (OpenGL backend)
├── glutin             — OpenGL context management
├── glutin-winit       — winit/glutin integration helpers
├── clap               — CLI argument parsing
└── notify             — file system watcher (live reload)
```

**pdfium dependency**: `rustybara`'s raster feature requires the pdfium shared library at runtime. Place `pdfium.dll` (Windows) or `libpdfium.dylib` (macOS) in the same directory as the `rbv` executable. Pre-built binaries are available from the `pdfium-binaries` project.

---

## Coordinate systems

Two coordinate spaces are in play at all times:

### PDF page space
- Origin: **bottom-left** of the media box
- Y-axis: **up** (positive Y goes toward the top of the page)
- Units: PDF points (1 pt = 1/72 inch)
- Objects in the `ObjectTree` live in this space after CTM application

### Screen space
- Origin: **top-left** of the window
- Y-axis: **down** (positive Y goes toward the bottom of the screen)
- Units: logical pixels

### Conversion helpers (in `viewer.rs`)

```
screen_to_pdf(screen: [f32; 2]) -> Option<(f64, f64)>
```
Maps a window-space cursor position to PDF page coordinates. Returns `None` if the cursor is outside the page rect.

```
compute_page_rect() -> Option<skia_safe::Rect>
```
Returns the current screen-space rectangle the page image occupies, accounting for zoom and pan.

### Conversion helpers (in `renderer.rs`)

```
pdf_rect_to_skia(pdf_rect, media_box, page_screen_rect) -> skia_safe::Rect
pdf_point_to_screen(pdf_x, pdf_y, media_box, page_screen_rect) -> skia_safe::Point
```
Used by the wireframe and glyph-outline drawers to project PDF coordinates onto the Skia canvas.

---

## Rendering pipeline (per frame)

`SkiaRenderer::draw()` renders in back-to-front order:

1. **Background clear** — dark grey `rgba(30, 30, 30, 255)` fills the entire window
2. **Page content** (one of):
   - *Normal mode* — the rasterized `skia_safe::Image` is blitted into the page rect
   - *Wireframe mode* — white page rect is drawn; all `PageObject`s are stroked as thin black outlines; glyph outline paths (when available) are drawn on top; the selected object receives a 2px blue highlight stroke
3. **Prepress box overlays** (`O` key) — bleed (orange dashed), trim (cyan dashed), crop (green dashed) boxes drawn in both modes
4. **Sampling crosshair marker** — orange-red crosshair at the last click position, projected from stored PDF coordinates each frame so it tracks zoom and pan correctly
5. **Color diagnostics panel** (bottom-left) — three rows: pixel RGBA, PDF object color, ICC CMYK estimate
6. **Debug overlay** (top-right, `Ctrl+Shift+D`) — viewport state, cursor PDF position, selected object info, recent log entries

---

## Wireframe mode

Activated with `W`. Replaces the raster image with a vector outline view derived from the page's `ObjectTree`.

**Object rendering by type:**

| `ObjectKind` | Wireframe representation |
|---|---|
| `Fill` / `Stroke` / `FillStroke` | Actual subpath geometry stroked thin black; falls back to bbox rect if no subpath data |
| `Image` | Bounding rect with an × through it (Acrobat-style placeholder) |
| `Text(_)` | Bounding rect (AABB of all 4 text-box corners through Tm matrix) |
| `FormXObject` | Bounding rect |

**Glyph outlines** (experimental): when `outline_page_text()` successfully extracts glyph paths from embedded TrueType fonts, the actual curve outlines are drawn on top of the text bounding boxes.

**Selected object** gets a 2px blue stroke on top of the normal wireframe.

**Export** (`Ctrl+Shift+E`): writes a minimal PDF containing the wireframe paths in page space. Useful for cross-referencing with `qpdf --qdf` to diagnose CTM or coordinate bugs. Output path: `<source_stem>_wireframe_diag.pdf` in the same directory.

---

## Object selection and hit-testing

Left-click (with < 4px drag displacement) runs a hit-test against the `ObjectTree`.

`hit_test(tree, pdf_x, pdf_y)` returns all `PageObject`s whose bbox contains the PDF-space click point. rbv then picks the **smallest-area bbox** from the results, which surfaces the most specific object under the cursor. Using the topmost paint-order object (`.last()`) would cause full-page border strokes to win every click.

On selection:
- `selected_object` is updated (drives the blue wireframe highlight)
- `sampling_pdf_pos` records the click in PDF coords (drives the crosshair marker)
- `color_info` captures pixel RGBA + PDF color + ICC CMYK into a `ColorPanel`

---

## Color diagnostics panel

Shown whenever an object has been selected. Three rows:

```
Pixel   R:142  G:89  B:47  A:255
PdfColor  CMYK: 0.00  0.35  0.67  0.44
ICC CMYK  C:12%  M:40%  Y:68%  K:29%
```

**Row 1 — Pixel RGBA**: sampled from the rasterized `DynamicImage` at the click position using bilinear mapping back through the page rect. This is what the monitor is actually displaying.

**Row 2 — PDF color**: the fill (or stroke) color declared in the PDF content stream for the hit object, decoded from `PdfColor` (DeviceGray / DeviceRgb / DeviceCmyk). This is what the document *specifies*, independent of rendering.

**Row 3 — ICC CMYK**: the pixel RGB converted to CMYK via a Little CMS 2 transform, shown as percentages. The destination profile is always **US Web Coated SWOP**. The source profile is determined at first click:

1. The OS ICC system is scanned for an sRGB profile (Windows: `System32\spool\drivers\color\sRGB Color Space Profile.icm`; macOS: ColorSync directories; Linux: `/usr/share/color/icc/`).
2. If a valid sRGB RGB profile is found, it is used as the source.
3. If not, the bundled **Adobe RGB 1998** profile is used as fallback.

The transform is built once on first click and reused for all subsequent clicks (no per-click transform construction overhead).

### Sampling crosshair marker

A two-ring crosshair (white halo + orange-red accent) is drawn at the sampled point. The marker position is stored in **PDF coordinates** and projected to screen coordinates each frame, so it stays locked to the correct page location when the user zooms or pans after clicking.

---

## File watching

`notify::recommended_watcher` monitors the opened PDF file. On any modification event, rbv:

1. Re-opens the document via `PdfPipeline::open()`
2. Rebuilds `ObjectTree`, `PageBoxes`, and glyph outlines for the current page
3. Clears the selection and color panel
4. Spawns a new preview + full-res render

This supports a **save-and-preview** workflow: a prepress operator can make a fix in InDesign, export, and see rbv update automatically without restarting.

---

## Keyboard shortcuts

| Key | Action |
|---|---|
| `W` | Toggle wireframe mode |
| `O` | Toggle prepress box overlays (bleed / trim / crop) |
| `Ctrl + =` / `Ctrl + +` | Zoom in |
| `Ctrl + -` | Zoom out |
| `Ctrl + 0` | Reset zoom and pan |
| `Ctrl + Scroll` | Zoom toward cursor |
| `Left drag` | Pan |
| `Left click` (< 4px drag) | Select object + sample color |
| `Ctrl+Shift+D` | Toggle debug overlay |
| `Ctrl+Shift+E` | Export wireframe diagnostic PDF |
| `Esc` | Exit |

---

## Debug overlay (`Ctrl+Shift+D`)

A terminal-style panel in the top-right corner showing:

- Current zoom factor and pan offset
- Cursor position in screen space and PDF page space
- Page number and total object count
- Overlays / wireframe mode flags
- Selected object: kind, color, bbox dimensions
- Rolling log of the last 12 events (renders, clicks, file reloads, ICC init)

The debug log is capped at 24 entries (ring buffer). It is the primary diagnostic tool for coordinate and rendering issues during development.

---

## CLI usage

```
rbv <file> [page] [--dpi <dpi>]
```

| Argument | Default | Description |
|---|---|---|
| `file` | *(required)* | Path to the PDF file |
| `page` | `0` | Zero-based page index |
| `--dpi` | `150` | Render DPI for the raster preview |

**Examples:**

```sh
rbv brochure.pdf
rbv brochure.pdf 2 --dpi 300
rbv /path/to/job.pdf 0 --dpi 96
```

The initial preview renders at 72 DPI for fast startup, then the full-resolution render (at the specified DPI) replaces it in the background.

---

## Known limitations

- **Zoom quality**: raster-only rendering degrades past ~150–200% zoom. Planned for future LOD-aware tiling.
- **Page 0 only (effective)**: the page argument is supported but there is no in-viewer page navigation yet. Re-launch with a different page index to switch pages.
- **CFF / Type1 glyph outlines**: `ttf_parser` requires an sfnt container. Fonts embedded as raw CFF (FontFile3 / Type1C) are wrapped in a minimal OTTO header before parsing, but the glyph lookup path for these fonts is still being refined. Text bounding boxes are always drawn as fallback.
- **Form XObjects**: `build_object_tree` and `outline_page_text` do not recurse into `/Do` Form XObjects. Nested content will not appear in wireframe mode or hit-testing.
- **Spot colors**: `PdfColor::Separation` is not yet decoded; spot color objects show no color in the color panel.
- **Overprint**: `/ExtGState` OP/op/OPM flags are not read; overprint behavior is not reflected in the wireframe.

---

## Contributing

### Adding a new overlay type

1. Define a new struct in `renderer.rs` (following the `OverlayData` / `DebugOverlay` pattern).
2. Add a field or parameter to `SkiaRenderer::draw()`.
3. Add the corresponding state and toggle to `Viewer` in `viewer.rs`.
4. Draw in `draw()` at the appropriate z-order position (the comment block in `draw()` lists the current rendering order).

### Adding a new keyboard shortcut

Add a `KeyCode::KeyX` match arm in the `WindowEvent::KeyboardInput` block in `viewer.rs`. Modifiers (`ctrl_held`, `shift_held`) are tracked as boolean fields on `Viewer`.

### Coordinate conventions

Any function that converts between PDF space and screen space **must** use `pdf_rect_to_skia` / `pdf_point_to_screen` from `renderer.rs`, or the equivalent inline math (`page_rect.left() + rel_x * page_rect.width()`). Do not hardcode Y-axis direction — PDF is Y-up and screen is Y-down; the flip is applied in both helpers.

When storing a position for use across frames (e.g., the sampling marker), always store **PDF coordinates** and project to screen at draw time. Storing screen coordinates means the position becomes stale whenever the user zooms or pans.

### Reusing existing objects

Per project convention: **do not create new crate dependencies or duplicate types** that already exist in `rustybara` or `rustybara-icc`. Before adding a helper, check `rustybara::objects`, `rustybara::geometry`, and `rustybara_icc::profiles` first.

### ICC profile selection

The ICC diagnostic readout uses US Web Coated SWOP as the output profile because it is the standard North American press condition. If a different press condition is needed, change the `&profiles::US_WEB_COATED_SWOP` reference in `build_icc_transform()` in `viewer.rs`. All bundled profiles are listed in `rustybara-icc/src/profiles/mod.rs`.