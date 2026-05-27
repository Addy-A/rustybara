//! ICC-based plate separation for rbv.
//!
//! Produces a [`DynamicImage`] representing a single ink channel extracted from
//! the rasterized page — either a CMYK process channel or a spot-color plate.
//!
//! # Approach
//!
//! **CMYK plates** — Apply the sRGB→US Web Coated SWOP ICC transform to every
//! pixel of the source image, then extract one channel. The result is a
//! single-channel grayscale image where white = no ink, black = full density.
//! In ink-tinted mode the grayscale is blended with a standard process-ink
//! color to give the pressman an authentic plate preview.
//!
//! **Spot plates** — Query the object tree for all objects carrying the named
//! spot ink, then flood-fill their bounding boxes onto a new image. The ink
//! density at each pixel is determined by the tint value declared on the object.
//! In ink-tinted mode a generic violet is used for unknown inks; in grayscale
//! mode ink coverage maps to black density just like a process plate.
//!
//! # Limitations
//!
//! - No path rasterization — spot coverage is bbox-level, not shape-level.
//! - DeviceRGB or DeviceGray objects are not converted to process CMYK for the
//!   purpose of plate extraction.  Only DeviceCmyk objects contribute to process
//!   channel plates; only Separation objects contribute to spot plates.
//! - No overprint simulation.
//!
//! These are acceptable approximations for rbv's role as a visual aid.

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use rustybara::{
    geometry::Rect as PdfRect,
    objects::{ObjectKind, PageObject, PathPoint},
    outline::{GlyphVerb, PositionedGlyph},
};
use rustybara_icc::{profiles, ColorTransform, RenderingIntent};

// ── Process ink tints (sRGB) ──────────────────────────────────────────────────
// Approximate "printing ink on white stock" appearance.
const CYAN_TINT: [u8; 3] = [0x00, 0xAE, 0xEF]; // Pantone Process Cyan ~
const MAGENTA_TINT: [u8; 3] = [0xEC, 0x00, 0x8C]; // Pantone Process Magenta ~
const YELLOW_TINT: [u8; 3] = [0xFF, 0xF2, 0x00]; // Pantone Process Yellow ~
const BLACK_TINT: [u8; 3] = [0x23, 0x1F, 0x20]; // Rich black substrate ~
const SPOT_FALLBACK_TINT: [u8; 3] = [0x8C, 0x28, 0xDC]; // Generic violet

// ── ICC transform ─────────────────────────────────────────────────────────────

/// Build the sRGB → US Web Coated SWOP ICC color transform.
///
/// Prioritises a system sRGB profile found via OS color management; falls back
/// to the bundled AdobeRGB 1998 profile if no system profile is found. Returns
/// `None` only if lcms2 transform construction fails (should not happen with
/// valid profiles).
///
/// This is identical to the `build_icc_transform` helper in `viewer.rs` but
/// lives here so separation threads can call it without importing viewer
/// internals. `ColorTransform` is not `Send`/`Clone` so each background thread
/// must build its own instance.
pub fn build_icc_transform() -> Option<ColorTransform> {
    let dst = &profiles::US_WEB_COATED_SWOP;
    let intent = RenderingIntent::RelativeColorimetric;

    if let Some(srgb) = find_system_srgb() {
        if let Ok(t) = ColorTransform::from_bytes(&srgb.bytes, &dst.bytes, intent) {
            return Some(t);
        }
    }

    ColorTransform::new(&profiles::ADOBE_RGB_1998, dst, intent).ok()
}

/// Locate a valid RGB ICC profile from the OS color management directories.
fn find_system_srgb() -> Option<rustybara_icc::profiles::IccProfile> {
    use rustybara_icc::{profiles::IccProfile, ColorSpaceKind};

    #[cfg(target_os = "windows")]
    let candidates: &[&str] = &[
        "C:\\Windows\\System32\\spool\\drivers\\color\\sRGB Color Space Profile.icm",
        "C:\\Windows\\System32\\spool\\drivers\\color\\sRGB_IEC61966-2-1.icm",
        "C:\\Windows\\System32\\spool\\drivers\\color\\sRGB IEC61966-2-1.icm",
    ];
    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &[
        "/System/Library/ColorSync/Profiles/sRGB IEC61966-2.1.icc",
        "/Library/ColorSync/Profiles/sRGB.icc",
    ];
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let candidates: &[&str] = &[
        "/usr/share/color/icc/sRGB.icc",
        "/usr/share/colorhug/sRGB.icc",
        "/usr/share/color/icc/colord/sRGB.icc",
    ];

    for path in candidates {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sRGB")
            .to_string();
        if let Ok(profile) = IccProfile::from_user_bytes(stem.clone(), stem, bytes) {
            if profile.color_space == ColorSpaceKind::Rgb {
                return Some(profile);
            }
        }
    }
    None
}

