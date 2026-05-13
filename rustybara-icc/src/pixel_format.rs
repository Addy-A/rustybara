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
