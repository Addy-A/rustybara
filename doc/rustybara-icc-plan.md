# rustybara-icc — Implementation Plan

**Author:** Addy Alvarado  
**Crate:** `rustybara-icc`  
**Workspace:** `rustybara` monorepo  
**License:** LGPL-3.0-only (matches rustybara lib)  
**Status:** Scaffolded — `cargo new` boilerplate only, no implementation

---

## Purpose

`rustybara-icc` is the color management authority for the rustybara workspace. It owns three
distinct responsibilities that currently live in separate, incomplete, or absent locations:

1. **Profile storage** — Adobe ICC profile bytes embedded as static data, exposed as typed
   constants with structured metadata. Replaces ad-hoc profile loading elsewhere in the
   workspace.

2. **Transform engine** — lcms2-backed color transforms between any two ICC profiles.
   Migrates and supersedes the logic currently in `rustybara/src/color/icc.rs`. After
   migration, `rustybara` depends on `rustybara-icc` under its `color` feature flag rather
   than calling lcms2 directly.

3. **PDF color surgery** — lopdf-backed document-level color space rewriting. Walks a PDF's
   object graph and applies a color transform throughout: page content streams, image
   XObjects, color space resource dictionaries, and inline images. This is where operations
   like RGB → CMYK and Spot → CMYK live at the PDF level.

### Why a Separate Crate

- **Single source of truth for profiles.** Neither `rustybara` nor future crates (`duudler`,
  `rustybara-check`) should each bundle their own copy of ICC profiles or their own lcms2
  wrappers. One crate owns this.
- **Clean feature boundary.** Downstream crates that only need geometry or preflight
  operations do not pay the compile cost of lcms2 or embedded profile data.
- **Duudler integration point.** When the Duudler PDF backend needs color management,
  it depends on `rustybara-icc` directly. The transform engine is the bridge.
- **rustybara-check integration.** The `ConversionReport` type and color space information
  produced by this crate feed directly into rustybara-check's structural preflight layer.

---

## Workspace Changes Required

### 1. Root `Cargo.toml` — add workspace member

```toml
[workspace]
members = [
    "rustybara",
    "rbara",
    "rbara-gui",
    "rustybara-icc",   # ← add this
]
resolver = "2"
```

### 2. `rustybara/Cargo.toml` — replace direct lcms2 dependency

Current state:
```toml
[dependencies]
lcms2 = { version = "6.1.1", optional = true }

[features]
color = ["dep:lcms2"]
```

After migration:
```toml
[dependencies]
rustybara-icc = { path = "../rustybara-icc", optional = true }

[features]
color = ["dep:rustybara-icc"]
```

### 3. `rustybara/src/color/icc.rs` — gut and re-export

Once Phase 2 is complete, this file becomes a thin re-export shim so existing
call sites in `rustybara` do not break:

```rust
// rustybara/src/color/icc.rs — after migration
pub use rustybara_icc::{
    ColorTransform,
    RenderingIntent,
    ColorSpaceKind,
    profiles,
};
```

Remove all lcms2 calls from this file. Do not delete the file — the re-exports
preserve the existing public API surface of `rustybara`.

### 4. `rustybara-icc/Cargo.toml` — new crate manifest

```toml
[package]
name = "rustybara-icc"
version = "0.1.0"
edition = "2024"
license = "LGPL-3.0-only"
description = "ICC color profile storage and transform engine for the rustybara workspace"
repository = "https://github.com/Addy-A/rustybara"

[dependencies]
lcms2       = "6.1.1"
lopdf       = "0.40.0"
image       = { version = "0.25.10", features = ["jpeg", "tiff", "png", "webp"] }
thiserror   = "2"

[dev-dependencies]
# test fixtures and golden file comparison tooling as needed
```

**Note:** Before finalizing versions, verify against the root workspace `Cargo.lock`.
Use the same versions already present — do not introduce version divergence within
the workspace.

---

## Public API — Target Surface

The following is the intended public API at completion. Implementation happens
incrementally across phases; the types are defined here so the surface is designed
before any code is written.

### Error type