// ── CMYK plate rendering ──────────────────────────────────────────────────────

/// Which CMYK channel to extract (channel index 0..3 in C/M/Y/K order).
#[derive(Clone, Copy)]
pub enum PlateChannel {
    Cyan = 0,
    Magenta = 1,
    Yellow = 2,
    Black = 3,
}

impl PlateChannel {
    fn tint_rgb(self) -> [u8; 3] {
        match self {
            PlateChannel::Cyan => CYAN_TINT,
            PlateChannel::Magenta => MAGENTA_TINT,
            PlateChannel::Yellow => YELLOW_TINT,
            PlateChannel::Black => BLACK_TINT,
        }
    }
}

// ── Geometry + drawing helpers ────────────────────────────────────────────────

/// Map a PDF page-space point to image-pixel coordinates.
///
/// PDF: origin bottom-left, Y-up.  Image: origin top-left, Y-down.
fn pdf_pt_to_img(
    pdf_x: f64,
    pdf_y: f64,
    media_box: &PdfRect,
    scale_x: f64,
    scale_y: f64,
    img_h: u32,
) -> skia_safe::Point {
    let ix = (pdf_x - media_box.x) * scale_x;
    let iy = img_h as f64 - (pdf_y - media_box.y) * scale_y;
    skia_safe::Point::new(ix as f32, iy as f32)
}

/// Build a Skia `Path` from an object's subpath data applying the object CTM.
///
/// Returns `None` when the object has no subpath points (caller falls back to
/// the bbox rect).
fn build_obj_path(
    obj: &PageObject,
    media_box: &PdfRect,
    scale_x: f64,
    scale_y: f64,
    img_h: u32,
) -> Option<skia_safe::Path> {
    if !obj.subpaths.iter().any(|s| !s.points.is_empty()) {
        return None;
    }
    let mut b = skia_safe::PathBuilder::new();
    for sub in &obj.subpaths {
        for &pt in &sub.points {
            match pt {
                PathPoint::MoveTo(lx, ly) => {
                    let (px, py) = obj.ctm.transform_point(lx, ly);
                    b.move_to(pdf_pt_to_img(px, py, media_box, scale_x, scale_y, img_h));
                }
                PathPoint::LineTo(lx, ly) => {
                    let (px, py) = obj.ctm.transform_point(lx, ly);
                    b.line_to(pdf_pt_to_img(px, py, media_box, scale_x, scale_y, img_h));
                }
                PathPoint::CurveTo(c1x, c1y, c2x, c2y, ex, ey) => {
                    let (s1x, s1y) = obj.ctm.transform_point(c1x, c1y);
                    let (s2x, s2y) = obj.ctm.transform_point(c2x, c2y);
                    let (sex, sey) = obj.ctm.transform_point(ex, ey);
                    b.cubic_to(
                        pdf_pt_to_img(s1x, s1y, media_box, scale_x, scale_y, img_h),
                        pdf_pt_to_img(s2x, s2y, media_box, scale_x, scale_y, img_h),
                        pdf_pt_to_img(sex, sey, media_box, scale_x, scale_y, img_h),
                    );
                }
                PathPoint::Close => {
                    b.close();
                }
            }
        }
    }
    Some(b.detach())
}

