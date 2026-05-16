use crate::ColorSpaceKind;
use std::sync::{Arc, LazyLock};

/// An ICC color profile with typed metadata.
///
/// Bundled profiles are exposed as `pub static` [`LazyLock`] values in this module
/// (e.g. [`COATED_FOGRA_39`], [`ADOBE_RGB_1998`]). User-supplied profiles can be
/// constructed with [`IccProfile::from_user_bytes`]. Pass any profile to
/// [`crate::ColorTransform::new`] to build a color transform.
#[derive(Clone)]
pub struct IccProfile {
    /// Short machine-readable identifier (e.g. `"CoatedFOGRA39"`).
    pub name: String,
    /// Human-readable description (e.g. `"Coated FOGRA 39"`).
    pub description: String,
    /// Color space this profile operates in.
    pub color_space: ColorSpaceKind,
    /// Raw ICC profile bytes.
    pub bytes: Arc<[u8]>,
}

impl IccProfile {
    /// Validate and construct an [`IccProfile`] from user-supplied bytes.
    ///
    /// The color space is detected from the ICC header via lcms2. Returns an error if the bytes
    /// are not a valid ICC profile.
    pub fn from_user_bytes(
        name: String,
        description: String,
        bytes: Vec<u8>,
    ) -> crate::Result<Self> {
        let profile = lcms2::Profile::new_icc(&bytes)
            .map_err(|e| crate::IccError::Profile(format!("invalid ICC profile: {e}")))?;
        let color_space = match profile.color_space() {
            lcms2::ColorSpaceSignature::RgbData => ColorSpaceKind::Rgb,
            lcms2::ColorSpaceSignature::CmykData => ColorSpaceKind::Cmyk,
            lcms2::ColorSpaceSignature::GrayData => ColorSpaceKind::Gray,
            lcms2::ColorSpaceSignature::LabData => ColorSpaceKind::Lab,
            _ => ColorSpaceKind::Unknown,
        };
        Ok(Self {
            name,
            description,
            color_space,
            bytes: Arc::from(bytes),
        })
    }
}

// ── CMYK ─────────────────────────────────────────────────────────────────────

pub static COATED_FOGRA_27: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "CoatedFOGRA27".to_string(),
    description: "Coated FOGRA 27".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/CoatedFOGRA27.icc").as_slice()),
});

pub static COATED_FOGRA_39: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "CoatedFOGRA39".to_string(),
    description: "Coated FOGRA 39".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/CoatedFOGRA39.icc").as_slice()),
});

pub static COATED_GRACOL_2006: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "CoatedGRACoL2006".to_string(),
    description: "Coated GRACoL 2006".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/CoatedGRACoL2006.icc").as_slice()),
});

pub static JAPAN_COLOR_2001_COATED: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "JapanColor2001Coated".to_string(),
    description: "Japan Color 2001 Coated".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/JapanColor2001Coated.icc").as_slice()),
});

pub static JAPAN_COLOR_2001_UNCOATED: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "JapanColor2001Uncoated".to_string(),
    description: "Japan Color 2001 Uncoated".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/JapanColor2001Uncoated.icc").as_slice()),
});

pub static JAPAN_COLOR_2002_NEWSPAPER: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "JapanColor2002Newspaper".to_string(),
    description: "Japan Color 2002 Newspaper".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/JapanColor2002Newspaper.icc").as_slice()),
});

pub static JAPAN_COLOR_2003_WEB_COATED: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "JapanColor2003WebCoated".to_string(),
    description: "Japan Color 2003 Web Coated".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/JapanColor2003WebCoated.icc").as_slice()),
});

pub static JAPAN_WEB_COATED: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "JapanWebCoated".to_string(),
    description: "Japan Web Coated".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/JapanWebCoated.icc").as_slice()),
});

pub static UNCOATED_FOGRA_29: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "UncoatedFOGRA29".to_string(),
    description: "Uncoated FOGRA 29".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/UncoatedFOGRA29.icc").as_slice()),
});

pub static US_WEB_COATED_SWOP: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "USWebCoatedSWOP".to_string(),
    description: "US Web Coated SWOP".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/USWebCoatedSWOP.icc").as_slice()),
});

pub static US_WEB_UNCOATED: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "USWebUncoated".to_string(),
    description: "US Web Uncoated".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/USWebUncoated.icc").as_slice()),
});

pub static WEB_COATED_FOGRA_28: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "WebCoatedFOGRA28".to_string(),
    description: "Web Coated FOGRA 28".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/WebCoatedFOGRA28.icc").as_slice()),
});

pub static WEB_COATED_SWOP_2006_GRADE3: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "WebCoatedSWOP2006Grade3".to_string(),
    description: "Web Coated SWOP 2006 Grade 3".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/WebCoatedSWOP2006Grade3.icc").as_slice()),
});

pub static WEB_COATED_SWOP_2006_GRADE5: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "WebCoatedSWOP2006Grade5".to_string(),
    description: "Web Coated SWOP 2006 Grade 5".to_string(),
    color_space: ColorSpaceKind::Cmyk,
    bytes: Arc::from(include_bytes!("data/CMYK/WebCoatedSWOP2006Grade5.icc").as_slice()),
});