```rust
// src/error.rs
#[derive(thiserror::Error, Debug)]
pub enum IccError {
    #[error("lcms2 transform error: {0}")]
    Transform(String),
    #[error("unsupported color space: {0}")]
    UnsupportedColorSpace(String),
    #[error("PDF object error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("image processing error: {0}")]
    Image(String),
    #[error("profile loading error: {0}")]
    Profile(String),
}

pub type Result<T> = std::result::Result<T, IccError>;
```

### Profile catalog

```rust
// src/profiles/mod.rs

pub struct IccProfile {
    pub name: &'static str,
    pub description: &'static str,
    pub color_space: ColorSpaceKind,
    pub bytes: &'static [u8],
}

// RGB profiles
pub const SRGB_IEC61966:       IccProfile = ...;
pub const ADOBE_RGB_1998:       IccProfile = ...;
pub const DISPLAY_P3:           IccProfile = ...;
pub const PROPHOTO_RGB:         IccProfile = ...;

// CMYK profiles — print standard
pub const COATED_FOGRA39:       IccProfile = ...;
pub const COATED_FOGRA27:       IccProfile = ...;
pub const UNCOATED_FOGRA29:     IccProfile = ...;
pub const ISO_COATED_V2:        IccProfile = ...;
pub const US_WEB_COATED_SWOP:   IccProfile = ...;
pub const US_SHEETFED_COATED:   IccProfile = ...;
pub const GRACOL_COATED:        IccProfile = ...;

// Grayscale
pub const DOT_GAIN_10:          IccProfile = ...;
pub const DOT_GAIN_15:          IccProfile = ...;
pub const DOT_GAIN_20:          IccProfile = ...;
pub const DOT_GAIN_25:          IccProfile = ...;
pub const DOT_GAIN_30:          IccProfile = ...;
pub const GRAY_GAMMA_18:        IccProfile = ...;
pub const GRAY_GAMMA_22:        IccProfile = ...;

// Lab (PCS reference)
pub const LAB_D50:              IccProfile = ...;
```

### Color space kind

```rust
// src/color_space.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorSpaceKind {
    Rgb,
    Cmyk,
    Gray,
    Lab,
    Unknown,
}
```

This type replaces `rustybara::color::icc::ColorSpaceKind` after migration.
The re-export in `rustybara/src/color/icc.rs` preserves existing call sites.

### Rendering intent

```rust
// src/intent.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderingIntent {
    Perceptual,
    #[default]
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}
```

### Color transform

```rust
// src/transform.rs
pub struct ColorTransform { /* lcms2 handle, private */ }

impl ColorTransform {
    /// Build a transform between two bundled profiles.
    pub fn new(
        from: &IccProfile,
        to: &IccProfile,
        intent: RenderingIntent,
    ) -> Result<Self>;

    /// Build a transform from arbitrary profile bytes.
    pub fn from_bytes(
        from: &[u8],
        to: &[u8],
        intent: RenderingIntent,
    ) -> Result<Self>;

    /// Apply transform to a raw packed pixel buffer in-place.
    /// `format` describes the channel layout and bit depth.
    pub fn apply_to_pixels(&self, pixels: &mut [u8], format: PixelFormat);

    /// Apply transform to an image::DynamicImage in-place.
    pub fn apply_to_image(&self, image: &mut image::DynamicImage) -> Result<()>;

    pub fn input_color_space(&self)  -> ColorSpaceKind;
    pub fn output_color_space(&self) -> ColorSpaceKind;
}

#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    Rgb8,
    Rgba8,
    Cmyk8,
    Gray8,
    Rgb16,
    Cmyk16,
}
```

### PDF color converter

```rust
// src/pdf.rs
pub struct PdfColorConverter<'a> {
    doc: &'a mut lopdf::Document,
    transform: ColorTransform,
}

impl<'a> PdfColorConverter<'a> {
    pub fn new(
        doc: &'a mut lopdf::Document,
        transform: ColorTransform,
    ) -> Self;

    /// Convert color spaces throughout the entire document.
    pub fn convert_document(&mut self) -> Result<ConversionReport>;

    /// Convert a single page by its lopdf ObjectId.
    pub fn convert_page(
        &mut self,
        page_id: lopdf::ObjectId,
    ) -> Result<()>;
}

pub struct ConversionReport {
    pub pages_processed:        u32,
    pub images_converted:       u32,
    pub spot_colors_flattened:  u32,
    pub color_spaces_rewritten: u32,
    pub warnings:               Vec<String>,
}
```

