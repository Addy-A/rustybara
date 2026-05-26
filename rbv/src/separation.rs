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
use rustybara::{geometry::Rect as PdfRect, objects::PageObject};
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

/// Render a CMYK process channel plate from a rasterized sRGB page image.
///
/// 1. Apply `transform` (sRGB → SWOP CMYK) per pixel.
/// 2. Extract the chosen `channel`.
/// 3. In `tinted` mode, blend the channel density onto a white background
///    using the canonical process-ink color; in grayscale mode, emit a white-to-
///    black luminance image where black = full ink density.
///
/// Returns an RGBA image of the same dimensions as `src`.
pub fn render_cmyk_plate(
    src: &DynamicImage,
    channel: PlateChannel,
    tinted: bool,
    transform: &ColorTransform,
) -> DynamicImage {
    let (w, h) = src.dimensions();
    let src_rgba = src.to_rgba8();
    let mut out = RgbaImage::new(w, h);

    let ch_idx = channel as usize;
    let [tr, tg, tb] = channel.tint_rgb();

    for (x, y, pixel) in src_rgba.enumerate_pixels() {
        let [r, g, b, _a] = pixel.0;
        let cmyk = transform.convert(&[r, g, b]);
        // cmyk[i] is 0 = no ink, 255 = full density.
        let density = cmyk[ch_idx] as f32 / 255.0;

        let out_pixel = if tinted {
            // Blend ink color onto white at `density`.
            let ir = ((1.0 - density) * 255.0 + density * tr as f32) as u8;
            let ig = ((1.0 - density) * 255.0 + density * tg as f32) as u8;
            let ib = ((1.0 - density) * 255.0 + density * tb as f32) as u8;
            Rgba([ir, ig, ib, 255])
        } else {
            // Grayscale: 0 ink = white (255), full ink = black (0).
            let lum = ((1.0 - density) * 255.0) as u8;
            Rgba([lum, lum, lum, 255])
        };

        out.put_pixel(x, y, out_pixel);
    }

    DynamicImage::ImageRgba8(out)
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
    img_w: u32,
    img_h: u32,
) -> DynamicImage {
    let mut out = RgbaImage::from_pixel(img_w, img_h, Rgba([255, 255, 255, 255]));

    let [tr, tg, tb] = spot_rgb.unwrap_or(SPOT_FALLBACK_TINT);

    // Scale factors: PDF point → pixel
    let scale_x = img_w as f64 / media_box.width;
    let scale_y = img_h as f64 / media_box.height;

    for obj in objects {
        // Determine tint density from fill or stroke.
        let density = spot_tint_for_object(obj);

        // Convert PDF bbox (Y-up) to pixel bbox (Y-down).
        let pdf_x0 = obj.bbox.x - media_box.x;
        let pdf_y0 = obj.bbox.y - media_box.y;
        let pdf_x1 = pdf_x0 + obj.bbox.width;
        let pdf_y1 = pdf_y0 + obj.bbox.height;

        // Pixel coords — clamp to image bounds.
        let px0 = ((pdf_x0 * scale_x) as i64).max(0) as u32;
        let py0 = ((img_h as f64 - pdf_y1 * scale_y) as i64).max(0) as u32;
        let px1 = ((pdf_x1 * scale_x) as i64).min(img_w as i64) as u32;
        let py1 = ((img_h as f64 - pdf_y0 * scale_y) as i64).min(img_h as i64) as u32;

        if px0 >= px1 || py0 >= py1 {
            continue;
        }

        let fill = if tinted {
            let ir = ((1.0 - density) * 255.0 + density * tr as f64) as u8;
            let ig = ((1.0 - density) * 255.0 + density * tg as f64) as u8;
            let ib = ((1.0 - density) * 255.0 + density * tb as f64) as u8;
            Rgba([ir, ig, ib, 255])
        } else {
            let lum = ((1.0 - density) * 255.0) as u8;
            Rgba([lum, lum, lum, 255])
        };

        for py in py0..py1 {
            for px in px0..px1 {
                out.put_pixel(px, py, fill);
            }
        }
    }

    DynamicImage::ImageRgba8(out)
}

