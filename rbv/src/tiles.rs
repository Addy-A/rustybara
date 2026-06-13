/// Tile size in pixels for both width and height.
pub const TILE_SIZE: u32 = 512;

/// Map a viewer zoom level to a render DPI bucket.
///
/// Returns `(bucket_index, dpi)`. Bucket 0 is the base level (no tiling);
/// buckets 1–5 yield progressively higher resolution tiles.
///
/// | Bucket | Zoom range  | DPI  | Est. render | Est. memory |
/// |--------|-------------|------|-------------|-------------|
/// | 0      | < 1.5×      |  150 | (base)      | (base)      |
/// | 1      | 1.5× – 3×   |  300 | ~1.0s       | ~26 MB      |
/// | 2      | 3× – 10×    |  450 | ~1.9s       | ~58 MB      |
/// | 3      | 10× – 15×   |  500 | ~2.3s       | ~71 MB      |
/// | 4      | 15× – 20×   |  600 | ~3.0s       | ~103 MB     |
/// | 5      | 20× – 30×   |  800 | ~5.1s       | ~183 MB     |
/// | 6      | 30× – 40×   | 1000 | ~8.0s       | ~286 MB     |
/// | 7      | ≥ 40×       | 1200 | ~11.5s      | ~413 MB     |
pub fn zoom_bucket(zoom: f32) -> (u8, f32) {
    if zoom < 1.5 {
        (0, 150.0)
    } else if zoom < 3.0 {
        (1, 300.0)
    } else if zoom < 10.0 {
        (2, 450.0)
    } else if zoom < 15.0 {
        (3, 500.0)
    } else if zoom < 20.0 {
        (4, 600.0)
    } else if zoom < 30.0 {
        (5, 800.0)
    } else if zoom < 40.0 {
        (6, 1000.0)
    } else {
        (7, 1200.0)
    }
}

/// Cache-key identifying a single rendered tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub page: u32,
    pub zoom_bucket: u8,
    pub col: u32,
    pub row: u32,
}

/// A render request sent from the main thread to the tile worker.
pub struct RenderRequest {
    pub key: TileKey,
    /// DPI at which to render this tile (derived from `key.zoom_bucket`).
    pub dpi: f32,
}

/// Tracks which tiles are currently queued or in-flight.
///
/// Lives entirely on the main thread — no interior mutability needed.
/// The actual `skia_safe::Image` tiles live in `SkiaState::tile_images` once
/// they arrive via `ViewerEvent::TileReady`.
#[derive(Default)]
pub struct TileCache {
    queued: std::collections::HashSet<TileKey>,
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            queued: std::collections::HashSet::new(),
        }
    }

    pub fn is_queued(&self, k: &TileKey) -> bool {
        self.queued.contains(k)
    }

    pub fn mark_queued(&mut self, k: TileKey) {
        self.queued.insert(k);
    }

    /// Remove all queued entries except those matching `(page, keep_bucket)`.
    pub fn evict_other_buckets(&mut self, page: u32, keep_bucket: u8) {
        self.queued
            .retain(|k| k.page == page && k.zoom_bucket == keep_bucket);
    }

    pub fn clear(&mut self) {
        self.queued.clear();
    }
}

/// Background worker that holds a pdfium instance and renders tiles on demand.
pub struct RenderWorker;

