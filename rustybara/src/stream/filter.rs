use crate::geometry::{Matrix, Rect};
use crate::pages::PageBoxes;
use crate::pages::boxes::object_to_f64;
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object};
use std::collections::HashSet;

/// A filter for processing and filtering content based on specified criteria.
///
/// The `ContentFilter` struct provides functionality to evaluate content against
/// various filtering rules and conditions. It can be used to determine whether
/// content should be included or excluded based on predefined filter parameters.
///
/// # Notes
///
/// This is a placeholder struct definition. Actual implementation details,
/// methods, and fields will need to be added based on the specific filtering
/// requirements and use cases.
pub struct ContentFilter;

impl ContentFilter {
    /// Removes content outside the trim box from all pages in a PDF document.
    ///
    /// This function processes each page in the document and removes any graphical content
    /// that falls outside the trim box (or media box if trim box is not defined).
    ///
    /// # Arguments
    ///
    /// * `doc` - A mutable reference to the PDF document to process
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the operation completes successfully
    /// * `Err(Error)` if there is an error reading or modifying the document
    ///
    /// # Process
    ///
    /// 1. Retrieves all pages from the document
    /// 2. For each page, reads the page boxes (including trim box)
    /// 3. Filters the page content to remove anything outside the trim (or media) box
    /// 4. Prunes unused objects from the document to clean up references
    ///
    /// # Note
    ///
    /// This operation modifies the document in-place and cannot be undone.
    /// The trim box defines the finished page size after trimming, while the media box
    /// defines the full page size including bleed area.
    pub fn remove_outside_trim(doc: &mut Document) -> crate::Result<()> {
        let pages = doc.get_pages();
        for &page_id in pages.values() {
            let trim = PageBoxes::read(doc, page_id);
            Self::filter_page(doc, page_id, trim?.trim_or_media())?;
        }
        doc.prune_objects();
        Ok(())
    }

    /// Filters the content of a PDF page by removing operations outside the specified trim rectangle.
    ///
    /// This function takes a PDF document, a page ID, and a trim rectangle, then modifies the page's
    /// content stream to remove any drawing operations that fall outside the trim area. The filtering
    /// is performed by analyzing the bounding boxes of graphical operations and excluding those
    /// that don't intersect with the trim rectangle.
    ///
    /// # Arguments
    ///
    /// * `doc` - A mutable reference to the PDF document to modify
    /// * `page_id` - The object ID of the page to filter
    /// * `trim` - A reference to the rectangle defining the visible area to keep
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the filtering was successful
    /// * `Err(Error)` if there was an error accessing or modifying the PDF content
    ///
    /// # Example
    ///
    /// ```no_test
    /// use lopdf::{Document, Rect};
    ///
    /// let mut doc = Document::load("input.pdf")?;
    /// let page_id = doc.page_iter().next().unwrap();
    /// let trim_rect = Rect::new(0.0, 0.0, 595.0, 842.0); // A4 size
    /// filter_page(&mut doc, page_id, &trim_rect)?;
    /// doc.save("output.pdf")?;
    /// ```
    ///
    /// # Notes
    ///
    /// * This function only processes the first content stream of the page
    /// * The original PDF structure is modified in-place
    /// * Coordinate system follows PDF conventions (bottom-left origin)
    pub fn filter_page(
        doc: &mut Document,
        page_id: lopdf::ObjectId,
        trim: &Rect,
    ) -> crate::Result<()> {
        let content = doc.get_and_decode_page_content(page_id)?;
        let stream_ids = doc.get_page_contents(page_id);
        let filtered = filter_operations(&content.operations, Some(*trim));
        let new_content = Content {
            operations: filtered,
        };
        let bytes = new_content.encode()?;

        // Write filtered content to the first stream
        let stream_id = stream_ids[0];
        if let Ok(Object::Stream(stream)) = doc.get_object_mut(stream_id) {
            stream.set_plain_content(bytes);
        }

        // If the page had multiple content streams, clear the extras and
        // collapse the Contents entry to a single reference so that
        // re-decoding doesn't re-append the old (unfiltered) streams.
        if stream_ids.len() > 1 {
            for &extra_id in &stream_ids[1..] {
                if let Ok(Object::Stream(s)) = doc.get_object_mut(extra_id) {
                    s.set_plain_content(Vec::new());
                }
            }
            // Replace the Contents array with a single reference
            if let Ok(page_obj) = doc.get_object_mut(page_id)
                && let Ok(dict) = page_obj.as_dict_mut()
            {
                dict.set("Contents", Object::Reference(stream_id));
            }
        }

        let referenced = collect_referenced_resources(&new_content.operations);
        prune_page_resources(doc, page_id, &referenced)?;
        Ok(())
    }
}

/// Determines if the bounding box of a subpath is completely outside a trimming rectangle.
///
/// This function calculates the axis-aligned bounding box (AABB) of a set of points after
/// applying a coordinate transformation matrix, then checks if this bounding box
/// is completely outside the specified trimming rectangle.
///
/// # Arguments
///
/// * `points` - A slice of (x, y) coordinate pairs representing the subpath points
/// * `ctm` - A transformation matrix to apply to the points before calculating bounds
/// * `trim` - A rectangle defining the trimming area to check against
///
/// # Returns
///
/// Returns `true` if the subpath's bounding box is completely outside the trimming rectangle,
/// `false` otherwise (including when points is empty or bounding boxes overlap).
///
/// # Examples
///
/// ```rust
/// // Example usage would go here
/// ```
///
/// # Notes
///
/// * Empty point lists return `false` (not considered outside)
/// * The bounding box is calculated in transformed coordinate space
/// * Uses axis-aligned bounding box approximation (may be conservative for rotated shapes)
fn subpath_bbox_is_outside(points: &[(f64, f64)], ctm: &Matrix, trim: &Rect) -> bool {
    if points.is_empty() {
        return false;
    }
    let mut xmin = f64::INFINITY;
    let mut xmax = f64::NEG_INFINITY;
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for &(x, y) in points {
        let (px, py) = ctm.transform_point(x, y);
        xmin = xmin.min(px);
        xmax = xmax.max(px);
        ymin = ymin.min(py);
        ymax = ymax.max(py);
    }
    Rect::from_corners(xmin, ymin, xmax, ymax).is_outside(trim)
}