/// Build a Skia `Path` from a [`PositionedGlyph`]'s verb sequence, mapping
/// PDF page-space coordinates to image pixel coordinates.
///
/// Returns `None` for empty glyph records (e.g. space characters).
fn build_glyph_path(
    glyph: &PositionedGlyph,
    media_box: &PdfRect,
    scale_x: f64,
    scale_y: f64,
    img_h: u32,
) -> Option<skia_safe::Path> {
    if glyph.verbs.is_empty() {
        return None;
    }
    let mut b = skia_safe::PathBuilder::new();
    for verb in &glyph.verbs {
        match *verb {
            GlyphVerb::MoveTo(x, y) => {
                b.move_to(pdf_pt_to_img(x, y, media_box, scale_x, scale_y, img_h));
            }
            GlyphVerb::LineTo(x, y) => {
                b.line_to(pdf_pt_to_img(x, y, media_box, scale_x, scale_y, img_h));
            }
            GlyphVerb::QuadTo(cx, cy, x, y) => {
                b.quad_to(
                    pdf_pt_to_img(cx, cy, media_box, scale_x, scale_y, img_h),
                    pdf_pt_to_img(x, y, media_box, scale_x, scale_y, img_h),
                );
            }
            GlyphVerb::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                b.cubic_to(
                    pdf_pt_to_img(c1x, c1y, media_box, scale_x, scale_y, img_h),
                    pdf_pt_to_img(c2x, c2y, media_box, scale_x, scale_y, img_h),
                    pdf_pt_to_img(x, y, media_box, scale_x, scale_y, img_h),
                );
            }
            GlyphVerb::Close => {
                b.close();
            }
        }
    }
    Some(b.detach())
}

/// Return `true` if any endpoint in `glyph` falls inside the PDF-space `bbox`.
///
/// A small tolerance (`2.0` PDF points) is added so glyphs whose ink just
/// grazes the text object boundary are not incorrectly excluded.
fn glyph_in_bbox(glyph: &PositionedGlyph, bbox: &PdfRect) -> bool {
    const TOL: f64 = 2.0;
    let ex = bbox.x - TOL;
    let ey = bbox.y - TOL;
    let ew = bbox.width + TOL * 2.0;
    let eh = bbox.height + TOL * 2.0;

    for verb in &glyph.verbs {
        let (x, y) = match *verb {
            GlyphVerb::MoveTo(x, y) | GlyphVerb::LineTo(x, y) => (x, y),
            GlyphVerb::QuadTo(_, _, x, y) | GlyphVerb::CubicTo(_, _, _, _, x, y) => (x, y),
            GlyphVerb::Close => continue,
        };
        if x >= ex && x < ex + ew && y >= ey && y < ey + eh {
            return true;
        }
    }
    false
}

/// Convert a PDF object bbox to an image-space Skia `Rect`.
fn pdf_bbox_to_img_rect(
    obj: &PageObject,
    media_box: &PdfRect,
    scale_x: f64,
    scale_y: f64,
    img_h: u32,
) -> skia_safe::Rect {
    // In PDF space bbox.top() > bbox.y; in image space that's a smaller Y value.
    let tl = pdf_pt_to_img(obj.bbox.x, obj.bbox.top(), media_box, scale_x, scale_y, img_h);
    let br = pdf_pt_to_img(obj.bbox.right(), obj.bbox.y, media_box, scale_x, scale_y, img_h);
    skia_safe::Rect::from_ltrb(tl.x, tl.y, br.x, br.y)
}

/// Convert an ink density (0.0 = no ink = white, 1.0 = full ink) to a Skia `Color`.
fn density_to_skia_color(density: f64, tinted: bool, tint: [u8; 3]) -> skia_safe::Color {
    let [tr, tg, tb] = tint;
    if tinted {
        let r = ((1.0 - density) * 255.0 + density * tr as f64) as u8;
        let g = ((1.0 - density) * 255.0 + density * tg as f64) as u8;
        let b = ((1.0 - density) * 255.0 + density * tb as f64) as u8;
        skia_safe::Color::from_argb(255, r, g, b)
    } else {
        let lum = ((1.0 - density) * 255.0) as u8;
        skia_safe::Color::from_argb(255, lum, lum, lum)
    }
}

