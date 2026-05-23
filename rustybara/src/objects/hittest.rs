//! Spatial hit-testing for the page object tree.
//!
//! All coordinates are in **PDF page space** (origin bottom-left, Y-up, units in points).

use crate::geometry::Matrix;
use crate::objects::tree::{ObjectKind, ObjectTree, PageObject, PathPoint, SubPath};

/// Returns references to every object whose geometry contains the point `(x, y)`,
/// in back-to-front paint order.
///
/// Reverse the returned slice for visual front-to-back (topmost first) ordering.
///
/// # Precision notes
///
/// - Filled paths use the **nonzero winding rule** on the polygon formed by their
/// vertices (Bézier curves are reduced to their endpoints – sufficient for prepress object picking).
/// - Stroked paths, text blocks, images, and form XObjects use bounding-box containment
/// only; stroke half-width is not modelled.
pub fn hit_test(tree: &ObjectTree, x: f64, y: f64) -> Vec<&PageObject> {
    tree.objects
        .iter()
        .filter(|obj| obj.bbox.contains_point(x, y) && object_hit(obj, x, y))
        .collect()
}

fn object_hit(obj: &PageObject, x: f64, y: f64) -> bool {
    match &obj.kind {
        ObjectKind::Fill | ObjectKind::FillStroke => {
            point_in_path_nonzero(&obj.subpaths, x, y, &obj.ctm)
        }
        ObjectKind::Stroke | ObjectKind::Text(_) | ObjectKind::Image | ObjectKind::FormXObject => {
            true
        }
    }
}

/// Returns `true` if `(x, y)` is inside the path under the **nonzero winding rule**.
///
/// `subpaths` are in *local* (pre-CTM) coordinates; `ctm` maps them to page space.
///
/// Bézier curves are represented by their endpoint only (conservative approximation).
/// For exact geometric containment of curved shapes, the caller should flatten the curve
/// before calling this function.
pub fn point_in_path_nonzero(subpaths: &[SubPath], x: f64, y: f64, ctm: &Matrix) -> bool {
    let mut winding = 0i32;

    for sub in subpaths {
        let pts: Vec<(f64, f64)> = sub
            .points
            .iter()
            .filter_map(|p| match *p {
                PathPoint::MoveTo(lx, ly) | PathPoint::LineTo(lx, ly) => {
                    Some(ctm.transform_point(lx, ly))
                }
                PathPoint::CurveTo(_, _, _, _, lx, ly) => Some(ctm.transform_point(lx, ly)),
                PathPoint::Close => None,
            })
            .collect();

        let n = pts.len();
        if n < 2 {
            continue;
        }

        // Standard ray-casting winding number: casta ray toward +x from (x, y).
        for i in 0..n {
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[(i + 1) % n];
            if y0 <= y {
                if y1 > y && cross2d(x0, y0, x1, y1, x, y) > 0.0 {
                    winding += 1;
                }
            } else if y1 <= y && cross2d(x0, y0, x1, y1, x, y) < 0.0 {
                winding -= 1;
            }
        }
    }

    winding != 0
}

