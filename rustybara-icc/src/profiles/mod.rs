use crate::ColorSpaceKind;

pub struct IccProfile {
    pub name: &'static str,
    pub description: &'static str,
    pub color_space: ColorSpaceKind,
    pub bytes: &'static [u8],
}

pub const COATED_FOGRA_27: IccProfile = IccProfile {
    name: "CoatedFOGRA27",
    description: "Coated FOGRA 27",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CoatedFOGRA27.icc"),
};

pub const COATED_FOGRA_39: IccProfile = IccProfile {
    name: "CoatedFOGRA39",
    description: "Coated FOGRA 39",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CoatedFOGRA39.icc"),
};

pub const COATED_GRACOL_2006: IccProfile = IccProfile {
    name: "CoatedGRACoL2006",
    description: "Coated GRACoL 2006",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/CoatedGRACoL2006.icc"),
};

pub const UNCOATED_FOGRA_29: IccProfile = IccProfile {
    name: "UncoatedFOGRA29",
    description: "Uncoated FOGRA 29",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/UncoatedFOGRA29.icc"),
};

pub const US_WEB_COATED_SWOP: IccProfile = IccProfile {
    name: "USWebCoatedSWOP",
    description: "US Web Coated SWOP",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/USWebCoatedSWOP.icc"),
};

pub const US_WEB_UNCOATED: IccProfile = IccProfile {
    name: "USWebUncoated",
    description: "US Web Uncoated",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/USWebUncoated.icc"),
};

pub const WEB_COATED_FOGRA_28: IccProfile = IccProfile {
    name: "WebCoatedFOGRA28",
    description: "Web Coated FOGRA 28",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/WebCoatedFOGRA28.icc"),
};

pub const WEB_COATED_SWOP_2006_GRADE3: IccProfile = IccProfile {
    name: "WebCoatedSWOP2006Grade3",
    description: "Web Coated SWOP 2006 Grade 3",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/WebCoatedSWOP2006Grade3.icc"),
};

