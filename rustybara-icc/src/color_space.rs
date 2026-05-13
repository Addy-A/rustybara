#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorSpaceKind {
    Cmyk,
    Rgb,
    Gray,
    Lab,
    Unknown,
}
