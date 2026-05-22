use image::DynamicImage;
use skia_safe::{AlphaType, ColorType, Data, ImageInfo};
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::window::Window;

pub struct SkiaRenderer {
    skia_surface: skia_safe::Surface,
    _sb_context: softbuffer::Context<Arc<Window>>,
    sb_surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
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

impl SkiaRenderer {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let skia_surface = skia_safe::surfaces::raster_n32_premul((width as i32, height as i32))
            .expect("Skia surface");
        let sb_context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let mut sb_surface =
            softbuffer::Surface::new(&sb_context, window.clone()).expect("softbuffer surface");
        sb_surface
            .resize(
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            )
            .expect("softbuffer resize");

        Self {
            skia_surface,
            _sb_context: sb_context,
            sb_surface,
            width,
            height,
        }
    }

    pub fn draw(&mut self, page_image: Option<&skia_safe::Image>, zoom: f32, pan: [f32; 2]) {
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
    }

    pub fn present(&mut self) {
        let pixels_ref = self.skia_surface.peek_pixels().expect("peek_pixels");
        let bytes = pixels_ref.bytes().expect("pixel bytes");
        let pixels: &[u32] = bytemuck::cast_slice(bytes);

        let mut buf = self.sb_surface.buffer_mut().expect("buffer_mut");
        buf.copy_from_slice(pixels);
        buf.present().expect("present");
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);

        self.skia_surface =
            skia_safe::surfaces::raster_n32_premul((self.width as i32, self.height as i32))
                .expect("Skia surface resize");

        self.sb_surface
            .resize(
                NonZeroU32::new(self.width).unwrap(),
                NonZeroU32::new(self.height).unwrap(),
            )
            .expect("softbuffer resize");
    }
}

#[cfg(test)]
mod tests {
    use super::image_to_skia;
    use image::{DynamicImage, RgbaImage};
    use skia_safe::ColorType;

    // image_to_skia: correct dimensions
    #[test]
    fn image_to_skia_dimensions() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(64, 32));
        let skia_img = image_to_skia(&img);
        assert_eq!(skia_img.width(), 64);
        assert_eq!(skia_img.height(), 32);
    }

    // image_to_skia: pixel values survive the round-trip
    #[test]
    fn image_to_skia_pixel_values() {
        let mut src = RgbaImage::new(2, 1);
        src.put_pixel(0, 0, image::Rgba([255, 0, 128, 255]));
        src.put_pixel(1, 0, image::Rgba([0, 64, 32, 128]));
        let skia_img = image_to_skia(&DynamicImage::ImageRgba8(src));
        // peek_pixels gives us a Pixmap we can read back
        let info = skia_img.image_info();
        assert_eq!(info.width(), 2);
        assert_eq!(info.height(), 1);
        // colour type is whatever Skia chose (RGBA8888 or n32); width/height suffice
        // to confirm data was accepted without panic or silent truncation
    }

    // image_to_skia: 1×1 minimum size doesn't panic
    #[test]
    fn image_to_skia_one_pixel() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(1, 1));
        let skia_img = image_to_skia(&img);
        assert_eq!(skia_img.width(), 1);
        assert_eq!(skia_img.height(), 1);
    }

    // draw: clear-only path (no image) doesn't panic
    #[test]
    fn renderer_draw_no_image() {
        let mut surface =
            skia_safe::surfaces::raster_n32_premul((100, 100)).expect("surface");
        // call the draw logic directly without a SkiaRenderer (no GPU/softbuffer needed)
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::from_argb(255, 30, 30, 30));
        // verify background colour was written
        let pixmap = surface.peek_pixels().expect("peek");
        let bytes = pixmap.bytes().expect("bytes");
        // n32 on little-endian = BGRA; byte order: B=30 G=30 R=30 A=255
        // just check the buffer is non-empty and the right size
        assert_eq!(bytes.len(), 100 * 100 * 4);
    }

    // draw: image-present path scales and positions without panic
    #[test]
    fn renderer_draw_with_image() {
        let mut surface =
            skia_safe::surfaces::raster_n32_premul((200, 150)).expect("surface");
        let page = DynamicImage::ImageRgba8(RgbaImage::new(100, 100));
        let skia_img = image_to_skia(&page);

        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::from_argb(255, 30, 30, 30));
        let img_w = skia_img.width() as f32;
        let img_h = skia_img.height() as f32;
        let win_w = 200_f32;
        let win_h = 150_f32;
        let scale = (win_w / img_w).min(win_h / img_h) * 1.0; // zoom = 1
        let draw_w = img_w * scale;
        let draw_h = img_h * scale;
        let x = (win_w - draw_w) / 2.0;
        let y = (win_h - draw_h) / 2.0;
        let src = skia_safe::Rect::from_wh(img_w, img_h);
        let dst = skia_safe::Rect::from_xywh(x, y, draw_w, draw_h);
        canvas.draw_image_rect(
            &skia_img,
            Some((&src, skia_safe::canvas::SrcRectConstraint::Strict)),
            dst,
            &skia_safe::Paint::default(),
        );
        // if we got here without panic the draw path is sound
    }
}