---

## Implementation Phases

Work in this order. Do not start a phase until the previous phase has passing tests.

---

### Phase 1 — Profile Storage

**Goal:** Compile-time embedded ICC profile bytes with typed metadata. No lcms2.
No lopdf. Shippable as `rustybara-icc v0.1.0` even in isolation.

**Steps:**

1. Create `rustybara-icc/src/lib.rs` — declare modules, `pub use` the public surface.

2. Create `rustybara-icc/src/error.rs` — the `IccError` enum and `Result` alias.
   Add `thiserror` to `Cargo.toml`.

3. Create `rustybara-icc/src/color_space.rs` — the `ColorSpaceKind` enum.

4. Create `rustybara-icc/src/profiles/mod.rs` — the `IccProfile` struct with
   `name`, `description`, `color_space`, and `bytes` fields.

5. Obtain the Adobe ICC profiles. These are the same profiles confirmed as
   license-compliant in a prior conversation. Download from:
   - `https://www.adobe.com/support/downloads/iccprofiles/iccprofiles_win.html`
   - `https://www.adobe.com/support/downloads/iccprofiles/iccprofiles_mac.html`

   Place `.icc` files in `rustybara-icc/src/profiles/data/`. Add this directory
   to `.gitignore` if the license requires it, or commit if redistribution is
   confirmed permitted. Confirm the Adobe ICC license terms before committing
   binary data to the repository.

6. Embed each profile using `include_bytes!`:

   ```rust
   pub const ADOBE_RGB_1998: IccProfile = IccProfile {
       name: "AdobeRGB1998",
       description: "Adobe RGB (1998)",
       color_space: ColorSpaceKind::Rgb,
       bytes: include_bytes!("data/AdobeRGB1998.icc"),
   };
   ```

7. Write unit tests verifying:
   - Each constant compiles and has non-zero `bytes.len()`
   - `color_space` field matches the expected space for each profile
   - Profile bytes begin with the correct ICC header signature (`b"acsp"` at offset 36)

**Tag:** `rustybara-icc-v0.1.0` — profile storage only.

---

### Phase 2 — Transform Engine

**Goal:** Move lcms2 color transform logic from `rustybara/src/color/icc.rs` into
`rustybara-icc`. Wire `rustybara` to use it via path dependency.

**Steps:**

1. Add `lcms2 = "6.1.1"` to `rustybara-icc/Cargo.toml`. Verify version matches
   the workspace `Cargo.lock`.

2. Create `rustybara-icc/src/intent.rs` — the `RenderingIntent` enum.

3. Create `rustybara-icc/src/transform.rs`:
   - Implement `ColorTransform::new()` using `lcms2::Profile::new_icc()` and
     `lcms2::Transform::new()`.
   - Implement `ColorTransform::from_bytes()`.
   - Implement `apply_to_pixels()` using the lcms2 transform's `convert_pixels()`
     or equivalent. Map `PixelFormat` to lcms2 pixel format constants.
   - `input_color_space()` and `output_color_space()` read from the lcms2 profile
     color space field.

4. Create `rustybara-icc/src/pixel_format.rs` — the `PixelFormat` enum with
   lcms2 format constant mappings.

5. Update `rustybara-icc/src/lib.rs` — export the new types.

6. **Migrate `rustybara/src/color/icc.rs`:**
   - Copy any lcms2 logic not yet replicated in `rustybara-icc` into the new crate.
   - Replace the file contents with re-exports as described in the workspace
     changes section.
   - Update `rustybara/Cargo.toml` — replace `lcms2` with `rustybara-icc` path dep.
   - Run `cargo build` in the workspace root. Fix any breakages in `rustybara`.