impl RenderWorker {
    /// Spawn the worker thread.
    ///
    /// `bytes` — serialized PDF bytes shared with the worker (zero-copy via slice borrow).
    /// `on_tile_ready` — callback invoked on the worker thread for each completed tile;
    ///   typically sends a `ViewerEvent::TileReady` via `EventLoopProxy`.
    ///
    /// Returns a `Sender` the caller uses to enqueue `RenderRequest`s. Dropping the
    /// sender closes the channel and causes the worker thread to exit cleanly.
    pub fn spawn(
        bytes: std::sync::Arc<Vec<u8>>,
        on_tile_ready: impl Fn(TileKey, image::DynamicImage) + Send + 'static,
        on_log: impl Fn(String) + Send + 'static,
        on_metrics: impl Fn(f32, u64, u32, u32) + Send + 'static,
    ) -> std::sync::mpsc::Sender<RenderRequest> {
        let (tx, rx) = std::sync::mpsc::channel::<RenderRequest>();

        std::thread::spawn(move || {
            use pdfium_render::prelude::*;

            let dylib_name = if cfg!(target_os = "windows") {
                "pdfium.dll"
            } else if cfg!(target_os = "macos") {
                "libpdfium.dylib"
            } else {
                "libpdfium.so"
            };

            let lib_path = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join(dylib_name)));

            let bindings_result = lib_path
                .as_ref()
                .and_then(|lib| Pdfium::bind_to_library(lib).ok())
                .map_or_else(|| Pdfium::bind_to_system_library(), Ok);

            let pdfium = match bindings_result {
                Ok(b) => {
                    on_log(format!(
                        "Tile worker: pdfium bound ({})",
                        lib_path.as_ref().map_or("system", |_| "local")
                    ));
                    Pdfium::new(b)
                }
                // Binding already loaded in this process (e.g. by the main render thread).
                // Pdfium (unit struct default) reuses the existing global binding.
                Err(e) => {
                    on_log(format!(
                        "Tile worker: reusing existing pdfium binding ({e})"
                    ));
                    Pdfium
                }
            };

            // Load the document once for the lifetime of this worker.
            // `bytes` is an Arc moved into this closure; `&bytes[..]` borrows from
            // the Arc local — both live for the entire thread scope, so this compiles.
            let pdf_doc = match pdfium.load_pdf_from_byte_slice(&bytes, None) {
                Ok(d) => {
                    on_log(format!(
                        "Tile worker: doc loaded ({} pages)",
                        d.pages().len()
                    ));
                    d
                }
                Err(e) => {
                    on_log(format!("Tile worker: doc load failed — {e}"));
                    return;
                }
            };

            // Cache the last full-page render to avoid re-rendering per tile.
            // Key: (page_index, dpi.to_bits()).
            let mut page_render_cache: Option<(u32, u32, image::DynamicImage)> = None;

            for req in rx {
                let Ok(page) = pdf_doc.pages().get(req.key.page as PdfPageIndex) else {
                    on_log(format!("Tile worker: page {} not found", req.key.page));
                    continue;
                };

                let dpi_bits = req.dpi.to_bits();
                let need_render = page_render_cache
                    .as_ref()
                    .map_or(true, |(cp, cd, _)| *cp != req.key.page || *cd != dpi_bits);

                if need_render {
                    let config = rustybara::raster::RenderConfig {
                        dpi: req.dpi as u32,
                        render_annotations: true,
                        render_form_data: false,
                    };
                    let t0 = std::time::Instant::now();
                    match rustybara::raster::render_page(&page, &config) {
                        Ok(img) => {
                            let render_ms = t0.elapsed().as_millis() as u64;
                            on_log(format!(
                                "Tile worker: full render p{} at {}dpi ({}×{}) in {}ms",
                                req.key.page,
                                req.dpi as u32,
                                img.width(),
                                img.height(),
                                render_ms,
                            ));
                            on_metrics(req.dpi, render_ms, img.width(), img.height());
                            page_render_cache = Some((req.key.page, dpi_bits, img));
                        }
                        Err(e) => {
                            on_log(format!("Tile worker: full render failed — {e}"));
                            continue;
                        }
                    }
                }

                let full_img = &page_render_cache.as_ref().unwrap().2;
                let x = req.key.col * TILE_SIZE;
                let y = req.key.row * TILE_SIZE;
                let crop_w = TILE_SIZE.min(full_img.width().saturating_sub(x));
                let crop_h = TILE_SIZE.min(full_img.height().saturating_sub(y));

                if crop_w == 0 || crop_h == 0 {
                    on_log(format!(
                        "Tile worker: ({},{}) out of bounds for p{}",
                        req.key.col, req.key.row, req.key.page
                    ));
                    continue;
                }

                on_tile_ready(req.key, full_img.crop_imm(x, y, crop_w, crop_h));
            }
            on_log("Tile worker: channel closed, thread exiting".to_string());
        });

        tx
    }
}

/// Compute which tiles intersect the current viewport.
///
/// `page_rect` — screen-space rect where the page is drawn (from `compute_page_rect`).
/// `page_pts`  — `(width, height)` of the PDF page in points (from `media_box`).
/// `viewport`  — window rect, typically `(0, 0, win_w, win_h)`.
/// `zoom`      — current viewer zoom level.
/// `page`      — 0-based page index.
pub fn compute_visible_tiles(
    page_rect: skia_safe::Rect,
    page_pts: (f32, f32),
    viewport: skia_safe::Rect,
    zoom: f32,
    page: u32,
) -> Vec<TileKey> {
    let (bucket, dpi) = zoom_bucket(zoom);

    // Total page dimensions in tile-DPI pixel space.
    let tile_page_w = page_pts.0 * dpi / 72.0;
    let tile_page_h = page_pts.1 * dpi / 72.0;
    let cols = (tile_page_w / TILE_SIZE as f32).ceil() as u32;
    let rows = (tile_page_h / TILE_SIZE as f32).ceil() as u32;

    if cols == 0 || rows == 0 {
        return vec![];
    }

    // Screen size of one tile.
    let tile_screen_w = page_rect.width() / (tile_page_w / TILE_SIZE as f32);
    let tile_screen_h = page_rect.height() / (tile_page_h / TILE_SIZE as f32);

    // Visible region on screen (intersection of page rect with the window).
    let vis_left = viewport.left().max(page_rect.left());
    let vis_top = viewport.top().max(page_rect.top());
    let vis_right = viewport.right().min(page_rect.right());
    let vis_bottom = viewport.bottom().min(page_rect.bottom());

    if vis_right <= vis_left || vis_bottom <= vis_top {
        return vec![];
    }

    // Convert visible screen rect to tile grid indices.
    let col_min = ((vis_left - page_rect.left()) / tile_screen_w).floor() as u32;
    let row_min = ((vis_top - page_rect.top()) / tile_screen_h).floor() as u32;
    let col_max = ((vis_right - page_rect.left()) / tile_screen_w).ceil() as u32;
    let row_max = ((vis_bottom - page_rect.top()) / tile_screen_h).ceil() as u32;

    let mut keys = Vec::new();
    for row in row_min..row_max.min(rows) {
        for col in col_min..col_max.min(cols) {
            keys.push(TileKey {
                page,
                zoom_bucket: bucket,
                col,
                row,
            });
        }
    }
    keys
}

