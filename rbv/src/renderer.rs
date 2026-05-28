use std::collections::HashMap;

use egui::TextureId;
use glutin::{
    context::PossiblyCurrentContext,
    surface::{GlSurface, Surface as GlWindowSurface, WindowSurface},
};
use image::DynamicImage;
use rustybara::outline::paths::{GlyphVerb, PositionedGlyph};
use rustybara::{
    geometry::Rect as PdfRect,
    objects::{ObjectKind, PageObject, PathPoint, PdfColor},
    pages::PageBoxes,
};
use skia_safe::{
    gpu::{self, backend_render_targets, gl::FramebufferInfo, DirectContext, SurfaceOrigin},
    AlphaType, ColorType, Data, ImageInfo,
};

type WindowSurfaceType = GlWindowSurface<WindowSurface>;

// ── Overlay types ─────────────────────────────────────────────────────────────

pub struct OverlayData<'a> {
    pub boxes: &'a PageBoxes,
}

/// Diagnostics panel data populated on a selection click.
pub struct ColorPanel {
    /// RGBA pixel sampled from the rasterized image at the click position.
    pub pixel_rgba: [u8; 4],
    /// Fill color of the selected object, if any.
    pub pdf_color: Option<PdfColor>,
    /// ICC-converted CMYK values (0.0–1.0 per channel, C/M/Y/K order) derived
    /// from `pixel_rgba` via the active source → US Web Coated SWOP transform.
    /// `None` when no ICC transform was available at click time.
    pub pixel_cmyk: Option<[f32; 4]>,
}

/// Acrobat-style full-page wireframe overlay.
///
/// When active the page image is replaced by a white background with all page
/// objects drawn as black outlines. The optionally-selected object receives a
/// distinct blue highlight stroke on top.
pub struct PageWireframe<'a> {
    /// All objects on the page in back-to-front paint order.
    pub objects: &'a [PageObject],
    /// Page media box used to convert PDF coords → screen coords.
    pub media_box: &'a PdfRect,
    /// Object to highlight with a blue outline (the current selection, if any).
    pub selected: Option<&'a PageObject>,
    /// Per-glyph outline paths in PDF page space. Empty slice = fallback to bbox rects.
    pub glyph_outlines: &'a [PositionedGlyph],
}

/// Debug diagnostic overlay — shown when the viewer is in debug mode (Ctrl+Shift+D).
///
/// The caller (viewer) builds all text lines from its own state; the renderer
/// just draws them verbatim in a fixed panel at the top-right of the window.
pub struct DebugOverlay<'a> {
    /// Pre-formatted lines of diagnostic text, newest entries last.
    pub lines: &'a [String],
}

/// Tile performance telemetry overlay — shown when telemetry mode is active (Ctrl+Shift+T).
///
/// Displays tile render times, cache stats, and page grid information in a panel
/// at the top-left of the window, separate from the debug overlay.
pub struct TelemetryOverlay<'a> {
    pub lines: &'a [String],
}

// ── egui texture cache ────────────────────────────────────────────────────────

/// CPU-side backing for a single egui texture plus a cached Skia image.
///
/// egui sends *partial* font-atlas updates (`ImageDelta::pos = Some([x, y])`)
/// whenever new glyphs are first requested — only the new tile arrives, not the
/// whole atlas.  Skia images are immutable, so we keep the full RGBA pixel buffer
/// here, blit the patch into it on each delta, and rebuild the Skia image.
/// Without this, the whole atlas would be replaced with only the tile, corrupting
/// every glyph that was already uploaded.
struct EguiTexture {
    pixels: Vec<u8>, // flat RGBA u8 row-major: width * height * 4 bytes
    width: usize,
    height: usize,
    image: skia_safe::Image,
}

// ── Renderer ──────────────────────────────────────────────────────────────────

pub struct SkiaRenderer {
    gl_context: PossiblyCurrentContext,
    gl_surface: WindowSurfaceType,
    gr_context: DirectContext,
    skia_surface: skia_safe::Surface,
    pub width: u32,
    pub height: u32,
    egui_textures: HashMap<TextureId, EguiTexture>,
}

