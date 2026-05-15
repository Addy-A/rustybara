use crate::ColorSpaceKind;

/// A compile-time embedded ICC color profile with typed metadata.
///
/// All bundled profiles are exposed as `pub const` values in this module
/// (e.g. [`COATED_FOGRA_39`], [`ADOBE_RGB_1998`]). Pass them to
/// [`crate::ColorTransform::new`] to build a color transform.
pub struct IccProfile {
    /// Short machine-readable identifier (e.g. `"CoatedFOGRA39"`).
    pub name: &'static str,
    /// Human-readable description (e.g. `"Coated FOGRA 39"`).
    pub description: &'static str,
    /// Color space this profile operates in.
    pub color_space: ColorSpaceKind,
    /// Raw ICC profile bytes embedded at compile time via `include_bytes!`.
    pub bytes: &'static [u8],
}

// ── CMYK ─────────────────────────────────────────────────────────────────────

pub const COATED_FOGRA_27: IccProfile = IccProfile {
    name: "CoatedFOGRA27",
    description: "Coated FOGRA 27",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/CoatedFOGRA27.icc"),
};

pub const COATED_FOGRA_39: IccProfile = IccProfile {
    name: "CoatedFOGRA39",
    description: "Coated FOGRA 39",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/CoatedFOGRA39.icc"),
};

pub const COATED_GRACOL_2006: IccProfile = IccProfile {
    name: "CoatedGRACoL2006",
    description: "Coated GRACoL 2006",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/CoatedGRACoL2006.icc"),
};

pub const JAPAN_COLOR_2001_COATED: IccProfile = IccProfile {
    name: "JapanColor2001Coated",
    description: "Japan Color 2001 Coated",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/JapanColor2001Coated.icc"),
};

pub const JAPAN_COLOR_2001_UNCOATED: IccProfile = IccProfile {
    name: "JapanColor2001Uncoated",
    description: "Japan Color 2001 Uncoated",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/JapanColor2001Uncoated.icc"),
};

pub const JAPAN_COLOR_2002_NEWSPAPER: IccProfile = IccProfile {
    name: "JapanColor2002Newspaper",
    description: "Japan Color 2002 Newspaper",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/JapanColor2002Newspaper.icc"),
};

pub const JAPAN_COLOR_2003_WEB_COATED: IccProfile = IccProfile {
    name: "JapanColor2003WebCoated",
    description: "Japan Color 2003 Web Coated",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/JapanColor2003WebCoated.icc"),
};

pub const JAPAN_WEB_COATED: IccProfile = IccProfile {
    name: "JapanWebCoated",
    description: "Japan Web Coated",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/JapanWebCoated.icc"),
};

pub const UNCOATED_FOGRA_29: IccProfile = IccProfile {
    name: "UncoatedFOGRA29",
    description: "Uncoated FOGRA 29",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/UncoatedFOGRA29.icc"),
};

pub const US_WEB_COATED_SWOP: IccProfile = IccProfile {
    name: "USWebCoatedSWOP",
    description: "US Web Coated SWOP",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/USWebCoatedSWOP.icc"),
};

pub const US_WEB_UNCOATED: IccProfile = IccProfile {
    name: "USWebUncoated",
    description: "US Web Uncoated",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/USWebUncoated.icc"),
};

pub const WEB_COATED_FOGRA_28: IccProfile = IccProfile {
    name: "WebCoatedFOGRA28",
    description: "Web Coated FOGRA 28",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/WebCoatedFOGRA28.icc"),
};

pub const WEB_COATED_SWOP_2006_GRADE3: IccProfile = IccProfile {
    name: "WebCoatedSWOP2006Grade3",
    description: "Web Coated SWOP 2006 Grade 3",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/WebCoatedSWOP2006Grade3.icc"),
};

pub const WEB_COATED_SWOP_2006_GRADE5: IccProfile = IccProfile {
    name: "WebCoatedSWOP2006Grade5",
    description: "Web Coated SWOP 2006 Grade 5",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CMYK/WebCoatedSWOP2006Grade5.icc"),
};