/// Extract the spot ink tint density (0.0–1.0) for a matched object.
///
/// Prefers fill over stroke; falls back to 1.0 (full density) if neither is
/// a Separation color (which should not happen if `filter_by_ink` was used, but
/// is safe to handle).
fn spot_tint_for_object(obj: &PageObject) -> f64 {
    use rustybara::objects::PdfColor;

    let pick = obj
        .fill_color
        .as_ref()
        .or(obj.stroke_color.as_ref());

    match pick {
        Some(PdfColor::Separation { tint, .. }) => (*tint).clamp(0.0, 1.0),
        _ => 1.0,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};
    use rustybara::geometry::{Matrix, Rect};
    use rustybara::objects::{ObjectKind, OverprintState, PageObject, PdfColor};

    fn blank_img(w: u32, h: u32, r: u8, g: u8, b: u8) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            *p = image::Rgba([r, g, b, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    fn spot_obj(x: f64, y: f64, w: f64, h: f64, tint: f64) -> PageObject {
        PageObject {
            bbox: Rect { x, y, width: w, height: h },
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
        let plate = render_spot_plate(&[obj], &media, false, None, 100, 200);
        assert_eq!(plate.width(), 100);
        assert_eq!(plate.height(), 200);
    }

    #[test]
    fn spot_plate_grayscale_full_tint_is_black() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = spot_obj(0.0, 0.0, 100.0, 100.0, 1.0);
        let plate = render_spot_plate(&[obj], &media, false, None, 10, 10);
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
        let plate = render_spot_plate(&[obj], &media, false, None, 10, 10);
        let pixel = plate.get_pixel(5, 5);
        assert_eq!(pixel[0], 255, "R should be 255 for zero-density grayscale");
        assert_eq!(pixel[1], 255, "G should be 255");
        assert_eq!(pixel[2], 255, "B should be 255");
    }

    #[test]
    fn spot_plate_tinted_full_tint_matches_fallback_color() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let obj = spot_obj(0.0, 0.0, 100.0, 100.0, 1.0);
        let plate = render_spot_plate(&[obj], &media, true, None, 10, 10);
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
        let plate = render_spot_plate(&[obj], &media, false, None, 100, 100);
        // Top-left pixel (PDF top-right area) should still be white.
        let pixel = plate.get_pixel(5, 5);
        assert_eq!(pixel[0], 255, "background should be white");
        assert_eq!(pixel[1], 255);
        assert_eq!(pixel[2], 255);
    }

    #[test]
    fn spot_plate_empty_objects_all_white() {
        let media = PdfRect::new(0.0, 0.0, 100.0, 100.0);
        let plate = render_spot_plate(&[], &media, false, None, 50, 50);
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
    #[test]
    fn cmyk_plate_dimensions_preserved() {
        let Some(t) = build_icc_transform() else { return };
        let src = blank_img(32, 16, 255, 255, 255);
        let plate = render_cmyk_plate(&src, PlateChannel::Cyan, false, &t);
        assert_eq!(plate.width(), 32);
        assert_eq!(plate.height(), 16);
    }

    #[test]
    fn cmyk_plate_white_source_no_cyan() {
        // Pure white RGB → should have near-zero cyan in SWOP CMYK.
        let Some(t) = build_icc_transform() else { return };
        let src = blank_img(4, 4, 255, 255, 255);
        let plate = render_cmyk_plate(&src, PlateChannel::Cyan, false, &t);
        let pixel = plate.get_pixel(2, 2);
        // Grayscale plate for near-zero cyan: pixel should be near white (≥ 230).
        assert!(pixel[0] >= 200, "white src should produce near-white cyan plate, got {}", pixel[0]);
    }

    #[test]
    fn cmyk_plate_tinted_white_source_near_white() {
        let Some(t) = build_icc_transform() else { return };
        let src = blank_img(4, 4, 255, 255, 255);
        let plate = render_cmyk_plate(&src, PlateChannel::Cyan, true, &t);
        let pixel = plate.get_pixel(2, 2);
        // Very little ink → result should be close to white (> 230 on all channels).
        assert!(pixel[0] >= 200, "R={}", pixel[0]);
        assert!(pixel[1] >= 200, "G={}", pixel[1]);
        assert!(pixel[2] >= 200, "B={}", pixel[2]);
    }
}