/// Describes the pixel format and bit depth for color transformations.
///
/// This enum specifies how pixel data is laid out in memory for color transforms.
/// Each variant represents a specific color space and bit depth combination.
///
/// # Variants
///
/// * `Rgb8` — 8-bit RGB (3 bytes per pixel)
/// * `Rgba8` — 8-bit RGBA with alpha channel (4 bytes per pixel)
/// * `Cmyk8` — 8-bit CMYK (4 bytes per pixel)
/// * `Gray8` — 8-bit grayscale (1 byte per pixel)
/// * `Rgb16` — 16-bit RGB (6 bytes per pixel)
/// * `Cmyk16` — 16-bit CMYK (8 bytes per pixel)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb8,
    Rgba8,
    Cmyk8,
    Gray8,
    Rgb16,
    Cmyk16,
}

impl PixelFormat {
    pub(crate) fn to_lcms(self) -> lcms2::PixelFormat {
        match self {
            PixelFormat::Rgb8 => lcms2::PixelFormat::RGB_8,
            PixelFormat::Rgba8 => lcms2::PixelFormat::RGBA_8,
            PixelFormat::Cmyk8 => lcms2::PixelFormat::CMYK_8,
            PixelFormat::Gray8 => lcms2::PixelFormat::GRAY_8,
            PixelFormat::Rgb16 => lcms2::PixelFormat::RGB_16,
            PixelFormat::Cmyk16 => lcms2::PixelFormat::CMYK_16,
        }
    }

    /// Returns the number of color channels for this pixel format.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rustybara_icc::PixelFormat;
    /// assert_eq!(PixelFormat::Rgb8.channels(), 3);
    /// assert_eq!(PixelFormat::Rgba8.channels(), 4);
    /// assert_eq!(PixelFormat::Cmyk8.channels(), 4);
    /// assert_eq!(PixelFormat::Gray8.channels(), 1);
    /// ```
    pub fn channels(self) -> usize {
        match self {
            PixelFormat::Rgb8 | PixelFormat::Rgb16 => 3,
            PixelFormat::Rgba8 => 4,
            PixelFormat::Cmyk8 | PixelFormat::Cmyk16 => 4,
            PixelFormat::Gray8 => 1,
        }
    }
}

impl From<crate::ColorSpaceKind> for PixelFormat {
    fn from(cs: crate::ColorSpaceKind) -> Self {
        match cs {
            crate::ColorSpaceKind::Rgb => PixelFormat::Rgb8,
            crate::ColorSpaceKind::Cmyk => PixelFormat::Cmyk8,
            crate::ColorSpaceKind::Gray => PixelFormat::Gray8,
            crate::ColorSpaceKind::Lab => PixelFormat::Rgb8,
            crate::ColorSpaceKind::Unknown => PixelFormat::Rgb8,
        }
    }
}
