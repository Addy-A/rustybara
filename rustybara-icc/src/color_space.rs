/// Classifies ICC profiles by their color space signature.
///
/// This enum identifies the color space of an ICC profile, which determines
/// the number and interpretation of color channels.
///
/// # Variants
///
/// * `Cmyk` — Cyan, Magenta, Yellow, Black (4 channels)
/// * `Rgb` — Red, Green, Blue (3 channels)
/// * `Gray` — Grayscale (1 channel)
/// * `Lab` — CIE L*a*b* (3 channels, perceptually uniform)
/// * `Unknown` — Unrecognized or unsupported color space
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorSpaceKind {
    Cmyk,
    Rgb,
    Gray,
    Lab,
    Unknown,
}
