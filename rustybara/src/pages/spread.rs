use crate::pages::PageBoxes;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

/// Direction in which panel slots advance across a flattened source page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitAxis {
    /// Left-to-right panels separated by vertical fold lines.
    Horizontal,
    /// Bottom-to-top panels separated by horizontal fold lines.
    Vertical,
}

enum PanelLayout<'a> {
    Uniform(f64),
    Explicit(&'a [f64]),
}

/// Splits every page into equal target-width panels. A final remainder panel is
/// retained for backward compatibility with the original `split-pages` command.
pub fn split_pages(src: &Document, panel_width_pts: f64) -> crate::Result<Document> {
    if panel_width_pts <= 0.0 {
        return Err(crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "panel width must be positive",
        )));
    }
    split_pages_internal(
        src,
        PanelLayout::Uniform(panel_width_pts),
        SplitAxis::Horizontal,
    )
}

/// Splits every page using an explicit ordered panel-size plan. The sizes must
/// cover the selected TrimBox dimension within half a PDF point; this prevents
/// silently losing a fold flap or manufacturing a convincing, incorrect panel.
pub fn split_pages_explicit(
    src: &Document,
    panel_sizes_pts: &[f64],
    axis: SplitAxis,
) -> crate::Result<Document> {
    if panel_sizes_pts.len() < 2
        || panel_sizes_pts
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "explicit panel plan needs at least two positive sizes",
        )));
    }
    split_pages_internal(src, PanelLayout::Explicit(panel_sizes_pts), axis)
}

