#[cfg(feature = "raster")]
use crate::encode::OutputFormat;
use crate::pages::PageBoxes;
#[cfg(feature = "raster")]
use crate::raster::RenderConfig;
use crate::stream::{ColorRemap, ContentFilter};
#[cfg(feature = "raster")]
use image::DynamicImage;
use lopdf::Document;
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
fn atomic_write_file(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    use std::io::Write;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".rustybara-")
        .tempfile_in(parent)?;

    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }

    write(temporary.as_file_mut())?;
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(feature = "raster")]
fn can_reuse_pdfium_binding(error: &pdfium_render::prelude::PdfiumError) -> bool {
    matches!(
        error,
        pdfium_render::prelude::PdfiumError::PdfiumLibraryBindingsAlreadyInitialized
    )
}

/// Describes the overall color operator usage found across a PDF document's content streams.
///
/// Returned by [`PdfPipeline::detect_color_space`]. Distinct from ICC profile classification,
/// which identifies individual color profiles.
///
/// # Variants
///
/// * `PureCMYK` — Only CMYK color operators (`k`, `K`) were found
/// * `PureRGB` — Only RGB color operators (`rg`, `RG`) were found
/// * `Mixed` — Both CMYK and RGB operators are present
/// * `Unknown` — No recognizable color operators were found
pub enum DocumentColorKind {
    PureCMYK,
    PureRGB,
    Mixed,
    Unknown,
}

#[derive(Default)]
struct DocumentColorUsage {
    has_cmyk: bool,
    has_rgb: bool,
}

impl DocumentColorUsage {
    fn classify_name(&mut self, name: &[u8]) {
        match name {
            b"DeviceCMYK" => self.has_cmyk = true,
            b"DeviceRGB" | b"CalRGB" => self.has_rgb = true,
            _ => {}
        }
    }

    fn kind(&self) -> DocumentColorKind {
        match (self.has_cmyk, self.has_rgb) {
            (true, true) => DocumentColorKind::Mixed,
            (true, false) => DocumentColorKind::PureCMYK,
            (false, true) => DocumentColorKind::PureRGB,
            (false, false) => DocumentColorKind::Unknown,
        }
    }
}

fn classify_color_space_object(
    doc: &Document,
    object: &lopdf::Object,
    usage: &mut DocumentColorUsage,
) {
    let Ok((_, resolved)) = doc.dereference(object) else {
        return;
    };

    match resolved {
        lopdf::Object::Name(name) => usage.classify_name(name),
        lopdf::Object::Array(items) => {
            let Some(first) = items.first() else { return };
            let Ok((_, lopdf::Object::Name(family))) = doc.dereference(first) else {
                return;
            };
            match family.as_slice() {
                b"ICCBased" => {
                    if let Some(profile) = items.get(1)
                        && let Ok((_, lopdf::Object::Stream(stream))) = doc.dereference(profile)
                    {
                        match stream.dict.get(b"N").and_then(lopdf::Object::as_i64) {
                            Ok(3) => usage.has_rgb = true,
                            Ok(4) => usage.has_cmyk = true,
                            _ => {}
                        }
                    }
                }
                b"Indexed" | b"Pattern" => {
                    if let Some(base) = items.get(1) {
                        classify_color_space_object(doc, base, usage);
                    }
                }
                b"Separation" | b"DeviceN" => {
                    if let Some(alternate) = items.get(2) {
                        classify_color_space_object(doc, alternate, usage);
                    }
                }
                other => usage.classify_name(other),
            }
        }
        _ => {}
    }
}

fn scan_resource_colors(
    doc: &Document,
    resources: &lopdf::Dictionary,
    usage: &mut DocumentColorUsage,
    visited: &mut std::collections::HashSet<lopdf::ObjectId>,
) {
    if let Ok(color_spaces) = resources.get(b"ColorSpace")
        && let Ok((_, resolved)) = doc.dereference(color_spaces)
        && let Ok(dictionary) = resolved.as_dict()
    {
        for (_, color_space) in dictionary.iter() {
            classify_color_space_object(doc, color_space, usage);
        }
    }

    if let Ok(xobjects) = resources.get(b"XObject")
        && let Ok((_, resolved)) = doc.dereference(xobjects)
        && let Ok(dictionary) = resolved.as_dict()
    {
        for (_, xobject) in dictionary.iter() {
            let Ok((object_id, lopdf::Object::Stream(stream))) = doc.dereference(xobject) else {
                continue;
            };
            if let Some(object_id) = object_id
                && !visited.insert(object_id)
            {
                continue;
            }

            match stream.dict.get(b"Subtype").and_then(lopdf::Object::as_name) {
                Ok(b"Image") => {
                    if let Ok(color_space) = stream.dict.get(b"ColorSpace") {
                        classify_color_space_object(doc, color_space, usage);
                    }
                }
                Ok(b"Form") => {
                    if let Ok(form_resources) = stream.dict.get(b"Resources")
                        && let Ok((_, resolved)) = doc.dereference(form_resources)
                        && let Ok(dictionary) = resolved.as_dict()
                    {
                        scan_resource_colors(doc, dictionary, usage, visited);
                    }
                }
                _ => {}
            }
        }
    }

    if let Ok(shadings) = resources.get(b"Shading")
        && let Ok((_, resolved)) = doc.dereference(shadings)
        && let Ok(dictionary) = resolved.as_dict()
    {
        for (_, shading) in dictionary.iter() {
            let Ok((_, resolved)) = doc.dereference(shading) else {
                continue;
            };
            let shading_dictionary = match resolved {
                lopdf::Object::Dictionary(dictionary) => Some(dictionary),
                lopdf::Object::Stream(stream) => Some(&stream.dict),
                _ => None,
            };
            if let Some(dictionary) = shading_dictionary
                && let Ok(color_space) = dictionary.get(b"ColorSpace")
            {
                classify_color_space_object(doc, color_space, usage);
            }
        }
    }
}