/// Converts a slice of PDF objects representing transformation matrix operands into a Matrix struct.
///
/// This function takes exactly 6 operands from a PDF transformation matrix and converts them
/// into a 2D transformation matrix. The operands should represent the values [a, b, c, d, e, f]
/// which correspond to the standard PDF transformation matrix format.
///
/// # Arguments
///
/// * `operands` - A slice of `lopdf::Object` references containing exactly 6 numeric values
///
/// # Returns
///
/// A `Matrix` struct initialized with the 6 transformation values
///
/// # Example
///
/// ```no_test
/// // PDF matrix operands: [1.0, 0.0, 0.0, 1.0, 100.0, 50.0] (translate by 100, 50)
/// let operands = vec![Object::Real(1.0), Object::Real(0.0), Object::Real(0.0),
///                    Object::Real(1.0), Object::Real(100.0), Object::Real(50.0)];
/// let matrix = operands_to_matrix(&operands);
/// ```
///
/// # Panics
///
/// May panic if there are fewer than 6 operands or if operand conversion fails
pub fn operands_to_matrix(operands: &[lopdf::Object]) -> Matrix {
    let value: Vec<f64> = operands.iter().map(object_to_f64).collect();
    Matrix::from_values(value[0], value[1], value[2], value[3], value[4], value[5])
}

/// Converts a slice of PDF objects representing rectangle coordinates into a Rect struct.
///
/// This function takes PDF operands that represent a rectangle in the format [x, y, width, height]
/// and converts them into a Rect struct using corner coordinates [x1, y1, x2, y2].
///
/// # Arguments
///
/// * `operands` - A slice of lopdf::Object references containing the rectangle coordinates
///
/// # Returns
///
/// * `Rect` - A rectangle struct created from the corner coordinates (x1, y1) and (x2, y2)
///
/// # Example
///
/// ```no_test
/// // Given operands representing [10.0, 20.0, 100.0, 50.0]
/// // Returns Rect with corners at (10.0, 20.0) and (110.0, 70.0)
/// ```
///
/// # Panics
///
/// Panics if the operands slice contains fewer than 4 elements or if object_to_f64 conversion fails.
pub fn operands_to_rect(operands: &[lopdf::Object]) -> Rect {
    let value: Vec<f64> = operands.iter().map(object_to_f64).collect();
    Rect::from_corners(value[0], value[1], value[0] + value[2], value[1] + value[3])
}

/// Determines if a rectangle defined by PDF operands is outside a specified trim area.
///
/// This function transforms a rectangle from local coordinate space to page coordinate space
/// using the current transformation matrix (CTM), then checks if the resulting rectangle
/// falls outside the specified trim boundary.
///
/// # Arguments
///
/// * `operands` - A slice of PDF objects that define the rectangle coordinates
/// * `ctm` - The current transformation matrix for coordinate conversion
/// * `trim` - The rectangular trim boundary to check against
///
/// # Returns
///
/// Returns `true` if the transformed rectangle is completely outside the trim area,
/// `false` if it intersects or is contained within the trim area.
fn re_is_outside(operands: &[lopdf::Object], ctm: &Matrix, trim: &Rect) -> bool {
    let local_rect = operands_to_rect(operands);
    let page_rect = ctm.transform_rect(&local_rect);
    page_rect.is_outside(trim)
}

