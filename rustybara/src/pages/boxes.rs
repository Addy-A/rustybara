use crate::geometry::Rect;
use lopdf::{Document, Object, ObjectId};
use pdfium_render::prelude::{PdfPageAnnotationCommon, PdfPageObjectsCommon};

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
/// ```rust
/// let page_boxes = PageBoxes {
///     media_box: Rect::new(0.0, 0.0, 612.0, 792.0), // 8.5" x 11" letter size
///     trim_box: Some(Rect::new(36.0, 36.0, 576.0, 756.0)), // 1/2" margins
///     bleed_box: None,
///     crop_box: Some(Rect::new(0.0, 0.0, 612.0, 792.0)),
/// };
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
    /// Returns a `Result<PageBoxes, lopdf::Error>` where:
    /// * `Ok(PageBoxes)` - Contains the extracted box information
    /// * `Err(lopdf::Error)` - If the page cannot be found or parsed
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
    /// ```rust
    /// // Assuming you have a Document and page ObjectId
    /// let page_boxes = PageBoxes::read(&document, page_object_id)?;
    /// println!("MediaBox: {:?}", page_boxes.media_box);
    /// ```
    pub fn read(doc: &Document, page_id: ObjectId) -> Result<Self, lopdf::Error> {
        let page_dict = doc.get_dictionary(page_id)?;
        let media_box = arr_to_rect(page_dict.get(b"MediaBox")?.as_array()?);
        let trim_box = page_dict
            .get(b"TrimBox")
            .and_then(|obj| obj.as_array())
            .map(arr_to_rect)
            .ok();

        let bleed_box = page_dict
            .get(b"BleedBox")
            .and_then(|obj| obj.as_array())
            .map(arr_to_rect)
            .ok();

        let crop_box = page_dict
            .get(b"CropBox")
            .and_then(|obj| obj.as_array())
            .map(arr_to_rect)
            .ok();

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
    /// ```rust
    /// // Create a 100x100 rectangle and add 3pt bleed
    /// let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
    /// let bleed_rect = rect.bleed_rect(3.0);
    /// // Result: Rect with coordinates (-3.0, -3.0, 103.0, 103.0)
    /// ```
    pub fn bleed_rect(&self, pts: f64) -> Rect {
        self.trim_or_media().expand(pts)
    }
}

fn arr_to_rect(arr: &Vec<Object>) -> Rect {
    Rect::from_corners(
        object_to_f64(&arr[0]),
        object_to_f64(&arr[1]),
        object_to_f64(&arr[2]),
        object_to_f64(&arr[3]),
    )
}

pub fn object_to_f64(obj: &lopdf::Object) -> f64 {
    match obj {
        lopdf::Object::Integer(i) => *i as f64,
        lopdf::Object::Real(r) => *r as f64,
        _ => panic!("expected numeric object, got {:?}", obj),
    }
}