/// 2-D cross product of vectors `(P1 - P0)` and `(Q - P0)`.
///
/// Positive when `Q` is strictly to the left of the directed edge `P0 -> P1`.
#[inline]
fn cross2d(x0: f64, y0: f64, x1: f64, y1: f64, qx: f64, qy: f64) -> f64 {
    (x1 - x0) * (qy - y0) - (y1 - y0) * (qx - x0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Matrix, Rect};
    use crate::objects::tree::{ObjectKind, ObjectTree, PageObject, PathPoint, PdfColor, SubPath};

    fn rect_fill_object(x: f64, y: f64, w: f64, h: f64) -> PageObject {
        let mut sub = SubPath::default();
        sub.points.push(PathPoint::MoveTo(x, y));
        sub.points.push(PathPoint::LineTo(x + w, y));
        sub.points.push(PathPoint::LineTo(x + w, y + h));
        sub.points.push(PathPoint::LineTo(x, y + h));
        sub.points.push(PathPoint::Close);
        PageObject {
            bbox: Rect {
                x,
                y,
                width: w,
                height: h,
            },
            ctm: Matrix::identity(),
            kind: ObjectKind::Fill,
            fill_color: Some(PdfColor::DeviceGray(0.0)),
            stroke_color: None,
            stroke_width: 0.0,
            subpaths: vec![sub],
        }
    }

    // ── cross2d ───────────────────────────────────────────────────────────────

    #[test]
    fn cross2d_left_of_edge_is_positive() {
        // Edge (0,0) → (1,0); point (0.5, 1.0) is above — positive cross product.
        assert!(cross2d(0.0, 0.0, 1.0, 0.0, 0.5, 1.0) > 0.0);
    }

    #[test]
    fn cross2d_right_of_edge_is_negative() {
        // Edge (0,0) → (1,0); point (0.5, -1.0) is below — negative cross product.
        assert!(cross2d(0.0, 0.0, 1.0, 0.0, 0.5, -1.0) < 0.0);
    }

    // ── point_in_path_nonzero ─────────────────────────────────────────────────

    #[test]
    fn inside_unit_square() {
        let mut sub = SubPath::default();
        sub.points.push(PathPoint::MoveTo(0.0, 0.0));
        sub.points.push(PathPoint::LineTo(1.0, 0.0));
        sub.points.push(PathPoint::LineTo(1.0, 1.0));
        sub.points.push(PathPoint::LineTo(0.0, 1.0));
        sub.points.push(PathPoint::Close);
        assert!(point_in_path_nonzero(&[sub], 0.5, 0.5, &Matrix::identity()));
    }

    #[test]
    fn outside_unit_square() {
        let mut sub = SubPath::default();
        sub.points.push(PathPoint::MoveTo(0.0, 0.0));
        sub.points.push(PathPoint::LineTo(1.0, 0.0));
        sub.points.push(PathPoint::LineTo(1.0, 1.0));
        sub.points.push(PathPoint::LineTo(0.0, 1.0));
        sub.points.push(PathPoint::Close);
        assert!(!point_in_path_nonzero(
            &[sub],
            2.0,
            2.0,
            &Matrix::identity()
        ));
    }

    #[test]
    fn ctm_translation_shifts_hit_region() {
        let mut sub = SubPath::default();

        sub.points.push(PathPoint::MoveTo(0.0, 0.0));
        sub.points.push(PathPoint::LineTo(10.0, 0.0));
        sub.points.push(PathPoint::LineTo(10.0, 10.0));
        sub.points.push(PathPoint::LineTo(0.0, 10.0));
        sub.points.push(PathPoint::Close);
        let ctm = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 100.0, 100.0);
        // local (5,5) maps to page (105,105) — hit
        assert!(point_in_path_nonzero(&[sub.clone()], 105.0, 105.0, &ctm));
        // page (5,5) is outside the translated region — miss
        assert!(!point_in_path_nonzero(&[sub], 5.0, 5.0, &ctm));
    }

    #[test]
    fn empty_subpaths_never_hit() {
        assert!(!point_in_path_nonzero(&[], 0.5, 0.5, &Matrix::identity()));
    }

    #[test]
    fn single_point_subpath_never_hit() {
        let mut sub = SubPath::default();
        sub.points.push(PathPoint::MoveTo(5.0, 5.0));
        assert!(!point_in_path_nonzero(
            &[sub],
            5.0,
            5.0,
            &Matrix::identity()
        ));
    }

    #[test]
    fn donut_hole_outside_inner_path() {
        // Outer square [0,100]², inner square [40,60]².
        // Point (50,50) is inside outer but inside the "hole" — nonzero rule
        // still counts it as inside because both subpaths wind the same direction.
        // This test just verifies the winding counter works with multiple subpaths.
        let mut outer = SubPath::default();
        outer.points.push(PathPoint::MoveTo(0.0, 0.0));
        outer.points.push(PathPoint::LineTo(100.0, 0.0));
        outer.points.push(PathPoint::LineTo(100.0, 100.0));
        outer.points.push(PathPoint::LineTo(0.0, 100.0));
        outer.points.push(PathPoint::Close);

        let mut inner = SubPath::default();
        inner.points.push(PathPoint::MoveTo(40.0, 40.0));
        inner.points.push(PathPoint::LineTo(60.0, 40.0));
        inner.points.push(PathPoint::LineTo(60.0, 60.0));
        inner.points.push(PathPoint::LineTo(40.0, 60.0));
        inner.points.push(PathPoint::Close);

        // Both same winding → nonzero rule: point inside outer is a hit
        let result = point_in_path_nonzero(&[outer, inner], 50.0, 50.0, &Matrix::identity());
        // Winding ≠ 0 from outer square; inner adds more — result is nonzero → true
        assert!(result);
    }

    // ── hit_test ─────────────────────────────────────────────────────────────

    #[test]
    fn hit_test_miss_returns_empty() {
        let tree = ObjectTree {
            objects: vec![rect_fill_object(0.0, 0.0, 100.0, 100.0)],
        };
        assert!(hit_test(&tree, 200.0, 200.0).is_empty());
    }

    #[test]
    fn hit_test_single_object_hit() {
        let tree = ObjectTree {
            objects: vec![rect_fill_object(0.0, 0.0, 100.0, 100.0)],
        };
        let hits = hit_test(&tree, 50.0, 50.0);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn hit_test_two_overlapping_objects_both_hit() {
        let tree = ObjectTree {
            objects: vec![
                rect_fill_object(0.0, 0.0, 200.0, 200.0),
                rect_fill_object(50.0, 50.0, 100.0, 100.0),
            ],
        };
        let hits = hit_test(&tree, 100.0, 100.0);
        assert_eq!(hits.len(), 2, "point inside both rects should hit both");
    }

    #[test]
    fn hit_test_preserves_paint_order() {
        let back = rect_fill_object(0.0, 0.0, 100.0, 100.0);
        let front = rect_fill_object(10.0, 10.0, 80.0, 80.0);
        let tree = ObjectTree {
            objects: vec![back, front],
        };
        let hits = hit_test(&tree, 50.0, 50.0);
        assert_eq!(hits.len(), 2);
        // Back object painted first → first in results (index 0)
        assert!(
            (hits[0].bbox.x - 0.0).abs() < 0.01,
            "first hit should be the back object"
        );
        assert!(
            (hits[1].bbox.x - 10.0).abs() < 0.01,
            "second hit should be the front object"
        );
    }

    #[test]
    fn hit_test_empty_tree() {
        let tree = ObjectTree { objects: vec![] };
        assert!(hit_test(&tree, 50.0, 50.0).is_empty());
    }

    #[test]
    fn hit_test_stroke_object_uses_bbox_only() {
        // A stroke object with an empty subpath — bbox containment is enough.
        let obj = PageObject {
            bbox: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            ctm: Matrix::identity(),
            kind: ObjectKind::Stroke,
            fill_color: None,
            stroke_color: Some(PdfColor::DeviceGray(0.0)),
            stroke_width: 2.0,
            subpaths: vec![],
        };
        let tree = ObjectTree { objects: vec![obj] };
        assert_eq!(hit_test(&tree, 50.0, 50.0).len(), 1);
    }

    #[test]
    fn hit_test_image_object_uses_bbox_only() {
        let obj = PageObject {
            bbox: Rect {
                x: 10.0,
                y: 10.0,
                width: 200.0,
                height: 150.0,
            },
            ctm: Matrix::identity(),
            kind: ObjectKind::Image,
            fill_color: None,
            stroke_color: None,
            stroke_width: 0.0,
            subpaths: vec![],
        };
        let tree = ObjectTree { objects: vec![obj] };
        assert_eq!(hit_test(&tree, 100.0, 80.0).len(), 1);
        assert!(hit_test(&tree, 5.0, 5.0).is_empty());
    }
}
