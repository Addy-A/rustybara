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
