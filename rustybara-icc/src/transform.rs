use crate::profiles::IccProfile;
use crate::{intent::RenderingIntent, IccError};
use image::DynamicImage;
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
    /// Build a transform between two bundled ICC profiles.
    ///
    /// # Example
    /// ```no_run
    /// use rustybara_icc::{ColorTransform, RenderingIntent, profiles};
    /// let t = ColorTransform::new(
    ///     &profiles::COATED_FOGRA_39,
    ///     &profiles::COATED_GRACOL_2006,
    ///     RenderingIntent::RelativeColorimetric,
    /// ).unwrap();
    /// ```
    pub fn new(src: &IccProfile, dst: &IccProfile, intent: RenderingIntent) -> crate::Result<Self> {
        let src_profile =
            Profile::new_icc(&src.bytes).map_err(|e| crate::IccError::Profile(e.to_string()))?;
        let dst_profile =
            Profile::new_icc(&dst.bytes).map_err(|e| crate::IccError::Profile(e.to_string()))?;

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

    /// Returns the color space kind of the transform's source profile.
    pub fn input_color_space(&self) -> &crate::ColorSpaceKind {
        &self.src_cs
    }

    /// Returns the color space kind of the transform's destination profile.
    pub fn output_color_space(&self) -> &crate::ColorSpaceKind {
        &self.dst_cs
    }

    /// Apply the transform to a packed 8-bit pixel buffer and return the converted bytes.
    ///
    /// Input stride is `src.len() / src_channels` pixels; output length is
    /// `pixel_count * dst_channels`. Use this for channel-changing transforms
    /// (e.g. RGB → CMYK) where `apply_to_pixels` cannot be used in-place.
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

    /// Build a transform from arbitrary ICC profile bytes (not necessarily bundled profiles).
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

    /// Apply the transform in-place to a packed 8-bit pixel buffer.
    ///
    /// Requires that source and destination channel counts are equal (same-space
    /// transforms, e.g. FOGRA39 → GRACoL). For channel-changing transforms such as
    /// RGB → CMYK, use [`Self::convert`] instead.
    ///
    /// # Panics
    /// Panics in debug builds if `src_channels != dst_channels`.
    pub fn apply_to_pixels(&self, pixels: &mut [u8]) {
        debug_assert_eq!(
            self.src_channels, self.dst_channels,
            "apply_to_pixels requires equal channel counts; use convert() for RGB -> CMYK"
        );
        let input = pixels.to_vec();
        self.transform.transform_pixels(&input, pixels);
    }

    /// Apply the transform in-place to an `image::DynamicImage`.
    ///
    /// **CMYK limitation:** The `image` crate has no native CMYK image variant.
    /// This method only supports same-channel-count transforms (RGB → RGB, Gray → Gray).
    /// For RGB → CMYK conversion, call [`Self::convert`] on the raw pixel bytes directly
    /// and handle the resulting 4-channel buffer explicitly. The PDF surgery layer in
    /// [`crate::pdf::PdfColorConverter`] handles this correctly by writing to PDF
    /// image XObject streams rather than going through `DynamicImage`.
    ///
    /// # Errors
    /// Returns [`IccError::UnsupportedColorSpace`] if the transform changes channel count
    /// or if the image variant does not match the transform's source color space.
    pub fn apply_to_image(&self, image: &mut DynamicImage) -> crate::Result<()> {
        if self.src_channels != self.dst_channels {
            return Err(IccError::UnsupportedColorSpace(format!(
                "apply_to_image requires equal src/dst channel counts ({} -> {}); \
                use convert() on raw bytes for channel-changing transforms (e.g. RGB -> CMYK)",
                self.src_channels, self.dst_channels
            )));
        }

        match image {
            DynamicImage::ImageRgb8(img) => {
                if self.src_cs != crate::ColorSpaceKind::Rgb {
                    return Err(IccError::UnsupportedColorSpace(format!(
                        "image is Rgb8 but transform input is {:?}",
                        self.src_cs
                    )));
                }
                self.apply_to_pixels(img.as_mut());
            }
            DynamicImage::ImageLuma8(img) => {
                if self.src_cs != crate::ColorSpaceKind::Gray {
                    return Err(IccError::UnsupportedColorSpace(format!(
                        "image is Luma8 but transform input is {:?}",
                        self.src_cs
                    )));
                }
                self.apply_to_pixels(img.as_mut());
            }
            _ => {
                return Err(IccError::UnsupportedColorSpace(
                    "only Rgb8 and Luma8 are supported; convert first with \
                    DynamicImage::into_rgb8() or into_luma8()"
                        .to_string(),
                ));
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{
        COATED_FOGRA_39, COATED_GRACOL_2006, UNCOATED_FOGRA_29, US_WEB_COATED_SWOP,
    };
    use crate::{ColorSpaceKind, RenderingIntent};

    // --- constructor ---

    #[test]
    fn new_cmyk_to_cmyk_ok() {
        assert!(ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::RelativeColorimetric,
        )
        .is_ok());
    }

    #[test]
    fn from_bytes_cmyk_to_cmyk_ok() {
        assert!(ColorTransform::from_bytes(
            &COATED_FOGRA_39.bytes,
            &COATED_GRACOL_2006.bytes,
            RenderingIntent::RelativeColorimetric,
        )
        .is_ok());
    }

    #[test]
    fn from_bytes_invalid_src_returns_err() {
        assert!(ColorTransform::from_bytes(
            b"not an icc profile",
            &COATED_FOGRA_39.bytes,
            RenderingIntent::Perceptual,
        )
        .is_err());
    }

    #[test]
    fn from_bytes_invalid_dst_returns_err() {
        assert!(ColorTransform::from_bytes(
            &COATED_FOGRA_39.bytes,
            b"not an icc profile",
            RenderingIntent::Perceptual,
        )
        .is_err());
    }

    // --- color space accessors ---

    #[test]
    fn new_input_color_space_is_cmyk() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::Perceptual,
        )
        .unwrap();
        assert_eq!(t.input_color_space(), &ColorSpaceKind::Cmyk);
    }

    #[test]
    fn new_output_color_space_is_cmyk() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::Perceptual,
        )
        .unwrap();
        assert_eq!(t.output_color_space(), &ColorSpaceKind::Cmyk);
    }

    #[test]
    fn from_bytes_detects_cmyk_color_spaces() {
        let t = ColorTransform::from_bytes(
            &COATED_FOGRA_39.bytes,
            &US_WEB_COATED_SWOP.bytes,
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap();
        assert_eq!(t.input_color_space(), &ColorSpaceKind::Cmyk);
        assert_eq!(t.output_color_space(), &ColorSpaceKind::Cmyk);
    }

    // --- channel counts ---

    #[test]
    fn cmyk_to_cmyk_src_channels_is_4() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::Perceptual,
        )
        .unwrap();
        assert_eq!(t.src_channels(), 4);
    }

    #[test]
    fn cmyk_to_cmyk_dst_channels_is_4() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::Perceptual,
        )
        .unwrap();
        assert_eq!(t.dst_channels(), 4);
    }

    // --- convert output size ---

    #[test]
    fn convert_output_length_matches_pixel_count() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap();
        let input = vec![0u8; 5 * 4]; // 5 CMYK pixels
        let output = t.convert(&input);
        assert_eq!(output.len(), 5 * t.dst_channels());
    }

    #[test]
    fn convert_empty_input_produces_empty_output() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::Perceptual,
        )
        .unwrap();
        assert!(t.convert(&[]).is_empty());
    }

    // --- apply_to_pixels ---

    #[test]
    fn apply_to_pixels_preserves_slice_length() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap();
        // 2 CMYK pixels
        let mut pixels = vec![50u8, 70, 80, 10, 0, 0, 0, 255];
        let len_before = pixels.len();
        t.apply_to_pixels(&mut pixels);
        assert_eq!(pixels.len(), len_before);
    }

    // --- stability ---

    // Paper white (C=0 M=0 Y=0 K=0) with RelativeColorimetric maps the source white point
    // to the destination white point, so the output should also be near ink-free.
    #[test]
    fn paper_white_stays_light_after_cmyk_to_cmyk() {
        let t = ColorTransform::new(
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap();
        let white = [0u8, 0, 0, 0];
        let out = t.convert(&white);
        for (i, &ch) in out.iter().enumerate() {
            assert!(
                ch < 10,
                "channel {i} = {ch}: expected near-zero ink for paper white input"
            );
        }
    }

    // Verify that a different profile pair also builds cleanly (not just FOGRA39↔GRACoL).
    #[test]
    fn different_profile_pair_constructs_ok() {
        assert!(ColorTransform::new(
            &US_WEB_COATED_SWOP,
            &UNCOATED_FOGRA_29,
            RenderingIntent::Perceptual,
        )
        .is_ok());
    }

    // --- all intents ---

    #[test]
    fn all_rendering_intents_construct_ok() {
        use RenderingIntent::*;
        for intent in [
            Perceptual,
            RelativeColorimetric,
            Saturation,
            AbsoluteColorimetric,
        ] {
            ColorTransform::new(&COATED_FOGRA_39, &COATED_GRACOL_2006, intent)
                .unwrap_or_else(|e| panic!("intent {intent:?} failed: {e}"));
        }
    }

    // --- FOGRA39 → sRGB reference values (STUBBED) ---
    //
    // These tests pin FOGRA39 CMYK → sRGB output against values produced by a validated
    // reference tool (Argyll CMS `xicclu`, Little CMS CLI `transicc`, or Adobe ACE).
    //
    // How to generate reference values with `transicc` (Little CMS CLI):
    //   transicc -i CoatedFOGRA39.icc -o sRGB_IEC61966-2-1.icc -t 1
    //   # -t 1 = RelativeColorimetric; input CMYK as 0..255, output RGB 0..255
    //
    // Acceptance criterion: ±2 per channel (8-bit quantisation + rounding tolerance).
    //
    // To enable a test:
    //   1. Obtain sRGB bytes at runtime:
    //      let srgb_bytes = lcms2::Profile::new_srgb().icc_data().unwrap().to_vec();
    //   2. Build the transform:
    //      let t = ColorTransform::from_bytes(
    //          &COATED_FOGRA_39.bytes, &srgb_bytes, RenderingIntent::RelativeColorimetric
    //      ).unwrap();
    //   3. Convert the patch and compare:
    //      let out = t.convert(&cmyk);
    //      for (i, (&a, &e)) in out.iter().zip(expected_rgb.iter()).enumerate() {
    //          assert!((a as i16 - e as i16).unsigned_abs() <= 2, "ch{i}: {a} ≠ {e}±2");
    //      }

    // --- FOGRA39 → sRGB reference values ---
    //
    // Acceptance criterion: ±2 per channel (8-bit quantisation + rounding tolerance).
    // Reference values generated by lcms2 (same engine as `transicc`), RelativeColorimetric.

    fn srgb_bytes() -> Vec<u8> {
        lcms2::Profile::new_srgb().icc().unwrap()
    }

    fn fogra39_to_srgb() -> ColorTransform {
        ColorTransform::from_bytes(
            &COATED_FOGRA_39.bytes,
            &srgb_bytes(),
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap()
    }

    fn swop_to_srgb() -> ColorTransform {
        ColorTransform::from_bytes(
            &US_WEB_COATED_SWOP.bytes,
            &srgb_bytes(),
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap()
    }

    fn srgb_to_swop() -> ColorTransform {
        ColorTransform::from_bytes(
            &srgb_bytes(),
            &US_WEB_COATED_SWOP.bytes,
            RenderingIntent::RelativeColorimetric,
        )
        .unwrap()
    }

    fn assert_rgb_close(actual: &[u8], expected: [u8; 3], label: &str) {
        for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (a as i16 - e as i16).unsigned_abs() <= 2,
                "{label} ch{i}: got {a}, expected {e}±2"
            );
        }
    }

    fn assert_cmyk_close(actual: &[u8], expected: [u8; 4], label: &str) {
        for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (a as i16 - e as i16).unsigned_abs() <= 2,
                "{label} ch{i}: got {a}, expected {e}±2"
            );
        }
    }

    // FOGRA39 → sRGB

    #[test]
    fn fogra39_to_srgb_rich_black() {
        // Patch: C=191 M=173 Y=171 K=229  (75% 68% 67% 90% scaled to 0..255)
        let out = fogra39_to_srgb().convert(&[191u8, 173, 171, 229]);
        assert_rgb_close(&out, [37, 36, 34], "fogra39 rich_black");
    }

    #[test]
    fn fogra39_to_srgb_pure_cyan() {
        // Patch: C=255 M=0 Y=0 K=0
        let out = fogra39_to_srgb().convert(&[255u8, 0, 0, 0]);
        assert_rgb_close(&out, [0, 160, 228], "fogra39 pure_cyan");
    }

    #[test]
    fn fogra39_to_srgb_paper_white() {
        // Patch: C=0 M=0 Y=0 K=0 — RelativeColorimetric maps source white to output white
        let out = fogra39_to_srgb().convert(&[0u8, 0, 0, 0]);
        assert_rgb_close(&out, [255, 255, 255], "fogra39 paper_white");
    }

    #[test]
    fn fogra39_to_srgb_output_has_three_channels() {
        let t = fogra39_to_srgb();
        assert_eq!(t.dst_channels(), 3);
        assert_eq!(t.output_color_space(), &ColorSpaceKind::Rgb);
    }

    #[test]
    fn fogra39_to_srgb_input_is_cmyk() {
        let t = fogra39_to_srgb();
        assert_eq!(t.src_channels(), 4);
        assert_eq!(t.input_color_space(), &ColorSpaceKind::Cmyk);
    }

    // SWOP → sRGB

    #[test]
    fn swop_to_srgb_rich_black() {
        // Patch: C=191 M=173 Y=171 K=229 under US Web Coated SWOP
        let out = swop_to_srgb().convert(&[191u8, 173, 171, 229]);
        assert_rgb_close(&out, [41, 41, 41], "swop rich_black");
    }

    #[test]
    fn swop_to_srgb_pure_cyan() {
        // Patch: C=255 M=0 Y=0 K=0 under US Web Coated SWOP
        let out = swop_to_srgb().convert(&[255u8, 0, 0, 0]);
        assert_rgb_close(&out, [0, 176, 240], "swop pure_cyan");
    }

    #[test]
    fn swop_to_srgb_paper_white() {
        let out = swop_to_srgb().convert(&[0u8, 0, 0, 0]);
        assert_rgb_close(&out, [255, 255, 255], "swop paper_white");
    }

    #[test]
    fn swop_to_srgb_constructs_ok() {
        assert!(ColorTransform::from_bytes(
            &US_WEB_COATED_SWOP.bytes,
            &srgb_bytes(),
            RenderingIntent::RelativeColorimetric,
        )
        .is_ok());
    }

    #[test]
    fn swop_to_srgb_output_has_three_channels() {
        let t = swop_to_srgb();
        assert_eq!(t.dst_channels(), 3);
        assert_eq!(t.output_color_space(), &ColorSpaceKind::Rgb);
    }

    // sRGB → SWOP

    #[test]
    fn srgb_to_swop_white_is_near_zero_ink() {
        // sRGB white → CMYK should be near-zero ink
        let out = srgb_to_swop().convert(&[255u8, 255, 255]);
        assert_cmyk_close(&out, [0, 0, 0, 0], "srgb white→swop");
    }

    #[test]
    fn srgb_to_swop_red_hits_m_and_y() {
        // sRGB red → CMYK: high Magenta + Yellow, near-zero Cyan + Black
        let out = srgb_to_swop().convert(&[255u8, 0, 0]);
        assert_cmyk_close(&out, [0, 255, 255, 0], "srgb red→swop");
    }

    #[test]
    fn srgb_to_swop_black_is_rich_black() {
        // sRGB black → CMYK should be a rich black (high ink all channels)
        let out = srgb_to_swop().convert(&[0u8, 0, 0]);
        assert_cmyk_close(&out, [190, 173, 167, 230], "srgb black→swop");
    }

    #[test]
    fn srgb_to_swop_constructs_ok() {
        assert!(ColorTransform::from_bytes(
            &srgb_bytes(),
            &US_WEB_COATED_SWOP.bytes,
            RenderingIntent::RelativeColorimetric,
        )
        .is_ok());
    }

    #[test]
    fn srgb_to_swop_output_has_four_channels() {
        let t = srgb_to_swop();
        assert_eq!(t.dst_channels(), 4);
        assert_eq!(t.output_color_space(), &ColorSpaceKind::Cmyk);
    }

    #[test]
    fn srgb_to_swop_input_is_rgb() {
        let t = srgb_to_swop();
        assert_eq!(t.src_channels(), 3);
        assert_eq!(t.input_color_space(), &ColorSpaceKind::Rgb);
    }

    // convert output sizing

    #[test]
    fn swop_to_srgb_output_length_is_three_per_pixel() {
        let t = swop_to_srgb();
        let out = t.convert(&[0u8; 4 * 5]); // 5 CMYK pixels
        assert_eq!(out.len(), 5 * 3);
    }

    #[test]
    fn srgb_to_swop_output_length_is_four_per_pixel() {
        let t = srgb_to_swop();
        let out = t.convert(&[0u8; 3 * 5]); // 5 RGB pixels
        assert_eq!(out.len(), 5 * 4);
    }
}