/// Filters a slice of PDF operations based on a trimming rectangle.
///
/// This function processes a sequence of PDF operations and removes any drawing operations
/// that fall outside the specified trimming rectangle. It maintains the structure of
/// graphics state changes (save/restore operations) while filtering out irrelevant content.
///
/// # Arguments
///
/// * `operations` - A slice of `Operation` structs representing PDF graphics operations
/// * `trim` - An optional `Rect` that defines the visible area. Operations outside this
///   rectangle will be filtered out. If `None`, no filtering is applied.
///
/// # Returns
///
/// A new `Vec<Operation>` containing only the operations that are relevant (inside the
/// trim rectangle) and properly handles graphics state management operations.
///
/// # Processing Logic
///
/// The function uses a stack-based approach to handle nested graphics state changes:
///
/// - `q` (save state): Pushes the current transformation matrix onto the stack
/// - `Q` (restore state): Pops the matrix and filters the operations in the completed block
/// - `cm` (concatenate matrix): Updates the current transformation matrix
/// - All other operations: Buffered and conditionally included based on their position
///
/// When a graphics state block ends (`Q`), the entire block is analyzed:
/// - If all drawable content is outside the trim area → entire block is dropped
/// - If content is mixed → only outside-trim operations are removed
/// - If all content is inside → entire block is preserved
///
/// # Examples
///
/// ```no_test
/// let operations = vec![/* PDF operations */];
/// let trim_rect = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
/// let filtered = filter_operations(&operations, trim_rect);
/// ```
fn filter_operations(operations: &[Operation], trim: Option<Rect>) -> Vec<Operation> {
    let mut output: Vec<Operation> = Vec::new();

    let mut ctm_stack: Vec<Matrix> = vec![Matrix::identity()];

    let mut block_stack: Vec<Vec<Operation>> = Vec::new();

    for operation in operations {
        match operation.operator.as_str() {
            "q" => {
                let last = ctm_stack.last().copied().unwrap_or(Matrix::identity());
                ctm_stack.push(last);
                block_stack.push(vec![operation.clone()]);
            }

            "Q" => {
                ctm_stack.pop();

                if let Some(mut block) = block_stack.pop() {
                    block.push(operation.clone());

                    // Scan the block: does it contain any outside-trim re+f?
                    // If ALL drawable content is outside → drop entire block.
                    // If MIXED → surgically remove outside-trim re f pairs.
                    // If all inside → flush as-is.
                    let filtered_block = filter_block(block, trim.as_ref(), &ctm_stack);

                    // Push filtered ops to the right place —
                    // either the parent block buffer or final output
                    if let Some(parent) = block_stack.last_mut() {
                        parent.extend(filtered_block);
                    } else {
                        output.extend(filtered_block);
                    }
                }
            }

            "cm" => {
                let m = operands_to_matrix(&operation.operands);
                if let Some(top) = ctm_stack.last_mut() {
                    *top = m.concat(top);
                } else {
                    ctm_stack.push(m);
                }

                if let Some(block) = block_stack.last_mut() {
                    block.push(operation.clone());
                } else {
                    output.push(operation.clone());
                }
            }

            // Literally everything else – just buffer or pass through
            _ => {
                if let Some(block) = block_stack.last_mut() {
                    block.push(operation.clone());
                } else {
                    output.push(operation.clone());
                }
            }
        }
    }
    let output = remove_outside_re_f_pairs(output, &Matrix::identity(), trim.as_ref());

    output
}

/// Filters a block of operations by removing those that fall outside the specified trimming rectangle.
///
/// This function takes a vector of operations and filters out any operations that are determined
/// to be outside the bounds of the trimming rectangle when transformed by the current transformation
/// matrix (CTM). It first checks if the entire block is outside the image bounds and returns
/// an empty vector early if so. Otherwise, it removes individual operations that fall outside
/// the trimming rectangle.
///
/// # Arguments
///
/// * `block` - A vector of `Operation` structs representing the operations to filter
/// * `trim` - An optional reference to a `Rect` that defines the trimming boundaries.
///   If `None`, no trimming is applied based on rectangle bounds.
/// * `ctm_stack` - A slice of `Matrix` elements representing the current transformation matrix stack.
///   The last element is used as the base CTM for filtering decisions.
///
/// # Returns
///
/// A new vector containing only the operations that fall within the trimming rectangle
/// after applying the current transformation matrix. Returns an empty vector if all
/// operations are determined to be outside the image bounds.
///
/// # Logic
///
/// 1. Extracts the base CTM from the stack (uses identity matrix if stack is empty)
/// 2. Checks if the entire block is outside the image bounds using `block_is_outside_image`
/// 3. If block is outside, returns empty vector immediately
/// 4. Otherwise, calls `remove_outside_re_f_pairs` to filter individual operations
fn filter_block(
    block: Vec<Operation>,
    trim: Option<&Rect>,
    ctm_stack: &[Matrix],
) -> Vec<Operation> {
    let base_ctm = ctm_stack.last().copied().unwrap_or(Matrix::identity());
    if block_is_outside_image(&block, &base_ctm, trim) {
        return vec![];
    }

    remove_outside_re_f_pairs(block, &base_ctm, trim)
}