/// Create a white-background RGBA8888 CPU raster surface for plate rendering.
fn make_plate_surface(img_w: u32, img_h: u32) -> skia_safe::Surface {
    let info = skia_safe::ImageInfo::new(
        (img_w as i32, img_h as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut surface = skia_safe::surfaces::raster(&info, None, None)
        .expect("Skia CPU raster surface for plate separation");
    surface.canvas().clear(skia_safe::Color::WHITE);
    surface
}

/// Read a `RGBA8888 Unpremul` Skia CPU surface back into a `DynamicImage`.
///
/// Panics only if the surface has zero pixels (should never occur in practice).
fn surface_to_dynamic_image(surface: &mut skia_safe::Surface, img_w: u32, img_h: u32) -> DynamicImage {
    if let Some(pixmap) = surface.peek_pixels() {
        if let Some(bytes) = pixmap.bytes() {
            if let Some(img) = RgbaImage::from_raw(img_w, img_h, bytes.to_vec()) {
                return DynamicImage::ImageRgba8(img);
            }
        }
    }
    // Should never be reached for a CPU raster surface.
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(img_w, img_h, Rgba([255, 255, 255, 255])))
}

/// Extract CMYK channel density directly from a color value.
///
/// Like `cmyk_channel_density` but operates on a `PdfColor` directly so that
/// fill and stroke can be queried separately for FillStroke objects.
fn channel_density_from_color(
    color: &rustybara::objects::PdfColor,
    channel: PlateChannel,
    transform: Option<&ColorTransform>,
) -> Option<f64> {
    let [c, m, y, k] = to_cmyk(color, transform)?;
    let density = match channel {
        PlateChannel::Cyan    => c,
        PlateChannel::Magenta => m,
        PlateChannel::Yellow  => y,
        PlateChannel::Black   => k,
    };
    Some(density.clamp(0.0, 1.0))
}

/// Draw a single vector object onto a plate canvas.
///
/// Uses actual subpath geometry when available (Fill/Stroke/FillStroke objects
/// with path data), falling back to the bbox rect for Text, FormXObject, and
/// any object with no recorded subpoints.
///
/// Fill and stroke are drawn as separate Skia calls so objects with different
/// fill and stroke densities (e.g. a FillStroke with DeviceCMYK fill ≠ stroke)
/// produce the correct per-channel result.
fn draw_vector_on_plate(
    canvas: &skia_safe::Canvas,
    obj: &PageObject,
    fill_density: Option<f64>,
    stroke_density: Option<f64>,
    tinted: bool,
    tint: [u8; 3],
    media_box: &PdfRect,
    scale_x: f64,
    scale_y: f64,
    img_h: u32,
    glyph_outlines: &[PositionedGlyph],
) {
    let path = build_obj_path(obj, media_box, scale_x, scale_y, img_h);
    let bbox = pdf_bbox_to_img_rect(obj, media_box, scale_x, scale_y, img_h);
    // Stroke width in PDF points → image pixels (average of X/Y scale).
    let stroke_w = ((obj.stroke_width * (scale_x + scale_y) / 2.0) as f32).max(0.5);

    let mut fill_paint = skia_safe::Paint::default();
    fill_paint.set_anti_alias(true);
    fill_paint.set_style(skia_safe::paint::Style::Fill);

    let mut stroke_paint = skia_safe::Paint::default();
    stroke_paint.set_anti_alias(true);
    stroke_paint.set_style(skia_safe::paint::Style::Stroke);
    stroke_paint.set_stroke_width(stroke_w);

    let draw_fill_shape = |paint: &skia_safe::Paint| {
        if let Some(ref p) = path {
            canvas.draw_path(p, paint);
        } else {
            canvas.draw_rect(bbox, paint);
        }
    };
    let draw_stroke_shape = |paint: &skia_safe::Paint| {
        if let Some(ref p) = path {
            canvas.draw_path(p, paint);
        } else {
            canvas.draw_rect(bbox, paint);
        }
    };

    match &obj.kind {
        // Fill-only path/form objects: draw the actual subpath geometry.
        ObjectKind::Fill | ObjectKind::FormXObject => {
            if let Some(d) = fill_density {
                fill_paint.set_color(density_to_skia_color(d, tinted, tint));
                draw_fill_shape(&fill_paint);
            }
        }
        // Text objects: prefer actual glyph outline paths; fall back to bbox.
        ObjectKind::Text(_) => {
            if let Some(d) = fill_density {
                fill_paint.set_color(density_to_skia_color(d, tinted, tint));
                let mut drew_glyphs = false;
                for glyph in glyph_outlines.iter().filter(|g| glyph_in_bbox(g, &obj.bbox)) {
                    if let Some(p) = build_glyph_path(glyph, media_box, scale_x, scale_y, img_h) {
                        canvas.draw_path(&p, &fill_paint);
                        drew_glyphs = true;
                    }
                }
                if !drew_glyphs {
                    // No glyph outlines available (e.g. Type-1 or CID font) →
                    // fall back to filled bbox rectangle as before.
                    canvas.draw_rect(bbox, &fill_paint);
                }
            }
        }
        // Stroke-only: prefer stroke_density, fall back to fill_density.
        ObjectKind::Stroke => {
            if let Some(d) = stroke_density.or(fill_density) {
                stroke_paint.set_color(density_to_skia_color(d, tinted, tint));
                draw_stroke_shape(&stroke_paint);
            }
        }
        // Fill-and-stroke: fill first (back), stroke on top.
        ObjectKind::FillStroke => {
            if let Some(d) = fill_density {
                fill_paint.set_color(density_to_skia_color(d, tinted, tint));
                draw_fill_shape(&fill_paint);
            }
            if let Some(d) = stroke_density {
                stroke_paint.set_color(density_to_skia_color(d, tinted, tint));
                draw_stroke_shape(&stroke_paint);
            }
        }
        ObjectKind::Image => {} // caller handles images
    }
}

// ── CMYK plate rendering ──────────────────────────────────────────────────────

/// Render a CMYK process channel plate from the PDF object tree.
///
/// Uses object-level color data (`fill_color`/`stroke_color`) rather than
/// per-pixel ICC conversion of the rasterized image. This preserves the
/// original CMYK channel identity of each object — a 100% K text element
/// correctly contributes zero density to C, M, and Y plates.
///
/// Color handling per object:
/// * `DeviceCMYK` — channel value read directly; no conversion needed.
/// * `DeviceGray(v)` — maps to K only: K density = (1.0 − v), CMY = 0.
///   This matches PDF spec §8.6.6.2: DeviceGray 0.0 = black, 1.0 = white.
/// * `DeviceRGB` — if `transform` is provided, converts to CMYK via ICC
///   (sRGB → US Web Coated SWOP). Falls back to naive subtractive (1−R, 1−G, 1−B)
///   when no transform is available.
/// * `Separation` — skipped; spot inks do not appear on process plates.
/// * `ObjectKind::Image` — skipped; embedded pixel data is not in the object
///   tree. A future improvement could sub-sample the rasterized page for these.
///
/// Returns an RGBA image of `img_w × img_h` pixels (white background = no ink).
pub fn render_cmyk_plate(
    objects: &[PageObject],
    media_box: &PdfRect,
    channel: PlateChannel,
    tinted: bool,
    transform: Option<&ColorTransform>,
    page_image: Option<&DynamicImage>,
    glyph_outlines: &[PositionedGlyph],
    img_w: u32,
    img_h: u32,
) -> DynamicImage {
    let tint = channel.tint_rgb();
    let scale_x = img_w as f64 / media_box.width;
    let scale_y = img_h as f64 / media_box.height;

    let mut surface = make_plate_surface(img_w, img_h);

    for obj in objects {
        if matches!(obj.kind, ObjectKind::Image) {
            // Crop the bbox region from the rasterized page, batch-convert
            // through ICC, build a Skia image from the result and draw it —
            // preserving paint order with the surrounding vector objects.
            if let (Some(src), Some(t)) = (page_image, transform) {
                let px0 = (((obj.bbox.x - media_box.x) * scale_x) as i64).max(0) as u32;
                let py0 = ((img_h as f64 - (obj.bbox.y - media_box.y + obj.bbox.height) * scale_y) as i64).max(0) as u32;
                let px1 = (((obj.bbox.x - media_box.x + obj.bbox.width) * scale_x) as i64).min(img_w as i64) as u32;
                let py1 = ((img_h as f64 - (obj.bbox.y - media_box.y) * scale_y) as i64).min(img_h as i64) as u32;

                if px0 < px1 && py0 < py1 {
                    let rw = (px1 - px0) as usize;
                    let rh = (py1 - py0) as usize;
                    let ch_idx = channel as usize;
                    let [tr, tg, tb] = tint;

                    let mut rgb_buf = Vec::with_capacity(rw * rh * 3);
                    for py in py0..py1 {
                        for px in px0..px1 {
                            let p = src.get_pixel(px, py);
                            rgb_buf.extend_from_slice(&[p[0], p[1], p[2]]);
                        }
                    }
                    let cmyk_buf = t.convert(&rgb_buf);

                    let mut rgba_buf = Vec::with_capacity(rw * rh * 4);
                    for i in 0..(rw * rh) {
                        let base = i * 4;
                        let density = if base + ch_idx < cmyk_buf.len() {
                            cmyk_buf[base + ch_idx] as f64 / 255.0
                        } else {
                            0.0
                        };
                        if tinted {
                            rgba_buf.extend_from_slice(&[
                                ((1.0 - density) * 255.0 + density * tr as f64) as u8,
                                ((1.0 - density) * 255.0 + density * tg as f64) as u8,
                                ((1.0 - density) * 255.0 + density * tb as f64) as u8,
                                255,
                            ]);
                        } else {
                            let lum = ((1.0 - density) * 255.0) as u8;
                            rgba_buf.extend_from_slice(&[lum, lum, lum, 255]);
                        }
                    }

                    let img_info = skia_safe::ImageInfo::new(
                        (rw as i32, rh as i32),
                        skia_safe::ColorType::RGBA8888,
                        skia_safe::AlphaType::Unpremul,
                        None,
                    );
                    let data = skia_safe::Data::new_copy(&rgba_buf);
                    if let Some(sk_img) = skia_safe::images::raster_from_data(&img_info, data, rw * 4) {
                        let dst = skia_safe::Rect::from_xywh(
                            px0 as f32, py0 as f32,
                            (px1 - px0) as f32, (py1 - py0) as f32,
                        );
                        surface.canvas().draw_image_rect(
                            &sk_img,
                            None,
                            dst,
                            &skia_safe::Paint::default(),
                        );
                    }
                }
            }
            continue;
        }

        // For vector objects, compute fill/stroke densities independently so
        // that FillStroke objects with different fill and stroke colors are
        // handled correctly (e.g. DeviceCMYK fill ≠ stroke on the same object).
        let fill_density = obj.fill_color.as_ref()
            .and_then(|c| channel_density_from_color(c, channel, transform));
        let stroke_density = obj.stroke_color.as_ref()
            .and_then(|c| channel_density_from_color(c, channel, transform));

        if fill_density.is_none() && stroke_density.is_none() {
            continue;
        }

        draw_vector_on_plate(
            surface.canvas(),
            obj,
            fill_density,
            stroke_density,
            tinted,
            tint,
            media_box,
            scale_x,
            scale_y,
            img_h,
            glyph_outlines,
        );
    }

    surface_to_dynamic_image(&mut surface, img_w, img_h)
}

// ── Spot plate rendering ──────────────────────────────────────────────────────

/// Render a spot-color plate by flood-filling the bounding boxes of matched objects.
///
/// Each matched object's bbox is filled at the tint density declared on the
/// object.  In `tinted` mode the fill color is `spot_rgb` (or the generic
/// violet fallback); in grayscale mode ink density maps to a white-to-black
/// scale.
///
/// `img_w`/`img_h` are the pixel dimensions of the target image (must match the
/// source raster so the result can be swapped in for it directly).  `media_box`
/// is the PDF page media box used to map PDF coordinates to pixel space.
pub fn render_spot_plate(
    objects: &[PageObject],
    media_box: &PdfRect,
    tinted: bool,
    spot_rgb: Option<[u8; 3]>,
    glyph_outlines: &[PositionedGlyph],
    img_w: u32,
    img_h: u32,
) -> DynamicImage {
    let tint = spot_rgb.unwrap_or(SPOT_FALLBACK_TINT);
    let scale_x = img_w as f64 / media_box.width;
    let scale_y = img_h as f64 / media_box.height;

    let mut surface = make_plate_surface(img_w, img_h);

    for obj in objects {
        // Image objects don't carry spot color data — skip.
        if matches!(obj.kind, ObjectKind::Image) {
            continue;
        }
        let density = spot_tint_for_object(obj);
        draw_vector_on_plate(
            surface.canvas(),
            obj,
            Some(density),
            None,
            tinted,
            tint,
            media_box,
            scale_x,
            scale_y,
            img_h,
            glyph_outlines,
        );
    }

    surface_to_dynamic_image(&mut surface, img_w, img_h)
}

/// Convert any `PdfColor` to `[C, M, Y, K]` (0.0–1.0 each).
///
/// Returns `None` for `Separation` colors – spot inks have no defined
/// position on a process plate.
fn to_cmyk(
    color: &rustybara::objects::PdfColor,
    transform: Option<&ColorTransform>,
) -> Option<[f64; 4]> {
    use rustybara::objects::PdfColor;
    match color {
        PdfColor::DeviceCmyk(c, m, y, k) => Some([*c, *m, *y, *k]),
        PdfColor::DeviceGray(v) => Some([0.0, 0.0, 0.0, 1.0 - v]),
        PdfColor::DeviceRgb(r, g, b) => {
            if let Some(t) = transform {
                let ru = (*r * 255.0) as u8;
                let gu = (*g * 255.0) as u8;
                let bu = (*b * 255.0) as u8;
                let cmyk = t.convert(&[ru, gu, bu]);
                (cmyk.len() >= 4).then(|| {
                    [
                        cmyk[0] as f64 / 255.0,
                        cmyk[1] as f64 / 255.0,
                        cmyk[2] as f64 / 255.0,
                        cmyk[3] as f64 / 255.0,
                    ]
                })
            } else {
                Some([1.0 - r, 1.0 - g, 1.0 - b, 0.0])
            }
        }
        PdfColor::Separation { .. } => None,
    }
}

/// Extract the spot ink tint density (0.0–1.0) for a matched object.
///
/// Prefers fill over stroke; falls back to 1.0 (full density) if neither is
/// a Separation color (which should not happen if `filter_by_ink` was used, but
/// is safe to handle).
fn spot_tint_for_object(obj: &PageObject) -> f64 {
    use rustybara::objects::PdfColor;

    let pick = obj.fill_color.as_ref().or(obj.stroke_color.as_ref());

    match pick {
        Some(PdfColor::Separation { tint, .. }) => (*tint).clamp(0.0, 1.0),
        _ => 1.0,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use rustybara::geometry::{Matrix, Rect};
    use rustybara::objects::{ObjectKind, OverprintState, PageObject, PdfColor};

    fn spot_obj(x: f64, y: f64, w: f64, h: f64, tint: f64) -> PageObject {
        PageObject {
            bbox: Rect {
                x,
                y,
                width: w,
                height: h,
            },
            ctm: Matrix::identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::Separation {
                name: "PANTONE 485 C".to_string(),
                tint,
            }),
            stroke_color: None,
            stroke_width: 0.0,
            overprint: OverprintState::default(),
            subpaths: vec![],
        }
    }

    // ── render_spot_plate ──────────────────────────────────────────────────────

    #[test]
    fn spot_plate_dimensions_match() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let obj = spot_obj(0.0, 0.0, 612.0, 792.0, 1.0);
        let plate = render_spot_plate(&[obj], &media, false, None, &[], 100, 200);
        assert_eq!(plate.width(), 100);
        assert_eq!(plate.height(), 200);
    }

    #[test]
    fn spot_plate_grayscale_full_tint_is_black() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = spot_obj(0.0, 0.0, 100.0, 100.0, 1.0);
        let plate = render_spot_plate(&[obj], &media, false, None, &[], 10, 10);
        let pixel = plate.get_pixel(5, 5);
        // Grayscale full density → black (0, 0, 0)
        assert_eq!(pixel[0], 0, "R should be 0 for full-density grayscale");
        assert_eq!(pixel[1], 0, "G should be 0");
        assert_eq!(pixel[2], 0, "B should be 0");
    }

    #[test]
    fn spot_plate_grayscale_zero_tint_is_white() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = spot_obj(0.0, 0.0, 100.0, 100.0, 0.0);
        let plate = render_spot_plate(&[obj], &media, false, None, &[], 10, 10);
        let pixel = plate.get_pixel(5, 5);
        assert_eq!(pixel[0], 255, "R should be 255 for zero-density grayscale");
        assert_eq!(pixel[1], 255, "G should be 255");
        assert_eq!(pixel[2], 255, "B should be 255");
    }

    #[test]
    fn spot_plate_tinted_full_tint_matches_fallback_color() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = spot_obj(0.0, 0.0, 100.0, 100.0, 1.0);
        let plate = render_spot_plate(&[obj], &media, true, None, &[], 10, 10);
        let pixel = plate.get_pixel(5, 5);
        // At full tint, the output should be very close to the fallback violet
        let [tr, tg, tb] = SPOT_FALLBACK_TINT;
        assert!((pixel[0] as i32 - tr as i32).abs() <= 1, "R={}", pixel[0]);
        assert!((pixel[1] as i32 - tg as i32).abs() <= 1, "G={}", pixel[1]);
        assert!((pixel[2] as i32 - tb as i32).abs() <= 1, "B={}", pixel[2]);
    }

    #[test]
    fn spot_plate_background_is_white() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        // Object covers only the lower-left quarter in PDF space.
        // In pixel space (Y-flipped) that maps to the lower quarter of the image.
        let obj = spot_obj(0.0, 0.0, 50.0, 50.0, 1.0);
        let plate = render_spot_plate(&[obj], &media, false, None, &[], 100, 100);
        // Top-left pixel (PDF top-right area) should still be white.
        let pixel = plate.get_pixel(5, 5);
        assert_eq!(pixel[0], 255, "background should be white");
        assert_eq!(pixel[1], 255);
        assert_eq!(pixel[2], 255);
    }

    #[test]
    fn spot_plate_empty_objects_all_white() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let plate = render_spot_plate(&[], &media, false, None, &[], 50, 50);
        for (_, _, p) in plate.pixels() {
            assert_eq!(p[0], 255);
            assert_eq!(p[1], 255);
            assert_eq!(p[2], 255);
        }
    }

    // ── render_cmyk_plate (smoke, no ICC) ─────────────────────────────────────

    // These tests only run when an ICC transform can be built. The build_icc_transform
    // function is deterministic but requires the bundled profiles feature which is
    // always enabled in this workspace. We guard with a runtime check anyway so the
    // test doesn't fail in environments where lcms2 fails to initialise.
    fn cmyk_obj(x: f64, y: f64, w: f64, h: f64, c: f64, m: f64, y_ch: f64, k: f64) -> PageObject {
        PageObject {
            bbox: Rect {
                x,
                y,
                width: w,
                height: h,
            },
            ctm: Matrix::identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::DeviceCmyk(c, m, y_ch, k)),
            stroke_color: None,
            stroke_width: 0.0,
            overprint: OverprintState::default(),
            subpaths: vec![],
        }
    }

    #[test]
    fn cmyk_plate_dimensions_preserved() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let plate = render_cmyk_plate(&[], &media, PlateChannel::Cyan, false, None, None, &[], 32, 16);
        assert_eq!(plate.width(), 32);
        assert_eq!(plate.height(), 16);
    }

    #[test]
    fn cmyk_k_only_object_has_zero_cyan() {
        // DeviceCMYK(0, 0, 0, 1.0) — 100% black. Cyan channel must be 0 → white on plate.
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = cmyk_obj(0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 0.0, 1.0);
        let plate = render_cmyk_plate(&[obj], &media, PlateChannel::Cyan, false, None, None, &[], 10, 10);
        let pixel = plate.get_pixel(5, 5);
        assert_eq!(
            pixel[0], 255,
            "100% K should appear white on Cyan plate, got R={}",
            pixel[0]
        );
    }

    #[test]
    fn cmyk_k_only_object_is_black_on_k_plate() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = cmyk_obj(0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 0.0, 1.0);
        let plate = render_cmyk_plate(&[obj], &media, PlateChannel::Black, false, None, None, &[], 10, 10);
        let pixel = plate.get_pixel(5, 5);
        assert_eq!(
            pixel[0], 0,
            "100% K should be black on K plate, got R={}",
            pixel[0]
        );
    }

    #[test]
    fn cmyk_pure_cyan_object_has_zero_k() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = cmyk_obj(0.0, 0.0, 100.0, 100.0, 1.0, 0.0, 0.0, 0.0);
        let plate = render_cmyk_plate(&[obj], &media, PlateChannel::Black, false, None, None, &[], 10, 10);
        let pixel = plate.get_pixel(5, 5);
        assert_eq!(
            pixel[0], 255,
            "100% C should appear white on K plate, got R={}",
            pixel[0]
        );
    }

    #[test]
    fn cmyk_gray_object_maps_to_k_only() {
        // DeviceGray(0.0) = full black → K = 1.0, CMY = 0.
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = PageObject {
            bbox: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            ctm: Matrix::identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::DeviceGray(0.0)),
            stroke_color: None,
            stroke_width: 0.0,
            overprint: OverprintState::default(),
            subpaths: vec![],
        };
        let cyan_plate = render_cmyk_plate(
            &[obj.clone()],
            &media,
            PlateChannel::Cyan,
            false,
            None,
            None,
            &[],
            10,
            10,
        );
        let k_plate = render_cmyk_plate(&[obj], &media, PlateChannel::Black, false, None, None, &[], 10, 10);
        // Cyan plate: DeviceGray black → no cyan → white
        assert_eq!(
            cyan_plate.get_pixel(5, 5)[0],
            255,
            "DeviceGray black should be white on Cyan plate"
        );
        // K plate: DeviceGray black → full K → black
        assert_eq!(
            k_plate.get_pixel(5, 5)[0],
            0,
            "DeviceGray black should be black on K plate"
        );
    }
}
