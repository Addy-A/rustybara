use crate::geometry::Rect;
use lopdf::{Document, Object, ObjectId};
use std::collections::HashSet;
use std::io;

/// Represents the various bounding boxes that define the dimensions and boundaries of a PDF page.
///
/// Each PDF page can have multiple bounding boxes that serve different purposes in document layout
/// and printing. This structure encapsulates the essential boxes needed to properly render and
/// position page content.
///
/// # Fields
///
/// * `media_box` - The primary page boundary that defines the full extent of the page media.
///   This is the only required box and represents the physical dimensions of the page.
///
/// * `trim_box` - Optional box that defines the intended finished size of the page after trimming.
///   When present, this is typically smaller than or equal to the media box.
///
/// * `bleed_box` - Optional box that extends beyond the trim box to include any bleed area.
///   Used in printing to ensure content extends to the edge of the trimmed page.
///
/// * `crop_box` - Optional box that defines the region to which the page content should be clipped.
///   This determines what portion of the page is visible when displayed or printed.
///
/// # Examples
///
/// ```no_test
/// use rustybara::geometry::Rect;
/// use rustybara::pages::PageBoxes;
/// let page_boxes = PageBoxes {
///     media_box: Rect::new(0.0, 0.0, 612.0, 792.0), // 8.5" x 11" letter size
///     trim_box: Some(Rect::new(36.0, 36.0, 576.0, 756.0)), // 1/2" margins
///     bleed_box: None,
///     crop_box: Some(Rect::new(0.0, 0.0, 612.0, 792.0)),
/// };
/// ```
pub struct PageBoxes {
    pub media_box: Rect,
    pub trim_box: Option<Rect>,
    pub bleed_box: Option<Rect>,
    pub crop_box: Option<Rect>,
}

impl PageBoxes {
    /// Reads page box information from a PDF document page.
    ///
    /// This function extracts the various box definitions (MediaBox, TrimBox, BleedBox, and CropBox)
    /// from a PDF page dictionary. These boxes define different boundaries and regions of the page
    /// for rendering and printing purposes.
    ///
    /// # Arguments
    ///
    /// * `doc` - A reference to the PDF document to read from
    /// * `page_id` - The object ID of the page to extract box information from
    ///
    /// # Returns
    ///
    /// Returns a `Result<PageBoxes>` where:
    /// * `Ok(PageBoxes)` - Contains the extracted box information
    /// * `Err(Error)` - If the page cannot be found or parsed
    ///
    /// # Box Types
    ///
    /// * `media_box` - Defines the full area of the physical medium on which the page will be printed
    /// * `trim_box` - Defines the intended dimensions of the finished page after trimming (optional)
    /// * `bleed_box` - Defines the region to which all page content should be clipped (optional)
    /// * `crop_box` - Defines the region to which the contents of the page shall be clipped when displayed (optional)
    ///
    /// # Example
    ///
    /// ```no_test
    /// let page_boxes = PageBoxes::read(&document, page_object_id)?;
    /// println!("MediaBox: {:?}", page_boxes.media_box);
    /// ```
    pub fn read(doc: &Document, page_id: ObjectId) -> crate::Result<Self> {
        let media_box = read_page_box(doc, page_id, b"MediaBox", true)?.ok_or_else(|| {
            crate::Error::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "PDF page has no MediaBox in its page-tree ancestry",
            ))
        })?;
        let trim_box = read_page_box(doc, page_id, b"TrimBox", false)?;
        let bleed_box = read_page_box(doc, page_id, b"BleedBox", false)?;
        let crop_box = read_page_box(doc, page_id, b"CropBox", true)?;

        Ok(PageBoxes {
            media_box,
            trim_box,
            bleed_box,
            crop_box,
        })
    }

    /// Returns a reference to the trim box if it exists, otherwise returns a reference to the media box.
    ///
    /// This method provides access to the page's trim box, which defines the intended dimensions
    /// of the finished page after trimming. If no trim box is explicitly set, it falls back to
    /// the media box which represents the full physical page size.
    ///
    /// # Returns
    /// A reference to the `Rect` representing either the trim box or media box
    pub fn trim_or_media(&self) -> &Rect {
        self.trim_box.as_ref().unwrap_or(&self.media_box)
    }

    /// Expands the trim or media rectangle by the specified bleed amount.
    ///
    /// This method takes the current trim box (if defined) or media box and expands
    /// it outward by the given number of points on all sides. This is typically used
    /// to create a bleed area for printing purposes, where artwork extends beyond
    /// the final trim edge to ensure no white borders appear after cutting.
    ///
    /// # Arguments
    ///
    /// * `pts` - The bleed amount in points to expand the rectangle on all sides
    ///
    /// # Returns
    ///
    /// A new `Rect` representing the expanded bleed area
    ///
    /// # Example
    ///
    /// ```no_test
    /// let page_boxes = PageBoxes::read(&document, page_id)?;
    /// let bleed = page_boxes.bleed_rect(3.0);
    /// ```
    pub fn bleed_rect(&self, pts: f64) -> Rect {
        self.trim_or_media().expand(pts)
    }
}

