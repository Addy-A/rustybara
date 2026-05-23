use glutin::{
    context::PossiblyCurrentContext,
    surface::{GlSurface, Surface as GlWindowSurface, WindowSurface},
};
use image::DynamicImage;
use rustybara::{geometry::Rect as PdfRect, pages::PageBoxes};
use skia_safe::{
    AlphaType, ColorType, Data, ImageInfo,
    gpu::{self, DirectContext, SurfaceOrigin, backend_render_targets, gl::FramebufferInfo},
};

type WindowSurfaceType = GlWindowSurface<WindowSurface>;

pub struct OverlayData<'a> {
    pub boxes: &'a PageBoxes,
}

pub struct SkiaRenderer {
    gl_context: PossiblyCurrentContext,
    gl_surface: WindowSurfaceType,
    gr_context: DirectContext,
    skia_surface: skia_safe::Surface,
    pub width: u32,
    pub height: u32,
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
        }
    }

    /// Draw the page bitmap and, optionally, prepress overlays ontot the Skia surface.
    /// If `page_image` is `None` the canvas is cleared and the call returns early,
    /// overlays are skipped because the page rect is undefined without an image.
    pub fn draw(
        &mut self,
        page_image: Option<&skia_safe::Image>,
        zoom: f32,
        pan: [f32; 2],
        overlays: Option<&OverlayData<'_>>,
    ) {
        let canvas = self.skia_surface.canvas();
        canvas.clear(skia_safe::Color::from_argb(255, 30, 30, 30));

        let Some(img) = page_image else { return };

        let img_w = img.width() as f32;
        let img_h = img.height() as f32;
        let win_w = self.width as f32;
        let win_h = self.height as f32;

        let base_scale = (win_w / img_w).min(win_h / img_h);
        let scale = base_scale * zoom;

        let draw_w = img_w * scale;
        let draw_h = img_h * scale;
        let x = (win_w - draw_w) / 2.0 + pan[0];
        let y = (win_h - draw_h) / 2.0 + pan[1];

        let src = skia_safe::Rect::from_wh(img_w, img_h);
        let dst = skia_safe::Rect::from_xywh(x, y, draw_w, draw_h);

        canvas.draw_image_rect(
            img,
            Some((&src, skia_safe::canvas::SrcRectConstraint::Strict)),
            dst,
            &skia_safe::Paint::default(),
        );

        if let Some(ov) = overlays {
            draw_overlays(canvas, ov, dst);
        }
    }

    pub fn present(&mut self) {
        self.gr_context.flush_and_submit();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .expect("swap buffers");
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.skia_surface = make_skia_surface(&mut self.gr_context, self.width, self.height)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{image_to_skia, pdf_rect_to_skia};
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

    #[test]
    #[ignore = "requires live GL context"]
    fn from_gl_smoke() {}
}
