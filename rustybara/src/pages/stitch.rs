use crate::pages::PageBoxes;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

/// Stitches consecutive pages side-by-side into spread pages.
///
/// `panels_per_spread = round(spread_width_pts / avg_source_page_width)`, min 1.
/// Each spread's width is the sum of its panels; height is the tallest panel.
pub fn stitch_pages(src: &Document, spread_width_pts: f64) -> crate::Result<Document> {
    if spread_width_pts <= 0.0 {
        return Err(crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "spread width must be positive",
        )));
    }

    let mut new_doc = src.clone();

    let pages_id: ObjectId = {
        let catalog = new_doc.catalog()?;
        catalog.get(b"Pages")?.as_reference()?
    };

    struct Panel {
        stream_ids: Vec<ObjectId>,
        resources: Option<Object>,
        width: f64,
        height: f64,
        trim_x: f64,
        trim_y: f64,
    }

    // BTreeMap from get_pages() iterates in ascending page-number order
    let panels: Vec<Panel> = src
        .get_pages()
        .values()
        .map(|&id| {
            let trim = PageBoxes::read(src, id)
                .map(|b| *b.trim_or_media())
                .unwrap_or(crate::geometry::Rect { x: 0.0, y: 0.0, width: 612.0, height: 792.0 });
            Panel {
                stream_ids: src.get_page_contents(id),
                resources: src
                    .get_dictionary(id)
                    .ok()
                    .and_then(|d| d.get(b"Resources").ok())
                    .map(Object::clone),
                width: trim.width,
                height: trim.height,
                trim_x: trim.x,
                trim_y: trim.y,
            }
        })
        .collect();

    if panels.is_empty() {
        return Err(crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "document has no pages",
        )));
    }

    let avg_width = panels.iter().map(|p| p.width).sum::<f64>() / panels.len() as f64;
    let per_spread = ((spread_width_pts / avg_width).round() as usize).max(1);

    let mut new_kids: Vec<Object> = Vec::new();

    for chunk in panels.chunks(per_spread) {
        let spread_w: f64 = chunk.iter().map(|p| p.width).sum();
        let spread_h: f64 = chunk.iter().map(|p| p.height).fold(0.0_f64, f64::max);
        let box_arr = box_array(spread_w, spread_h);

        let mut contents: Vec<Object> = Vec::new();
        let mut x_offset = 0.0_f64;

        for p in chunk {
            // Clip to this panel's column, then translate source coords to spread coords.
            // Source content at (trim_x, trim_y) maps to (x_offset, 0) on the spread.
            let start = format!(
                "q\n{} 0 {} {} re W n\n1 0 0 1 {} {} cm\n",
                pdf_num(x_offset),
                pdf_num(p.width),
                pdf_num(spread_h),
                pdf_num(x_offset - p.trim_x),
                pdf_num(-p.trim_y),
            )
            .into_bytes();

            let s_id = new_doc
                .add_object(Object::Stream(Stream::new(Dictionary::new(), start)));
            let e_id = new_doc
                .add_object(Object::Stream(Stream::new(Dictionary::new(), b"Q\n".to_vec())));

            contents.push(Object::Reference(s_id));
            for &oid in &p.stream_ids {
                contents.push(Object::Reference(oid));
            }
            contents.push(Object::Reference(e_id));

            x_offset += p.width;
        }

        let resource_list: Vec<Option<Object>> =
            chunk.iter().map(|p| p.resources.clone()).collect();
        let merged = merge_resources(&resource_list);

        let mut page = Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set("Parent", Object::Reference(pages_id));
        page.set("MediaBox", box_arr.clone());
        page.set("TrimBox", box_arr);
        page.set("Contents", Object::Array(contents));
        if let Some(res) = merged {
            page.set("Resources", res);
        }

        let spread_id = new_doc.add_object(Object::Dictionary(page));
        new_kids.push(Object::Reference(spread_id));
    }

    let count = new_kids.len() as i64;
    if let Ok(Object::Dictionary(dict)) = new_doc.get_object_mut(pages_id) {
        dict.set("Kids", Object::Array(new_kids));
        dict.set("Count", Object::Integer(count));
    }

    new_doc.prune_objects();
    Ok(new_doc)
}

/// Merge a list of resource dictionaries. Sub-dicts (Font, XObject, …) are merged
/// entry-by-entry with first-occurrence winning on name conflicts.
fn merge_resources(list: &[Option<Object>]) -> Option<Object> {
    let dicts: Vec<&Dictionary> = list
        .iter()
        .filter_map(|o| o.as_ref())
        .filter_map(|o| if let Object::Dictionary(d) = o { Some(d) } else { None })
        .collect();

    match dicts.len() {
        0 => None,
        1 => Some(Object::Dictionary(dicts[0].clone())),
        _ => {
            let mut merged = Dictionary::new();
            for dict in &dicts {
                for (key, val) in dict.iter() {
                    if !merged.has(key.as_slice()) {
                        merged.set(key.clone(), val.clone());
                        continue;
                    }
                    if let Object::Dictionary(incoming) = val {
                        if let Ok(existing) = merged.get(key.as_slice()).cloned() {
                            if let Object::Dictionary(mut sub) = existing {
                                for (sk, sv) in incoming.iter() {
                                    if !sub.has(sk.as_slice()) {
                                        sub.set(sk.clone(), sv.clone());
                                    }
                                }
                                merged.set(key.clone(), Object::Dictionary(sub));
                            }
                        }
                    }
                }
            }
            Some(Object::Dictionary(merged))
        }
    }
}

