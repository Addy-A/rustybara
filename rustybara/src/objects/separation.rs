//! Separation / plate filtering for the object tree.
//!
//! Given an [`ObjectTree`] and an [`InkSelector`], [`filter_by_ink`] returns
//! references to every [`PageObject`] that carries ink on the selected plate.
//! This is the building block for plate-preview overlays in `rbv` and for
//! pre-flight ink-coverage reports.
//!
//! # Ink selection
//!
//! | Selector | Matches |
//! |---|---|
//! | [`InkSelector::Separation`] | Objects whose fill **or** stroke color is [`PdfColor::Separation`] with a matching name (case-sensitive) |
//! | [`InkSelector::CmykChannel`] | Objects whose fill **or** stroke color has a non-zero value in the chosen [`CmykChannel`] |
//!
//! Objects with `None` color (images, form XObject placeholders) are never matched.

use crate::objects::tree::{CmykChannel, ObjectTree, PageObject, PdfColor};

/// Selects which inkplate to isolate.
#[derive(Clone, Debug, PartialEq)]
pub enum InkSelector {
    Separation(String),
    CmykChannel(CmykChannel),
}

/// Return references to every object in `tree` that carries ink on `selector`,
/// in back-to-front paint order (same order as [`ObjectTree::objects`]).
///
/// Both fill and stroke color are checked; an object is included if either
/// carries the selected ink. Objects with no color information (images, form
/// XObject placeholders) are always excluded
pub fn filter_by_ink<'a>(tree: &'a ObjectTree, selector: &InkSelector) -> Vec<&'a PageObject> {
    tree.objects
        .iter()
        .filter(|obj| {
            color_matches(obj.fill_color.as_ref(), selector)
                || color_matches(obj.stroke_color.as_ref(), selector)
        })
        .collect()
}

/// Returns `true` if `color` carries ink on `selector`.
fn color_matches(color: Option<&PdfColor>, selector: &InkSelector) -> bool {
    let Some(color) = color else { return false };
    match selector {
        InkSelector::Separation(target) => {
            matches!(color, PdfColor::Separation { name, .. } if name == target)
        }
        InkSelector::CmykChannel(ch) => channel_nonzero(color, ch),
    }
}