/// Determines if a block of PDF operations renders content outside the specified trim area.
///
/// This function analyzes a sequence of PDF operations to determine if any content would be
/// rendered outside the specified trim rectangle. It tracks the current transformation matrix
/// (CTM) through save/restore operations and checks if transformed content falls outside
/// the trim bounds.
///
/// # Arguments
///
/// * `block` - A slice of PDF operations to analyze
/// * `base_ctm` - The base transformation matrix to start with
/// * `trim` - Optional trim rectangle to check against. If None, always returns false
///
/// # Returns
///
/// Returns `true` if content is detected that would render outside the trim area,
/// `false` otherwise or if no relevant content is found.
///
/// # Logic
///
/// The function maintains two stacks:
/// - `ctm_stack`: tracks the current transformation matrix through q/Q operations
/// - `has_cm_stack`: tracks whether a cm (concatenate matrix) operation has been applied
///
/// For each operation:
/// - "q": Push current CTM onto stack (save graphics state)
/// - "Q": Pop CTM from stack (restore graphics state)
/// - "cm": Apply matrix transformation to current CTM
/// - "Do": Check if transformed content is outside trim area
///
/// The "Do" operation triggers the actual check - if a transformation has been applied
/// and the resulting transform has a large enough scale factor (determinant > 2.0), it
/// transforms a unit rectangle and checks if it falls outside the trim area.
fn block_is_outside_image(block: &[Operation], base_ctm: &Matrix, trim: Option<&Rect>) -> bool {
    let mut ctm_stack: Vec<Matrix> = vec![*base_ctm];
    let mut has_cm_stack: Vec<bool> = vec![false];

    for operation in block {
        match operation.operator.as_str() {
            "q" => {
                let last = ctm_stack.last().cloned().unwrap_or(Matrix::identity());
                ctm_stack.push(last);
                has_cm_stack.push(false);
            }
            "Q" => {
                if !ctm_stack.is_empty() {
                    ctm_stack.pop();
                }
                has_cm_stack.pop();
            }
            "cm" => {
                let m = operands_to_matrix(&operation.operands);
                if let Some(top) = ctm_stack.last_mut() {
                    *top = m.concat(top)
                } else {
                    ctm_stack.push(m)
                }
                if let Some(flag) = has_cm_stack.last_mut() {
                    *flag = true;
                }
            }
            "Do" => {
                let has_ctm = has_cm_stack.last().copied().unwrap_or(false);
                if has_ctm && let Some(trim) = trim {
                    let ctm = ctm_stack.last().copied().unwrap_or(Matrix::identity());
                    let det = (ctm.a * ctm.d - ctm.b * ctm.c).abs();
                    if det > 2.0 {
                        let unit_rect = Rect::new(0.0, 0.0, 1.0, 1.0);
                        let page_rect = ctm.transform_rect(&unit_rect);
                        if page_rect.is_outside(trim) {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Removes path operations that are entirely outside the given trim rectangle.
///
/// This function processes a sequence of PDF graphics operations and removes
/// drawing commands (paths, rectangles) that fall completely outside of a
/// specified trimming rectangle. It handles both individual path construction
/// operators and rectangle fill operations.
///
/// The algorithm maintains a stack of coordinate transformation matrices (CTM)
/// to properly evaluate the position of graphical elements in device space.
/// Path construction operators like `m`, `l`, `c`, etc., are grouped together
/// until a path painting operator (`S`, `f`, `n`, etc.) is encountered.
///
/// Special care is taken for clipping paths (operators `W` and `W*`) which
/// are never removed, as they affect subsequent drawing operations even if
/// the clipping path itself is outside the visible area.
///
/// Rectangle operations (`re`) followed immediately by a fill (`f` or `f*`)
/// are evaluated directly against the trim rectangle using their bounding box.
///
/// # Arguments
///
/// * `block` - A vector of PDF operations to process
/// * `base_ctm` - The base coordinate transformation matrix for the current context
/// * `trim` - Optional rectangle defining the visible area; if None, no filtering is performed
///
/// # Returns
///
/// A new vector of operations with outside elements removed, preserving
/// the logical structure and rendering semantics of the original content.
fn remove_outside_re_f_pairs(
    block: Vec<Operation>,
    base_ctm: &Matrix,
    trim: Option<&Rect>,
) -> Vec<Operation> {
    let mut result: Vec<Operation> = Vec::new();
    let mut ctm_stack: Vec<Matrix> = vec![*base_ctm];
    let mut i = 0;

    let mut in_path = false;
    #[allow(clippy::type_complexity)]
    let mut subpaths: Vec<(Vec<Operation>, Vec<(f64, f64)>)> = Vec::new();
    let mut current_operation: Vec<Operation> = Vec::new();
    let mut current_points: Vec<(f64, f64)> = Vec::new();
    let mut has_clip = false; // set if a W/W* clipping operator appears in the path, these paths must never be dropped.

    while i < block.len() {
        let operation = &block[i];

        if in_path {
            match operation.operator.as_str() {
                "m" => {
                    subpaths.push((
                        std::mem::take(&mut current_operation),
                        std::mem::take(&mut current_points),
                    ));
                    let x = object_to_f64(&operation.operands[0]);
                    let y = object_to_f64(&operation.operands[1]);
                    current_operation = vec![operation.clone()];
                    current_points = vec![(x, y)];
                    i += 1;
                }
                "l" => {
                    current_points.push((
                        object_to_f64(&operation.operands[0]),
                        object_to_f64(&operation.operands[1]),
                    ));
                    current_operation.push(operation.clone());
                    i += 1;
                }
                "c" => {
                    // 6 operands: x1 y1 x2 y2 x3 y3
                    for chunk in operation.operands.chunks(2) {
                        current_points.push((object_to_f64(&chunk[0]), object_to_f64(&chunk[1])));
                    }
                    current_operation.push(operation.clone());
                    i += 1;
                }
                "v" | "y" => {
                    // 4 operands: two xy pairs
                    for chunk in operation.operands.chunks(2) {
                        current_points.push((object_to_f64(&chunk[0]), object_to_f64(&chunk[1])));
                    }
                    current_operation.push(operation.clone());
                    i += 1;
                }
                "h" => {
                    current_operation.push(operation.clone());
                    i += 1;
                }
                "re" => {
                    subpaths.push((
                        std::mem::take(&mut current_operation),
                        std::mem::take(&mut current_points),
                    ));
                    let x = object_to_f64(&operation.operands[0]);
                    let y = object_to_f64(&operation.operands[1]);
                    let w = object_to_f64(&operation.operands[2]);
                    let h_val = object_to_f64(&operation.operands[3]);
                    current_operation = vec![operation.clone()];
                    current_points = vec![(x, y), (x + w, y), (x + w, y + h_val), (x, y + h_val)];
                    i += 1;
                }
                "W" | "W*" => {
                    has_clip = true;
                    current_operation.push(operation.clone());
                    i += 1;
                }
                "S" | "s" | "f" | "f*" | "F" | "B" | "B*" | "b" | "b*" | "n" => {
                    subpaths.push((
                        std::mem::take(&mut current_operation),
                        std::mem::take(&mut current_points),
                    ));
                    in_path = false;
                    let paint = operation.operator.as_str();
                    let ctm = ctm_stack.last().copied().unwrap_or(Matrix::identity());

                    if has_clip || paint == "n" {
                        for (ops, _) in subpaths.drain(..) {
                            result.extend(ops);
                        }
                        result.push(operation.clone());
                    } else if paint == "S" || paint == "s" {
                        let mut kept: Vec<Operation> = Vec::new();
                        for (sub_ops, sub_pts) in subpaths.drain(..) {
                            let outside =
                                trim.is_some_and(|t| subpath_bbox_is_outside(&sub_pts, &ctm, t));
                            if !outside {
                                kept.extend(sub_ops);
                            }
                        }
                        if !kept.is_empty() {
                            result.extend(kept);
                            result.push(Operation {
                                operator: paint.to_string(),
                                operands: vec![],
                            });
                        }
                    } else {
                        let all_outside = trim.is_some_and(|t| {
                            !subpaths.is_empty()
                                && subpaths
                                    .iter()
                                    .all(|(_, pts)| subpath_bbox_is_outside(pts, &ctm, t))
                        });
                        if !all_outside {
                            for (ops, _) in subpaths.drain(..) {
                                result.extend(ops);
                            }
                            result.push(operation.clone());
                        }
                        subpaths.clear();
                    }
                    has_clip = false;
                    i += 1;
                }
                _ => {
                    subpaths.push((
                        std::mem::take(&mut current_operation),
                        std::mem::take(&mut current_points),
                    ));
                    for (ops, _) in subpaths.drain(..) {
                        result.extend(ops);
                    }
                    in_path = false;
                    has_clip = false;
                    result.push(operation.clone());
                    i += 1;
                }
            }
            continue;
        }

        match operation.operator.as_str() {
            "q" => {
                let last = ctm_stack.last().copied().unwrap_or(Matrix::identity());
                ctm_stack.push(last);
                result.push(operation.clone());
                i += 1;
            }
            "Q" => {
                if !ctm_stack.is_empty() {
                    ctm_stack.pop();
                }
                result.push(operation.clone());
                i += 1;
            }
            "cm" => {
                let m = operands_to_matrix(&operation.operands);
                if let Some(top) = ctm_stack.last_mut() {
                    *top = m.concat(top);
                } else {
                    ctm_stack.push(m);
                };
                result.push(operation.clone());
                i += 1;
            }
            "re" => {
                let next_operation = block.get(i + 1).map(|o| o.operator.as_str());
                if next_operation == Some("f") || next_operation == Some("f*") {
                    if let Some(trim) = trim {
                        let local_ctm = ctm_stack.last().copied().unwrap_or(Matrix::identity());
                        if re_is_outside(&operation.operands, &local_ctm, trim) {
                            i += 2;
                            continue;
                        }
                    }
                    result.push(operation.clone());
                    i += 1;
                } else {
                    in_path = true;
                    subpaths.clear();
                    has_clip = false;
                    let x = object_to_f64(&operation.operands[0]);
                    let y = object_to_f64(&operation.operands[1]);
                    let w = object_to_f64(&operation.operands[2]);
                    let h_val = object_to_f64(&operation.operands[3]);
                    current_operation = vec![operation.clone()];
                    current_points = vec![(x, y), (x + w, y), (x + w, y + h_val), (x, y + h_val)];
                    i += 1;
                }
            }
            "m" => {
                in_path = true;
                subpaths.clear();
                has_clip = false;
                let x = object_to_f64(&operation.operands[0]);
                let y = object_to_f64(&operation.operands[1]);
                current_operation = vec![operation.clone()];
                current_points = vec![(x, y)];
                i += 1;
            }
            _ => {
                result.push(operation.clone());
                i += 1;
            }
        }
    }
    result
}

fn collect_referenced_resources(operations: &[Operation]) -> HashSet<Vec<u8>> {
    let mut names = HashSet::new();
    for operation in operations {
        match operation.operator.as_str() {
            "gs" | "Do" | "cs" | "CS" | "scn" | "SCN" | "sh" => {
                if let Some(lopdf::Object::Name(n)) = operation.operands.first() {
                    names.insert(n.clone());
                }
            }
            "Tf" => {
                if let Some(lopdf::Object::Name(n)) = operation.operands.first() {
                    names.insert(n.clone());
                }
            }
            _ => {}
        }
    }
    names
}

fn prune_page_resources(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    referenced: &HashSet<Vec<u8>>,
) -> lopdf::Result<()> {
    let page = document.get_dictionary(page_id)?;
    let resources_obj = page.get(b"Resources")?;

    // Resources can be an indirect reference or an inline dictionary.
    let resources_id = match resources_obj {
        Object::Reference(id) => *id,
        Object::Dictionary(_) => {
            // Inline dict — it lives inside the page object itself.
            // We need the page object's id to mutate it later.
            page_id
        }
        _ => return Ok(()),
    };
    let is_inline = resources_id == page_id;

    // Pass 1: collect indirect sub-dict ObjectIds and inline keys (immutable borrow)
    let mut indirect_subs: Vec<lopdf::ObjectId> = Vec::new();
    let mut inline_keys: Vec<Vec<u8>> = Vec::new();
    {
        let resources = if is_inline {
            let page_dict = document.get_dictionary(page_id)?;
            page_dict.get(b"Resources")?.as_dict()?
        } else {
            document.get_dictionary(resources_id)?
        };
        for key in &[b"ExtGState" as &[u8], b"Font", b"XObject", b"ColorSpace"] {
            match resources.get(key) {
                Ok(Object::Reference(sub_id)) => indirect_subs.push(*sub_id),
                Ok(Object::Dictionary(_)) => inline_keys.push(key.to_vec()),
                _ => {}
            }
        }
    }

    // Pass 2a: prune indirect sub-dictionaries
    for sub_id in indirect_subs {
        if let Ok(sub_dict) = document.get_object_mut(sub_id)?.as_dict_mut() {
            let to_remove: Vec<Vec<u8>> = sub_dict
                .iter()
                .filter(|(name, _)| !referenced.contains(*name))
                .map(|(name, _)| name.clone())
                .collect();
            for name in to_remove {
                sub_dict.remove(&name);
            }
        }
    }

    // Pass 2b: prune inline sub-dictionaries
    let resources_dict = if is_inline {
        let page_dict = document.get_object_mut(page_id)?.as_dict_mut()?;
        page_dict.get_mut(b"Resources")?.as_dict_mut()?
    } else {
        document.get_object_mut(resources_id)?.as_dict_mut()?
    };
    for key in &inline_keys {
        if let Ok(Object::Dictionary(sub_dict)) = resources_dict.get_mut(key.as_slice()) {
            let to_remove: Vec<Vec<u8>> = sub_dict
                .iter()
                .filter(|(name, _)| !referenced.contains(*name))
                .map(|(name, _)| name.clone())
                .collect();
            for name in to_remove {
                sub_dict.remove(&name);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::boxes::object_to_f64;
    use lopdf::Object;
    use lopdf::content::Operation;

    // ── object_to_f64 ───────────────────────────────────────────────

    #[test]
    fn object_to_f64_integer() {
        assert!((object_to_f64(&Object::Integer(42)) - 42.0).abs() < 1e-10);
    }

    #[test]
    fn object_to_f64_real() {
        assert!((object_to_f64(&Object::Real(3.14)) - 3.14 as f64).abs() < 0.01);
    }

    #[test]
    fn object_to_f64_negative() {
        assert!((object_to_f64(&Object::Integer(-7)) - (-7.0)).abs() < 1e-10);
        assert!((object_to_f64(&Object::Real(-2.5)) - (-2.5 as f64)).abs() < 0.01);
    }

    // ── operands_to_rect ────────────────────────────────────────────

    #[test]
    fn operands_to_rect_basic() {
        let ops = vec![
            Object::Real(10.0),
            Object::Real(20.0),
            Object::Real(100.0),
            Object::Real(50.0),
        ];
        let r = operands_to_rect(&ops);
        assert!((r.x - 10.0).abs() < 1e-10);
        assert!((r.y - 20.0).abs() < 1e-10);
        // from_corners(10, 20, 110, 70) → width=100, height=50
        assert!((r.width - 100.0).abs() < 1e-10);
        assert!((r.height - 50.0).abs() < 1e-10);
    }

    #[test]
    fn operands_to_rect_integers() {
        let ops = vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ];
        let r = operands_to_rect(&ops);
        assert!((r.x - 0.0).abs() < 1e-10);
        assert!((r.y - 0.0).abs() < 1e-10);
        assert!((r.width - 612.0).abs() < 1e-10);
        assert!((r.height - 792.0).abs() < 1e-10);
    }

    #[test]
    fn operands_to_rect_negative_dims() {
        // Negative width/height (common in PDF: re with negative w or h)
        let ops = vec![
            Object::Real(100.0),
            Object::Real(200.0),
            Object::Real(-50.0),
            Object::Real(-30.0),
        ];
        let r = operands_to_rect(&ops);
        // from_corners(100, 200, 50, 170) → normalised: x=50, y=170, w=50, h=30
        assert!((r.x - 50.0).abs() < 1e-10);
        assert!((r.y - 170.0).abs() < 1e-10);
        assert!((r.width - 50.0).abs() < 1e-10);
        assert!((r.height - 30.0).abs() < 1e-10);
    }

    // ── operands_to_matrix ──────────────────────────────────────────

    #[test]
    fn operands_to_matrix_identity() {
        let ops = vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ];
        let m = operands_to_matrix(&ops);
        let (x, y) = m.transform_point(5.0, 7.0);
        assert!((x - 5.0).abs() < 1e-10);
        assert!((y - 7.0).abs() < 1e-10);
    }

    #[test]
    fn operands_to_matrix_translation() {
        let ops = vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(100.0),
            Object::Real(200.0),
        ];
        let m = operands_to_matrix(&ops);
        let (x, y) = m.transform_point(0.0, 0.0);
        assert!((x - 100.0).abs() < 1e-10);
        assert!((y - 200.0).abs() < 1e-10);
    }

    #[test]
    fn operands_to_matrix_known_ctm() {
        // The actual CTM from the test PDF's PlacedPDF block
        let ops = vec![
            Object::Real(1.02883),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(-1.03942),
            Object::Real(336.0),
            Object::Real(426.0),
        ];
        let m = operands_to_matrix(&ops);
        assert!((m.a - 1.02883).abs() < 1e-5);
        assert!((m.d - (-1.03942)).abs() < 1e-5);
        assert!((m.e - 336.0).abs() < 1e-5);
        assert!((m.f - 426.0).abs() < 1e-5);
    }

    // ── re_is_outside ───────────────────────────────────────────────

    #[test]
    fn re_is_outside_identity_ctm_inside() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        // Rect fully inside
        let ops = vec![
            Object::Real(100.0),
            Object::Real(100.0),
            Object::Real(50.0),
            Object::Real(50.0),
        ];
        assert!(!re_is_outside(&ops, &ctm, &trim));
    }

    #[test]
    fn re_is_outside_identity_ctm_outside() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        // Rect fully outside (right)
        let ops = vec![
            Object::Real(650.0),
            Object::Real(100.0),
            Object::Real(50.0),
            Object::Real(50.0),
        ];
        assert!(re_is_outside(&ops, &ctm, &trim));
    }

    #[test]
    fn re_is_outside_straddling() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        // Straddling right edge
        let ops = vec![
            Object::Real(635.0),
            Object::Real(100.0),
            Object::Real(20.0),
            Object::Real(10.0),
        ];
        assert!(!re_is_outside(&ops, &ctm, &trim));
    }

    #[test]
    fn re_is_outside_with_ctm_transform() {
        // CTM that translates +700 in x → pushes rect outside
        let ctm = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 700.0, 0.0);
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        let ops = vec![
            Object::Real(0.0),
            Object::Real(100.0),
            Object::Real(50.0),
            Object::Real(50.0),
        ];
        assert!(re_is_outside(&ops, &ctm, &trim));
    }

    // ── subpath_bbox_is_outside ─────────────────────────────────────

    #[test]
    fn subpath_empty_is_not_outside() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(0.0, 0.0, 100.0, 100.0);
        assert!(!subpath_bbox_is_outside(&[], &ctm, &trim));
    }

    #[test]
    fn subpath_inside() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(0.0, 0.0, 100.0, 100.0);
        let pts = vec![(10.0, 10.0), (50.0, 50.0), (30.0, 70.0)];
        assert!(!subpath_bbox_is_outside(&pts, &ctm, &trim));
    }

    #[test]
    fn subpath_outside() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(0.0, 0.0, 100.0, 100.0);
        let pts = vec![(200.0, 200.0), (300.0, 300.0)];
        assert!(subpath_bbox_is_outside(&pts, &ctm, &trim));
    }

    #[test]
    fn subpath_with_ctm_moves_inside_outside() {
        // Points are at (10, 10) which is inside (0,0,100,100)
        // But CTM translates by +200 → (210, 210) which is outside
        let ctm = Matrix::from_values(1.0, 0.0, 0.0, 1.0, 200.0, 200.0);
        let trim = Rect::from_corners(0.0, 0.0, 100.0, 100.0);
        let pts = vec![(10.0, 10.0)];
        assert!(subpath_bbox_is_outside(&pts, &ctm, &trim));
    }

    // ── filter_operations ───────────────────────────────────────────

    fn op(operator: &str, operands: Vec<Object>) -> Operation {
        Operation {
            operator: operator.to_string(),
            operands,
        }
    }

    #[test]
    fn filter_operations_no_trim_passes_all() {
        let ops = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(10.0),
                    Object::Real(10.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let result = filter_operations(&ops, None);
        assert_eq!(result.len(), ops.len());
    }

    #[test]
    fn filter_operations_removes_outside_re_f() {
        let trim = Some(Rect::from_corners(30.0, 30.0, 642.0, 822.0));
        // A re+f completely outside the trim (x=700, way past right edge 642),
        // wrapped in q/Q since filter_operations only filters inside blocks.
        let ops = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(700.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let result = filter_operations(&ops, trim);
        // Should only contain balanced q/Q with the outside re+f removed
        let non_q: Vec<_> = result
            .iter()
            .filter(|o| o.operator != "q" && o.operator != "Q")
            .collect();
        assert!(
            non_q.is_empty(),
            "outside re+f should be removed, got {:?}",
            non_q.iter().map(|o| &o.operator).collect::<Vec<_>>()
        );
    }

    #[test]
    fn filter_operations_keeps_inside_re_f() {
        let trim = Some(Rect::from_corners(30.0, 30.0, 642.0, 822.0));
        let ops = vec![
            op(
                "re",
                vec![
                    Object::Real(100.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
        ];
        let result = filter_operations(&ops, trim);
        assert_eq!(result.len(), 2, "inside re+f should be kept");
    }

    #[test]
    fn filter_operations_block_all_outside_dropped() {
        let trim = Some(Rect::from_corners(30.0, 30.0, 642.0, 822.0));
        // q block with cm that moves things far outside + Do
        let ops = vec![
            op("q", vec![]),
            op(
                "cm",
                vec![
                    Object::Real(100.0),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(100.0),
                    Object::Real(1000.0),
                    Object::Real(1000.0),
                ],
            ),
            op("Do", vec![Object::Name(b"Im1".to_vec())]),
            op("Q", vec![]),
        ];
        let result = filter_operations(&ops, trim);
        // Entire block should be dropped since the image is outside
        assert!(
            result.is_empty() || !result.iter().any(|o| o.operator == "Do"),
            "outside image block should be dropped"
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn filter_operations_preserves_q_Q_balance() {
        let trim = Some(Rect::from_corners(30.0, 30.0, 642.0, 822.0));
        let ops = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(100.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(700.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let result = filter_operations(&ops, trim);
        let q_count = result.iter().filter(|o| o.operator == "q").count();
        let big_q_count = result.iter().filter(|o| o.operator == "Q").count();
        assert_eq!(q_count, big_q_count, "q/Q must be balanced");
    }

    // ── block_is_outside_image ──────────────────────────────────────

    #[test]
    fn block_no_do_is_not_outside() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        let block = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(100.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        assert!(!block_is_outside_image(&block, &ctm, Some(&trim)));
    }

    #[test]
    fn block_image_outside_trim_detected() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        // cm places a large image way outside
        let block = vec![
            op("q", vec![]),
            op(
                "cm",
                vec![
                    Object::Real(500.0),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(500.0),
                    Object::Real(2000.0),
                    Object::Real(2000.0),
                ],
            ),
            op("Do", vec![Object::Name(b"Im1".to_vec())]),
            op("Q", vec![]),
        ];
        assert!(block_is_outside_image(&block, &ctm, Some(&trim)));
    }

    #[test]
    fn block_image_inside_trim_not_detected() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        // cm places image inside trim
        let block = vec![
            op("q", vec![]),
            op(
                "cm",
                vec![
                    Object::Real(100.0),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(100.0),
                    Object::Real(100.0),
                    Object::Real(100.0),
                ],
            ),
            op("Do", vec![Object::Name(b"Im1".to_vec())]),
            op("Q", vec![]),
        ];
        assert!(!block_is_outside_image(&block, &ctm, Some(&trim)));
    }

    #[test]
    fn block_no_trim_always_false() {
        let ctm = Matrix::identity();
        let block = vec![
            op("q", vec![]),
            op(
                "cm",
                vec![
                    Object::Real(500.0),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(500.0),
                    Object::Real(2000.0),
                    Object::Real(2000.0),
                ],
            ),
            op("Do", vec![Object::Name(b"Im1".to_vec())]),
            op("Q", vec![]),
        ];
        assert!(!block_is_outside_image(&block, &ctm, None));
    }

    // ── collect_referenced_resources ────────────────────────────────

    #[test]
    fn collects_do_names() {
        let ops = vec![
            op("Do", vec![Object::Name(b"Im1".to_vec())]),
            op("Do", vec![Object::Name(b"Im2".to_vec())]),
        ];
        let refs = collect_referenced_resources(&ops);
        assert!(refs.contains(&b"Im1".to_vec()));
        assert!(refs.contains(&b"Im2".to_vec()));
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn collects_gs_and_tf_names() {
        let ops = vec![
            op("gs", vec![Object::Name(b"GS0".to_vec())]),
            op(
                "Tf",
                vec![Object::Name(b"F1".to_vec()), Object::Integer(12)],
            ),
        ];
        let refs = collect_referenced_resources(&ops);
        assert!(refs.contains(&b"GS0".to_vec()));
        assert!(refs.contains(&b"F1".to_vec()));
    }

    #[test]
    fn collects_colorspace_names() {
        let ops = vec![
            op("cs", vec![Object::Name(b"CS0".to_vec())]),
            op("CS", vec![Object::Name(b"CS1".to_vec())]),
            op("scn", vec![Object::Name(b"P0".to_vec())]),
            op("sh", vec![Object::Name(b"Sh0".to_vec())]),
        ];
        let refs = collect_referenced_resources(&ops);
        assert!(refs.contains(&b"CS0".to_vec()));
        assert!(refs.contains(&b"CS1".to_vec()));
        assert!(refs.contains(&b"P0".to_vec()));
        assert!(refs.contains(&b"Sh0".to_vec()));
    }

    #[test]
    fn ignores_non_resource_ops() {
        let ops = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(10.0),
                    Object::Real(10.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let refs = collect_referenced_resources(&ops);
        assert!(refs.is_empty());
    }

    // ── remove_outside_re_f_pairs ───────────────────────────────────

    #[test]
    fn removes_outside_re_f_pair() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        let block = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(700.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let result = remove_outside_re_f_pairs(block, &ctm, Some(&trim));
        // q/Q stay, but re+f should be removed
        let has_re = result.iter().any(|o| o.operator == "re");
        assert!(!has_re, "outside re should be removed");
    }

    #[test]
    fn keeps_inside_re_f_pair() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        let block = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(100.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let result = remove_outside_re_f_pairs(block, &ctm, Some(&trim));
        let has_re = result.iter().any(|o| o.operator == "re");
        assert!(has_re, "inside re should be kept");
    }

    #[test]
    fn preserves_clipping_paths() {
        let ctm = Matrix::identity();
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        // Even though rect is outside, W (clip) must force keeping it
        let block = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(700.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("W", vec![]),
            op("n", vec![]),
            op("Q", vec![]),
        ];
        let result = remove_outside_re_f_pairs(block, &ctm, Some(&trim));
        let has_w = result.iter().any(|o| o.operator == "W");
        assert!(has_w, "clipping path must be preserved even if outside");
    }

    #[test]
    fn no_trim_keeps_everything() {
        let ctm = Matrix::identity();
        let block = vec![
            op("q", vec![]),
            op(
                "re",
                vec![
                    Object::Real(700.0),
                    Object::Real(100.0),
                    Object::Real(50.0),
                    Object::Real(50.0),
                ],
            ),
            op("f", vec![]),
            op("Q", vec![]),
        ];
        let result = remove_outside_re_f_pairs(block.clone(), &ctm, None);
        assert_eq!(result.len(), block.len());
    }

    // ── filter_page (via fixture) ───────────────────────────────────

    fn fixture() -> Option<(lopdf::Document, lopdf::ObjectId)> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf_test_data_print_v2.pdf");
        if !path.exists() {
            return None;
        }
        let file = std::fs::File::open(&path).ok()?;
        let doc = lopdf::Document::load_from(file).ok()?;
        let page_id = doc.get_pages()[&1];
        Some((doc, page_id))
    }

    #[test]
    fn filter_page_reduces_operations() {
        let Some((mut doc, page_id)) = fixture() else {
            return;
        };
        let before = doc
            .get_and_decode_page_content(page_id)
            .unwrap()
            .operations
            .len();

        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        ContentFilter::filter_page(&mut doc, page_id, &trim).unwrap();

        let after = doc
            .get_and_decode_page_content(page_id)
            .unwrap()
            .operations
            .len();
        assert!(after < before, "before={before}, after={after}");
    }

    #[test]
    fn filter_page_keeps_at_least_one_do() {
        let Some((mut doc, page_id)) = fixture() else {
            return;
        };
        let trim = Rect::from_corners(30.0, 30.0, 642.0, 822.0);
        ContentFilter::filter_page(&mut doc, page_id, &trim).unwrap();

        let content = doc.get_and_decode_page_content(page_id).unwrap();
        let do_count = content
            .operations
            .iter()
            .filter(|o| o.operator == "Do")
            .count();
        assert!(do_count >= 1, "at least one Do should survive");
    }
}