/// Compute the screen-space `Rect` at which to draw a given tile.
///
/// `page_rect` — screen-space rect where the page is drawn.
/// `page_pts`  — `(width, height)` of the PDF page in points.
/// `dpi`       — DPI for this tile's zoom bucket.
pub fn tile_rect_on_screen(
    key: &TileKey,
    page_rect: &skia_safe::Rect,
    page_pts: (f32, f32),
    dpi: f32,
) -> skia_safe::Rect {
    let tile_page_w = page_pts.0 * dpi / 72.0;
    let tile_page_h = page_pts.1 * dpi / 72.0;

    let tile_screen_w = page_rect.width() / (tile_page_w / TILE_SIZE as f32);
    let tile_screen_h = page_rect.height() / (tile_page_h / TILE_SIZE as f32);

    let x = page_rect.left() + key.col as f32 * tile_screen_w;
    let y = page_rect.top() + key.row as f32 * tile_screen_h;

    skia_safe::Rect::from_xywh(x, y, tile_screen_w, tile_screen_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_bucket_thresholds() {
        assert_eq!(zoom_bucket(1.0).0, 0);
        assert_eq!(zoom_bucket(1.49).0, 0);
        assert_eq!(zoom_bucket(1.5).0, 1);
        assert_eq!(zoom_bucket(2.99).0, 1);
        assert_eq!(zoom_bucket(3.0).0, 2);
        assert_eq!(zoom_bucket(5.0).0, 2);
        assert_eq!(zoom_bucket(9.99).0, 2);
        assert_eq!(zoom_bucket(10.0).0, 3);
        assert_eq!(zoom_bucket(14.99).0, 3);
        assert_eq!(zoom_bucket(15.0).0, 4);
        assert_eq!(zoom_bucket(19.99).0, 4);
        assert_eq!(zoom_bucket(20.0).0, 5);
        assert_eq!(zoom_bucket(29.99).0, 5);
        assert_eq!(zoom_bucket(30.0).0, 6);
        assert_eq!(zoom_bucket(39.99).0, 6);
        assert_eq!(zoom_bucket(40.0).0, 7);
        assert_eq!(zoom_bucket(50.0).0, 7);
    }

    #[test]
    fn compute_visible_tiles_basic() {
        // A 612pt × 792pt US-Letter page at 300 DPI = 2550 × 3300 tile-pixels.
        // With TILE_SIZE=512: 5 cols, 7 rows.
        // page_rect = (0,0,612,792) screen pixels at zoom=1.5 bucket (dpi=300).
        let page_rect = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let page_pts = (612.0_f32, 792.0_f32);
        let viewport = skia_safe::Rect::from_xywh(0.0, 0.0, 612.0, 792.0);
        let keys = compute_visible_tiles(page_rect, page_pts, viewport, 1.5, 0);
        // All tiles should be visible; 5 × 7 = 35
        assert_eq!(keys.len(), 35);
        assert!(keys.iter().all(|k| k.page == 0 && k.zoom_bucket == 1));
    }

    #[test]
    fn compute_visible_tiles_empty_when_offscreen() {
        let page_rect = skia_safe::Rect::from_xywh(1000.0, 1000.0, 612.0, 792.0);
        let page_pts = (612.0_f32, 792.0_f32);
        let viewport = skia_safe::Rect::from_xywh(0.0, 0.0, 800.0, 600.0);
        let keys = compute_visible_tiles(page_rect, page_pts, viewport, 1.5, 0);
        assert!(keys.is_empty());
    }

    #[test]
    fn tile_rect_on_screen_origin() {
        let page_rect = skia_safe::Rect::from_xywh(10.0, 20.0, 612.0, 792.0);
        let page_pts = (612.0_f32, 792.0_f32);
        let dpi = 300.0_f32;
        let key = TileKey {
            page: 0,
            zoom_bucket: 1,
            col: 0,
            row: 0,
        };
        let r = tile_rect_on_screen(&key, &page_rect, page_pts, dpi);
        // Col=0, Row=0 → top-left should be at page_rect origin
        assert!((r.left() - 10.0).abs() < 0.01);
        assert!((r.top() - 20.0).abs() < 0.01);
    }
}