// ── RGB ──────────────────────────────────────────────────────────────────────

pub static ADOBE_RGB_1998: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "AdobeRGB1998".to_string(),
    description: "Adobe RGB (1998)".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/AdobeRGB1998.icc").as_slice()),
});

pub static APPLE_RGB: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "AppleRGB".to_string(),
    description: "Apple RGB".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/AppleRGB.icc").as_slice()),
});

pub static COLOR_MATCH_RGB: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "ColorMatchRGB".to_string(),
    description: "ColorMatch RGB".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/ColorMatchRGB.icc").as_slice()),
});

pub static PAL_SECAM: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "PAL_SECAM".to_string(),
    description: "PAL/SECAM".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/PAL_SECAM.icc").as_slice()),
});

pub static SMPTE_C: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "SMPTE-C".to_string(),
    description: "SMPTE-C".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/SMPTE-C.icc").as_slice()),
});

pub static VIDEO_HD: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "VideoHD".to_string(),
    description: "HDTV (Rec. 709)".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/VideoHD.icc").as_slice()),
});

pub static VIDEO_NTSC: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "VideoNTSC".to_string(),
    description: "NTSC (1953)".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/VideoNTSC.icc").as_slice()),
});

pub static VIDEO_PAL: LazyLock<IccProfile> = LazyLock::new(|| IccProfile {
    name: "VideoPAL".to_string(),
    description: "PAL (Video)".to_string(),
    color_space: ColorSpaceKind::Rgb,
    bytes: Arc::from(include_bytes!("data/RGB/VideoPAL.icc").as_slice()),
});

// ── Lookup ────────────────────────────────────────────────────────────────────

