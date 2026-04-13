use image::{DynamicImage, ImageFormat};
use std::path::Path;

/// Represents the supported output image formats for encoding.
///
/// This enum defines the available formats that can be used when saving or exporting
/// processed images. Each variant corresponds to a specific image format with its
/// own characteristics and use cases.
///
/// # Variants
///
/// * `Jpg` - JPEG format, best for photographs and images with gradients
/// * `Png` - PNG format, supports transparency and lossless compression
/// * `WebP` - Google's WebP format, provides better compression than JPEG/PNG
/// * `Tiff` - TIFF format, supports multiple compression methods and layers
///
/// # Example
///
/// ```
/// use rustybara::encode::OutputFormat;
/// let format = OutputFormat::Png;
/// match format {
///     OutputFormat::Png => println!("Saving as PNG"),
///     _ => println!("Saving in another format"),
/// }
/// ```
pub enum OutputFormat {
    Jpg,
    Png,
    WebP,
    Tiff,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
            Self::WebP => "webp",
            Self::Tiff => "tiff",
        }
    }

    pub fn image_format(&self) -> ImageFormat {
        match self {
            Self::Jpg => ImageFormat::Jpeg,
            Self::Png => ImageFormat::Png,
            Self::WebP => ImageFormat::WebP,
            Self::Tiff => ImageFormat::Tiff,
        }
    }
}

/// Saves a dynamic image to the specified path in the given output format.
///
/// This function wraps the standard image saving functionality and provides
/// a convenient way to save images with automatic format detection based on
/// the provided `OutputFormat` enum.
///
/// # Arguments
///
/// * `image` - A reference to the `DynamicImage` to be saved
/// * `path` - A reference to the `Path` where the image should be saved
/// * `format` - A reference to the `OutputFormat` specifying the desired output format
///
/// # Returns
///
/// Returns a `Result<()>` which is:
/// * `Ok(())` if the image was successfully saved
/// * `Err(Error)` if there was an error during the save operation
///
/// # Examples
///
/// ```no_test
/// use std::path::Path;
/// use rustybara::encode::{save, OutputFormat};
///
/// let image = image::DynamicImage::new_rgb8(100, 100);
/// let path = Path::new("output.png");
/// let format = OutputFormat::Png;
/// save(&image, &path, &format).unwrap();
/// ```
pub fn save(
    image: &DynamicImage,
    path: &Path,
    format: &OutputFormat,
    dpi: u32,
) -> crate::Result<()> {
    match format {
        OutputFormat::Jpg => {
            let mut buf = std::io::BufWriter::new(std::fs::File::create(path)?);
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90);
            encoder.set_pixel_density(image::codecs::jpeg::PixelDensity {
                density: (dpi as u16, dpi as u16),
                unit: image::codecs::jpeg::PixelDensityUnit::Inches,
            });
            encoder.encode_image(image)?;
            Ok(())
        }
        OutputFormat::Png => {
            let mut buf = std::io::BufWriter::new(std::fs::File::create(path)?);
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut buf,
                image::codecs::png::CompressionType::Default,
                image::codecs::png::FilterType::Adaptive,
            );
            image.write_with_encoder(encoder)?;
            Ok(())
        }
        OutputFormat::Tiff => {
            image.save_with_format(path, format.image_format())?;
            Ok(())
        }
        OutputFormat::WebP => {
            let encoder = webp::Encoder::from_image(image)
                .map_err(|e| crate::Error::Io(std::io::Error::other(e)))?;
            let memory = encoder.encode(75.0);
            std::fs::write(path, &*memory)?;
            Ok(())
        }
    }
}