7. Verify `remap_color` in `rbara/src/process.rs` still works end-to-end.
   The call chain should now go through `rustybara-icc` transparently.

8. Write unit tests:
   - Round-trip: transform sRGB white `[255, 255, 255]` through sRGB → AdobeRGB
     → sRGB and verify the result is within acceptable tolerance.
   - Known value: transform a specific CMYK value through FOGRA39 and verify
     the output matches a known reference computed from a trusted tool.
   - Color space accessors return correct `ColorSpaceKind` for each profile pair.

**Tag:** `rustybara-icc-v0.2.0` — transform engine complete.

---

### Phase 3 — Image Transform

**Goal:** Apply a `ColorTransform` to an `image::DynamicImage` in-place.
This is the raster image layer of RGB → CMYK conversion.

**Steps:**

1. Add `image = { version = "0.25.10", features = ["jpeg", "tiff", "png", "webp"] }`
   to `rustybara-icc/Cargo.toml`. Verify version matches workspace.

2. In `rustybara-icc/src/transform.rs`, implement `apply_to_image()`:
   - Match on `DynamicImage` variants to extract the raw pixel buffer.
   - Map the image color type to the corresponding `PixelFormat`.
   - Call `apply_to_pixels()` on the buffer in-place.
   - For RGB → CMYK: the output buffer has 4 channels where the input has 3.
     The buffer must be reallocated. The result is a `DynamicImage::ImageLuma8`
     is not appropriate — CMYK images need to be handled as raw byte buffers
     and re-wrapped. Document this clearly; the image crate has no native CMYK
     image type.
   - Return `IccError::UnsupportedColorSpace` for combinations the transform
     cannot handle.

3. Write integration tests:
   - Load a known PNG test image (commit a small fixture to `tests/fixtures/`).
   - Apply sRGB → FOGRA39 transform.
   - Verify the output buffer length is `width * height * 4` (CMYK).
   - Verify a known pixel value against a reference computed from a trusted tool.

**Note on CMYK images:** The `image` crate does not have a `DynamicImage::Cmyk8`
variant. After applying a CMYK transform, the output is raw bytes that must be
treated as 4-channel data and handled explicitly by the caller. Document this
limitation clearly in the public API docs. The PDF surgery layer in Phase 4
handles this correctly because it writes directly to PDF image XObject streams
rather than going through `DynamicImage`.

**Tag:** `rustybara-icc-v0.3.0` — image transform complete.

---

### Phase 4 — PDF Color Surgery

**Goal:** Walk a `lopdf::Document` and apply a `ColorTransform` throughout.
This is the highest-complexity phase. Work through sub-steps in order.

**Steps:**

1. Add `lopdf = "0.40.0"` to `rustybara-icc/Cargo.toml`.

2. Create `rustybara-icc/src/pdf.rs`. Add `PdfColorConverter` struct and
   `ConversionReport` struct.

3. **Sub-step 4a — Color space resource rewriting.**
   The most important structural change. For each page:
   - Access the page dictionary's `Resources` → `ColorSpace` dictionary.
   - Walk each declared color space.
   - If it is `/DeviceRGB` and the transform is RGB → CMYK, replace with
     `/DeviceCMYK`.
   - If it is `/ICCBased` with an embedded profile, replace with the target
     profile or `/DeviceCMYK` as appropriate.
   - If it is `/Separation` (spot color), handle in Phase 5. Skip for now,
     emit a warning in `ConversionReport`.
   - Increment `color_spaces_rewritten` in the report.

4. **Sub-step 4b — Content stream color operator rewriting.**
   For each page content stream:
   - Decompress if FlateDecode compressed (`lopdf::filters::decompress`).
   - Parse the content stream as a sequence of PDF operators.
   - Identify color-setting operators:
     - `rg` / `RG` — RGB fill/stroke (3 operands)
     - `k` / `K` — CMYK fill/stroke (4 operands)
     - `g` / `G` — Gray fill/stroke (1 operand)
     - `sc` / `SC` / `scn` / `SCN` — general color operators
   - For each `rg`/`RG` operator when converting RGB → CMYK:
     - Extract the three f32 operands as an RGB pixel.
     - Apply `transform.apply_to_pixels()` to that single pixel.
     - Replace the operator with `k`/`K` and the four CMYK output values.
   - Re-compress and write back.
   - This is the most error-prone step. Test with simple fixtures before
     attempting complex documents.

