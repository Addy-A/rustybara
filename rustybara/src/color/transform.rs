use super::icc::{ColorSpace, IccProfile};
use lcms2::{Flags, Intent, PixelFormat, Profile};

/// Specifies the rendering intent for color management operations.
///
/// Rendering intent determines how colors are mapped when converting between
/// different color spaces or devices. Each intent prioritizes different aspects
/// of color reproduction based on the intended use case.
///
/// # Variants
///
/// * `Perceptual` - Maintains the visual relationship between colors as perceived
///   by the human eye. Best for photographic images where maintaining overall
///   color harmony is important.
///
/// * `RelativeColorimetric` - Preserves exact color relationships within the
///   color gamut while compressing out-of-gamut colors to the closest reproducible
///   colors. Good for graphics and images where color accuracy is critical.
///
/// * `Saturation` - Prioritizes vivid, saturated colors over accurate color
///   reproduction. Best for business graphics like charts and presentations.
///
/// * `AbsoluteColorimetric` - Maintains exact color values regardless of the
///   white point of the destination device. Colors outside the destination
///   gamut are clipped to the nearest reproducible colors.
///
/// # Examples
///
/// ```no_test
/// use your_crate::RenderingIntent;
///
/// let intent = RenderingIntent::Perceptual;
/// println!("{:?}", intent);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderingIntent {
    Perceptual,
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}

impl From<RenderingIntent> for Intent {
    fn from(ri: RenderingIntent) -> Intent {
        match ri {
            RenderingIntent::Perceptual => Intent::Perceptual,
            RenderingIntent::RelativeColorimetric => Intent::RelativeColorimetric,
            RenderingIntent::Saturation => Intent::Saturation,
            RenderingIntent::AbsoluteColorimetric => Intent::AbsoluteColorimetric,
        }
    }
}

/// A color transformation wrapper that converts pixel data between different color spaces.
///
/// This struct encapsulates an LCMS2 transform for converting colors from one profile
/// to another. It maintains information about the source and destination channel counts
/// to properly handle pixel data during conversion.
///
/// # Type Parameters
/// * `u8` - Input pixel data type (8-bit per channel)
/// * `u8` - Output pixel data type (8-bit per channel)  
/// * `lcms2::GlobalContext` - Uses global LCMS context
/// * `lcms2::DisallowCache` - Disables caching for thread safety
///
/// # Fields
/// * `transform` - The underlying LCMS2 transform object that performs the actual color conversion
/// * `src_channels` - Number of channels in the source color space (e.g., 3 for RGB, 4 for CMYK)
/// * `dst_channels` - Number of channels in the destination color space (e.g., 3 for RGB, 4 for CMYK)
///
/// # Example
/// ```no_test
/// // Typical usage would involve creating a transform between two ICC profiles
/// // and then using it to convert pixel data
/// let transform = ColorTransform::new(source_profile, destination_profile);
/// let output_pixels = transform.convert(input_pixels);
/// ```
pub struct ColorTransform {
    transform: lcms2::Transform<u8, u8, lcms2::GlobalContext, lcms2::AllowCache>,
    src_channels: usize,
    dst_channels: usize,
}

impl ColorTransform {
    pub fn new(src: &IccProfile, dst: &IccProfile, intent: RenderingIntent) -> crate::Result<Self> {
        let src_profile = Profile::new_icc(src.as_bytes())?;
        let dst_profile = Profile::new_icc(dst.as_bytes())?;

        let (src_fmt, src_ch) = pixel_format(src.color_space());
        let (dst_fmt, dst_ch) = pixel_format(dst.color_space());

        let transform = lcms2::Transform::new_flags(
            &src_profile,
            src_fmt,
            &dst_profile,
            dst_fmt,
            intent.into(),
            Flags::NO_CACHE,
        )?;

        Ok(Self {
            transform,
            src_channels: src_ch,
            dst_channels: dst_ch,
        })
    }

    /// Converts image data from source format to destination format.
    ///
    /// This method takes a slice of bytes representing image data in the source
    /// color format and converts it to the destination color format using the
    /// configured transformation.
    ///
    /// # Parameters
    /// * `src` - A slice of bytes containing the source image data. The length
    ///          should be a multiple of `src_channels` representing complete pixels.
    ///
    /// # Returns
    /// A new vector containing the converted image data in the destination format.
    /// The returned vector will have length `pixel_count * dst_channels` where
    /// `pixel_count` is derived from `src.len() / src_channels`.
    ///
    /// # Example
    /// ```no_test
    /// // Convert RGB data to grayscale
    /// let converter = ImageConverter::new(ColorSpace::Rgb, ColorSpace::Gray);
    /// let rgb_data = vec![255, 128, 64, 0, 0, 0]; // 2 RGB pixels
    /// let gray_data = converter.convert(&rgb_data); // 2 grayscale pixels
    /// ```
    pub fn convert(&self, src: &[u8]) -> Vec<u8> {
        let pixel_count = src.len() / self.src_channels;
        let mut dst = vec![0u8; pixel_count * self.dst_channels];
        self.transform.transform_pixels(src, &mut dst);
        dst
    }

    pub fn src_channels(&self) -> usize {
        self.src_channels
    }
    pub fn dst_channels(&self) -> usize {
        self.dst_channels
    }
}

/// Determines the appropriate pixel format and byte count for a given color space.
///
/// This function maps color spaces to their corresponding pixel formats and returns
/// the number of bytes needed to represent a single pixel in that format.
///
/// # Arguments
///
/// * `cs` - The color space to convert to pixel format
///
/// # Returns
///
/// A tuple containing:
/// * `PixelFormat` - The pixel format enum variant for the given color space
/// * `usize` - The number of bytes per pixel for this format
///
/// # Supported Mappings
///
/// * `ColorSpace::Srgb` | `ColorSpace::Rgb` => `(PixelFormat::RGB_8, 3)` - 24-bit RGB
/// * `ColorSpace::Cmyk` => `(PixelFormat::CMYK_8, 4)` - 32-bit CMYK
/// * `ColorSpace::Gray` => `(PixelFormat::GRAY_8, 1)` - 8-bit grayscale
fn pixel_format(cs: ColorSpace) -> (PixelFormat, usize) {
    match cs {
        ColorSpace::Srgb | ColorSpace::Rgb => (PixelFormat::RGB_8, 3),
        ColorSpace::Cmyk => (PixelFormat::CMYK_8, 4),
        ColorSpace::Gray => (PixelFormat::GRAY_8, 1),
    }
}