fn pdf_num(v: f64) -> String {
    if v.fract() == 0.0 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.4}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn box_array(width: f64, height: f64) -> Object {
    Object::Array(vec![
        Object::Real(0.0_f32),
        Object::Real(0.0_f32),
        Object::Real(width as f32),
        Object::Real(height as f32),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn page_trim(doc: &Document, page_id: ObjectId) -> [f32; 4] {
        let dict = doc.get_dictionary(page_id).unwrap();
        let key = if dict.has(b"TrimBox") { b"TrimBox".as_ref() } else { b"MediaBox" };
        if let Ok(Object::Array(arr)) = dict.get(key) {
            let v: Vec<f32> = arr
                .iter()
                .map(|o| match o {
                    Object::Real(v) => *v,
                    Object::Integer(v) => *v as f32,
                    _ => 0.0,
                })
                .collect();
            [v[0], v[1], v[2], v[3]]
        } else {
            [0.0; 4]
        }
    }

    #[test]
    fn test_invalid_spread_width_rejected() {
        let doc = make_doc(&[(100.0, 200.0)]);
        assert!(stitch_pages(&doc, 0.0).is_err());
        assert!(stitch_pages(&doc, -50.0).is_err());
    }

    #[test]
    fn test_single_page_stays_single() {
        let doc = make_doc(&[(200.0, 100.0)]);
        let out = stitch_pages(&doc, 200.0).unwrap();
        assert_eq!(out.get_pages().len(), 1);
        let (&_, &pid) = out.get_pages().iter().next().unwrap();
        let t = page_trim(&out, pid);
        assert!((t[2] - 200.0_f32).abs() < 0.5);
    }

    #[test]
    fn test_two_panels_into_one_spread() {
        let doc = make_doc(&[(100.0, 200.0), (100.0, 200.0)]);
        let out = stitch_pages(&doc, 200.0).unwrap();
        assert_eq!(out.get_pages().len(), 1);
        let (&_, &pid) = out.get_pages().iter().next().unwrap();
        let t = page_trim(&out, pid);
        assert!((t[2] - 200.0_f32).abs() < 0.5, "spread width should be 200, got {}", t[2]);
        assert!((t[3] - 200.0_f32).abs() < 0.5);
    }

    #[test]
    fn test_three_panel_trifold() {
        let doc = make_doc(&[(100.0, 200.0); 3]);
        let out = stitch_pages(&doc, 300.0).unwrap();
        assert_eq!(out.get_pages().len(), 1);
        let (&_, &pid) = out.get_pages().iter().next().unwrap();
        let t = page_trim(&out, pid);
        assert!((t[2] - 300.0_f32).abs() < 0.5);
    }

    #[test]
    fn test_partial_last_group() {
        // 5 × 100-wide pages, spread=300 → panels_per_spread=3 → spreads of 300+200
        let doc = make_doc(&[(100.0, 200.0); 5]);
        let out = stitch_pages(&doc, 300.0).unwrap();
        assert_eq!(out.get_pages().len(), 2);
        let widths: Vec<f32> = out.get_pages().values().map(|&p| page_trim(&out, p)[2]).collect();
        assert!(widths.iter().any(|&w| (w - 300.0_f32).abs() < 0.5));
        assert!(widths.iter().any(|&w| (w - 200.0_f32).abs() < 0.5));
    }

    #[test]
    fn test_spread_height_is_max_panel_height() {
        let doc = make_doc(&[(100.0, 200.0), (100.0, 300.0)]);
        let out = stitch_pages(&doc, 200.0).unwrap();
        let (&_, &pid) = out.get_pages().iter().next().unwrap();
        assert!((page_trim(&out, pid)[3] - 300.0_f32).abs() < 0.5);
    }

    #[test]
    fn test_all_spreads_have_trim_box() {
        let doc = make_doc(&[(100.0, 200.0); 4]);
        let out = stitch_pages(&doc, 200.0).unwrap();
        for (&_, &pid) in out.get_pages().iter() {
            assert!(out.get_dictionary(pid).unwrap().has(b"TrimBox"));
        }
    }

    #[test]
    fn test_content_streams_wrapped() {
        let doc = make_doc(&[(100.0, 200.0); 2]);
        let out = stitch_pages(&doc, 200.0).unwrap();
        for (&_, &pid) in out.get_pages().iter() {
            let dict = out.get_dictionary(pid).unwrap();
            if let Ok(Object::Array(arr)) = dict.get(b"Contents") {
                assert!(arr.len() >= 2);
            }
        }
    }

    #[test]
    fn test_split_stitch_roundtrip_page_count() {
        use crate::pages::split_pages;
        // 612×792 → split at 204 (3 panels) → stitch at 612 → 1 page ≈ 612 wide
        let doc = make_doc(&[(612.0, 792.0)]);
        let split = split_pages(&doc, 204.0).unwrap();
        assert_eq!(split.get_pages().len(), 3);
        let stitched = stitch_pages(&split, 612.0).unwrap();
        assert_eq!(stitched.get_pages().len(), 1);
        let (&_, &pid) = stitched.get_pages().iter().next().unwrap();
        let t = page_trim(&stitched, pid);
        assert!((t[2] - 612.0_f32).abs() < 1.0, "got width {}", t[2]);
        assert!((t[3] - 792.0_f32).abs() < 1.0, "got height {}", t[3]);
    }
}