// ── RGB ──────────────────────────────────────────────────────────────────────

pub const ADOBE_RGB_1998: IccProfile = IccProfile {
    name: "AdobeRGB1998",
    description: "Adobe RGB (1998)",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/AdobeRGB1998.icc"),
};

pub const APPLE_RGB: IccProfile = IccProfile {
    name: "AppleRGB",
    description: "Apple RGB",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/AppleRGB.icc"),
};

pub const COLOR_MATCH_RGB: IccProfile = IccProfile {
    name: "ColorMatchRGB",
    description: "ColorMatch RGB",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/ColorMatchRGB.icc"),
};

pub const PAL_SECAM: IccProfile = IccProfile {
    name: "PAL_SECAM",
    description: "PAL/SECAM",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/PAL_SECAM.icc"),
};

pub const SMPTE_C: IccProfile = IccProfile {
    name: "SMPTE-C",
    description: "SMPTE-C",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/SMPTE-C.icc"),
};

pub const VIDEO_HD: IccProfile = IccProfile {
    name: "VideoHD",
    description: "HDTV (Rec. 709)",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/VideoHD.icc"),
};

pub const VIDEO_NTSC: IccProfile = IccProfile {
    name: "VideoNTSC",
    description: "NTSC (1953)",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/VideoNTSC.icc"),
};

pub const VIDEO_PAL: IccProfile = IccProfile {
    name: "VideoPAL",
    description: "PAL (Video)",
    color_space: ColorSpaceKind::Rgb,
    bytes: include_bytes!("data/RGB/VideoPAL.icc"),
};

// ── Lookup ────────────────────────────────────────────────────────────────────