/// Returns `true` if `color` has a non-zero value in `channel`.
///
/// - For [`PdfColor::DeviceCmyk`]: checks the corresponding component directly.
/// - For [`PdfColor::Separation`]: a spot color is considered to carry **all**
///   process channels at full tint (a conservative assumption for ink-coverage
///   purposes — the actual appearance depends on the alternate colorspace).
/// - For [`PdfColor::DeviceGray`] and [`PdfColor::DeviceRgb`]: never matches a
///   CMYK channel selector (the caller should use a Separation selector instead).
fn channel_nonzero(color: &PdfColor, channel: &CmykChannel) -> bool {
    match color {
        PdfColor::DeviceCmyk(c, m, y, k) => {
            let v = match channel {
                CmykChannel::Cyan => *c,
                CmykChannel::Magenta => *m,
                CmykChannel::Yellow => *y,
                CmykChannel::Black => *k,
            };
            v > 0.0
        }
        PdfColor::Separation { tint, .. } => *tint > 0.0,
        PdfColor::DeviceGray(_) | PdfColor::DeviceRgb(..) => false,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Matrix, Rect};
    use crate::objects::tree::{
        CmykChannel, ObjectKind, ObjectTree, OverprintState, PageObject, PdfColor,
    };

    fn cmyk_fill_object(c: f64, m: f64, y: f64, k: f64) -> PageObject {
        PageObject {
            bbox: Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
            ctm: Matrix::identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::DeviceCmyk(c, m, y, k)),
            stroke_color: None,
            stroke_width: 0.0,
            overprint: OverprintState::default(),
            subpaths: vec![],
        }
    }

    fn spot_fill_object(name: &str, tint: f64) -> PageObject {
        PageObject {
            bbox: Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
            ctm: Matrix::identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::Separation { name: name.to_string(), tint }),
            stroke_color: None,
            stroke_width: 0.0,
            overprint: OverprintState::default(),
            subpaths: vec![],
        }
    }

    fn image_object() -> PageObject {
        PageObject {
            bbox: Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
            ctm: Matrix::identity(),
            kind: ObjectKind::Image,
            fill_color: None,
            stroke_color: None,
            stroke_width: 0.0,
            overprint: OverprintState::default(),
            subpaths: vec![],
        }
    }

    #[test]
    fn filter_by_separation_name_matches_exact() {
        let tree = ObjectTree {
            objects: vec![
                spot_fill_object("PANTONE 485 C", 1.0),
                spot_fill_object("PANTONE Cool Gray 9 C", 1.0),
            ],
        };
        let hits = filter_by_ink(&tree, &InkSelector::Separation("PANTONE 485 C".to_string()));
        assert_eq!(hits.len(), 1);
        assert!(matches!(
            &hits[0].fill_color,
            Some(PdfColor::Separation { name, .. }) if name == "PANTONE 485 C"
        ));
    }

    #[test]
    fn filter_by_separation_zero_tint_still_matches() {
        // A 0% tint is still a valid use of the ink channel; it should be included.
        let tree = ObjectTree {
            objects: vec![spot_fill_object("Die Cut", 0.0)],
        };
        let hits = filter_by_ink(&tree, &InkSelector::Separation("Die Cut".to_string()));
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn filter_by_cmyk_channel_cyan() {
        let tree = ObjectTree {
            objects: vec![
                cmyk_fill_object(1.0, 0.0, 0.0, 0.0), // has cyan
                cmyk_fill_object(0.0, 1.0, 0.0, 0.0), // no cyan
            ],
        };
        let hits = filter_by_ink(&tree, &InkSelector::CmykChannel(CmykChannel::Cyan));
        assert_eq!(hits.len(), 1);
        assert!(matches!(
            hits[0].fill_color,
            Some(PdfColor::DeviceCmyk(c, ..)) if c > 0.0
        ));
    }

    #[test]
    fn filter_by_cmyk_channel_excludes_rgb() {
        let tree = ObjectTree {
            objects: vec![PageObject {
                bbox: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                ctm: Matrix::identity(),
                kind: ObjectKind::Fill,
                fill_color: Some(PdfColor::DeviceRgb(1.0, 0.0, 0.0)),
                stroke_color: None,
                stroke_width: 0.0,
                overprint: OverprintState::default(),
                subpaths: vec![],
            }],
        };
        let hits = filter_by_ink(&tree, &InkSelector::CmykChannel(CmykChannel::Cyan));
        assert!(
            hits.is_empty(),
            "RGB objects must not match CMYK channel selectors"
        );
    }

    #[test]
    fn filter_excludes_images_with_no_color() {
        let tree = ObjectTree {
            objects: vec![image_object()],
        };
        let hits = filter_by_ink(&tree, &InkSelector::Separation("Any".to_string()));
        assert!(hits.is_empty());
    }

    #[test]
    fn filter_checks_stroke_color_too() {
        let obj = PageObject {
            bbox: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            ctm: Matrix::identity(),
            kind: ObjectKind::Stroke,
            fill_color: None,
            stroke_color: Some(PdfColor::Separation {
                name: "Die Cut".to_string(),
                tint: 1.0,
            }),
            stroke_width: 1.0,
            overprint: OverprintState::default(),
            subpaths: vec![],
        };
        let tree = ObjectTree { objects: vec![obj] };
        let hits = filter_by_ink(&tree, &InkSelector::Separation("Die Cut".to_string()));
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn filter_paint_order_preserved() {
        let tree = ObjectTree {
            objects: vec![
                cmyk_fill_object(1.0, 0.0, 0.0, 0.0),
                image_object(),
                cmyk_fill_object(0.5, 0.0, 0.0, 0.0),
            ],
        };
        let hits = filter_by_ink(&tree, &InkSelector::CmykChannel(CmykChannel::Cyan));
        assert_eq!(hits.len(), 2);
        // First match should be the back object (index 0 in tree)
        assert!(
            matches!(hits[0].fill_color, Some(PdfColor::DeviceCmyk(c, ..)) if (c - 1.0).abs() < 1e-10)
        );
        assert!(
            matches!(hits[1].fill_color, Some(PdfColor::DeviceCmyk(c, ..)) if (c - 0.5).abs() < 1e-10)
        );
    }
}