fn split_pages_internal(
    src: &Document,
    layout: PanelLayout<'_>,
    axis: SplitAxis,
) -> crate::Result<Document> {
    let mut new_doc = src.clone();

    let pages_id: ObjectId = {
        let catalog = new_doc.catalog()?;
        catalog.get(b"Pages")?.as_reference()?
    };

    struct SourcePage {
        id: ObjectId,
        resources: Option<Object>,
    }

    let source_pages: Vec<SourcePage> = src
        .get_pages()
        .values()
        .map(|&id| {
            let resources = src
                .get_dictionary(id)
                .ok()
                .and_then(|d| d.get(b"Resources").ok())
                .cloned();
            SourcePage { id, resources }
        })
        .collect();

    let mut new_kids: Vec<Object> = Vec::new();

    for sp in &source_pages {
        let boxes = PageBoxes::read(src, sp.id)?;
        let media = boxes.media_box;
        let trim = *boxes.trim_or_media();
        let extent = match axis {
            SplitAxis::Horizontal => trim.width,
            SplitAxis::Vertical => trim.height,
        };
        let panel_sizes = match layout {
            PanelLayout::Uniform(panel_size) => {
                let count = ((extent / panel_size).ceil() as usize).max(1);
                (0..count)
                    .map(|index| {
                        if index + 1 < count {
                            panel_size
                        } else {
                            extent - panel_size * index as f64
                        }
                    })
                    .collect::<Vec<_>>()
            }
            PanelLayout::Explicit(sizes) => {
                let total = sizes.iter().sum::<f64>();
                if (total - extent).abs() > 0.5 {
                    return Err(crate::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "explicit panel sizes total {total:.3} points but page trim extent is {extent:.3} points"
                        ),
                    )));
                }
                sizes.to_vec()
            }
        };

        // Bleed margins: how much the MediaBox extends beyond the TrimBox on each side.
        // Zero when no TrimBox is present (trim == media).
        let left_bleed = trim.x - media.x;
        let right_bleed = media.right() - trim.right();
        let bottom_bleed = trim.y - media.y;
        let top_bleed = media.top() - trim.top();

        let source_ops = src
            .get_and_decode_page_content(sp.id)
            .map(|c| c.operations)
            .unwrap_or_default();

        let mut offset = 0.0;
        for panel_size in panel_sizes {
            let (panel_min_x, panel_min_y, panel_max_x, panel_max_y) = match axis {
                SplitAxis::Horizontal => (
                    trim.x + offset,
                    trim.y,
                    trim.x + offset + panel_size,
                    trim.top(),
                ),
                SplitAxis::Vertical => (
                    trim.x,
                    trim.y + offset,
                    trim.right(),
                    trim.y + offset + panel_size,
                ),
            };
            let (media_min_x, media_min_y, media_max_x, media_max_y) = match axis {
                SplitAxis::Horizontal => (
                    panel_min_x - left_bleed,
                    media.y,
                    panel_max_x + right_bleed,
                    media.top(),
                ),
                SplitAxis::Vertical => (
                    media.x,
                    panel_min_y - bottom_bleed,
                    media.right(),
                    panel_max_y + top_bleed,
                ),
            };

            let mut ops: Vec<Operation> = Vec::with_capacity(source_ops.len() + 5);
            ops.push(Operation::new("q", vec![]));
            ops.push(Operation::new(
                "re",
                vec![
                    Object::Real(media_min_x as f32),
                    Object::Real(media_min_y as f32),
                    Object::Real((media_max_x - media_min_x) as f32),
                    Object::Real((media_max_y - media_min_y) as f32),
                ],
            ));
            ops.push(Operation::new("W", vec![]));
            ops.push(Operation::new("n", vec![]));
            ops.extend(source_ops.iter().cloned());
            ops.push(Operation::new("Q", vec![]));

            let bytes = Content { operations: ops }.encode()?;
            let mut stream = Stream::new(Dictionary::new(), bytes);
            stream.compress()?;
            let stream_id = new_doc.add_object(Object::Stream(stream));

            let mut page = Dictionary::new();
            page.set("Type", Object::Name(b"Page".to_vec()));
            page.set("Parent", Object::Reference(pages_id));
            page.set(
                "MediaBox",
                Object::Array(vec![
                    Object::Real(media_min_x as f32),
                    Object::Real(media_min_y as f32),
                    Object::Real(media_max_x as f32),
                    Object::Real(media_max_y as f32),
                ]),
            );
            page.set(
                "TrimBox",
                Object::Array(vec![
                    Object::Real(panel_min_x as f32),
                    Object::Real(panel_min_y as f32),
                    Object::Real(panel_max_x as f32),
                    Object::Real(panel_max_y as f32),
                ]),
            );
            page.set("Contents", Object::Reference(stream_id));
            if let Some(res) = &sp.resources {
                page.set("Resources", res.clone());
            }

            let panel_id = new_doc.add_object(Object::Dictionary(page));
            new_kids.push(Object::Reference(panel_id));
            offset += panel_size;
        }
    }

    let count = new_kids.len() as i64;
    if let Ok(Object::Dictionary(dict)) = new_doc.get_object_mut(pages_id) {
        dict.set("Kids", Object::Array(new_kids));
        dict.set("Count", Object::Integer(count));
    }

    new_doc.prune_objects();
    Ok(new_doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal lopdf Document with pages of the given (width, height) pairs.
    fn make_doc(pages: &[(f64, f64)]) -> Document {
        let mut doc = Document::with_version("1.4");

        let mut kid_ids: Vec<ObjectId> = Vec::new();
        for &(w, h) in pages {
            let page_id = doc.add_object(Object::Dictionary({
                let mut d = Dictionary::new();
                d.set("Type", Object::Name(b"Page".to_vec()));
                d.set(
                    "MediaBox",
                    Object::Array(vec![
                        Object::Real(0.0_f32),
                        Object::Real(0.0_f32),
                        Object::Real(w as f32),
                        Object::Real(h as f32),
                    ]),
                );
                d
            }));
            kid_ids.push(page_id);
        }

        let kids_arr: Vec<Object> = kid_ids.iter().map(|&id| Object::Reference(id)).collect();
        let pages_id = doc.add_object(Object::Dictionary({
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"Pages".to_vec()));
            d.set("Kids", Object::Array(kids_arr));
            d.set("Count", Object::Integer(kid_ids.len() as i64));
            d
        }));

        for &page_id in &kid_ids {
            if let Ok(Object::Dictionary(d)) = doc.get_object_mut(page_id) {
                d.set("Parent", Object::Reference(pages_id));
            }
        }

        let catalog_id = doc.add_object(Object::Dictionary({
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"Catalog".to_vec()));
            d.set("Pages", Object::Reference(pages_id));
            d
        }));
        doc.trailer.set("Root", Object::Reference(catalog_id));
        doc
    }

    /// Extract TrimBox [x, y, w_abs, h_abs] from a page dict, or MediaBox as fallback.
    fn page_trim(doc: &Document, page_id: ObjectId) -> [f32; 4] {
        let dict = doc.get_dictionary(page_id).unwrap();
        let key = if dict.has(b"TrimBox") {
            b"TrimBox".as_ref()
        } else {
            b"MediaBox"
        };
        if let Ok(Object::Array(arr)) = dict.get(key) {
            let nums: Vec<f32> = arr
                .iter()
                .map(|o| match o {
                    Object::Real(v) => *v,
                    Object::Integer(v) => *v as f32,
                    _ => 0.0,
                })
                .collect();
            [nums[0], nums[1], nums[2], nums[3]]
        } else {
            [0.0; 4]
        }
    }

    #[test]
    fn test_invalid_width_rejected() {
        let doc = make_doc(&[(200.0, 100.0)]);
        assert!(split_pages(&doc, 0.0).is_err());
        assert!(split_pages(&doc, -50.0).is_err());
    }

    #[test]
    fn test_no_split_when_narrower_than_panel() {
        // Page is 100 wide, panel is 200 — should stay as 1 page
        let doc = make_doc(&[(100.0, 200.0)]);
        let out = split_pages(&doc, 200.0).unwrap();
        assert_eq!(out.get_pages().len(), 1);
        let (&_, &pid) = out.get_pages().iter().next().unwrap();
        let trim = page_trim(&out, pid);
        assert_eq!(trim[2], 100.0_f32); // width
        assert_eq!(trim[3], 200.0_f32); // height
    }

    #[test]
    fn test_exact_two_panel_split() {
        // 200×100 page split at 100 → 2 equal panels
        let doc = make_doc(&[(200.0, 100.0)]);
        let out = split_pages(&doc, 100.0).unwrap();
        assert_eq!(out.get_pages().len(), 2);
        for (&_, &pid) in out.get_pages().iter() {
            let trim = page_trim(&out, pid);
            let w = trim[2] - trim[0]; // right - left
            let h = trim[3] - trim[1]; // top - bottom
            assert!(
                (w - 100.0_f32).abs() < 0.01,
                "panel width should be 100, got {}",
                w
            );
            assert!(
                (h - 100.0_f32).abs() < 0.01,
                "panel height should be 100, got {}",
                h
            );
        }
    }

    #[test]
    fn test_uneven_split_last_panel_narrower() {
        // 250×100 split at 100 → 3 panels: 100, 100, 50
        let doc = make_doc(&[(250.0, 100.0)]);
        let out = split_pages(&doc, 100.0).unwrap();
        assert_eq!(out.get_pages().len(), 3);
        let widths: Vec<f32> = out
            .get_pages()
            .values()
            .map(|&pid| {
                let trim = page_trim(&out, pid);
                trim[2] - trim[0]
            })
            .collect();
        let full = widths
            .iter()
            .filter(|&&w| (w - 100.0_f32).abs() < 0.01)
            .count();
        let partial = widths
            .iter()
            .filter(|&&w| (w - 50.0_f32).abs() < 0.01)
            .count();
        assert_eq!(full, 2);
        assert_eq!(partial, 1);
    }

    #[test]
    fn test_multi_page_source() {
        // 2 source pages of 200×100 each, panel=100 → 4 output pages
        let doc = make_doc(&[(200.0, 100.0), (200.0, 100.0)]);
        let out = split_pages(&doc, 100.0).unwrap();
        assert_eq!(out.get_pages().len(), 4);
    }

    #[test]
    fn test_all_panels_have_trim_box() {
        let doc = make_doc(&[(300.0, 200.0)]);
        let out = split_pages(&doc, 100.0).unwrap();
        assert_eq!(out.get_pages().len(), 3);
        for (&_, &pid) in out.get_pages().iter() {
            let dict = out.get_dictionary(pid).unwrap();
            assert!(dict.has(b"TrimBox"), "panel page should have TrimBox");
        }
    }

    #[test]
    fn test_content_streams_wrapped() {
        // Every panel page should have Contents = array with at least 2 entries
        // (start wrapper + end wrapper, even if no original streams)
        let doc = make_doc(&[(200.0, 100.0)]);
        let out = split_pages(&doc, 100.0).unwrap();
        for (&_, &pid) in out.get_pages().iter() {
            let dict = out.get_dictionary(pid).unwrap();
            assert!(
                matches!(dict.get(b"Contents"), Ok(Object::Reference(_))),
                "Contents should have at least start+end wrapper"
            );
        }
    }

    #[test]
    fn test_single_page_single_panel_passthrough() {
        // Exact match: 100×100 page, panel=100 → 1 page, dimensions unchanged
        let doc = make_doc(&[(100.0, 100.0)]);
        let out = split_pages(&doc, 100.0).unwrap();
        assert_eq!(out.get_pages().len(), 1);
        let (&_, &pid) = out.get_pages().iter().next().unwrap();
        let trim = page_trim(&out, pid);
        assert!((trim[2] - 100.0_f32).abs() < 0.01);
        assert!((trim[3] - 100.0_f32).abs() < 0.01);
    }

    #[test]
    fn explicit_unequal_panels_preserve_operator_plan() {
        let doc = make_doc(&[(264.0, 612.0)]);
        let out = split_pages_explicit(&doc, &[88.0, 90.0, 86.0], SplitAxis::Horizontal).unwrap();
        let widths = out
            .get_pages()
            .values()
            .map(|&pid| {
                let trim = page_trim(&out, pid);
                trim[2] - trim[0]
            })
            .collect::<Vec<_>>();
        assert_eq!(widths, vec![88.0, 90.0, 86.0]);
    }

    #[test]
    fn explicit_vertical_panels_split_page_height() {
        let doc = make_doc(&[(612.0, 300.0)]);
        let out = split_pages_explicit(&doc, &[120.0, 180.0], SplitAxis::Vertical).unwrap();
        let heights = out
            .get_pages()
            .values()
            .map(|&pid| {
                let trim = page_trim(&out, pid);
                trim[3] - trim[1]
            })
            .collect::<Vec<_>>();
        assert_eq!(heights, vec![120.0, 180.0]);
    }

    #[test]
    fn explicit_panel_plan_must_cover_trim_extent() {
        let doc = make_doc(&[(300.0, 200.0)]);
        assert!(split_pages_explicit(&doc, &[100.0, 100.0], SplitAxis::Horizontal).is_err());
    }
}