5. **Sub-step 4c — Image XObject conversion.**
   For each image XObject referenced from page resources:
   - Check `/ColorSpace` entry on the image dictionary.
   - If it matches the transform's input color space:
     - Extract the raw image bytes from the stream.
     - Decompress if necessary (DCTDecode → raw bytes via the `image` crate).
     - Apply `apply_to_pixels()` to the raw buffer.
     - Update `/ColorSpace` to the output color space.
     - Update `/BitsPerComponent` if changed.
     - Re-encode (JPEG for photographs, FlateDecode for flat color) and write back.
     - Increment `images_converted`.

6. Implement `convert_page()` composing 4a, 4b, 4c for a single page.

7. Implement `convert_document()` iterating over all pages, calling
   `convert_page()` for each, aggregating into `ConversionReport`.

8. Write integration tests:
   - Create a minimal test PDF (single page, solid RGB rectangle) programmatically
     using lopdf directly in the test. Do not depend on an external fixture file.
   - Apply RGB → CMYK transform.
   - Verify the output PDF's color operators are `k`/`K` not `rg`/`RG`.
   - Verify the output PDF parses without error in lopdf.
   - Verify the image XObject (if present) has `DeviceCMYK` color space.

**Tag:** `rustybara-icc-v0.4.0` — PDF surgery complete (no spot colors).

---

### Phase 5 — Spot Color Flattening

**Goal:** Handle `Separation` and `DeviceN` color spaces — flatten spot colors
to their CMYK alternates or apply a full re-separation via lcms2.

**Steps:**

1. In `rustybara-icc/src/pdf.rs`, add spot color handling to `convert_page()`.

2. For each `Separation` color space in the resource dictionary:
   - The PDF object is `[/Separation name alternateSpace tintTransform]`.
   - `alternateSpace` is typically `/DeviceCMYK` with a `tintTransform` function
     that maps a tint value (0.0–1.0) to CMYK components.
   - **Simple flatten:** Replace the `Separation` color space with its
     `alternateSpace` directly. Apply the `tintTransform` at each use site
     in the content stream to compute the CMYK values. This handles the common
     case where the spot color's alternate is already the target space.
   - **Re-separation via lcms2:** If the alternate is RGB or Lab, apply the
     `ColorTransform` to the alternate values to produce CMYK. This is the
     path for spots with RGB or Lab alternates.
   - Increment `spot_colors_flattened`.

3. For `DeviceN` (multi-channel, including Hexachrome and other extended gamut):
   - This is significantly more complex. For V1, emit a warning and skip.
     Document this as a known limitation.
   - Full DeviceN support is a post-v1.0 item.

4. Update `ConversionReport` — `spot_colors_flattened` should now be non-zero
   for documents containing spot colors.

5. Write tests with a PDF containing a `Separation` color space.

**Tag:** `rustybara-icc-v0.5.0` — spot color flattening complete.

---

### Phase 6 — Integration and Polish

**Goal:** Wire everything into the rustybara workspace correctly, write
documentation, and tag v1.0.0.

**Steps:**

1. **rbara integration check.** Run the full rbara test suite with the new
   dependency chain. Verify `remap_color` behavior is unchanged.

2. **API documentation.** Every public type and function must have a doc comment.
   Run `cargo doc --no-deps` and review the output. Pay particular attention to:
   - The CMYK image limitation documented on `apply_to_image()`.
   - The DeviceN limitation documented on `convert_document()`.
   - Usage examples in doc comments for `ColorTransform::new()` and
     `PdfColorConverter::convert_document()`.

3. **Publish readiness check.**
   - `cargo publish --dry-run` from `rustybara-icc/`
   - Verify all `include_bytes!` paths resolve correctly in the published crate
     (they must be relative to `src/`, not workspace root).
   - Verify the ICC profile files are included in `include` or not excluded by
     `.gitignore` as appropriate for publishing.