/// Returns the profile constant for a given machine-readable name, or `None` if unknown.
pub fn by_name(name: &str) -> Option<&'static IccProfile> {
    match name {
        "CoatedFOGRA27" => Some(&COATED_FOGRA_27),
        "CoatedFOGRA39" => Some(&COATED_FOGRA_39),
        "CoatedGRACoL2006" => Some(&COATED_GRACOL_2006),
        "JapanColor2001Coated" => Some(&JAPAN_COLOR_2001_COATED),
        "JapanColor2001Uncoated" => Some(&JAPAN_COLOR_2001_UNCOATED),
        "JapanColor2002Newspaper" => Some(&JAPAN_COLOR_2002_NEWSPAPER),
        "JapanColor2003WebCoated" => Some(&JAPAN_COLOR_2003_WEB_COATED),
        "JapanWebCoated" => Some(&JAPAN_WEB_COATED),
        "UncoatedFOGRA29" => Some(&UNCOATED_FOGRA_29),
        "USWebCoatedSWOP" => Some(&US_WEB_COATED_SWOP),
        "USWebUncoated" => Some(&US_WEB_UNCOATED),
        "WebCoatedFOGRA28" => Some(&WEB_COATED_FOGRA_28),
        "WebCoatedSWOP2006Grade3" => Some(&WEB_COATED_SWOP_2006_GRADE3),
        "WebCoatedSWOP2006Grade5" => Some(&WEB_COATED_SWOP_2006_GRADE5),
        "AdobeRGB1998" => Some(&ADOBE_RGB_1998),
        "AppleRGB" => Some(&APPLE_RGB),
        "ColorMatchRGB" => Some(&COLOR_MATCH_RGB),
        "PAL_SECAM" => Some(&PAL_SECAM),
        "SMPTE-C" => Some(&SMPTE_C),
        "VideoHD" => Some(&VIDEO_HD),
        "VideoNTSC" => Some(&VIDEO_NTSC),
        "VideoPAL" => Some(&VIDEO_PAL),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorSpaceKind;

    const ALL_CMYK: &[&IccProfile] = &[
        &COATED_FOGRA_27,
        &COATED_FOGRA_39,
        &COATED_GRACOL_2006,
        &JAPAN_COLOR_2001_COATED,
        &JAPAN_COLOR_2001_UNCOATED,
        &JAPAN_COLOR_2002_NEWSPAPER,
        &JAPAN_COLOR_2003_WEB_COATED,
        &JAPAN_WEB_COATED,
        &UNCOATED_FOGRA_29,
        &US_WEB_COATED_SWOP,
        &US_WEB_UNCOATED,
        &WEB_COATED_FOGRA_28,
        &WEB_COATED_SWOP_2006_GRADE3,
        &WEB_COATED_SWOP_2006_GRADE5,
    ];

    const ALL_RGB: &[&IccProfile] = &[
        &ADOBE_RGB_1998,
        &APPLE_RGB,
        &COLOR_MATCH_RGB,
        &PAL_SECAM,
        &SMPTE_C,
        &VIDEO_HD,
        &VIDEO_NTSC,
        &VIDEO_PAL,
    ];

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
        for p in ALL_CMYK {
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
        for p in ALL_RGB {
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
        for p in ALL_CMYK {
            assert_icc_signature(p);
        }
    }

    #[test]
    fn all_rgb_profiles_have_valid_icc_signature() {
        for p in ALL_RGB {
            assert_icc_signature(p);
        }
    }

    // ── non-empty bytes ───────────────────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_have_non_empty_bytes() {
        for p in ALL_CMYK {
            assert!(!p.bytes.is_empty(), "{}: bytes is empty", p.name);
        }
    }

    #[test]
    fn all_rgb_profiles_have_non_empty_bytes() {
        for p in ALL_RGB {
            assert!(!p.bytes.is_empty(), "{}: bytes is empty", p.name);
        }
    }

    // ── name / description ────────────────────────────────────────────────────

    #[test]
    fn all_cmyk_profiles_have_non_empty_name_and_description() {
        for p in ALL_CMYK {
            assert!(!p.name.is_empty(), "a CMYK profile has an empty name");
            assert!(!p.description.is_empty(), "{}: empty description", p.name);
        }
    }

    #[test]
    fn all_rgb_profiles_have_non_empty_name_and_description() {
        for p in ALL_RGB {
            assert!(!p.name.is_empty(), "an RGB profile has an empty name");
            assert!(!p.description.is_empty(), "{}: empty description", p.name);
        }
    }

    // ── size sanity ───────────────────────────────────────────────────────────
    // A valid ICC profile is at minimum 128 bytes (header only). Anything under 1 KB
    // is almost certainly a truncated or corrupted embed.

    #[test]
    fn all_cmyk_profiles_exceed_minimum_icc_size() {
        for p in ALL_CMYK {
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
        for p in ALL_RGB {
            // Matrix-based RGB profiles are compact (AdobeRGB1998 is ~560 bytes).
            // 256 bytes is a safe floor: header (128) + a minimal tag table.
            assert!(
                p.bytes.len() >= 256,
                "{}: suspiciously small ({} bytes) — likely a broken embed",
                p.name,
                p.bytes.len(),
            );
        }
    }

    // ── ICC header size field ─────────────────────────────────────────────────
    // Bytes 0–3 of every ICC profile are a big-endian u32 declaring the total file size.
    // A mismatch means the file was truncated or the wrong file was embedded.

    #[test]
    fn all_cmyk_profiles_size_matches_icc_header_declaration() {
        for p in ALL_CMYK {
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
        for p in ALL_RGB {
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
    // Redundant with the array tests above but give a clear, named signal for the
    // most commonly-used profiles when something goes wrong.

    #[test]
    fn coated_fogra39_is_embedded() {
        assert!(!COATED_FOGRA_39.bytes.is_empty());
    }

    #[test]
    fn adobe_rgb_1998_is_embedded() {
        assert!(!ADOBE_RGB_1998.bytes.is_empty());
    }
}
