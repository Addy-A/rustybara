use std::collections::HashSet;
use lopdf::{Document, Object, ObjectId};

/// Extracts a subset of pages (0-indexed) from `src` into a new `Document`.
///
/// The returned document is a clone of `src` with its page tree rewritten to contain only
/// the requested pages, in their original order. Orphaned page objects remain in the clone
/// but are unreachable; the file will be slightly larger than strictly necessary.
///
/// Assumes a **flat page tree** — all pages are direct children of the Pages root. Nested
/// intermediate Pages nodes, rare in typical prepress output, may not extract correctly.
///
/// Returns an error if `page_nums` contains no valid indices.
pub fn extract_pages(src: &Document, page_nums: &[u32]) -> crate::Result<Document> {
    let all_pages = src.get_pages(); // BTreeMap<u32 (1-indexed), ObjectId>
    let keep_ids: HashSet<ObjectId> = page_nums.iter()
        .filter_map(|&n| all_pages.get(&(n + 1)).copied())
        .collect();

    if keep_ids.is_empty() {
        return Err(crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "no valid page numbers provided",
        )));
    }

    let mut new_doc = src.clone();

    // Resolve the Pages root ObjectId — immutable borrow ends at the closing brace.
    let pages_id: ObjectId = {
        let catalog = new_doc.catalog()?;
        catalog.get(b"Pages")?.as_reference()?
    };

    // Build the filtered Kids list — immutable borrow ends at the closing brace.
    let new_kids: Vec<Object> = {
        let pages_dict = new_doc.get_object(pages_id)?.as_dict()?;
        pages_dict
            .get(b"Kids")?
            .as_array()?
            .iter()
            .filter(|obj| {
                obj.as_reference()
                    .map(|id| keep_ids.contains(&id))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    };

    let new_count = new_kids.len() as i64;
    if let Ok(Object::Dictionary(dict)) = new_doc.get_object_mut(pages_id) {
        dict.set("Kids", Object::Array(new_kids));
        dict.set("Count", Object::Integer(new_count));
    }

    Ok(new_doc)
}
