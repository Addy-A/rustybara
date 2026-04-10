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
/// ```rust
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
/// Returns an `image::ImageResult<()>` which is:
/// * `Ok(())` if the image was successfully saved
/// * `Err(image::ImageError)` if there was an error during the save operation
///
/// # Examples
///
/// ```rust
/// use std::path::Path;
/// use image::{DynamicImage, ImageFormat};
/// use your_crate::{save, OutputFormat};
///
/// let image = DynamicImage::new_rgb8(100, 100);
/// let path = Path::new("output.png");
/// let format = OutputFormat::Png;
///
/// match save(&image, &path, &format) {
///     Ok(()) => println!("Image saved successfully"),
///     Err(e) => eprintln!("Failed to save image: {}", e),
/// }
/// ```
pub fn save(image: &DynamicImage, path: &Path, format: &OutputFormat) -> image::ImageResult<()> {
    image.save_with_format(path, format.image_format())
}