pub const WEB_COATED_SWOP_2006_GRADE5: IccProfile = IccProfile {
    name: "WebCoatedSWOP2006Grade5",
    description: "Web Coated SWOP 2006 Grade 5",
    color_space: ColorSpaceKind::Cmyk,
    bytes: include_bytes!("data/WebCoatedSWOP2006Grade5.icc"),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorSpaceKind;

    fn assert_icc_signature(profile: &IccProfile) {
        assert!(
            profile.bytes.len() >= 40,
            "{}: profile too short to contain an ICC header ({} bytes)",
            profile.name,
            profile.bytes.len()
        );
        assert_eq!(
            &profile.bytes[36..40],
            b"acsp",
            "{}: bytes[36..40] are not the ICC signature 'ascp'",
            profile.name,
        );
    }

    // -- non-zero bytes --

    #[test]
    fn coated_fogra27_has_bytes() {
        assert!(!COATED_FOGRA_27.bytes.is_empty());
    }

    #[test]
    fn coated_fogra_39_has_bytes() {
        assert!(!COATED_FOGRA_39.bytes.is_empty());
    }

    #[test]
    fn coated_gracol2006_has_bytes() {
        assert!(!COATED_GRACOL_2006.bytes.is_empty());
    }

    #[test]
    fn uncoated_fogra29_has_bytes() {
        assert!(!UNCOATED_FOGRA_29.bytes.is_empty());
    }

    #[test]
    fn us_web_coatedswop_has_bytes() {
        assert!(!US_WEB_COATED_SWOP.bytes.is_empty());
    }

    #[test]
    fn us_webuncoated_has_bytes() {
        assert!(!US_WEB_UNCOATED.bytes.is_empty());
    }

    #[test]
    fn web_coatedfogra28_has_bytes() {
        assert!(!WEB_COATED_FOGRA_28.bytes.is_empty());
    }

    #[test]
    fn web_coatedswop2006_grade3_has_bytes() {
        assert!(!WEB_COATED_SWOP_2006_GRADE3.bytes.is_empty());
    }

    #[test]
    fn web_coatedswop2006_grade5_has_bytes() {
        assert!(!WEB_COATED_SWOP_2006_GRADE5.bytes.is_empty());
    }

    // -- color_space field correctness --
    #[test]
    fn all_cmyk_profiles_report_cmyk() {
        let cmyk_profiles = [
            &COATED_FOGRA_27,
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            &UNCOATED_FOGRA_29,
            &US_WEB_COATED_SWOP,
            &US_WEB_UNCOATED,
            &WEB_COATED_FOGRA_28,
            &WEB_COATED_SWOP_2006_GRADE3,
            &WEB_COATED_SWOP_2006_GRADE5,
        ];
        for p in &cmyk_profiles {
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
    fn coated_fogra27_icc_signature() {
        assert_icc_signature(&COATED_FOGRA_27);
    }

    #[test]
    fn coated_fogra39_icc_signature() {
        assert_icc_signature(&COATED_FOGRA_39);
    }

    #[test]
    fn coated_gracol2006_icc_signatures() {
        assert_icc_signature(&COATED_GRACOL_2006);
    }

    #[test]
    fn uncoated_fogra29_icc_signatures() {
        assert_icc_signature(&UNCOATED_FOGRA_29);
    }

    #[test]
    fn us_web_coatedswop_icc_signatures() {
        assert_icc_signature(&US_WEB_COATED_SWOP);
    }

    #[test]
    fn us_webuncoated_icc_signatures() {
        assert_icc_signature(&US_WEB_UNCOATED);
    }

    #[test]
    fn web_coatedswop2006_grade3_icc_signatures() {
        assert_icc_signature(&WEB_COATED_SWOP_2006_GRADE3);
    }

    #[test]
    fn web_coatedswop2006_grade5_icc_signatures() {
        assert_icc_signature(&WEB_COATED_SWOP_2006_GRADE5);
    }

    #[test]
    fn all_profiles_have_non_empty_name() {
        let all: &[&IccProfile] = &[
            &COATED_FOGRA_27,
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            &UNCOATED_FOGRA_29,
            &US_WEB_COATED_SWOP,
            &US_WEB_UNCOATED,
            &WEB_COATED_SWOP_2006_GRADE3,
            &WEB_COATED_SWOP_2006_GRADE5,
        ];
        for p in all {
            assert!(!p.name.is_empty(), "profile has empty name field");
            assert!(!p.description.is_empty(), "{}: empty description", p.name);
        }
    }

    // --- profile size sanity --
    // Adobe CMYK profiles are hundreds of KB. A valid ICC profile is at minimum 128 bytes (just
    // the header). Anything under 1 KB is almost certainly a truncated or corrupted embed.

    #[test]
    fn all_profiles_exceed_minimum_icc_size() {
        let all: &[(&str, &[u8])] = &[
            (COATED_FOGRA_27.name, COATED_FOGRA_27.bytes),
            (COATED_FOGRA_39.name, COATED_FOGRA_39.bytes),
            (COATED_GRACOL_2006.name, COATED_GRACOL_2006.bytes),
            (UNCOATED_FOGRA_29.name, UNCOATED_FOGRA_29.bytes),
            (US_WEB_COATED_SWOP.name, US_WEB_COATED_SWOP.bytes),
            (WEB_COATED_FOGRA_28.name, WEB_COATED_FOGRA_28.bytes),
            (
                WEB_COATED_SWOP_2006_GRADE3.name,
                WEB_COATED_SWOP_2006_GRADE3.bytes,
            ),
            (
                WEB_COATED_SWOP_2006_GRADE5.name,
                WEB_COATED_SWOP_2006_GRADE5.bytes,
            ),
        ];
        for (name, bytes) in all {
            assert!(
                bytes.len() >= 1024,
                "{}: suspiciously small ({} bytes) - likely a broken embed",
                name,
                bytes.len(),
            );
        }
    }

    #[test]
    fn all_profiles_size_matches_icc_header_declaration() {
        let all: &[&IccProfile] = &[
            &COATED_FOGRA_27,
            &COATED_FOGRA_39,
            &COATED_GRACOL_2006,
            &UNCOATED_FOGRA_29,
            &US_WEB_COATED_SWOP,
            &US_WEB_UNCOATED,
            &WEB_COATED_SWOP_2006_GRADE3,
            &WEB_COATED_SWOP_2006_GRADE5,
        ];
        for p in all {
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
}