/// High-level pipeline for PDF preprocessing operations.
///
/// `PdfPipeline` wraps a `lopdf::Document` and provides a chainable API for common
/// prepress operations like trimming marks, resizing pages, remapping colors, and
/// exporting to images.
///
/// # Examples
///
/// ```no_run
/// use rustybara::PdfPipeline;
///
/// # fn main() -> rustybara::Result<()> {
/// // Chain multiple operations
/// PdfPipeline::open("input.pdf")?
///     .trim()?                    // Remove content outside TrimBox
///     .resize(9.0)?               // Add 9pt bleed
///     .save_pdf("output.pdf")?;
/// # Ok(())
/// # }
/// ```
///
/// ```no_run
/// use rustybara::{PdfPipeline, encode::OutputFormat, raster::RenderConfig};
///
/// # fn main() -> rustybara::Result<()> {
/// let pipeline = PdfPipeline::open("document.pdf")?;
/// let config = RenderConfig::prepress(); // 300 DPI
///
/// // Export first page as JPEG
/// pipeline.save_page_image(0, "page_1.jpg", &OutputFormat::Jpg, &config, 90)?;
/// # Ok(())
/// # }
/// ```
pub struct PdfPipeline {
    doc: Document,
}

impl PdfPipeline {
    /// Returns a reference to the underlying `lopdf` document.
    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// Opens a PDF from `path` and wraps it in a new pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed as a PDF.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rustybara::PdfPipeline;
    /// # fn main() -> rustybara::Result<()> {
    /// let pipeline = PdfPipeline::open("input.pdf")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn open(path: impl AsRef<Path>) -> crate::Result<Self> {
        let doc = Document::load(path)?;
        Ok(Self { doc })
    }

    /// Opens a document from raw PDF bytes.
    ///
    /// Use this in environments without a filesystem (e.g., WebAssembly) where the PDF
    /// data is already in memory.
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        let doc = Document::load_mem(bytes)?;
        Ok(Self { doc })
    }

    /// Removes all PDF content outside each page's `TrimBox`.
    ///
    /// Walks every page's content stream and drops paths, images, and fills that lie
    /// entirely outside the `TrimBox` boundary. Pages without a `TrimBox` fall back
    /// to the `MediaBox` and are left unchanged.
    ///
    /// Call this before [`Self::resize`] to discard printer marks and bleed content
    /// that should not appear in the final output.
    ///
    /// # Errors
    ///
    /// Returns an error if any page's content stream cannot be decoded or re-encoded.
    pub fn trim(&mut self) -> crate::Result<&mut Self> {
        ContentFilter::remove_outside_trim(&mut self.doc)?;
        Ok(self)
    }

    /// Resizes the document's page boxes by applying bleed margins.
    ///
    /// This method adjusts the MediaBox (and optionally CropBox) of all pages in the document
    /// by expanding them outward by the specified bleed points. Bleed is extra space added
    /// around the edges of a page to ensure proper printing and trimming.
    ///
    /// # Arguments
    ///
    /// * `bleed_pts` - The amount of bleed margin to add in points (1/72 of an inch)
    ///
    /// # Returns
    ///
    /// Returns a mutable reference to self on success, or an error if page box operations fail.
    ///
    /// # Errors
    ///
    /// Returns an error if any page's box entries cannot be read or written.
    pub fn resize(&mut self, bleed_pts: f64) -> crate::Result<&mut Self> {
        let pages = self.doc.get_pages();
        for &page_id in pages.values() {
            let boxes = PageBoxes::read(&self.doc, page_id)?;
            let new_media = boxes.bleed_rect(bleed_pts).to_pdf_array();
            let page_dict = self.doc.get_dictionary_mut(page_id)?;
            let arr: Vec<lopdf::Object> = new_media.iter().map(|&v| v.into()).collect();
            let has_cropbox = page_dict.has(b"CropBox");
            page_dict.set(b"MediaBox", arr.clone());
            if has_cropbox {
                page_dict.set(b"CropBox", arr);
            }
        }
        Ok(self)
    }

    /// Sets a `TrimBox` on every page by insetting the `MediaBox` by `bleed_pts` on all sides.
    ///
    /// Use this when a PDF arrives without a `TrimBox` but the bleed extent is known. The most
    /// common prepress default is `9.0` points (⅛ inch / ~3.175 mm). Any existing `TrimBox` is
    /// overwritten. This is the inverse of [`Self::resize`], which expands the `MediaBox` outward.
    ///
    /// # Errors
    ///
    /// Returns an error if any page dictionary cannot be accessed.
    pub fn add_trim_box(&mut self, bleed_pts: f64) -> crate::Result<&mut Self> {
        crate::pages::set_trim_boxes(&mut self.doc, bleed_pts)?;
        Ok(self)
    }

    /// Crops every page to an exact `width_pts` × `height_pts` MediaBox,
    /// centered on the current media. Dimensions are in PDF points (1/72").
    ///
    /// See [`crate::pages::set_media_box`] for the cropping/centering semantics.
    ///
    /// # Errors
    ///
    /// Returns an error if `width_pts` or `height_pts` is not positive, or if a
    /// page dictionary cannot be accessed.
    pub fn set_media_box(&mut self, width_pts: f64, height_pts: f64) -> crate::Result<&mut Self> {
        crate::pages::set_media_box(&mut self.doc, width_pts, height_pts)?;
        Ok(self)
    }

    /// Rotates every page by `degrees`, which must be a multiple of 90.
    ///
    /// The rotation is applied **additively** to each page's existing `/Rotate`
    /// entry and normalized into `[0, 360)`. Only the page's display rotation is
    /// changed — content streams and page boxes are left untouched.
    ///
    /// # Errors
    /// Returns an error if `degrees` is not a multiple of 90, or if a page
    /// dictionary cannot be accessed.
    pub fn rotate(&mut self, degrees: i32) -> crate::Result<&mut Self> {
        if degrees % 90 != 0 {
            return Err(crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rotation must be a multiple of 90",
            )));
        }
        let pages = self.doc.get_pages();
        for &page_id in pages.values() {
            let page = self.doc.get_dictionary_mut(page_id)?;
            let current = page
                .get(b"Rotate")
                .ok()
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0);
            let new_rotate = (current + degrees as i64).rem_euclid(360);
            page.set(b"Rotate", lopdf::Object::Integer(new_rotate));
        }
        Ok(self)
    }

    /// Extracts a subset of pages into a new [`PdfPipeline`].
    ///
    /// Page numbers are **zero-indexed** — page `0` is the first page, consistent with
    /// [`Self::save_page_image`]. Out-of-range values are silently ignored; output page order
    /// always matches the original document.
    ///
    /// # Errors
    ///
    /// Returns an error if `page_nums` contains no valid indices or if the page tree cannot
    /// be rewritten.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rustybara::PdfPipeline;
    /// let doc = PdfPipeline::open("brochure.pdf").unwrap();
    /// let page_count = doc.page_count() as u32;
    /// let mut cover = doc.extract_pages(&[0]).unwrap();
    /// let mut body  = doc.extract_pages(&(1..page_count).collect::<Vec<_>>()).unwrap();
    /// cover.save_pdf("cover.pdf").unwrap();
    /// body.save_pdf("body.pdf").unwrap();
    /// ```
    pub fn extract_pages(&self, page_nums: &[u32]) -> crate::Result<Self> {
        let doc = crate::pages::extract_pages(&self.doc, page_nums)?;
        Ok(Self { doc })
    }

    /// Splits each page wider than `panel_width_pts` into left/right halves.
    ///
    /// Returns a new pipeline whose pages are the split panels, in document order.
    ///
    /// # Errors
    ///
    /// Returns an error if the page tree cannot be rewritten.
    pub fn split_pages(&self, panel_width_pts: f64) -> crate::Result<Self> {
        let doc = crate::pages::split_pages(&self.doc, panel_width_pts)?;
        Ok(Self { doc })
    }

    /// Stitches adjacent page pairs into spreads of `spread_width_pts`.
    ///
    /// Returns a new pipeline whose pages are the stitched spreads, in document order.
    ///
    /// # Errors
    ///
    /// Returns an error if the page tree cannot be rewritten.
    pub fn stitch_pages(&self, spread_width_pts: f64) -> crate::Result<Self> {
        let doc = crate::pages::stitch_pages(&self.doc, spread_width_pts)?;
        Ok(Self { doc })
    }

    /// Reads the `rbara:` XMP block embedded by a previous rustybara run, if any.
    ///
    /// Returns `None` for files that have never been processed by rustybara.
    /// Use this to display provenance info, check for already-applied ops, or detect
    /// stale outputs before re-processing.
    pub fn read_xmp_block(&self) -> Option<crate::xmp::RbaraXmpBlock> {
        use crate::xmp;
        use lopdf::Object;

        let catalog_id = self
            .doc
            .trailer
            .get(b"Root")
            .ok()
            .and_then(|o| o.as_reference().ok())?;

        let cat = match self.doc.get_object(catalog_id) {
            Ok(Object::Dictionary(d)) => d.clone(),
            _ => return None,
        };

        let meta_id = cat
            .get(b"Metadata")
            .ok()
            .and_then(|o| o.as_reference().ok())?;

        let xmp_bytes = match self.doc.get_object(meta_id) {
            Ok(Object::Stream(s)) => s
                .decompressed_content()
                .unwrap_or_else(|_| s.content.clone()),
            _ => return None,
        };

        xmp::parse_rbara_block(&xmp_bytes)
    }

    /// Embeds rustybara processing metadata into the document's XMP stream.
    ///
    /// Call this after all processing operations and before [`Self::save_pdf`].
    /// If the document already has an XMP stream, the existing `rbara:` block is
    /// replaced in-place; all other XMP namespaces (`dc:`, `pdf:`, etc.) are
    /// preserved. If the input was previously processed by rustybara, its
    /// `rbara:uuid` is promoted to `rbara:parentId`, forming a lineage chain.
    ///
    /// # Arguments
    ///
    /// * `source_hash` – `"sha256:<hex>"` computed by [`crate::xmp::hash_file`]
    /// on the original input bytes **before** any mutations.
    /// * `timestamp` – ISO 8601 string (supplied by the caller; chrono is not a
    /// rustybara dependency).
    /// * `ops` – ordered `(name, params)` pairs, e.g.
    ///   `&[("resize", "bleed_in=0.125"), ("trim", "")]`. Empty params are omitted.
    ///
    ///   Silently skips embedding if the PDF catalog cannot be located (malformed input)
    ///   rather than propagating an error through the action pipeline.
    pub fn embed_metadata(
        &mut self,
        source_hash: &str,
        timestamp: &str,
        ops: &[(&str, &str)],
    ) -> crate::Result<&mut Self> {
        use crate::xmp::{self, RbaraXmpBlock};
        use lopdf::{Dictionary, Object, Stream};

        let catalog_id = match self
            .doc
            .trailer
            .get(b"Root")
            .ok()
            .and_then(|o| o.as_reference().ok())
        {
            Some(id) => id,
            None => return Ok(self),
        };

        let (existing_bytes, meta_ref) = {
            let cat = match self.doc.get_object(catalog_id) {
                Ok(Object::Dictionary(d)) => d.clone(),
                _ => return Ok(self),
            };
            let meta_ref = cat
                .get(b"Metadata")
                .ok()
                .and_then(|o| o.as_reference().ok());
            let bytes = match meta_ref.and_then(|id| self.doc.get_object(id).ok()) {
                Some(Object::Stream(s)) => s
                    .decompressed_content()
                    .unwrap_or_else(|_| s.content.clone()),
                _ => Vec::new(),
            };
            (bytes, meta_ref)
        };

        // Destructure the existing block into owned values so we can use both independently.
        let (prior_source_hash, mut combined_ops) = match xmp::parse_rbara_block(&existing_bytes) {
            Some(b) => (b.source_hash, b.ops),
            None => (String::new(), Vec::new()),
        };

        // Append the new ops. Each entry carries its own timestamp suffix so the
        // frontend can display per-op times in the history panel.
        for (name, params) in ops {
            let entry = if params.is_empty() {
                format!("{name}@{timestamp}")
            } else {
                format!("{name}({params})@{timestamp}")
            };
            combined_ops.push(entry);
        }

        // Inherit the root sourceHash from the lineage so it always identifies the
        // original unprocessed file, not the immediate input on each re-processing run.
        let effective_source_hash = if prior_source_hash.is_empty() {
            source_hash
        } else {
            &prior_source_hash
        };

        let block = RbaraXmpBlock {
            uuid: xmp::generate_uuid(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: timestamp.to_string(),
            source_hash: effective_source_hash.to_string(),
            parent_id: xmp::read_parent_id(&existing_bytes),
            ops: combined_ops,
        };

        let xmp_bytes = if existing_bytes.is_empty() {
            xmp::create_xmp(&block)
        } else {
            xmp::inject_into_xmp(&String::from_utf8_lossy(&existing_bytes), &block)
        }
        .into_bytes();

        if let Some(meta_id) = meta_ref {
            if let Ok(Object::Stream(s)) = self.doc.get_object_mut(meta_id) {
                s.set_plain_content(xmp_bytes);
            }
        } else {
            let mut dict = Dictionary::new();
            dict.set("Type", Object::Name(b"Metadata".to_vec()));
            dict.set("Subtype", Object::Name(b"XML".to_vec()));
            let stream = Stream::new(dict, xmp_bytes);
            let meta_id = self.doc.add_object(Object::Stream(stream));
            if let Ok(Object::Dictionary(cat)) = self.doc.get_object_mut(catalog_id) {
                cat.set("Metadata", Object::Reference(meta_id));
            }
        }

        Ok(self)
    }

    /// Converts all text on every page to outlined vector paths.
    ///
    /// Each character is replaced with its glyph outline as PDF path operators (`m`/`l`/`c`/`h`/`f`).
    /// The original BT...ET text blocks are stripped from the content stream and the glyph paths
    /// are appended, filled in CMYK black (0 0 0 1).
    ///
    /// After outlining, fonts are no longer required to render the PDF correctly.
    /// Note: colored text color preservation is not yet implemented — all glyphs are filled
    /// with CMYK black regardless of their original color.
    ///
    /// # Errors
    ///
    /// Returns an error if any page's content stream cannot be decoded or re-encoded.
    #[cfg(feature = "outline")]
    pub fn outline_text(&mut self) -> crate::Result<&mut Self> {
        use crate::outline::paths::outline_page_text;
        use crate::outline::writer::glyphs_to_content_stream;
        use lopdf::Object;
        use lopdf::content::Content;

        let page_ids: Vec<lopdf::ObjectId> = self.doc.get_pages().values().copied().collect();
        for page_id in page_ids {
            let glyphs = outline_page_text(&self.doc, page_id)?;
            let content = self.doc.get_and_decode_page_content(page_id)?;

            let mut in_bt = false;
            let filtered: Vec<lopdf::content::Operation> = content
                .operations
                .into_iter()
                .filter(|op| match op.operator.as_str() {
                    "BT" => {
                        in_bt = true;
                        false
                    }
                    "ET" => {
                        in_bt = false;
                        false
                    }
                    _ => !in_bt,
                })
                .collect();

            let mut combined = Content {
                operations: filtered,
            }
            .encode()?;

            if !glyphs.is_empty() {
                let mut glyph_stream = glyphs_to_content_stream(&glyphs);
                // Insert CMYK black fill operator after the opening "q\n" (2 bytes).
                glyph_stream.insert_str(2, "0 0 0 1 k\n");
                combined.extend_from_slice(glyph_stream.as_bytes());
            }

            let stream_ids = self.doc.get_page_contents(page_id);
            if let Some(&stream_id) = stream_ids.first() {
                if let Ok(Object::Stream(stream)) = self.doc.get_object_mut(stream_id) {
                    stream.set_plain_content(combined);
                }
                for &extra_id in stream_ids.get(1..).unwrap_or(&[]) {
                    if let Ok(Object::Stream(s)) = self.doc.get_object_mut(extra_id) {
                        s.set_plain_content(Vec::new());
                    }
                }
                if stream_ids.len() > 1 {
                    if let Ok(page_obj) = self.doc.get_object_mut(page_id) {
                        if let Ok(dict) = page_obj.as_dict_mut() {
                            dict.set("Contents", Object::Reference(stream_id));
                        }
                    }
                }
            }
        }
        Ok(self)
    }

    /// Returns approximate text and image bounding boxes for a page, as `[x, y, w, h]` in pts.
    ///
    /// `page_idx` is zero-based. Returns empty vecs if the page does not exist or has no
    /// parseable content. Positions are mid-tolerance estimates for layout preview use.
    pub fn page_layout_hint(&self, page_idx: u32) -> (Vec<[f32; 4]>, Vec<[f32; 4]>) {
        let pages = self.doc.get_pages();
        match pages.get(&(page_idx + 1)).copied() {
            Some(id) => crate::stream::page_layout(&self.doc, id),
            None => (vec![], vec![]),
        }
    }

    /// Analyzes a PDF document and classifies the color spaces used across all pages.
    ///
    /// Iterates through every page's content stream, inspecting PDF paint operators to
    /// determine whether the document uses CMYK (`k`/`K`), RGB (`rg`/`RG`), both, or
    /// neither.
    ///
    /// # Arguments
    ///
    /// * `doc` - A reference to the `lopdf::Document` to inspect.
    ///
    /// # Returns
    ///
    /// A [`DocumentColorKind`] variant describing the overall color usage:
    /// * `DocumentColorKind::PureCMYK`  – only CMYK paint operators were found.
    /// * `DocumentColorKind::PureRGB`   – only RGB paint operators were found.
    /// * `DocumentColorKind::Mixed`     – both CMYK and RGB operators are present.
    /// * `DocumentColorKind::Unknown`   – no recognizable color operators were found.
    ///
    /// # Notes
    ///
    /// Scans direct RGB/CMYK painting operators plus page color-space, image,
    /// form, and shading resources. Content streams that cannot be decoded are
    /// skipped while their resources are still inspected.
    pub fn detect_color_space(doc: &Document) -> DocumentColorKind {
        let mut usage = DocumentColorUsage::default();
        let mut visited_resources = std::collections::HashSet::new();

        for &page_id in doc.get_pages().values() {
            if let Ok(content) = doc.get_and_decode_page_content(page_id) {
                for op in &content.operations {
                    match op.operator.as_str() {
                        "k" | "K" => usage.has_cmyk = true,
                        "rg" | "RG" => usage.has_rgb = true,
                        _ => {}
                    }
                }
            }

            if let Ok((Some(resources), _)) = doc.get_page_resources(page_id) {
                scan_resource_colors(doc, resources, &mut usage, &mut visited_resources);
            }

            if usage.has_cmyk && usage.has_rgb {
                return DocumentColorKind::Mixed;
            }
        }

        usage.kind()
    }

    /// Remaps a specific CMYK color to another color throughout the document.
    ///
    /// Applies a color substitution rule to every page in the document. Any CMYK paint
    /// command whose channel values are within `tolerance` of the `from` color will have
    /// its operands replaced with the `to` color values. Both fill (`k`) and stroke (`K`)
    /// operators are processed.
    ///
    /// # Arguments
    ///
    /// * `from`      – CMYK source color as `[C, M, Y, K]` with each channel in `0.0..=1.0`.
    /// * `to`        – CMYK target color as `[C, M, Y, K]` with each channel in `0.0..=1.0`.
    /// * `tolerance` – Maximum per-channel absolute difference for a color to be considered
    ///   a match. `0.0` requires an exact match; `1.0` matches any color.
    ///
    /// # Returns
    ///
    /// Returns `Ok(&mut Self)` on success, allowing method chaining, or an error if any
    /// page content stream could not be decoded or re-encoded.
    ///
    /// # Errors
    ///
    /// Returns an error if page content decoding or encoding fails for any page.
    ///
    /// # Example
    ///
    /// ```no_test
    /// // Replace pure black with a warm black within a 5 % tolerance
    /// pipeline.remap_color([0.0, 0.0, 0.0, 1.0], [0.0, 0.06, 0.12, 0.88], 0.05)?;
    /// ```
    pub fn remap_color(
        &mut self,
        from: [f64; 4],
        to: [f64; 4],
        tolerance: f64,
    ) -> crate::Result<&mut Self> {
        let remaps = ColorRemap {
            from,
            to,
            tolerance,
        };
        ColorRemap::apply(&mut self.doc, &[remaps])?;
        Ok(self)
    }

    /// Flattens all `Separation` spot color uses to their device CMYK alternates without
    /// applying any ICC transform.
    ///
    /// This is a lighter alternative to [`Self::convert_color_space`] for documents that have
    /// spot inks but don't need a full profile-to-profile conversion. Each `cs`/`scn`
    /// operator pair referencing a `Separation` color space is replaced with the equivalent
    /// device CMYK `k` operator evaluated from the embedded tint function.
    ///
    /// Returns the total number of spot color operator sequences replaced across all pages.
    ///
    /// # Errors
    ///
    /// Returns an error if any page's content stream cannot be decoded or re-encoded.
    #[cfg(feature = "color")]
    pub fn flatten_spots(&mut self) -> crate::Result<u32> {
        use rustybara_icc::RenderingIntent;
        use rustybara_icc::pdf::flatten_spot_colors;
        Ok(flatten_spot_colors(
            &mut self.doc,
            None,
            RenderingIntent::RelativeColorimetric,
        )?)
    }

    /// Flattens spot colors using the supplied ICC destination profile bytes.
    ///
    /// When `dst_icc` is `Some`, Lab alternate-space values are converted to CMYK using
    /// that profile. When `None`, falls back to the bundled US Web Coated SWOP v2 profile.
    #[cfg(feature = "color")]
    pub fn flatten_spots_with_icc(&mut self, dst_icc: Option<&[u8]>) -> crate::Result<u32> {
        use rustybara_icc::RenderingIntent;
        use rustybara_icc::pdf::flatten_spot_colors;
        Ok(flatten_spot_colors(
            &mut self.doc,
            dst_icc,
            RenderingIntent::RelativeColorimetric,
        )?)
    }

    /// Applies an ICC color space conversion to every page in the document.
    ///
    /// Builds a [`rustybara_icc::ColorTransform`] from the named source and destination
    /// profiles, then walks every page's content stream, flattening spot colors and
    /// rewriting CMYK/RGB paint operators through the transform.
    ///
    /// # Arguments
    ///
    /// * `from_profile` – Machine-readable name of the source ICC profile (e.g. `"CoatedFOGRA39"`).
    /// * `to_profile`   – Machine-readable name of the destination ICC profile.
    /// * `intent`       – Rendering intent as a string: `"Perceptual"`, `"Saturation"`,
    ///   `"AbsoluteColorimetric"`, or anything else for `RelativeColorimetric` (the default).
    ///
    /// # Errors
    ///
    /// Returns an error if either profile name is unknown, if the lcms2 transform cannot be
    /// built, or if any page's content stream cannot be decoded or re-encoded.
    #[cfg(feature = "color")]
    pub fn convert_color_space(
        &mut self,
        from_profile: &str,
        to_profile: &str,
        intent: &str,
    ) -> crate::Result<()> {
        use rustybara_icc::pdf::PdfColorConverter;
        use rustybara_icc::{ColorTransform, IccError, RenderingIntent, profiles};

        let from = profiles::by_name(from_profile)
            .ok_or_else(|| IccError::Profile(format!("unknown source profile: {from_profile}")))?;
        let to = profiles::by_name(to_profile).ok_or_else(|| {
            IccError::Profile(format!("unknown destination profile: {to_profile}"))
        })?;
        let ri = match intent {
            "Perceptual" => RenderingIntent::Perceptual,
            "Saturation" => RenderingIntent::Saturation,
            "AbsoluteColorimetric" => RenderingIntent::AbsoluteColorimetric,
            _ => RenderingIntent::RelativeColorimetric,
        };
        let transform = ColorTransform::new(from, to, ri)?;
        PdfColorConverter::new(&mut self.doc, transform).convert_document()?;
        Ok(())
    }

    /// Applies an ICC color space conversion using raw profile bytes.
    ///
    /// Accepts pre-resolved bytes for both profiles, allowing callers to supply
    /// bundled or user-imported profiles without going through name lookup.
    ///
    /// # Errors
    ///
    /// Returns an error if the lcms2 transform cannot be built or if any page's
    /// content stream cannot be decoded or re-encoded.
    #[cfg(feature = "color")]
    pub fn convert_color_space_raw(
        &mut self,
        from_bytes: &[u8],
        to_bytes: &[u8],
        intent: &str,
    ) -> crate::Result<()> {
        use rustybara_icc::pdf::PdfColorConverter;
        use rustybara_icc::{ColorTransform, RenderingIntent};
        let ri = match intent {
            "Perceptual" => RenderingIntent::Perceptual,
            "Saturation" => RenderingIntent::Saturation,
            "AbsoluteColorimetric" => RenderingIntent::AbsoluteColorimetric,
            _ => RenderingIntent::RelativeColorimetric,
        };
        let transform = ColorTransform::from_bytes(from_bytes, to_bytes, ri)?;
        PdfColorConverter::new(&mut self.doc, transform).convert_document()?;
        Ok(())
    }

    /// Returns the total number of pages in the document.
    pub fn page_count(&self) -> usize {
        self.doc.get_pages().len()
    }

    /// Saves the document to a PDF file at `path`, overwriting if it exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or the document cannot be serialized.
    pub fn save_pdf(&mut self, path: impl AsRef<Path>) -> crate::Result<()> {
        let path = path.as_ref();
        #[cfg(not(target_arch = "wasm32"))]
        atomic_write_file(path, |file| self.doc.save_to(file)).map_err(crate::Error::Io)?;
        #[cfg(target_arch = "wasm32")]
        self.doc.save(path)?;
        Ok(())
    }

    /// Serializes the document to PDF bytes.
    ///
    /// Use this in environments without a filesystem (e.g., WebAssembly) to get the
    /// result as an in-memory byte vector rather than writing to a file.
    pub fn to_bytes(&mut self) -> crate::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.doc.save_to(&mut buf).map_err(crate::Error::Io)?;
        Ok(buf)
    }

    /// Rasterizes `page_num` (zero-based) to a [`DynamicImage`] using PDFium.
    ///
    /// # Platform Support
    ///
    /// Loads the PDFium shared library from the executable's directory, then falls
    /// back to the system library search path:
    /// * Windows: `pdfium.dll`
    /// * macOS: `libpdfium.dylib`
    /// * Linux: `libpdfium.so`
    ///
    /// # Errors
    ///
    /// Returns an error if the page index is out of range, the PDFium library cannot
    /// be loaded, or rendering fails.
    #[cfg(feature = "raster")]
    pub fn render_page(&self, page_num: u32, config: &RenderConfig) -> crate::Result<DynamicImage> {
        use pdfium_render::prelude::*;

        let mut doc_clone = self.doc.clone();
        let mut buf = Vec::new();
        doc_clone.save_to(&mut buf).map_err(crate::Error::Io)?;

        let dylib_name = if cfg!(target_os = "windows") {
            "pdfium.dll"
        } else if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else {
            "libpdfium.so" // Linux
        };

        let bindings_result = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join(dylib_name)))
            .and_then(|lib| Pdfium::bind_to_library(lib).ok())
            .map_or_else(|| Pdfium::bind_to_system_library(), Ok);

        let pdfium = match bindings_result {
            Ok(bindings) => Pdfium::new(bindings),
            // Pdfium can only be bound once per process. Reuse that known-good
            // global binding, but propagate every other error (including a
            // missing runtime library) instead of waiting on an uninitialized
            // global binding forever.
            Err(error) if can_reuse_pdfium_binding(&error) => Pdfium,
            Err(error) => return Err(error.into()),
        };

        let pdf_doc = pdfium.load_pdf_from_byte_vec(buf, None)?;
        let page = pdf_doc.pages().get(page_num as PdfPageIndex)?;
        crate::raster::render_page(&page, config)
    }

    /// Serialize the document to bytes for use by background render workers.
    ///
    /// Unlike `render_page`, this takes `&self` so it can be called through an
    /// `Arc<PdfPipeline>` without requiring exclusive access.
    #[cfg(feature = "raster")]
    pub fn pdf_bytes(&self) -> crate::Result<Vec<u8>> {
        let mut clone = self.doc.clone();
        let mut buf = Vec::new();
        clone.save_to(&mut buf).map_err(crate::Error::Io)?;
        Ok(buf)
    }

    /// Renders `page_num` (zero-based) and saves it to `path` in the given format.
    ///
    /// # Errors
    ///
    /// Returns an error if the page index is out of range, rendering fails, or the
    /// image file cannot be written.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rustybara::{PdfPipeline, encode::OutputFormat, raster::RenderConfig};
    /// # fn main() -> rustybara::Result<()> {
    /// let pipeline = PdfPipeline::open("input.pdf")?;
    /// pipeline.save_page_image(0, "page_1.jpg", &OutputFormat::Jpg, &RenderConfig::prepress(), 90)?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "raster")]
    pub fn save_page_image(
        &self,
        page_num: u32,
        path: impl AsRef<Path>,
        format: &OutputFormat,
        config: &RenderConfig,
        quality: u8,
    ) -> crate::Result<()> {
        let image = self.render_page(page_num, config)?;
        crate::encode::save(&image, path.as_ref(), format, config.dpi, quality)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::PageBoxes;
    use lopdf::{Object, Stream, dictionary};

    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf_test_data_print_v2.pdf")
    }

    fn document_with_image_color_spaces(color_spaces: &[&[u8]]) -> Document {
        let mut document = Document::with_version("1.5");
        let mut xobjects = lopdf::Dictionary::new();
        for (index, color_space) in color_spaces.iter().enumerate() {
            let image_id = document.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => 1,
                    "Height" => 1,
                    "BitsPerComponent" => 8,
                    "ColorSpace" => Object::Name(color_space.to_vec()),
                },
                vec![0, 0, 0, 0],
            ));
            xobjects.set(
                format!("Im{index}").into_bytes(),
                Object::Reference(image_id),
            );
        }

        let resources = dictionary! {
            "XObject" => Object::Dictionary(xobjects),
        };
        let pages_id = document.new_object_id();
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Resources" => Object::Dictionary(resources),
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        document.trailer.set("Root", Object::Reference(catalog_id));
        document
    }

    #[test]
    fn open_and_page_count() {
        let p = PdfPipeline::open(fixture()).unwrap();
        assert!(p.page_count() > 0);
    }

    #[test]
    fn open_nonexistent_fails() {
        let err = PdfPipeline::open("no_such_file.pdf");
        assert!(err.is_err());
    }

    #[test]
    fn detects_rgb_in_image_only_pdf() {
        let document = document_with_image_color_spaces(&[b"DeviceRGB"]);
        assert!(matches!(
            PdfPipeline::detect_color_space(&document),
            DocumentColorKind::PureRGB
        ));
    }

    #[test]
    fn detects_mixed_color_spaces_across_image_resources() {
        let document = document_with_image_color_spaces(&[b"DeviceRGB", b"DeviceCMYK"]);
        assert!(matches!(
            PdfPipeline::detect_color_space(&document),
            DocumentColorKind::Mixed
        ));
    }

    #[test]
    fn trim_succeeds() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        p.trim().unwrap();
    }

    #[test]
    fn trim_is_chainable() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let out = std::env::temp_dir().join("rustybara_pipeline_trim_chain.pdf");
        p.trim().unwrap().save_pdf(&out).unwrap();
        assert!(out.exists());
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn resize_expands_mediabox() {
        let bleed = 9.0;
        let mut p = PdfPipeline::open(fixture()).unwrap();

        // Grab original trim dimensions for comparison
        let orig_doc = Document::load(fixture()).unwrap();
        let orig_pages = orig_doc.get_pages();
        let first_id = *orig_pages.values().next().unwrap();
        let orig_boxes = PageBoxes::read(&orig_doc, first_id).unwrap();
        let orig_trim = orig_boxes.trim_or_media();

        p.resize(bleed).unwrap();

        // Read back from the mutated doc
        let pages = p.doc.get_pages();
        let page_id = *pages.values().next().unwrap();
        let boxes = PageBoxes::read(&p.doc, page_id).unwrap();
        let media = boxes.media_box;

        assert!(
            (media.width - (orig_trim.width + 2.0 * bleed)).abs() < 0.01,
            "media width should be trim + 2*bleed"
        );
        assert!(
            (media.height - (orig_trim.height + 2.0 * bleed)).abs() < 0.01,
            "media height should be trim + 2*bleed"
        );
    }

    #[test]
    fn set_media_box_sets_exact_size_centered() {
        let (w, h) = (400.0, 600.0);
        let mut p = PdfPipeline::open(fixture()).unwrap();

        // Capture the original media center; the new box must stay centered on it.
        let orig_doc = Document::load(fixture()).unwrap();
        let orig_id = *orig_doc.get_pages().values().next().unwrap();
        let orig = PageBoxes::read(&orig_doc, orig_id).unwrap().media_box;
        let (cx, cy) = (orig.x + orig.width / 2.0, orig.y + orig.height / 2.0);

        p.set_media_box(w, h).unwrap();

        let id = *p.doc.get_pages().values().next().unwrap();
        let media = PageBoxes::read(&p.doc, id).unwrap().media_box;

        assert!(
            (media.width - w).abs() < 0.01,
            "width should equal requested"
        );
        assert!(
            (media.height - h).abs() < 0.01,
            "height should equal requested"
        );
        assert!(
            (media.x + media.width / 2.0 - cx).abs() < 0.01,
            "stays centered in X"
        );
        assert!(
            (media.y + media.height / 2.0 - cy).abs() < 0.01,
            "stays centered in Y"
        );
    }

    #[test]
    fn set_media_box_rejects_nonpositive() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        assert!(p.set_media_box(0.0, 100.0).is_err());
        assert!(p.set_media_box(100.0, -5.0).is_err());
    }
    #[test]
    fn save_roundtrip() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let original_count = p.page_count();
        let out = std::env::temp_dir().join("rustybara_pipeline_roundtrip.pdf");

        p.trim().unwrap().save_pdf(&out).unwrap();

        let reopened = PdfPipeline::open(&out).unwrap();
        assert_eq!(reopened.page_count(), original_count);
        std::fs::remove_file(&out).ok();
    }

    #[test]
    fn resize_then_save() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let out = std::env::temp_dir().join("rustybara_pipeline_resize_save.pdf");
        p.resize(9.0).unwrap().save_pdf(&out).unwrap();
        assert!(out.exists());

        // Verify the saved file is loadable
        let reopened = PdfPipeline::open(&out).unwrap();
        assert!(reopened.page_count() > 0);
        std::fs::remove_file(&out).ok();
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn atomic_write_failure_preserves_existing_file() {
        use std::io::{Error, Write};

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("document.pdf");
        std::fs::write(&target, b"original").unwrap();

        let result = atomic_write_file(&target, |file| {
            file.write_all(b"partial replacement")?;
            Err(Error::other("simulated serialization failure"))
        });

        assert!(result.is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn trim_then_resize_pipeline() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        let out = std::env::temp_dir().join("rustybara_pipeline_trim_resize.pdf");
        p.trim()
            .unwrap()
            .resize(9.0)
            .unwrap()
            .save_pdf(&out)
            .unwrap();
        assert!(out.exists());
        std::fs::remove_file(&out).ok();
    }

    /// Read a page's own `/Rotate` (default 0 when absent), for rotation assertions.
    fn page_rotation(doc: &lopdf::Document, id: lopdf::ObjectId) -> i64 {
        doc.get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Rotate").ok())
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0)
    }

    #[test]
    fn rotate_adds_to_every_page() {
        // Capture each page's starting rotation from a fresh open.
        let baseline = PdfPipeline::open(fixture()).unwrap();
        let before: Vec<(lopdf::ObjectId, i64)> = baseline
            .doc()
            .get_pages()
            .values()
            .map(|&id| (id, page_rotation(baseline.doc(), id)))
            .collect();

        let mut p = PdfPipeline::open(fixture()).unwrap();
        p.rotate(90).unwrap();
        for (id, prev) in before {
            assert_eq!(
                page_rotation(p.doc(), id),
                (prev + 90).rem_euclid(360),
                "each page should advance 90° from its prior rotation"
            );
        }
    }

    #[test]
    fn rotate_accumulates_and_normalizes() {
        let baseline = PdfPipeline::open(fixture()).unwrap();
        let first = *baseline.doc().get_pages().values().next().unwrap();
        let start = page_rotation(baseline.doc(), first);

        let mut p = PdfPipeline::open(fixture()).unwrap();
        p.rotate(270).unwrap();
        p.rotate(180).unwrap(); // +450 total → +90 after mod 360
        assert_eq!(
            page_rotation(p.doc(), first),
            (start + 90).rem_euclid(360),
            "rotations accumulate and wrap mod 360"
        );
    }

    #[test]
    fn rotate_rejects_non_multiple_of_90() {
        let mut p = PdfPipeline::open(fixture()).unwrap();
        assert!(
            p.rotate(45).is_err(),
            "a non-multiple of 90 must be rejected"
        );
    }

    #[test]
    #[cfg(feature = "raster")]
    #[ignore = "requires pdfium runtime library"]
    fn render_page_produces_image() {
        let p = PdfPipeline::open(fixture()).unwrap();
        let config = RenderConfig::default();
        let img = p.render_page(0, &config).unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    /// Regression guard for the pdfium binding fallback (v0.1.9 pages>1 bug).
    ///
    /// pdfium can only be bound once per process, so the *second* `render_page`
    /// call must fall back to reusing the existing global binding instead of
    /// erroring. The v0.1.9 regression made a re-binding failure fatal, which
    /// broke every render after the first — export stopped after page 1 and rbv
    /// could only display the first page.
    #[test]
    #[cfg(feature = "raster")]
    #[ignore = "requires pdfium runtime library"]
    fn render_page_twice_in_one_process() {
        let p = PdfPipeline::open(fixture()).unwrap();
        let config = RenderConfig::default();

        let first = p.render_page(0, &config).unwrap();
        assert!(first.width() > 0);

        let second = p
            .render_page(0, &config)
            .expect("second render must reuse the existing pdfium binding");
        assert_eq!(
            (second.width(), second.height()),
            (first.width(), first.height()),
            "repeat render of the same page must produce identical dimensions"
        );
    }

    #[test]
    #[cfg(feature = "raster")]
    fn only_an_initialized_pdfium_binding_can_be_reused() {
        use pdfium_render::prelude::PdfiumError;

        assert!(can_reuse_pdfium_binding(
            &PdfiumError::PdfiumLibraryBindingsAlreadyInitialized
        ));
        assert!(!can_reuse_pdfium_binding(&PdfiumError::UnrecognizedPath));
    }
}