/// Sets every page's `MediaBox` to an exact `width_pts` × `height_pts`
/// rectangle, centered on the page's current MediaBox. Content that falls
/// outside the new box is cropped (clipped) by viewers and RIPs, not deleted.
/// Any existing `CropBox` is rewritten to the same rectangle so it never
/// exceeds the new MediaBox.
///
/// Dimensions are in PDF points (1/72").
///
/// # Errors
///
/// Returns an error if `width_pts` or `height_pts` is not positive, or if a
/// page dictionary cannot be accessed.
pub fn set_media_box(doc: &mut Document, width_pts: f64, height_pts: f64) -> crate::Result<()> {
    if width_pts <= 0.0 || height_pts <= 0.0 {
        return Err(crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "width and height must be positive",
        )));
    }
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();

    let new_boxes: Vec<(ObjectId, [f64; 4], bool)> = page_ids
        .iter()
        .map(|&page_id| {
            let boxes = PageBoxes::read(doc, page_id)?;
            let m = &boxes.media_box;
            let cx = m.x + m.width / 2.0;
            let cy = m.y + m.height / 2.0;
            Ok((
                page_id,
                [
                    cx - width_pts / 2.0,
                    cy - height_pts / 2.0,
                    cx + width_pts / 2.0,
                    cy + height_pts / 2.0,
                ],
                boxes.crop_box.is_some(),
            ))
        })
        .collect::<crate::Result<Vec<_>>>()?;

    for (page_id, [x0, y0, x1, y1], had_cropbox) in new_boxes {
        let arr = vec![
            Object::Real(x0 as f32),
            Object::Real(y0 as f32),
            Object::Real(x1 as f32),
            Object::Real(y1 as f32),
        ];
        let dict = doc.get_dictionary_mut(page_id)?;
        dict.set(b"MediaBox", Object::Array(arr.clone()));
        if had_cropbox {
            dict.set(b"CropBox", Object::Array(arr));
        }
    }

    Ok(())
}

/// Sets a `TrimBox` on every page by insetting the `MediaBox` by `bleed_pts` on all sides.
///
/// Reads each page's `MediaBox`, shrinks it inward by `bleed_pts`, and writes the result
/// back as the page's `TrimBox`. Any existing `TrimBox` is overwritten.
pub fn set_trim_boxes(doc: &mut Document, bleed_pts: f64) -> crate::Result<()> {
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();

    let trim_rects: Vec<(ObjectId, [f64; 4])> = page_ids
        .iter()
        .map(|&page_id| {
            let boxes = PageBoxes::read(doc, page_id)?;
            let m = &boxes.media_box;
            Ok((
                page_id,
                [
                    m.x + bleed_pts,
                    m.y + bleed_pts,
                    m.right() - bleed_pts,
                    m.top() - bleed_pts,
                ],
            ))
        })
        .collect::<crate::Result<Vec<_>>>()?;

    for (page_id, [x0, y0, x1, y1]) in trim_rects {
        let arr = vec![
            Object::Real(x0 as f32),
            Object::Real(y0 as f32),
            Object::Real(x1 as f32),
            Object::Real(y1 as f32),
        ];
        doc.get_dictionary_mut(page_id)?
            .set(b"TrimBox", Object::Array(arr));
    }

    Ok(())
}