/// Returns the bundled profile for a given machine-readable name, or `None` if unknown.
pub fn by_name(name: &str) -> Option<&'static IccProfile> {
    match name {
        "CoatedFOGRA27" => Some(&*COATED_FOGRA_27),
        "CoatedFOGRA39" => Some(&*COATED_FOGRA_39),
        "CoatedGRACoL2006" => Some(&*COATED_GRACOL_2006),
        "JapanColor2001Coated" => Some(&*JAPAN_COLOR_2001_COATED),
        "JapanColor2001Uncoated" => Some(&*JAPAN_COLOR_2001_UNCOATED),
        "JapanColor2002Newspaper" => Some(&*JAPAN_COLOR_2002_NEWSPAPER),
        "JapanColor2003WebCoated" => Some(&*JAPAN_COLOR_2003_WEB_COATED),
        "JapanWebCoated" => Some(&*JAPAN_WEB_COATED),
        "UncoatedFOGRA29" => Some(&*UNCOATED_FOGRA_29),
        "USWebCoatedSWOP" => Some(&*US_WEB_COATED_SWOP),
        "USWebUncoated" => Some(&*US_WEB_UNCOATED),
        "WebCoatedFOGRA28" => Some(&*WEB_COATED_FOGRA_28),
        "WebCoatedSWOP2006Grade3" => Some(&*WEB_COATED_SWOP_2006_GRADE3),
        "WebCoatedSWOP2006Grade5" => Some(&*WEB_COATED_SWOP_2006_GRADE5),
        "AdobeRGB1998" => Some(&*ADOBE_RGB_1998),
        "AppleRGB" => Some(&*APPLE_RGB),
        "ColorMatchRGB" => Some(&*COLOR_MATCH_RGB),
        "PAL_SECAM" => Some(&*PAL_SECAM),
        "SMPTE-C" => Some(&*SMPTE_C),
        "VideoHD" => Some(&*VIDEO_HD),
        "VideoNTSC" => Some(&*VIDEO_NTSC),
        "VideoPAL" => Some(&*VIDEO_PAL),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorSpaceKind;

    fn all_cmyk_profiles() -> Vec<&'static IccProfile> {
        vec![
            &*COATED_FOGRA_27,
            &*COATED_FOGRA_39,
            &*COATED_GRACOL_2006,
            &*JAPAN_COLOR_2001_COATED,
            &*JAPAN_COLOR_2001_UNCOATED,
            &*JAPAN_COLOR_2002_NEWSPAPER,
            &*JAPAN_COLOR_2003_WEB_COATED,
            &*JAPAN_WEB_COATED,
            &*UNCOATED_FOGRA_29,
            &*US_WEB_COATED_SWOP,
            &*US_WEB_UNCOATED,
            &*WEB_COATED_FOGRA_28,
            &*WEB_COATED_SWOP_2006_GRADE3,
            &*WEB_COATED_SWOP_2006_GRADE5,
        ]
    }

    fn all_rgb_profiles() -> Vec<&'static IccProfile> {
        vec![
            &*ADOBE_RGB_1998,
            &*APPLE_RGB,
            &*COLOR_MATCH_RGB,
            &*PAL_SECAM,
            &*SMPTE_C,
            &*VIDEO_HD,
            &*VIDEO_NTSC,
            &*VIDEO_PAL,
        ]
    }

    fn assert_icc_signature(p: &IccProfile) {
        assert!(
            p.bytes.len() >= 40,
            "{}: profile too short to contain an ICC header ({} bytes)",
            p.name,
            p.bytes.len()
        );
        assert_eq!(
            &p.bytes[36..40],
            b"acsp",
            "{}: bytes[36..40] are not the ICC signature 'acsp'",
            p.name,
        );
    }

    // ── color_space field correctness ─────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_report_cmyk() {
        for p in all_cmyk_profiles() {
            assert_eq!(
                p.color_space,
                ColorSpaceKind::Cmyk,
                "{}: expected Cmyk, got {:?}",
                p.name,
                p.color_space
            );
        }
    }

    #[test]
    fn all_rgb_profiles_report_rgb() {
        for p in all_rgb_profiles() {
            assert_eq!(
                p.color_space,
                ColorSpaceKind::Rgb,
                "{}: expected Rgb, got {:?}",
                p.name,
                p.color_space
            );
        }
    }

    // ── ICC signature ─────────────────────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_have_valid_icc_signature() {
        for p in all_cmyk_profiles() {
            assert_icc_signature(p);
        }
    }

    #[test]
    fn all_rgb_profiles_have_valid_icc_signature() {
        for p in all_rgb_profiles() {
            assert_icc_signature(p);
        }
    }

    // ── non-empty bytes ───────────────────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_have_non_empty_bytes() {
        for p in all_cmyk_profiles() {
            assert!(!p.bytes.is_empty(), "{}: bytes is empty", p.name);
        }
    }

    #[test]
    fn all_rgb_profiles_have_non_empty_bytes() {
        for p in all_rgb_profiles() {
            assert!(!p.bytes.is_empty(), "{}: bytes is empty", p.name);
        }
    }

    // ── name / description ────────────────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_have_non_empty_name_and_description() {
        for p in all_cmyk_profiles() {
            assert!(!p.name.is_empty(), "a CMYK profile has an empty name");
            assert!(!p.description.is_empty(), "{}: empty description", p.name);
        }
    }

    #[test]
    fn all_rgb_profiles_have_non_empty_name_and_description() {
        for p in all_rgb_profiles() {
            assert!(!p.name.is_empty(), "an RGB profile has an empty name");
            assert!(!p.description.is_empty(), "{}: empty description", p.name);
        }
    }

    // ── size sanity ───────────────────────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_exceed_minimum_icc_size() {
        for p in all_cmyk_profiles() {
            assert!(
                p.bytes.len() >= 1024,
                "{}: suspiciously small ({} bytes) — likely a broken embed",
                p.name,
                p.bytes.len(),
            );
        }
    }

    #[test]
    fn all_rgb_profiles_exceed_minimum_icc_size() {
        for p in all_rgb_profiles() {
            assert!(
                p.bytes.len() >= 256,
                "{}: suspiciously small ({} bytes) — likely a broken embed",
                p.name,
                p.bytes.len(),
            );
        }
    }

    // ── ICC header size field ─────────────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_size_matches_icc_header_declaration() {
        for p in all_cmyk_profiles() {
            assert!(p.bytes.len() >= 4, "{}: too short for size field", p.name);
            let declared = u32::from_be_bytes(p.bytes[0..4].try_into().unwrap()) as usize;
            assert_eq!(
                declared,
                p.bytes.len(),
                "{}: ICC header declares {} bytes but embed has {}",
                p.name,
                declared,
                p.bytes.len()
            );
        }
    }

    #[test]
    fn all_rgb_profiles_size_matches_icc_header_declaration() {
        for p in all_rgb_profiles() {
            assert!(p.bytes.len() >= 4, "{}: too short for size field", p.name);
            let declared = u32::from_be_bytes(p.bytes[0..4].try_into().unwrap()) as usize;
            assert_eq!(
                declared,
                p.bytes.len(),
                "{}: ICC header declares {} bytes but embed has {}",
                p.name,
                declared,
                p.bytes.len()
            );
        }
    }

    // ── key individual spot checks ────────────────────────────────────────────

    #[test]
    fn coated_fogra39_is_embedded() {
        assert!(!COATED_FOGRA_39.bytes.is_empty());
    }

    #[test]
    fn adobe_rgb_1998_is_embedded() {
        assert!(!ADOBE_RGB_1998.bytes.is_empty());
    }

    // ── from_user_bytes ───────────────────────────────────────────────────────

    #[test]
    fn from_user_bytes_accepts_valid_icc() {
        let bytes = COATED_FOGRA_39.bytes.to_vec();
        let p = IccProfile::from_user_bytes("TestProfile".to_string(), "Test".to_string(), bytes)
            .unwrap();
        assert_eq!(p.color_space, ColorSpaceKind::Cmyk);
    }

    #[test]
    fn from_user_bytes_rejects_invalid_bytes() {
        let result = IccProfile::from_user_bytes(
            "Bad".to_string(),
            "Bad".to_string(),
            b"not an icc profile".to_vec(),
        );
        assert!(result.is_err());
    }
}
