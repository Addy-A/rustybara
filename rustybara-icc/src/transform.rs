use crate::intent::RenderingIntent;
use crate::profiles::IccProfile;
use lcms2::{Flags, Profile};

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
    src_cs: crate::ColorSpaceKind,
    dst_cs: crate::ColorSpaceKind,
}

impl ColorTransform {
    pub fn new(src: &IccProfile, dst: &IccProfile, intent: RenderingIntent) -> crate::Result<Self> {
        let src_profile =
            Profile::new_icc(src.bytes).map_err(|e| crate::IccError::Profile(e.to_string()))?;
        let dst_profile =
            Profile::new_icc(dst.bytes).map_err(|e| crate::IccError::Profile(e.to_string()))?;

        let src_fmt = crate::pixel_format::PixelFormat::from(src.color_space.clone());
        let dst_fmt = crate::pixel_format::PixelFormat::from(dst.color_space.clone());

        let transform = lcms2::Transform::new_flags(
            &src_profile,
            src_fmt.to_lcms(),
            &dst_profile,
            dst_fmt.to_lcms(),
            intent.into(),
            Flags::NO_CACHE,
        )
        .map_err(|e| crate::IccError::Transform(e.to_string()))?;

        Ok(Self {
            transform,
            src_channels: src_fmt.channels(),
            dst_channels: dst_fmt.channels(),
            src_cs: src.color_space.clone(),
            dst_cs: dst.color_space.clone(),
        })
    }

    pub fn input_color_space(&self) -> &crate::ColorSpaceKind {
        &self.src_cs
    }

    pub fn output_color_space(&self) -> &crate::ColorSpaceKind {
        &self.dst_cs
    }

    pub fn convert(&self, src: &[u8]) -> Vec<u8> {
        let pixel_count = src.len() / self.src_channels;
        let mut dst = vec![0u8; pixel_count * self.dst_channels];
        self.transform.transform_pixels(src, &mut dst);
        dst
    }

    /// Returns the number of channels in the source color space.
    ///
    /// For example, `3` for RGB/sRGB and `4` for CMYK. This value determines how many
    /// consecutive bytes in the input slice are consumed per pixel during conversion.
    pub fn src_channels(&self) -> usize {
        self.src_channels
    }

    /// Returns the number of channels in the destination color space.
    ///
    /// For example, `3` for RGB/sRGB, `4` for CMYK, and `1` for grayscale. This value
    /// determines how many bytes are written per pixel into the output buffer during conversion.
    pub fn dst_channels(&self) -> usize {
        self.dst_channels
    }

    pub fn from_bytes(from: &[u8], to: &[u8], intent: RenderingIntent) -> crate::Result<Self> {
        let src_profile =
            Profile::new_icc(from).map_err(|e| crate::IccError::Profile(e.to_string()))?;
        let dst_profile =
            Profile::new_icc(to).map_err(|e| crate::IccError::Profile(e.to_string()))?;
        let src_cs = color_space_from_sig(src_profile.color_space());
        let dst_cs = color_space_from_sig(dst_profile.color_space());

        let src_fmt = crate::pixel_format::PixelFormat::from(src_cs.clone());
        let dst_fmt = crate::pixel_format::PixelFormat::from(dst_cs.clone());

        let transform = lcms2::Transform::new_flags(
            &src_profile,
            src_fmt.to_lcms(),
            &dst_profile,
            dst_fmt.to_lcms(),
            intent.into(),
            Flags::NO_CACHE,
        )
        .map_err(|e| crate::IccError::Transform(e.to_string()))?;

        Ok(Self {
            transform,
            src_channels: src_fmt.channels(),
            dst_channels: dst_fmt.channels(),
            src_cs,
            dst_cs,
        })
    }

    pub fn apply_to_pixels(&self, pixels: &mut [u8]) {
        debug_assert_eq!(
            self.src_channels, self.dst_channels,
            "apply_to_pixels requires equal channel counts; use convert() for RGB -> CMYK"
        );
        let input = pixels.to_vec();
        self.transform.transform_pixels(&input, pixels);
    }
}

fn color_space_from_sig(sig: lcms2::ColorSpaceSignature) -> crate::ColorSpaceKind {
    use lcms2::ColorSpaceSignature::*;
    match sig {
        RgbData => crate::ColorSpaceKind::Rgb,
        CmykData => crate::ColorSpaceKind::Cmyk,
        GrayData => crate::ColorSpaceKind::Gray,
        LabData => crate::ColorSpaceKind::Lab,
        _ => crate::ColorSpaceKind::Unknown,
    }
}
