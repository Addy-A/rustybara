use crate::pages::boxes::object_to_f64;
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object};

/// A color remapping configuration that defines how to transform one color to another.
///
/// This struct represents a mapping rule that can be used to replace colors within a specified
/// tolerance range. When a color matches the `from` color within the tolerance, it will be
/// replaced with the corresponding `to` color.
///
/// # Fields
///
/// * `from` - The source color represented as RGBA values in the range [0.0, 1.0]
/// * `to` - The target color represented as RGBA values in the range [0.0, 1.0]  
/// * `tolerance` - The maximum allowed difference between colors for a match, where 0.0 means
///   exact match and 1.0 allows maximum variation
///
/// # Examples
///
/// ```
/// let remap = ColorRemap {
///     from: [1.0, 0.0, 0.0, 1.0], // Red
///     to: [0.0, 1.0, 0.0, 1.0],   // Green
///     tolerance: 0.1,              // 10% tolerance
/// };
/// ```
pub struct ColorRemap {
    pub from: [f64; 4],
    pub to: [f64; 4],
    pub tolerance: f64,
}

impl ColorRemap {
    /// Applies color remappings to all pages in a PDF document.
    ///
    /// This function iterates through all pages in the provided document and applies
    /// the specified color remappings to each page. The remapping process modifies
    /// the color values according to the provided `ColorRemap` rules.
    ///
    /// # Arguments
    ///
    /// * `doc` - A mutable reference to the PDF document to modify
    /// * `remaps` - A slice of `ColorRemap` objects defining the color transformation rules
    ///
    /// # Returns
    ///
    /// * `Ok(())` if all pages were successfully processed
    /// * `Err(crate::Error)` if an error occurred during processing
    ///
    /// # Examples
    ///
    /// ```no_test
    /// use pdf_writer::{Document, ColorRemap};
    ///
    /// let mut doc = Document::load("input.pdf")?;
    /// let remaps = vec![ColorRemap::new(...)];
    /// ColorRemapper::apply(&mut doc, &remaps)?;
    /// ```
    pub fn apply(doc: &mut Document, remaps: &[ColorRemap]) -> crate::Result<()> {
        let pages = doc.get_pages();
        for &page_id in pages.values() {
            Self::remap_page(doc, page_id, remaps)?;
        }
        Ok(())
    }

    fn remap_page(
        doc: &mut Document,
        page_id: lopdf::ObjectId,
        remaps: &[ColorRemap],
    ) -> crate::Result<()> {
        let content = doc.get_and_decode_page_content(page_id)?;
        let rewritten = remap_operations(&content.operations, remaps);
        let new_content = Content {
            operations: rewritten,
        };
        let bytes = new_content.encode()?;

        let stream_ids = doc.get_page_contents(page_id);
        let stream_id = stream_ids[0];
        if let Ok(Object::Stream(stream)) = doc.get_object_mut(stream_id) {
            stream.set_plain_content(bytes);
        }

        if stream_ids.len() > 1 {
            for &extra_id in &stream_ids[1..] {
                if let Ok(Object::Stream(s)) = doc.get_object_mut(extra_id) {
                    s.set_plain_content(Vec::new());
                }
            }
            if let Ok(page_obj) = doc.get_object_mut(page_id)
                && let Ok(dict) = page_obj.as_dict_mut()
            {
                dict.set("Contents", Object::Reference(stream_id));
            }
        }
        Ok(())
    }
}

fn remap_operations(operations: &[Operation], remaps: &[ColorRemap]) -> Vec<Operation> {
    operations
        .iter()
        .map(|op| match op.operator.as_str() {
            "k" | "K" if op.operands.len() == 4 => {
                let cmyk = read_cmyk(&op.operands);
                for remap in remaps {
                    if cmyk_matches(&cmyk, &remap.from, remap.tolerance) {
                        return Operation {
                            operator: op.operator.clone(),
                            operands: cmyk_to_operands(&remap.to),
                        };
                    }
                }
                op.clone()
            }
            _ => op.clone(),
        })
        .collect()
}

fn read_cmyk(operands: &[Object]) -> [f64; 4] {
    [
        object_to_f64(&operands[0]),
        object_to_f64(&operands[1]),
        object_to_f64(&operands[2]),
        object_to_f64(&operands[3]),
    ]
}

fn cmyk_matches(a: &[f64; 4], b: &[f64; 4], tolerance: f64) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(av, bv)| (av - bv).abs() <= tolerance)
}

fn cmyk_to_operands(cmyk: &[f64; 4]) -> Vec<Object> {
    cmyk.iter().map(|&v| Object::Real(v as f32)).collect()
}
