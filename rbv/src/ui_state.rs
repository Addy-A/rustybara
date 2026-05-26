//! Viewer UI state types — plate selection and document spot name extraction.

use rustybara::objects::{CmykChannel, InkSelector, ObjectTree, PdfColor};
use std::collections::BTreeSet;

/// Controls which ink plate the viewer isolates.
///
/// Mirrors [`InkSelector`] but adds [`PlateMode::All`] so the panel can
/// represent "no filter" without wrapping `InkSelector` in an `Option`.
#[derive(Clone, PartialEq, Default)]
pub enum PlateMode {
    #[default]
    All,
    Cmyk(CmykChannel),
    Spot(String),
}

impl PlateMode {
    /// Convert to an [`InkSelector`] for use with [`filter_by_ink`].
    /// Returns `None` when [`PlateMode::All`] is active (no filtering).
    pub fn to_ink_selector(&self) -> Option<InkSelector> {
        match self {
            PlateMode::All => None,
            PlateMode::Cmyk(ch) => Some(InkSelector::CmykChannel(*ch)),
            PlateMode::Spot(name) => Some(InkSelector::Separation(name.clone())),
        }
    }
}

/// Collect unique spot-color names from the object tree, sorted alphabetically.
///
/// Checks both fill and stroke colors on every [`PageObject`]. Uses a
/// [`BTreeSet`] so the result is always stable and deduplicated.
pub fn extract_spot_names(tree: &ObjectTree) -> Vec<String> {
    let mut names = BTreeSet::new();
    for obj in &tree.objects {
        for color in [obj.fill_color.as_ref(), obj.stroke_color.as_ref()]
            .into_iter()
            .flatten()
        {
            if let PdfColor::Separation { name, .. } = color {
                names.insert(name.clone());
            }
        }
    }
    names.into_iter().collect()
}