fn read_page_box(
    doc: &Document,
    page_id: ObjectId,
    key: &[u8],
    inheritable: bool,
) -> crate::Result<Option<Rect>> {
    let mut current_id = page_id;
    let mut visited = HashSet::new();

    loop {
        if !visited.insert(current_id) {
            return Err(crate::Error::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "cycle detected in PDF page-tree ancestry",
            )));
        }

        let dictionary = doc.get_dictionary(current_id)?;
        if let Ok(value) = dictionary.get(key) {
            let (_, resolved) = doc.dereference(value)?;
            return Ok(Some(arr_to_rect(resolved.as_array()?)?));
        }
        if !inheritable {
            return Ok(None);
        }

        current_id = match dictionary.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent_id) => parent_id,
            Err(_) => return Ok(None),
        };
    }
}

fn arr_to_rect(arr: &[Object]) -> crate::Result<Rect> {
    if arr.len() != 4 {
        return Err(crate::Error::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "page box must contain exactly four numbers, found {}",
                arr.len()
            ),
        )));
    }

    Ok(Rect::from_corners(
        try_object_to_f64(&arr[0])?,
        try_object_to_f64(&arr[1])?,
        try_object_to_f64(&arr[2])?,
        try_object_to_f64(&arr[3])?,
    ))
}

fn try_object_to_f64(obj: &lopdf::Object) -> crate::Result<f64> {
    match obj {
        lopdf::Object::Integer(i) => Ok(*i as f64),
        lopdf::Object::Real(r) => Ok(*r as f64),
        _ => Err(crate::Error::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("page box contains a non-numeric value: {obj:?}"),
        ))),
    }
}

pub(crate) fn object_to_f64(obj: &lopdf::Object) -> f64 {
    match obj {
        lopdf::Object::Integer(value) => *value as f64,
        lopdf::Object::Real(value) => *value as f64,
        _ => panic!("expected numeric object, got {obj:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Object, dictionary};

    fn document_with_page(
        page_entries: lopdf::Dictionary,
        media_box: Object,
    ) -> (Document, ObjectId) {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page_id = document.add_object(Object::Dictionary(page_entries));
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
                "MediaBox" => media_box,
            }),
        );
        document
            .get_dictionary_mut(page_id)
            .unwrap()
            .set("Parent", Object::Reference(pages_id));
        (document, page_id)
    }

    #[test]
    fn media_box_is_inherited_from_pages_ancestor() {
        let (document, page_id) = document_with_page(
            dictionary! { "Type" => "Page" },
            Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
        );

        let boxes = PageBoxes::read(&document, page_id).unwrap();
        assert_eq!(boxes.media_box.x, 0.0);
        assert_eq!(boxes.media_box.y, 0.0);
        assert_eq!(boxes.media_box.width, 612.0);
        assert_eq!(boxes.media_box.height, 792.0);
    }

    #[test]
    fn malformed_page_box_returns_error_instead_of_panicking() {
        let (document, page_id) = document_with_page(
            dictionary! { "Type" => "Page" },
            Object::Array(vec![0.into(), 0.into(), 612.into()]),
        );

        let error = PageBoxes::read(&document, page_id).err().unwrap();
        assert!(error.to_string().contains("exactly four"));
    }
}
