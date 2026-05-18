use image::DynamicImage;

pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, image: &DynamicImage) -> wgpu::Texture {
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        view_formats: &[],
    });

    queue.write_texture(
        texture.as_image_copy(),
        rgba.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    texture
}

#[cfg(test)]
mod tests {
    use super::upload;
    use image::{DynamicImage, RgbaImage};

    #[test]
    #[ignore = "requires GPU or software fallback adapter"]
    fn upload_rgba_image() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter: true,
            ..Default::default()
        }))
        .expect("no software fallback adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("no device");

        let img = DynamicImage::ImageRgba8(RgbaImage::new(16, 16));
        let tex = upload(&device, &queue, &img);
        let size = tex.size();
        assert_eq!(size.width, 16);
        assert_eq!(size.height, 16);
        assert_eq!(size.depth_or_array_layers, 1);
    }
}