4. **Update workspace `README.md`** to document `rustybara-icc` as a workspace
   member and describe its role.

5. **Update `rustybara` crate documentation** to reflect that the `color` feature
   now depends on `rustybara-icc` and link to its documentation.

**Tag:** `rustybara-icc-v1.0.0`

---

## File Structure at Completion

```
rustybara-icc/
├── Cargo.toml
├── src/
│   ├── lib.rs              ← public re-exports, crate root
│   ├── error.rs            ← IccError, Result
│   ├── color_space.rs      ← ColorSpaceKind enum
│   ├── intent.rs           ← RenderingIntent enum
│   ├── pixel_format.rs     ← PixelFormat enum + lcms2 mappings
│   ├── transform.rs        ← ColorTransform
│   ├── pdf.rs              ← PdfColorConverter, ConversionReport
│   └── profiles/
│       ├── mod.rs          ← IccProfile struct + all constants
│       └── data/           ← embedded .icc files (binary)
│           ├── AdobeRGB1998.icc
│           ├── CoatedFOGRA39.icc
│           ├── UncoatedFOGRA29.icc
│           ├── USWebCoatedSWOP.icc
│           └── ... (all profiles)
└── tests/
    ├── fixtures/           ← small test images and PDFs
    ├── transform_tests.rs
    ├── image_tests.rs
    └── pdf_tests.rs
```

---

## Dependencies Summary

| Crate       | Version  | Purpose                              | Already in workspace? |
|-------------|----------|--------------------------------------|-----------------------|
| `lcms2`     | `6.1.1`  | ICC color transform engine           | Yes — verify lock     |
| `lopdf`     | `0.40.0` | PDF object graph read/write          | Yes — verify lock     |
| `image`     | `0.25.10`| Raster image pixel manipulation      | Yes — verify lock     |
| `thiserror` | `2`      | Error type derivation                | Verify workspace      |

No new external dependencies are introduced. All versions must match the existing
workspace `Cargo.lock` before adding to `rustybara-icc/Cargo.toml`.

---

## Key Decisions and Constraints

**lcms2 is the only color math engine.** No hand-rolled color math. lcms2 has
been battle-tested in production print workflows for decades. Trust it.

**CMYK is a first-class citizen.** The API never treats CMYK as a conversion
target. `ColorTransform` can go CMYK → CMYK (e.g., FOGRA39 → SWOP) as
naturally as RGB → CMYK.

**No unsafe code.** The lcms2 Rust bindings are safe. lopdf is safe. There is no
reason to write unsafe code in this crate.

**PDF surgery is non-destructive by convention.** `PdfColorConverter` mutates the
document passed to it. The caller is responsible for saving to a new path via
`rustybara`'s existing `output_path()` helper. This crate never writes files.

**DeviceN is out of scope for v1.0.** Document it clearly. Do not attempt it in
the v1.0 release.

**The Adobe ICC license must be verified before embedding.** The previous
conversation confirmed intent to use Adobe's bundled profiles, but the actual
license terms must be re-read before the `include_bytes!` calls are committed
and before publishing to crates.io. If redistribution is not permitted, the
crate ships with the ability to load profiles from a user-supplied path and
the Adobe profiles are documented as a recommended download rather than bundled.

---

## Relationship to Future Work

| Future crate/feature        | How rustybara-icc supports it                                          |
|-----------------------------|------------------------------------------------------------------------|
| `rustybara-check`           | `ConversionReport` and `ColorSpaceKind` feed the structural preflight layer |
| Duudler PDF backend         | `ColorTransform` is the bridge for CMYK-native Duudler output          |
| `rbara-gui` Remap Colors    | `PdfColorConverter` replaces the current manual lcms2 calls            |
| RGB → CMYK operation        | Phase 4 sub-steps 4a–4c are the complete implementation                |
| Spot → CMYK operation       | Phase 5 is the complete implementation                                 |
| rustybara-icc v2.x          | DeviceN support, Lab color space surgery, PDF/X OutputIntent embedding |