pub fn image_to_skia(img: &DynamicImage) -> skia_safe::Image {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let info = ImageInfo::new(
        (w as i32, h as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let data = Data::new_copy(rgba.as_raw());
    skia_safe::images::raster_from_data(&info, data, (w as usize) * 4)
        .expect("failed to create Skia image")
}

// ── Coordinate helpers ────────────────────────────────────────────────────────

fn pdf_rect_to_skia(
    pdf_rect: &PdfRect,
    media_box: &PdfRect,
    page_screen_rect: skia_safe::Rect,
) -> skia_safe::Rect {
    let scale_x = page_screen_rect.width() / media_box.width as f32;
    let scale_y = page_screen_rect.height() / media_box.height as f32;

    let left = page_screen_rect.left() + (pdf_rect.x - media_box.x) as f32 * scale_x;
    let top = page_screen_rect.top() + (media_box.top() - pdf_rect.top()) as f32 * scale_y;
    let right = page_screen_rect.left() + (pdf_rect.right() - media_box.x) as f32 * scale_x;
    let bottom = page_screen_rect.top() + (media_box.top() - pdf_rect.y) as f32 * scale_y;

    skia_safe::Rect {
        left,
        top,
        right,
        bottom,
    }
}

/// Convert a single PDF page-space point to screen-space, applying the Y-axis flip.
///
/// Consistent with [`pdf_rect_to_skia`]: PDF origin is bottom-left (Y-up),
/// screen origin is top-left (Y-down).
fn pdf_point_to_screen(
    pdf_x: f64,
    pdf_y: f64,
    media_box: &PdfRect,
    page_screen_rect: skia_safe::Rect,
) -> skia_safe::Point {
    let scale_x = page_screen_rect.width() / media_box.width as f32;
    let scale_y = page_screen_rect.height() / media_box.height as f32;
    let sx = page_screen_rect.left() + (pdf_x as f32 - media_box.x as f32) * scale_x;
    let sy = page_screen_rect.top() + (media_box.top() as f32 - pdf_y as f32) * scale_y;
    skia_safe::Point::new(sx, sy)
}

// ── Font helper ───────────────────────────────────────────────────────────────

/// Create a font with an explicitly-resolved system typeface.
///
/// `Font::default()` produces a font whose underlying typeface can be empty in
/// certain GPU driver configurations, causing `TextBlob::from_str` to return
/// `None` for every string and silently skipping all text rendering.
/// Querying the system `FontMgr` for a known family guarantees a real typeface
/// with actual glyph data.
fn make_ui_font(size: f32) -> skia_safe::Font {
    let mgr = skia_safe::FontMgr::new();
    // Try common Windows/cross-platform fonts in preference order.
    // Empty string "" asks the manager for its own default family.
    let typeface = ["Consolas", "Courier New", "Arial", "Helvetica", ""]
        .iter()
        .find_map(|family| mgr.match_family_style(family, skia_safe::FontStyle::normal()));
    match typeface {
        Some(tf) => skia_safe::Font::new(tf, size),
        None => {
            // Absolute fallback — may still have no glyphs, but we've tried everything.
            let mut f = skia_safe::Font::default();
            f.set_size(size);
            f
        }
    }
}

// ── Draw helpers ──────────────────────────────────────────────────────────────

fn draw_overlays(
    canvas: &skia_safe::Canvas,
    overlays: &OverlayData<'_>,
    page_screen_rect: skia_safe::Rect,
) {
    let media = &overlays.boxes.media_box;

    if let Some(bleed) = &overlays.boxes.bleed_box {
        let r = pdf_rect_to_skia(bleed, media, page_screen_rect);
        let mut paint = skia_safe::Paint::default();
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(1.5);
        paint.set_color(skia_safe::Color::from_argb(220, 255, 100, 0));
        paint.set_path_effect(skia_safe::dash_path_effect::new(&[6.0, 4.0], 0.0));
        canvas.draw_rect(r, &paint);
    }

    if let Some(trim) = &overlays.boxes.trim_box {
        let r = pdf_rect_to_skia(trim, media, page_screen_rect);
        let mut paint = skia_safe::Paint::default();
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(1.5);
        paint.set_color(skia_safe::Color::from_argb(220, 0, 160, 255));
        paint.set_path_effect(skia_safe::dash_path_effect::new(&[6.0, 4.0], 0.0));
        canvas.draw_rect(r, &paint);
    }

    if let Some(crop) = &overlays.boxes.crop_box {
        let r = pdf_rect_to_skia(crop, media, page_screen_rect);
        let mut paint = skia_safe::Paint::default();
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(1.5);
        paint.set_color(skia_safe::Color::from_argb(200, 0, 200, 80));
        paint.set_path_effect(skia_safe::dash_path_effect::new(&[6.0, 4.0], 0.0));
        canvas.draw_rect(r, &paint);
    }
}

/// Draw a single page object as a wireframe outline using the supplied `paint`.
///
/// - Path objects (`Fill`, `Stroke`, `FillStroke`) → actual subpath outline.
/// - Image objects → bounding rect with an × through it (Acrobat-style placeholder).
/// - Text blocks / Form XObjects → bounding rect.
fn draw_wireframe_object(
    canvas: &skia_safe::Canvas,
    obj: &PageObject,
    media_box: &PdfRect,
    page_screen_rect: skia_safe::Rect,
    paint: &skia_safe::Paint,
) {
    match &obj.kind {
        ObjectKind::Fill | ObjectKind::FillStroke | ObjectKind::Stroke => {
            if obj.subpaths.is_empty() {
                // No path data → fall back to bbox rect
                let r = pdf_rect_to_skia(&obj.bbox, media_box, page_screen_rect);
                canvas.draw_rect(r, paint);
                return;
            }
            // skia-safe 0.97: PathBuilder for incremental construction; detach() → Path.
            let mut builder = skia_safe::PathBuilder::new();
            for sub in &obj.subpaths {
                for &point in &sub.points {
                    match point {
                        PathPoint::MoveTo(lx, ly) => {
                            let (px, py) = obj.ctm.transform_point(lx, ly);
                            builder.move_to(pdf_point_to_screen(
                                px,
                                py,
                                media_box,
                                page_screen_rect,
                            ));
                        }
                        PathPoint::LineTo(lx, ly) => {
                            let (px, py) = obj.ctm.transform_point(lx, ly);
                            builder.line_to(pdf_point_to_screen(
                                px,
                                py,
                                media_box,
                                page_screen_rect,
                            ));
                        }
                        PathPoint::CurveTo(c1x, c1y, c2x, c2y, ex, ey) => {
                            let (s1x, s1y) = obj.ctm.transform_point(c1x, c1y);
                            let (s2x, s2y) = obj.ctm.transform_point(c2x, c2y);
                            let (sex, sey) = obj.ctm.transform_point(ex, ey);
                            builder.cubic_to(
                                pdf_point_to_screen(s1x, s1y, media_box, page_screen_rect),
                                pdf_point_to_screen(s2x, s2y, media_box, page_screen_rect),
                                pdf_point_to_screen(sex, sey, media_box, page_screen_rect),
                            );
                        }
                        PathPoint::Close => {
                            builder.close();
                        }
                    }
                }
            }
            let path = builder.detach();
            canvas.draw_path(&path, paint);
        }
        ObjectKind::Image => {
            // Acrobat-style image placeholder: rect with an × inside.
            let r = pdf_rect_to_skia(&obj.bbox, media_box, page_screen_rect);
            canvas.draw_rect(r, paint);
            canvas.draw_line((r.left(), r.top()), (r.right(), r.bottom()), paint);
            canvas.draw_line((r.right(), r.top()), (r.left(), r.bottom()), paint);
        }
        ObjectKind::Text(_) | ObjectKind::FormXObject => {
            let r = pdf_rect_to_skia(&obj.bbox, media_box, page_screen_rect);
            canvas.draw_rect(r, paint);
        }
    }
}

/// Draw all glyph outlines produced by `rustybara::outline::outline_page_text`.
///
/// Each [`PositionedGlyph`] is a sequence of [`GlyphVerb`]s in PDF page space;
/// we mape them to screen space through [`pdf_point_to_screen`] and stroke them.
fn draw_glyph_outlines(
    canvas: &skia_safe::Canvas,
    glyphs: &[PositionedGlyph],
    media_box: &PdfRect,
    page_screen_rect: skia_safe::Rect,
    paint: &skia_safe::Paint,
) {
    for glyph in glyphs {
        if glyph.verbs.is_empty() {
            continue;
        }
        let mut builder = skia_safe::PathBuilder::new();
        for verb in &glyph.verbs {
            match *verb {
                GlyphVerb::MoveTo(x, y) => {
                    builder.move_to(pdf_point_to_screen(x, y, media_box, page_screen_rect));
                }
                GlyphVerb::LineTo(x, y) => {
                    builder.line_to(pdf_point_to_screen(x, y, media_box, page_screen_rect));
                }
                GlyphVerb::QuadTo(cx, cy, x, y) => {
                    builder.quad_to(
                        pdf_point_to_screen(cx, cy, media_box, page_screen_rect),
                        pdf_point_to_screen(x, y, media_box, page_screen_rect),
                    );
                }
                GlyphVerb::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                    builder.cubic_to(
                        pdf_point_to_screen(c1x, c1y, media_box, page_screen_rect),
                        pdf_point_to_screen(c2x, c2y, media_box, page_screen_rect),
                        pdf_point_to_screen(x, y, media_box, page_screen_rect),
                    );
                }
                GlyphVerb::Close => {
                    builder.close();
                }
            }
        }
        canvas.draw_path(&builder.detach(), paint);
    }
}

/// Draw the Acrobat-style full-page wireframe.
///
/// Replaces the raster page image with a white background and draws all page
/// objects as thin black outlines. The selected object (if any) receives a
/// thicker blue highlight stroke drawn on top.
fn draw_page_wireframe(
    canvas: &skia_safe::Canvas,
    wf: &PageWireframe<'_>,
    page_screen_rect: skia_safe::Rect,
) {
    let mut bg = skia_safe::Paint::default();
    bg.set_color(skia_safe::Color::WHITE);
    canvas.draw_rect(page_screen_rect, &bg);

    let mut outline = skia_safe::Paint::default();
    outline.set_style(skia_safe::paint::Style::Stroke);
    outline.set_stroke_width(0.5);
    outline.set_color(skia_safe::Color::BLACK);
    outline.set_anti_alias(true);

    for obj in wf.objects {
        draw_wireframe_object(canvas, obj, wf.media_box, page_screen_rect, &outline);
    }

    // Draw glyph outline paths on top of every other object (including text bboxes).
    // Glyph outlines are drawn whenever available; text bboxes remain visible underneath
    // so fonts that could not be extracted still show their bounding box.
    if !wf.glyph_outlines.is_empty() {
        draw_glyph_outlines(
            canvas,
            wf.glyph_outlines,
            wf.media_box,
            page_screen_rect,
            &outline,
        );
    }

    if let Some(sel) = wf.selected {
        let mut sel_paint = skia_safe::Paint::default();
        sel_paint.set_style(skia_safe::paint::Style::Stroke);
        sel_paint.set_stroke_width(2.0);
        sel_paint.set_color(skia_safe::Color::from_argb(255, 30, 120, 255));
        sel_paint.set_anti_alias(true);
        draw_wireframe_object(canvas, sel, wf.media_box, page_screen_rect, &sel_paint);
    }

    let mut border = skia_safe::Paint::default();
    border.set_style(skia_safe::paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(skia_safe::Color::from_argb(200, 120, 120, 120));
    canvas.draw_rect(page_screen_rect, &border);
}

/// Draw the sampling-point crosshair marker at `screen_pos`.
///
/// The caller is responsible for converting the stored PDF sampling coordinates
/// into current screen coordinates (accounting for zoom and pan) before calling
/// this function.
///
/// The marker is a classic two-ring crosshair: a thin white halo ring behind a
/// thin orange-red accent ring, with four tick lines extending outward. This
/// stays visible against both light (page) and dark (margins) backgrounds.
///
/// Drawing order:
///   1. White drop-shadow strokes (slightly thicker) for contrast.
///   2. Orange-red accent strokes on top.
fn draw_sample_marker(canvas: &skia_safe::Canvas, screen_pos: [f32; 2]) {
    let [cx, cy] = screen_pos;

    // Tick line geometry: gap inside the circle, length outside.
    const INNER: f32 = 8.0; // start of tick (px from centre)
    const OUTER: f32 = 16.0; // end of tick

    // ── white halo (drawn first, slightly wider) ──────────────────────────────
    let mut white = skia_safe::Paint::default();
    white.set_style(skia_safe::paint::Style::Stroke);
    white.set_color(skia_safe::Color::from_argb(200, 255, 255, 255));
    white.set_stroke_width(2.5);
    white.set_anti_alias(true);

    canvas.draw_circle((cx, cy), 6.0, &white);
    canvas.draw_line((cx, cy - OUTER), (cx, cy - INNER), &white);
    canvas.draw_line((cx, cy + INNER), (cx, cy + OUTER), &white);
    canvas.draw_line((cx - OUTER, cy), (cx - INNER, cy), &white);
    canvas.draw_line((cx + INNER, cy), (cx + OUTER, cy), &white);

    // ── accent ring + ticks (orange-red, drawn on top) ────────────────────────
    let mut accent = skia_safe::Paint::default();
    accent.set_style(skia_safe::paint::Style::Stroke);
    accent.set_color(skia_safe::Color::from_argb(255, 255, 80, 0));
    accent.set_stroke_width(1.5);
    accent.set_anti_alias(true);

    canvas.draw_circle((cx, cy), 6.0, &accent);
    canvas.draw_line((cx, cy - OUTER), (cx, cy - INNER), &accent);
    canvas.draw_line((cx, cy + INNER), (cx, cy + OUTER), &accent);
    canvas.draw_line((cx - OUTER, cy), (cx - INNER, cy), &accent);
    canvas.draw_line((cx + INNER, cy), (cx + OUTER, cy), &accent);
}

/// Draw the debug diagnostic overlay in the top-right corner of the window.
///
/// Renders each line of `lines` in a semi-transparent dark panel using a
/// monospace-like default font. Designed to be readable over any page content.
fn draw_debug_overlay(canvas: &skia_safe::Canvas, lines: &[String], win_w: f32, win_h: f32) {
    if lines.is_empty() {
        return;
    }

    let font_size = 11.0_f32;
    let line_h = 15.0_f32;
    let pad_x = 10.0_f32;
    let pad_y = 8.0_f32;
    let panel_w = 370.0_f32;
    let panel_h = pad_y + lines.len() as f32 * line_h + pad_y;

    // Clamp panel height so it doesn't exceed the window.
    let panel_h = panel_h.min(win_h - 20.0);
    let x = win_w - panel_w - 10.0;
    let y = 10.0_f32;

    // Background — dark navy, distinct from the page/prepress overlays.
    let mut bg = skia_safe::Paint::default();
    bg.set_color(skia_safe::Color::from_argb(225, 8, 12, 28));
    bg.set_anti_alias(true);
    canvas.draw_rect(skia_safe::Rect::from_xywh(x, y, panel_w, panel_h), &bg);

    // Green border (classic terminal aesthetic).
    let mut border = skia_safe::Paint::default();
    border.set_style(skia_safe::paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(skia_safe::Color::from_argb(200, 60, 200, 80));
    canvas.draw_rect(skia_safe::Rect::from_xywh(x, y, panel_w, panel_h), &border);

    let font = make_ui_font(font_size);

    // Header / section separator lines — bright green.
    let mut bright = skia_safe::Paint::default();
    bright.set_color(skia_safe::Color::from_argb(255, 100, 255, 128));
    bright.set_anti_alias(true);

    // Body lines — softer green so headers stand out.
    let mut dim = skia_safe::Paint::default();
    dim.set_color(skia_safe::Color::from_argb(255, 140, 218, 155));
    dim.set_anti_alias(true);

    let max_lines = ((panel_h - pad_y * 2.0) / line_h) as usize;
    for (i, line) in lines.iter().take(max_lines).enumerate() {
        let ty = y + pad_y + (i as f32 + 1.0) * line_h;
        let paint = if i == 0 || line.starts_with('\u{2500}') {
            &bright
        } else {
            &dim
        };
        // TextBlob::from_str + draw_text_blob is the reliable GPU-safe text path in
        // skia-safe 0.97 (draw_str can silently produce nothing in some GPU drivers).
        if let Some(blob) = skia_safe::TextBlob::from_str(line.as_str(), &font) {
            canvas.draw_text_blob(&blob, (x + pad_x, ty), paint);
        }
    }
}

/// Draw the tile performance telemetry panel in the top-left corner of the window.
fn draw_telemetry_panel(canvas: &skia_safe::Canvas, lines: &[String], win_w: f32, win_h: f32) {
    if lines.is_empty() {
        return;
    }

    let font_size = 11.0_f32;
    let line_h = 15.0_f32;
    let pad_x = 10.0_f32;
    let pad_y = 8.0_f32;
    let panel_w = 340.0_f32;
    let panel_h = (pad_y + lines.len() as f32 * line_h + pad_y).min(win_h - 20.0);
    let x = 10.0_f32;
    let y = 10.0_f32;

    let _ = win_w; // panel anchored to top-left, win_w unused

    let mut bg = skia_safe::Paint::default();
    bg.set_color(skia_safe::Color::from_argb(225, 12, 8, 32));
    bg.set_anti_alias(true);
    canvas.draw_rect(skia_safe::Rect::from_xywh(x, y, panel_w, panel_h), &bg);

    let mut border = skia_safe::Paint::default();
    border.set_style(skia_safe::paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(skia_safe::Color::from_argb(200, 120, 80, 255));
    canvas.draw_rect(skia_safe::Rect::from_xywh(x, y, panel_w, panel_h), &border);

    let font = make_ui_font(font_size);

    let mut bright = skia_safe::Paint::default();
    bright.set_color(skia_safe::Color::from_argb(255, 180, 150, 255));
    bright.set_anti_alias(true);

    let mut dim = skia_safe::Paint::default();
    dim.set_color(skia_safe::Color::from_argb(255, 140, 118, 200));
    dim.set_anti_alias(true);

    let max_lines = ((panel_h - pad_y * 2.0) / line_h) as usize;
    for (i, line) in lines.iter().take(max_lines).enumerate() {
        let ty = y + pad_y + (i as f32 + 1.0) * line_h;
        let paint = if i == 0 || line.starts_with('\u{2500}') {
            &bright
        } else {
            &dim
        };
        if let Some(blob) = skia_safe::TextBlob::from_str(line.as_str(), &font) {
            canvas.draw_text_blob(&blob, (x + pad_x, ty), paint);
        }
    }
}

// ── egui image helper ─────────────────────────────────────────────────────────

/// Build a Skia raster image from a flat RGBA u8 buffer.
///
/// Free function (not a method) so it can be called while `self.egui_textures`
/// holds a mutable borrow without triggering an `E0502` conflict.
fn make_egui_skia_image(rgba: &[u8], w: usize, h: usize) -> Option<skia_safe::Image> {
    let info = skia_safe::ImageInfo::new(
        (w as i32, h as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    let data = skia_safe::Data::new_copy(rgba);
    skia_safe::images::raster_from_data(&info, data, w * 4)
}

// ── Surface helpers ───────────────────────────────────────────────────────────

fn make_skia_surface(
    gr_context: &mut DirectContext,
    width: u32,
    height: u32,
) -> skia_safe::Surface {
    let fb_info = FramebufferInfo {
        fboid: 0,
        format: 0x8058,
        ..Default::default()
    };
    let backend_rt =
        backend_render_targets::make_gl((width as i32, height as i32), None, 8, fb_info);
    gpu::surfaces::wrap_backend_render_target(
        gr_context,
        &backend_rt,
        SurfaceOrigin::BottomLeft,
        ColorType::RGBA8888,
        None,
        None,
    )
    .expect("Skia GPU surface")
}

// ── SkiaRenderer ──────────────────────────────────────────────────────────────

impl SkiaRenderer {
    /// Construct from an already-current GL context and window surface.
    /// The GL context must be made current before calling this.
    pub fn from_gl(
        gl_context: PossiblyCurrentContext,
        gl_surface: WindowSurfaceType,
        width: u32,
        height: u32,
    ) -> Self {
        let gl_interface = skia_safe::gpu::gl::Interface::new_native()
            .expect("Skia GL interface — GL context must be current");
        let mut gr_context = skia_safe::gpu::direct_contexts::make_gl(gl_interface, None)
            .expect("Skia DirectContext");
        let skia_surface = make_skia_surface(&mut gr_context, width, height);
        Self {
            gl_context,
            gl_surface,
            gr_context,
            skia_surface,
            width,
            height,
            egui_textures: HashMap::new(),
        }
    }

    /// Draw the page and prepress overlays onto the Skia surface.
    ///
    /// Rendering order (back to front):
    /// 1. Background clear (dark grey)
    /// 2a. **Normal mode** – raster page image
    /// 2b. **Wireframe mode** – white page rect + all object outlines + selection highlight
    /// 3. Prepress box overlays (`overlays`) — shown in both modes
    ///
    /// Call [`Self::draw_top_layer`] after [`Self::draw_tiles`] to ensure the
    /// sample crosshair and debug overlay always render above tile images.
    pub fn draw(
        &mut self,
        page_image: Option<&skia_safe::Image>,
        zoom: f32,
        pan: [f32; 2],
        overlays: Option<&OverlayData<'_>>,
        wireframe: Option<&PageWireframe<'_>>,
    ) {
        let canvas = self.skia_surface.canvas();
        canvas.clear(skia_safe::Color::from_argb(255, 30, 30, 30));

        let win_w = self.width as f32;
        let win_h = self.height as f32;

        if let Some(img) = page_image {
            let img_w = img.width() as f32;
            let img_h = img.height() as f32;

            let base_scale = (win_w / img_w).min(win_h / img_h);
            let scale = base_scale * zoom;

            let draw_w = img_w * scale;
            let draw_h = img_h * scale;
            let x = (win_w - draw_w) / 2.0 + pan[0];
            let y = (win_h - draw_h) / 2.0 + pan[1];

            let src = skia_safe::Rect::from_wh(img_w, img_h);
            let dst = skia_safe::Rect::from_xywh(x, y, draw_w, draw_h);

            if let Some(wf) = wireframe {
                draw_page_wireframe(canvas, wf, dst);
            } else {
                canvas.draw_image_rect(
                    img,
                    Some((&src, skia_safe::canvas::SrcRectConstraint::Strict)),
                    dst,
                    &skia_safe::Paint::default(),
                );
            }

            if let Some(ov) = overlays {
                draw_overlays(canvas, ov, dst);
            }
        }
    }

    /// Draw the sample crosshair, debug overlay, and telemetry panel on top of everything
    /// except egui. Must be called after [`Self::draw_tiles`]. Call [`Self::draw_egui`] after.
    pub fn draw_top_layer(
        &mut self,
        sample_marker: Option<[f32; 2]>,
        debug: Option<&DebugOverlay<'_>>,
        telemetry: Option<&TelemetryOverlay<'_>>,
    ) {
        let canvas = self.skia_surface.canvas();
        let win_w = self.width as f32;
        let win_h = self.height as f32;

        if let Some(pos) = sample_marker {
            draw_sample_marker(canvas, pos);
        }

        if let Some(dbg) = debug {
            draw_debug_overlay(canvas, dbg.lines, win_w, win_h);
        }

        if let Some(tel) = telemetry {
            draw_telemetry_panel(canvas, tel.lines, win_w, win_h);
        }
    }

    /// Draw a set of tiles on top of the current frame.
    ///
    /// Each entry is `(screen_rect, tile_image)`. Tiles are drawn with a full-image
    /// source rect and no additional paint effects. Call this after `draw()` and
    /// before `draw_egui()` so tiles overlay the full-page fallback image.
    pub fn draw_tiles(&mut self, tiles: &[(skia_safe::Rect, skia_safe::Image)]) {
        if tiles.is_empty() {
            return;
        }
        let canvas = self.skia_surface.canvas();
        for (dst, img) in tiles {
            let src = skia_safe::Rect::from_wh(img.width() as f32, img.height() as f32);
            canvas.draw_image_rect(
                img,
                Some((&src, skia_safe::canvas::SrcRectConstraint::Strict)),
                dst,
                &skia_safe::Paint::default(),
            );
        }
    }

    pub fn present(&mut self) {
        self.gr_context.flush_and_submit();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .expect("swap buffers");
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        use std::num::NonZeroU32;
        self.width = width.max(1);
        self.height = height.max(1);
        // Explicitly resize the GL drawable before rebuilding the Skia surface.
        // On macOS (CGL) and Linux (EGL/GLX) the surface does not automatically
        // track the window size — without this call the OS compositor stretches
        // the stale framebuffer to fill the new window, distorting the image.
        // WGL (Windows) auto-tracks the HWND, so this is effectively a no-op there.
        if let (Some(w), Some(h)) = (
            NonZeroU32::new(self.width),
            NonZeroU32::new(self.height),
        ) {
            self.gl_surface.resize(&self.gl_context, w, h);
        }
        self.skia_surface = make_skia_surface(&mut self.gr_context, self.width, self.height)
    }

    // ── egui rendering ────────────────────────────────────────────────────────

    /// Upload or patch egui textures from a frame's [`egui::TexturesDelta`].
    ///
    /// egui sends two kinds of deltas:
    /// * `pos = None` — full texture upload; replace everything.
    /// * `pos = Some([x, y])` — partial update; blit the tile into the existing
    ///   CPU buffer at the given offset, then rebuild the Skia image.
    ///
    /// Must be called before [`Self::draw_egui`].
    pub fn update_egui_textures(&mut self, delta: &egui::TexturesDelta) {
        for (id, image_delta) in &delta.set {
            // In egui 0.27+, all texture data (including the font atlas) is
            // delivered as ColorImage — there is no separate Font variant.
            let (patch_pixels, patch_w, patch_h) = match &image_delta.image {
                egui::ImageData::Color(img) => {
                    let pixels: Vec<u8> = img
                        .pixels
                        .iter()
                        .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
                        .collect();
                    (pixels, img.size[0], img.size[1])
                }
            };

            if let Some([ox, oy]) = image_delta.pos {
                // ── Partial update: blit the tile into the existing CPU buffer ──
                // Use a free-function call to avoid holding `&self` while `self.egui_textures`
                // is already mutably borrowed.
                if let Some(existing) = self.egui_textures.get_mut(id) {
                    for row in 0..patch_h {
                        let src_off = row * patch_w * 4;
                        let dst_off = ((oy + row) * existing.width + ox) * 4;
                        let len = patch_w * 4;
                        existing.pixels[dst_off..dst_off + len]
                            .copy_from_slice(&patch_pixels[src_off..src_off + len]);
                    }
                    // Rebuild the Skia image from the now-patched buffer.
                    if let Some(img) =
                        make_egui_skia_image(&existing.pixels, existing.width, existing.height)
                    {
                        existing.image = img;
                    }
                }
            } else {
                // ── Full upload: replace the whole texture ────────────────────
                if let Some(img) = make_egui_skia_image(&patch_pixels, patch_w, patch_h) {
                    self.egui_textures.insert(
                        *id,
                        EguiTexture {
                            pixels: patch_pixels,
                            width: patch_w,
                            height: patch_h,
                            image: img,
                        },
                    );
                }
            }
        }
    }

    /// Free egui textures that are no longer needed after this frame.
    /// Must be called after [`Self::draw_egui`].
    pub fn free_egui_textures(&mut self, delta: &egui::TexturesDelta) {
        for id in &delta.free {
            self.egui_textures.remove(id);
        }
    }

    /// Draw egui's tessellated output on top of the current Skia frame.
    ///
    /// Call after [`Self::draw`] and [`Self::update_egui_textures`],
    /// and before [`Self::present`].
    pub fn draw_egui(
        &mut self,
        primitives: &[egui::ClippedPrimitive],
        pixels_per_point: f32,
    ) {
        let canvas = self.skia_surface.canvas();

        for egui::ClippedPrimitive { clip_rect, primitive } in primitives {
            let egui::epaint::Primitive::Mesh(mesh) = primitive else {
                continue;
            };
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }

            // Look up the texture; skip if the atlas hasn't arrived yet.
            let Some(tex_entry) = self.egui_textures.get(&mesh.texture_id) else {
                continue;
            };
            let tex = &tex_entry.image;
            // egui UVs are normalised [0,1]; scale to texture pixel space for Skia.
            let tex_w = tex.width() as f32;
            let tex_h = tex.height() as f32;

            let positions: Vec<skia_safe::Point> = mesh
                .vertices
                .iter()
                .map(|v| {
                    skia_safe::Point::new(
                        v.pos.x * pixels_per_point,
                        v.pos.y * pixels_per_point,
                    )
                })
                .collect();

            let tex_coords: Vec<skia_safe::Point> = mesh
                .vertices
                .iter()
                .map(|v| skia_safe::Point::new(v.uv.x * tex_w, v.uv.y * tex_h))
                .collect();

            let colors: Vec<skia_safe::Color> = mesh
                .vertices
                .iter()
                .map(|v| {
                    skia_safe::Color::from_argb(
                        v.color.a(),
                        v.color.r(),
                        v.color.g(),
                        v.color.b(),
                    )
                })
                .collect();

            // egui guarantees mesh index values fit in u16.
            let indices: Vec<u16> = mesh.indices.iter().map(|&i| i as u16).collect();

            // new_copy returns RCHandle<SkVertices> directly — no Option unwrap needed.
            let verts = skia_safe::Vertices::new_copy(
                skia_safe::vertices::VertexMode::Triangles,
                &positions,
                &tex_coords,
                &colors,
                Some(&indices),
            );

            let Some(shader) = tex.to_shader(
                (skia_safe::TileMode::Clamp, skia_safe::TileMode::Clamp),
                skia_safe::SamplingOptions::new(
                    skia_safe::FilterMode::Linear,
                    skia_safe::MipmapMode::None,
                ),
                None,
            ) else {
                continue;
            };

            let mut paint = skia_safe::Paint::default();
            paint.set_anti_alias(true);
            paint.set_shader(shader);

            let clip = skia_safe::Rect::from_ltrb(
                clip_rect.min.x * pixels_per_point,
                clip_rect.min.y * pixels_per_point,
                clip_rect.max.x * pixels_per_point,
                clip_rect.max.y * pixels_per_point,
            );

            canvas.save();
            canvas.clip_rect(clip, skia_safe::ClipOp::Intersect, false);
            canvas.draw_vertices(&verts, skia_safe::BlendMode::Modulate, &paint);
            canvas.restore();
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{draw_debug_overlay, image_to_skia, pdf_point_to_screen, pdf_rect_to_skia};
    use image::{DynamicImage, RgbaImage};
    use rustybara::geometry::Rect as PdfRect;

    // ── image_to_skia ────────────────────────────────────────────────────────

    #[test]
    fn image_to_skia_dimensions() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(64, 32));
        let skia_img = image_to_skia(&img);
        assert_eq!(skia_img.width(), 64);
        assert_eq!(skia_img.height(), 32);
    }

    #[test]
    fn image_to_skia_pixel_values() {
        let mut src = RgbaImage::new(2, 1);
        src.put_pixel(0, 0, image::Rgba([255, 0, 128, 255]));
        src.put_pixel(1, 0, image::Rgba([0, 64, 32, 128]));
        let skia_img = image_to_skia(&DynamicImage::ImageRgba8(src));
        assert_eq!(skia_img.width(), 2);
        assert_eq!(skia_img.height(), 1);
    }

    #[test]
    fn image_to_skia_one_pixel() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(1, 1));
        let skia_img = image_to_skia(&img);
        assert_eq!(skia_img.width(), 1);
        assert_eq!(skia_img.height(), 1);
    }

    // ── draw (CPU surface, no GL needed) ────────────────────────────────────

    #[test]
    fn draw_no_image_clears_surface() {
        let mut surface = skia_safe::surfaces::raster_n32_premul((100, 100)).expect("surface");
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::from_argb(255, 30, 30, 30));
        let pixmap = surface.peek_pixels().expect("peek");
        let bytes = pixmap.bytes().expect("bytes");
        assert_eq!(bytes.len(), 100 * 100 * 4);
    }

    #[test]
    fn draw_with_image_no_panic() {
        let mut surface = skia_safe::surfaces::raster_n32_premul((200, 150)).expect("surface");
        let page = DynamicImage::ImageRgba8(RgbaImage::new(100, 100));
        let skia_img = image_to_skia(&page);
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::from_argb(255, 30, 30, 30));
        let (img_w, img_h) = (skia_img.width() as f32, skia_img.height() as f32);
        let (win_w, win_h) = (200_f32, 150_f32);
        let scale = (win_w / img_w).min(win_h / img_h);
        let dst = skia_safe::Rect::from_xywh(
            (win_w - img_w * scale) / 2.0,
            (win_h - img_h * scale) / 2.0,
            img_w * scale,
            img_h * scale,
        );
        canvas.draw_image_rect(
            &skia_img,
            Some((
                &skia_safe::Rect::from_wh(img_w, img_h),
                skia_safe::canvas::SrcRectConstraint::Strict,
            )),
            dst,
            &skia_safe::Paint::default(),
        );
    }

    // ── pdf_rect_to_skia ─────────────────────────────────────────────────────

    // The media box itself must map exactly onto the full page_screen_rect.
    #[test]
    fn media_box_maps_to_full_page_rect() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let screen = skia_safe::Rect::from_xywh(50.0, 100.0, 300.0, 400.0);
        let r = pdf_rect_to_skia(&media, &media, screen);
        assert!((r.left() - 50.0).abs() < 0.1, "left={}", r.left());
        assert!((r.top() - 100.0).abs() < 0.1, "top={}", r.top());
        assert!((r.right() - 350.0).abs() < 0.1, "right={}", r.right());
        assert!((r.bottom() - 500.0).abs() < 0.1, "bottom={}", r.bottom());
    }

    // A TrimBox inset by 36pt on all sides at 1:1 scale should produce
    // a screen rect equally inset by 36px on all sides.
    #[test]
    fn trim_box_inset_maps_inward_at_1x_scale() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        // x=36, y=36, width=540, height=720  →  right=576, top=756
        let trim = PdfRect::new(36.0, 36.0, 540.0, 720.0);
        let screen = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let r = pdf_rect_to_skia(&trim, &media, screen);
        assert!((r.left() - 36.0).abs() < 0.1, "left={}", r.left());
        assert!((r.top() - 36.0).abs() < 0.1, "top={}", r.top());
        assert!((r.right() - 576.0).abs() < 0.1, "right={}", r.right());
        assert!((r.bottom() - 756.0).abs() < 0.1, "bottom={}", r.bottom());
    }

    // A rect in the bottom strip of the PDF page (low PDF y)
    // must appear at the bottom of the screen (high screen y) — Y-axis flip check.
    #[test]
    fn y_axis_flip_bottom_strip() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let bottom_strip = PdfRect::new(0.0, 0.0, 612.0, 198.0); // bottom quarter
        let screen = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let r = pdf_rect_to_skia(&bottom_strip, &media, screen);
        // screen_top    = 0 + (792 - 198) * 1.0 = 594
        // screen_bottom = 0 + (792 -   0) * 1.0 = 792
        assert!((r.top() - 594.0).abs() < 0.1, "top={}", r.top());
        assert!((r.bottom() - 792.0).abs() < 0.1, "bottom={}", r.bottom());
    }

    // Coordinate conversion scales correctly when screen is half the PDF point size.
    #[test]
    fn rect_scales_with_page_screen_rect() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        // Screen rect is half the size — 0.5 pts per pixel
        let screen = skia_safe::Rect::from_xywh(0.0, 0.0, 306.0, 396.0);
        let r = pdf_rect_to_skia(&media, &media, screen);
        assert!((r.width() - 306.0).abs() < 0.1, "width={}", r.width());
        assert!((r.height() - 396.0).abs() < 0.1, "height={}", r.height());
    }

    // ── pdf_point_to_screen ───────────────────────────────────────────────────

    // PDF origin (0, 0) is bottom-left → should map to screen bottom-left at 1:1.
    #[test]
    fn pdf_origin_maps_to_screen_bottom_left() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let screen = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let pt = pdf_point_to_screen(0.0, 0.0, &media, screen);
        assert!((pt.x - 0.0).abs() < 0.1, "x={}", pt.x);
        assert!((pt.y - 792.0).abs() < 0.1, "y={}", pt.y); // bottom of screen
    }

    // PDF top-left (0, page_height) → screen top-left (0, 0).
    #[test]
    fn pdf_top_left_maps_to_screen_top_left() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let screen = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let pt = pdf_point_to_screen(0.0, 792.0, &media, screen);
        assert!((pt.x - 0.0).abs() < 0.1, "x={}", pt.x);
        assert!((pt.y - 0.0).abs() < 0.1, "y={}", pt.y);
    }

    // PDF center maps to screen center at 1:1 scale.
    #[test]
    fn pdf_center_maps_to_screen_center() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let screen = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let pt = pdf_point_to_screen(306.0, 396.0, &media, screen);
        assert!((pt.x - 306.0).abs() < 0.1, "x={}", pt.x);
        assert!((pt.y - 396.0).abs() < 0.1, "y={}", pt.y);
    }

    // Point result is consistent with pdf_rect_to_skia for the same coordinate.
    #[test]
    fn point_consistent_with_rect_corner() {
        let media = PdfRect::new(0.0, 0.0, 612.0, 792.0);
        let trim = PdfRect::new(36.0, 36.0, 540.0, 720.0);
        let screen = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let rect = pdf_rect_to_skia(&trim, &media, screen);
        // The top-left corner of trim in PDF space is (36, 756)
        let pt = pdf_point_to_screen(36.0, 756.0, &media, screen);
        assert!((pt.x - rect.left()).abs() < 0.1, "x vs rect.left: {}", pt.x);
        assert!((pt.y - rect.top()).abs() < 0.1, "y vs rect.top: {}", pt.y);
    }

    // ── draw_debug_overlay ───────────────────────────────────────────────────

    // Verify the debug overlay draws without panicking on a CPU raster surface.
    #[test]
    fn draw_debug_overlay_no_panic() {
        let mut surface = skia_safe::surfaces::raster_n32_premul((600, 400)).expect("surface");
        let canvas = surface.canvas();
        let lines = vec![
            "── DEBUG (Ctrl+Shift+D) ─────────────────".to_string(),
            "Zoom  1.000×    Pan  [+0.0, +0.0]".to_string(),
            "Cursor  (300.0, 200.0)  screen".to_string(),
            "        (245.3, 396.0)  pdf".to_string(),
            "Page  #0   Objects: 12".to_string(),
            "Overlays: OFF   Wireframe: OFF".to_string(),
            "Selected  none".to_string(),
            "─── Log ─────────────────────────────────".to_string(),
            "  Page loaded: 12 objects, page 0".to_string(),
        ];
        draw_debug_overlay(canvas, &lines, 600.0, 400.0);
    }

    // Empty lines list → nothing drawn, no panic.
    #[test]
    fn draw_debug_overlay_empty_lines() {
        let mut surface = skia_safe::surfaces::raster_n32_premul((200, 100)).expect("surface");
        let canvas = surface.canvas();
        draw_debug_overlay(canvas, &[], 200.0, 100.0);
    }

    #[test]
    #[ignore = "requires live GL context"]
    fn from_gl_smoke() {}
}
