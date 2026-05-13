use lcms2::Intent;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderingIntent {
    Perceptual,
    #[default]
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
