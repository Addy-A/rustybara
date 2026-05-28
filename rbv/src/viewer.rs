use crate::export::export_wireframe;
use crate::renderer::{
    image_to_skia, ColorPanel, DebugOverlay, OverlayData, PageWireframe, SkiaRenderer,
    TelemetryOverlay,
};
use crate::separation::{build_icc_transform as sep_build_icc_transform, PlateChannel};
use crate::ui_state::{extract_spot_names, PlateMode};
use image::{DynamicImage, GenericImageView};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rustybara::objects::{
    build_object_tree, filter_by_ink, hit_test, CmykChannel, ObjectKind, ObjectTree, PageObject,
    PdfColor,
};
use rustybara::outline::paths::{outline_page_text, PositionedGlyph};
use rustybara::pages::PageBoxes;
use rustybara::raster::RenderConfig;
use rustybara::PdfPipeline;
use rustybara_icc::{ColorTransform, RenderingIntent};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

// ── Max debug log capacity ────────────────────────────────────────────────────
const DEBUG_LOG_CAP: usize = 24;

/// A command received over stdin when rbv is run with `--listen`.
pub enum IpcCmd {
    /// Open a new PDF file at the given absolute path, resetting to page 0.
    Open(PathBuf),
    /// Jump to the given 0-based page number.
    Page(u32),
    Quit,
}

pub enum ViewerEvent {
    PreviewReady {
        page: u32,
        image: DynamicImage,
    },
    PageReady {
        page: u32,
        image: DynamicImage,
    },
    /// A plate separation render has finished.  The `plate` snapshot allows the
    /// viewer to discard stale results when the user changes plates mid-render.
    PlateReady {
        page: u32,
        plate: PlateMode,
        image: DynamicImage,
    },
    /// A single tile has finished rendering in the background worker.
    TileReady {
        key: crate::tiles::TileKey,
        image: DynamicImage,
    },
    /// A debug/status message from the tile worker thread.
    TileLog(String),
    /// Performance metrics from the worker after a full-page render at a given DPI.
    TileMetrics { dpi: f32, render_ms: u64, img_w: u32, img_h: u32 },
    FileChanged,
    IpcCommand(IpcCmd),
}

struct SkiaState {
    window: Arc<Window>,
    renderer: SkiaRenderer,
    /// egui-winit integration — translates winit events into egui input and
    /// handles platform output (cursor changes, clipboard, IME).
    egui_winit: egui_winit::State,
    page_image: Option<skia_safe::Image>,
    /// ICC plate separation result — replaces `page_image` when a non-All plate is active.
    /// Cleared whenever the plate selection changes or a new page render starts.
    plate_image: Option<skia_safe::Image>,
    /// Rendered tiles for the current page, keyed by TileKey.
    tile_images: std::collections::HashMap<crate::tiles::TileKey, skia_safe::Image>,
    width: u32,
    height: u32,
}

struct Viewer {
    file: PathBuf,
    pipeline: Arc<PdfPipeline>,
    page: u32,
    /// Total number of pages in the document. Used to clamp navigation.
    page_count: u32,
    config: RenderConfig,
    state: Option<SkiaState>,
    pending_image: Option<DynamicImage>,
    /// Retained rasterized page image for pixel sampling on click.
    current_image: Option<DynamicImage>,
    page_boxes: Option<PageBoxes>,
    /// Cached object tree for the current page, used for hit-testing.
    object_tree: Option<ObjectTree>,
    /// Glyph outlines for wireframe text rendering. `none` until page loads.
    glyph_outlines: Option<Vec<PositionedGlyph>>,
    /// The topmost object under the last selection click (owned clone).
    selected_object: Option<PageObject>,
    /// Color diagnostic data captured at the last selection click.
    color_info: Option<ColorPanel>,
    /// PDF-space coordinates of the last sampling click, used to project the
    /// crosshair marker each frame through the current zoom/pan transform.
    /// `None` when no selection has been made or the click was outside the page.
    sampling_pdf_pos: Option<(f64, f64)>,
    show_overlays: bool,
    /// When `true`, the selected object's wireframe is drawn each frame.
    show_wireframe: bool,
    zoom: f32,
    pan: [f32; 2],
    ctrl_held: bool,
    shift_held: bool,
    cursor_pos: [f32; 2],
    drag_origin: Option<([f32; 2], [f32; 2])>,
    /// When `true`, the debug overlay is visible (toggled by Ctrl+Shift+D).
    debug_mode: bool,
    /// Ring buffer of recent debug log entries (capped at [`DEBUG_LOG_CAP`]).
    debug_log: VecDeque<String>,
    /// Cached sRGB (or AdobeRGB 1998 fallback) → US Web Coated SWOP transform
    /// for the ICC CMYK pixel readout. Built lazily on first selection click and
    /// reused for all subsequent clicks.
    icc_transform: Option<ColorTransform>,
    watcher: RecommendedWatcher,
    proxy: EventLoopProxy<ViewerEvent>,
    /// Accumulated digit characters for a `<N>g` page-jump prefix.
    /// Cleared on every navigation action or Escape.
    digit_buf: String,
    /// Shared egui context — cloned cheaply (Arc) into the egui-winit State.
    egui_ctx: egui::Context,
    /// Which ink plate is currently isolated in the tools panel.
    active_plate: PlateMode,
    /// Spot-color names present on the current page, sorted alphabetically.
    plate_spot_names: Vec<String>,
    /// Whether the prepress tools panel is currently visible.
    /// Hidden by default; toggled by the floating ⚙ button.
    show_tools_panel: bool,
    /// When `true`, plate images are rendered with ink-specific tint colors
    /// (C=cyan, M=magenta, Y=yellow, K=near-black, spot=violet).
    /// When `false`, all plates are shown as grayscale (white=no ink, black=full).
    plate_tinted: bool,
    /// The plate that the current `state.plate_image` was computed for.
    /// Used to detect stale results: if `active_plate != plate_image_for`, the
    /// plate_image is cleared and a new separation is triggered.
    plate_image_for: PlateMode,
    /// True when the pointer was over any egui surface in the last rendered frame.
    ///
    /// Used to gate PDF pan / selection so clicks inside the egui panel or the
    /// floating toggle button don't also trigger viewer interactions.
    /// One-frame delay is acceptable — the pointer must have been hovering there
    /// before a press can register.
    egui_pointer_over_panel: bool,
    /// Tile system: tracks which tiles have been enqueued to the worker.
    tile_cache: crate::tiles::TileCache,
    /// Channel to the background tile render worker. `None` before the first
    /// `PageReady` event. Dropping this sender closes the channel and causes the
    /// worker thread to exit cleanly.
    tile_sender: Option<std::sync::mpsc::Sender<crate::tiles::RenderRequest>>,
    /// When `true`, the tile performance telemetry panel is visible (Ctrl+Shift+T).
    telemetry_mode: bool,
    /// Latest full-page render timing per DPI level, keyed by `dpi as u32`.
    /// Updated each time the worker completes a new full-page render.
    tile_perf: std::collections::HashMap<u32, (u64, u32, u32)>,
}

impl Viewer {
    // ── Tile rendering ────────────────────────────────────────────────────────

    /// Serialize the current PDF once and start the background tile worker.
    /// Replaces any previously running worker (old sender is dropped → old thread exits).
    fn spawn_tile_worker(&mut self) {
        let bytes = match self.pipeline.pdf_bytes() {
            Ok(b) => std::sync::Arc::new(b),
            Err(e) => {
                self.push_log(format!("Tile worker: pdf_bytes failed — {e}"));
                return;
            }
        };
        self.push_log(format!("Tile worker spawned ({} bytes)", bytes.len()));
        let proxy = self.proxy.clone();
        let log_proxy = self.proxy.clone();
        let metrics_proxy = self.proxy.clone();
        let sender = crate::tiles::RenderWorker::spawn(
            std::sync::Arc::clone(&bytes),
            move |key, image| {
                let _ = proxy.send_event(ViewerEvent::TileReady { key, image });
            },
            move |msg| {
                let _ = log_proxy.send_event(ViewerEvent::TileLog(msg));
            },
            move |dpi, render_ms, img_w, img_h| {
                let _ = metrics_proxy.send_event(ViewerEvent::TileMetrics {
                    dpi,
                    render_ms,
                    img_w,
                    img_h,
                });
            },
        );
        self.tile_sender = Some(sender);
        self.tile_cache.clear();
        if let Some(state) = self.state.as_mut() {
            state.tile_images.clear();
        }
    }

    /// Compute which tiles are currently visible and send missing ones to the worker.
    /// Also evicts tiles for stale zoom buckets from `state.tile_images`.
    fn enqueue_visible_tiles(&mut self) {
        let (bucket, dpi) = crate::tiles::zoom_bucket(self.zoom);
        if bucket == 0 {
            return;
        }
        let Some(page_rect) = self.compute_page_rect() else {
            return;
        };
        let Some(media) = self.page_boxes.as_ref().map(|b| &b.media_box) else {
            return;
        };
        let page_pts = (media.width as f32, media.height as f32);
        let viewport = match self.state.as_ref() {
            Some(s) => skia_safe::Rect::from_xywh(0.0, 0.0, s.width as f32, s.height as f32),
            None => return,
        };

        let page = self.page;

        // Evict tiles for other zoom buckets to free GPU memory.
        if let Some(state) = self.state.as_mut() {
            state
                .tile_images
                .retain(|k, _| k.page == page && k.zoom_bucket == bucket);
        }
        self.tile_cache.evict_other_buckets(page, bucket);

        let keys =
            crate::tiles::compute_visible_tiles(page_rect, page_pts, viewport, self.zoom, page);

        let Some(sender) = self.tile_sender.as_ref() else {
            self.push_log("Tile enqueue: no worker (sender is None)".to_string());
            return;
        };
        let mut new_count = 0u32;
        for key in &keys {
            let already_rendered = self
                .state
                .as_ref()
                .map_or(false, |s| s.tile_images.contains_key(key));
            if !self.tile_cache.is_queued(key) && !already_rendered {
                self.tile_cache.mark_queued(*key);
                let _ = sender.send(crate::tiles::RenderRequest { key: *key, dpi });
                new_count += 1;
            }
        }
        self.push_log(format!(
            "Tiles: bucket={} dpi={} visible={} queued={}",
            bucket, dpi as u32, keys.len(), new_count
        ));
    }

    // ── Zoom ─────────────────────────────────────────────────────────────────

    fn apply_zoom(&mut self, factor: f32, focal: Option<[f32; 2]>, win_w: f32, win_h: f32) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.05, 50.0);
        let r = self.zoom / old_zoom;
        let (cx, cy) = match focal {
            Some([x, y]) => (x, y),
            None => (win_w / 2.0, win_h / 2.0),
        };
        self.pan[0] = self.pan[0] * r + (cx - win_w / 2.0) * (1.0 - r);
        self.pan[1] = self.pan[1] * r + (cy - win_h / 2.0) * (1.0 - r);
    }

    fn spawn_render(&self, page: u32) {
        let pipeline = self.pipeline.clone();
        let proxy = self.proxy.clone();
        let preview = RenderConfig {
            dpi: 72,
            ..self.config.clone()
        };
        let full = self.config.clone();
        std::thread::spawn(move || {
            if let Ok(img) = pipeline.render_page(page, &preview) {
                let _ = proxy.send_event(ViewerEvent::PreviewReady { page, image: img });
            }
            if let Ok(img) = pipeline.render_page(page, &full) {
                let _ = proxy.send_event(ViewerEvent::PageReady { page, image: img });
            }
        });
    }

    // ── Plate separation ──────────────────────────────────────────────────────

    /// Spawn a background thread that renders the ICC plate image for `active_plate`.
    ///
    /// Does nothing when `active_plate` is [`PlateMode::All`].  On completion
    /// the thread sends [`ViewerEvent::PlateReady`] so the main thread can swap
    /// in the new image.
    ///
    /// A snapshot of the current plate mode and a clone of `current_image` are
    /// captured at call time; the result is discarded on arrival if either the
    /// page or the plate has changed since.
    fn spawn_plate_separation(&self) {
        let plate = self.active_plate.clone();
        let page = self.page;
        let tinted = self.plate_tinted;
        let proxy = self.proxy.clone();

        match plate {
            PlateMode::All => (), // nothing to do

            PlateMode::Cmyk(ch) => {
                let Some(tree) = self.object_tree.as_ref() else {
                    return;
                };
                let Some(media) = self.page_boxes.as_ref().map(|b| b.media_box) else {
                    return;
                };
                let Some(src) = self.current_image.as_ref() else {
                    return;
                };
                let (img_w, img_h) = src.dimensions();
                let objects: Vec<PageObject> = tree.objects.clone();
                // Clone the rasterized page so the thread can sub-sample image
                // object regions through ICC without borrowing self.
                let page_image = src.clone();
                // Clone glyph outlines so the thread can render actual text paths.
                let glyph_outlines: Vec<PositionedGlyph> = self
                    .glyph_outlines
                    .as_deref()
                    .unwrap_or(&[])
                    .to_vec();
                let tinted = self.plate_tinted;
                let proxy = self.proxy.clone();
                let page = self.page;

                std::thread::spawn(move || {
                    let transform = sep_build_icc_transform();
                    let plate_ch = match ch {
                        CmykChannel::Cyan => PlateChannel::Cyan,
                        CmykChannel::Magenta => PlateChannel::Magenta,
                        CmykChannel::Yellow => PlateChannel::Yellow,
                        CmykChannel::Black => PlateChannel::Black,
                    };
                    let image = crate::separation::render_cmyk_plate(
                        &objects,
                        &media,
                        plate_ch,
                        tinted,
                        transform.as_ref(),
                        Some(&page_image),
                        &glyph_outlines,
                        img_w,
                        img_h,
                    );
                    let _ = proxy.send_event(ViewerEvent::PlateReady {
                        page,
                        plate: PlateMode::Cmyk(ch),
                        image,
                    });
                });
            }

            PlateMode::Spot(name) => {
                // Collect matching objects so we don't send the whole tree.
                let Some(tree) = self.object_tree.as_ref() else {
                    return;
                };
                let Some(media) = self.page_boxes.as_ref().map(|b| b.media_box.clone()) else {
                    return;
                };
                let Some(src) = self.current_image.as_ref() else {
                    return;
                };
                let (img_w, img_h) = src.dimensions();
                let selector = rustybara::objects::InkSelector::Separation(name.clone());
                let matched: Vec<PageObject> = filter_by_ink(tree, &selector)
                    .into_iter()
                    .cloned()
                    .collect();
                // Clone glyph outlines for spot plate text rendering.
                let glyph_outlines: Vec<PositionedGlyph> = self
                    .glyph_outlines
                    .as_deref()
                    .unwrap_or(&[])
                    .to_vec();
                let name = name.clone();
                std::thread::spawn(move || {
                    let image = crate::separation::render_spot_plate(
                        &matched, &media, tinted, None, &glyph_outlines, img_w, img_h,
                    );
                    let _ = proxy.send_event(ViewerEvent::PlateReady {
                        page,
                        plate: PlateMode::Spot(name),
                        image,
                    });
                });
            }
        }
    }

    // ── Debug log ─────────────────────────────────────────────────────────────

    /// Append a message to the debug ring-buffer, evicting the oldest entry when full.
    fn push_log(&mut self, msg: impl Into<String>) {
        if self.debug_log.len() >= DEBUG_LOG_CAP {
            self.debug_log.pop_front();
        }
        self.debug_log.push_back(msg.into());
    }

    // ── Coordinate helpers ────────────────────────────────────────────────────

    /// Compute the screen-space rectangle the page image is drawn into,
    /// given the current zoom and pan. Returns `None` if no image is loaded.
    fn compute_page_rect(&self) -> Option<skia_safe::Rect> {
        let state = self.state.as_ref()?;
        let img = state.page_image.as_ref()?;

        let img_w = img.width() as f32;
        let img_h = img.height() as f32;
        let win_w = state.width as f32;
        let win_h = state.height as f32;

        let base_scale = (win_w / img_w).min(win_h / img_h);
        let scale = base_scale * self.zoom;

        let draw_w = img_w * scale;
        let draw_h = img_h * scale;
        let x = (win_w - draw_w) / 2.0 + self.pan[0];
        let y = (win_h - draw_h) / 2.0 + self.pan[1];

        Some(skia_safe::Rect::from_xywh(x, y, draw_w, draw_h))
    }

    /// Convert a screen-space position to PDF page-space coordinates (Y-up).
    ///
    /// Returns `None` if no image or page boxes are loaded.
    fn screen_to_pdf(&self, screen: [f32; 2]) -> Option<(f64, f64)> {
        let page_rect = self.compute_page_rect()?;
        let media = &self.page_boxes.as_ref()?.media_box;

        let rel_x = (screen[0] - page_rect.left()) / page_rect.width();
        let rel_y = (screen[1] - page_rect.top()) / page_rect.height();

        let pdf_x = media.x + rel_x as f64 * media.width;
        let pdf_y = media.top() - rel_y as f64 * media.height; // Y-axis flip

        Some((pdf_x, pdf_y))
    }

    /// Sample the RGBA pixel from the retained page image at the given screen position.
    ///
    /// Returns `None` if no image is available or the cursor is outside the page area.
    fn sample_pixel(&self, screen: [f32; 2]) -> Option<[u8; 4]> {
        let page_rect = self.compute_page_rect()?;
        let img = self.current_image.as_ref()?;

        let img_w = img.width() as f32;
        let img_h = img.height() as f32;

        let rel_x = (screen[0] - page_rect.left()) / page_rect.width();
        let rel_y = (screen[1] - page_rect.top()) / page_rect.height();

        let px = rel_x * img_w;
        let py = rel_y * img_h;

        if px < 0.0 || py < 0.0 || px >= img_w || py >= img_h {
            return None;
        }

        let pixel = img.get_pixel(px as u32, py as u32);
        Some([pixel[0], pixel[1], pixel[2], pixel[3]])
    }

    // ── Selection ─────────────────────────────────────────────────────────────

    /// Run a hit-test at the current cursor position, update `selected_object`,
    /// and capture pixel and color info into `color_info`.
    ///
    /// Clears the selection if the cursor is outside the page or no tree is loaded.
    fn handle_selection_click(&mut self) {
        let Some((pdf_x, pdf_y)) = self.screen_to_pdf(self.cursor_pos) else {
            self.selected_object = None;
            self.color_info = None;
            self.sampling_pdf_pos = None;
            self.push_log("Click outside page — selection cleared");
            return;
        };

        // Select the most specific (smallest-area bbox) hit.
        //
        // hit_test returns all objects whose geometry contains (pdf_x, pdf_y) in
        // back-to-front paint order.  Using `last()` (topmost paint order) causes
        // full-page border strokes to win every click because their bbox always
        // contains every point on the page.  Picking the smallest bbox instead
        // surfaces the most specific object under the cursor.
        self.selected_object = self.object_tree.as_ref().and_then(|tree| {
            let hits = hit_test(tree, pdf_x, pdf_y);
            hits.into_iter()
                .min_by(|a, b| {
                    let area_a = a.bbox.width * a.bbox.height;
                    let area_b = b.bbox.width * b.bbox.height;
                    area_a
                        .partial_cmp(&area_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned()
        });

        // Sample the rasterized pixel at the click position.
        let pixel_rgba = self.sample_pixel(self.cursor_pos).unwrap_or([0, 0, 0, 0]);

        // Prefer fill color; fall back to stroke color.
        let pdf_color = self.selected_object.as_ref().and_then(|obj| {
            obj.fill_color
                .as_ref()
                .or(obj.stroke_color.as_ref())
                .cloned()
        });

        // Build the ICC transform lazily on first click; reuse for all subsequent clicks.
        if self.icc_transform.is_none() {
            match build_icc_transform() {
                Some(t) => {
                    self.push_log(format!(
                        "ICC transform ready: {} → SWOP",
                        if t.src_channels() == 3 { "RGB" } else { "?" }
                    ));
                    self.icc_transform = Some(t);
                }
                None => {
                    self.push_log("ICC transform init failed — pixel CMYK unavailable".to_string());
                }
            }
        }

        // Convert the sampled RGB pixel to CMYK via the ICC transform.
        // Alpha is not passed through — ICC operates on device RGB only.
        let pixel_cmyk = self.icc_transform.as_ref().map(|t| {
            let [r, g, b, _a] = pixel_rgba;
            let cmyk_u8 = t.convert(&[r, g, b]);
            [
                cmyk_u8[0] as f32 / 255.0,
                cmyk_u8[1] as f32 / 255.0,
                cmyk_u8[2] as f32 / 255.0,
                cmyk_u8[3] as f32 / 255.0,
            ]
        });

        // Store PDF coords for zoom/pan-invariant marker projection each frame.
        self.sampling_pdf_pos = Some((pdf_x, pdf_y));

        self.color_info = Some(ColorPanel {
            pixel_rgba,
            pdf_color,
            pixel_cmyk,
        });

        // Debug log.
        let kind_str = self
            .selected_object
            .as_ref()
            .map_or("none", |obj| match &obj.kind {
                ObjectKind::Fill => "Fill",
                ObjectKind::Stroke => "Stroke",
                ObjectKind::FillStroke => "FillStroke",
                ObjectKind::Text(_) => "Text",
                ObjectKind::Image => "Image",
                ObjectKind::FormXObject => "FormXObject",
            });
        self.push_log(format!("Click ({pdf_x:.1}, {pdf_y:.1}) → {kind_str}"));
    }

    // ── Wireframe PDF export ──────────────────────────────────────────────────

    /// Export the current page's object tree as a diagnostic wireframe PDF.
    ///
    /// Bound to `Ctrl+Shift+E`.  The output file is written next to the source
    /// PDF with the suffix `_wireframe_diag.pdf`.  The full output path is
    /// pushed to the debug ring-buffer so it appears in the debug overlay.
    ///
    /// Does nothing (and logs a message) if no object tree or page boxes are
    /// loaded yet.
    fn export_wireframe_pdf(&mut self) {
        let (Some(tree), Some(boxes)) = (self.object_tree.as_ref(), self.page_boxes.as_ref())
        else {
            self.push_log("Export skipped: no object tree loaded");
            return;
        };

        // Derive output path: same directory as the source PDF, same stem + suffix.
        let output_path = {
            let stem = self
                .file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("page");
            let dir = self.file.parent().unwrap_or(std::path::Path::new("."));
            dir.join(format!("{}_wireframe_diag.pdf", stem))
        };

        match export_wireframe(tree, &boxes.media_box, &output_path) {
            Ok(()) => {
                self.push_log(format!("Exported: {}", output_path.display()));
            }
            Err(e) => {
                self.push_log(format!("Export failed: {e}"));
            }
        }
    }

    // ── Page navigation ───────────────────────────────────────────────────────

    /// Navigate to `page` (0-indexed), reloading all page metadata and
    /// triggering a background render. Values outside `[0, page_count)` are
    /// clamped. Does nothing when the requested page equals the current page.
    fn navigate_to_page(&mut self, page: u32) {
        let page = page.min(self.page_count.saturating_sub(1));
        if page == self.page {
            return;
        }
        self.page = page;

        // Reload page-level metadata synchronously (lopdf access, no rasterisation).
        let page_id = self
            .pipeline
            .doc()
            .get_pages()
            .values()
            .nth(page as usize)
            .copied();
        self.page_boxes = page_id.and_then(|id| PageBoxes::read(self.pipeline.doc(), id).ok());
        self.object_tree = page_id.and_then(|id| build_object_tree(self.pipeline.doc(), id).ok());
        self.glyph_outlines =
            page_id.and_then(|id| outline_page_text(self.pipeline.doc(), id).ok());
        self.plate_spot_names = self
            .object_tree
            .as_ref()
            .map(extract_spot_names)
            .unwrap_or_default();

        // Clear stale selection and images from the previous page.
        self.selected_object = None;
        self.color_info = None;
        self.sampling_pdf_pos = None;
        self.pending_image = None;
        self.current_image = None;
        self.tile_sender = None; // drop → worker channel closes → worker exits
        self.tile_cache.clear();
        if let Some(state) = self.state.as_mut() {
            state.page_image = None;
            state.plate_image = None;
            state.tile_images.clear();
        }
        self.plate_image_for = PlateMode::All;

        // Update window title and log.
        let title = format!("rbv \u{2014} {}/{}", page + 1, self.page_count);
        if let Some(state) = self.state.as_ref() {
            state.window.set_title(&title);
            state.window.request_redraw();
        }
        self.push_log(format!("Page {}/{}", page + 1, self.page_count));
        self.spawn_render(page);
    }

    /// Consume `digit_buf` as a step count and return it (minimum 0).
    /// Clears the buffer regardless of whether parsing succeeded.
    fn take_digit_step(&mut self) -> u32 {
        let n = self.digit_buf.parse::<u32>().unwrap_or(0);
        self.digit_buf.clear();
        n
    }

    // ── Debug overlay lines ───────────────────────────────────────────────────

    /// Build the list of text lines for the tile performance telemetry panel.
    ///
    /// Called once per frame when telemetry mode is active. Reads `self` immutably.
    fn build_telemetry_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        lines.push("── TELEMETRY  (Ctrl+Shift+T to close) ──────────".to_string());

        let (bucket, dpi) = crate::tiles::zoom_bucket(self.zoom);
        lines.push(format!("Zoom  {:.3}×   Bucket {}   {:.0} DPI", self.zoom, bucket, dpi));

        lines.push("\u{2500}\u{2500}\u{2500} Full-page Render Times \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string());

        if self.tile_perf.is_empty() {
            lines.push("  No renders yet  (zoom past 1.5\u{00d7} to activate)".to_string());
        } else {
            let mut sorted: Vec<(u32, (u64, u32, u32))> =
                self.tile_perf.iter().map(|(k, v)| (*k, *v)).collect();
            sorted.sort_by_key(|(d, _)| *d);
            for (rdpi, (rms, rw, rh)) in &sorted {
                let bytes = *rw as u64 * *rh as u64 * 4;
                let mem_str = if bytes >= 1024 * 1024 {
                    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
                } else {
                    format!("{} KB", bytes / 1024)
                };
                lines.push(format!(
                    "  {:>4}dpi  {:>5}ms   {}×{}  {}",
                    rdpi, rms, rw, rh, mem_str
                ));
            }
        }

        lines.push("\u{2500}\u{2500}\u{2500} Tile Cache \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string());

        let mut per_bucket: std::collections::HashMap<u8, usize> =
            std::collections::HashMap::new();
        if let Some(state) = self.state.as_ref() {
            for k in state.tile_images.keys() {
                *per_bucket.entry(k.zoom_bucket).or_insert(0) += 1;
            }
        }

        const BUCKET_INFO: &[(u8, f32)] = &[
            (1, 300.0), (2, 450.0), (3, 500.0), (4, 600.0),
            (5, 800.0), (6, 1000.0), (7, 1200.0),
        ];
        for (b, bdpi) in BUCKET_INFO {
            let count = per_bucket.get(b).copied().unwrap_or(0);
            if count > 0 {
                let kb = count as u64
                    * crate::tiles::TILE_SIZE as u64
                    * crate::tiles::TILE_SIZE as u64
                    * 4
                    / 1024;
                lines.push(format!(
                    "  Bucket {} ({:>3}dpi)  {:>3} tiles  ~{} KB",
                    b, *bdpi as u32, count, kb
                ));
            } else {
                lines.push(format!("  Bucket {} ({:>3}dpi)  —", b, *bdpi as u32));
            }
        }

        lines.push("\u{2500}\u{2500}\u{2500} Page Grid \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}".to_string());
        if bucket > 0 {
            let page_pts = self
                .page_boxes
                .as_ref()
                .map(|b| (b.media_box.width as f32, b.media_box.height as f32))
                .unwrap_or((612.0, 792.0));
            let tile_page_w = page_pts.0 * dpi / 72.0;
            let tile_page_h = page_pts.1 * dpi / 72.0;
            let cols =
                (tile_page_w / crate::tiles::TILE_SIZE as f32).ceil() as u32;
            let rows =
                (tile_page_h / crate::tiles::TILE_SIZE as f32).ceil() as u32;
            let cached = per_bucket.get(&bucket).copied().unwrap_or(0);
            lines.push(format!(
                "  {}×{} = {} tiles total   {} cached",
                cols,
                rows,
                cols * rows,
                cached
            ));
        } else {
            lines.push("  Tiling inactive below 1.5\u{00d7} zoom".to_string());
        }

        lines
    }

    /// Build the list of text lines for the debug overlay.
    ///
    /// Called once per frame when debug mode is active. Reads `self` immutably
    /// so it must be called **before** taking a mutable borrow of `self.state`
    /// in the `RedrawRequested` handler.
    fn build_debug_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();

        lines.push("── DEBUG  (Ctrl+Shift+D to close) ────────────────".to_string());

        // Viewport
        lines.push(format!(
            "Zoom  {:.3}×    Pan  [{:+.1}, {:+.1}]",
            self.zoom, self.pan[0], self.pan[1]
        ));
        lines.push(format!(
            "Cursor  ({:.0}, {:.0})  screen",
            self.cursor_pos[0], self.cursor_pos[1]
        ));
        match self.screen_to_pdf(self.cursor_pos) {
            Some((px, py)) => {
                lines.push(format!("        ({:.2}, {:.2})  pdf", px, py));
            }
            None => {
                lines.push("        —  pdf (outside page)".to_string());
            }
        }

        // Page info
        let obj_count = self.object_tree.as_ref().map_or(0, |t| t.objects.len());
        lines.push(format!(
            "Page  {}/{}   Objects: {}",
            self.page + 1,
            self.page_count,
            obj_count
        ));
        if !self.digit_buf.is_empty() {
            lines.push(format!("Jump prefix: {}▋", self.digit_buf));
        }

        // Mode flags
        lines.push(format!(
            "Overlays: {}   Wireframe: {}",
            if self.show_overlays { "ON " } else { "OFF" },
            if self.show_wireframe { "ON " } else { "OFF" },
        ));

        // Selected object
        if let Some(obj) = &self.selected_object {
            let kind_str = match &obj.kind {
                ObjectKind::Fill => "Fill",
                ObjectKind::Stroke => "Stroke",
                ObjectKind::FillStroke => "FillStroke",
                ObjectKind::Text(_) => "Text",
                ObjectKind::Image => "Image",
                ObjectKind::FormXObject => "FormXObject",
            };
            let color_str = match obj.fill_color.as_ref().or(obj.stroke_color.as_ref()) {
                Some(PdfColor::DeviceGray(v)) => format!("Gray({v:.3})"),
                Some(PdfColor::DeviceRgb(r, g, b)) => {
                    format!("RGB({r:.2} {g:.2} {b:.2})")
                }
                Some(PdfColor::DeviceCmyk(c, m, y, k)) => {
                    format!("CMYK({c:.2} {m:.2} {y:.2} {k:.2})")
                }
                Some(PdfColor::Separation { name, tint }) => {
                    format!("Spot({name} @ {tint:.3})")
                }
                None => "n/a".to_string(),
            };
            lines.push(format!("Selected  {}   color: {}", kind_str, color_str));
            lines.push(format!(
                "  bbox  x:{:.1} y:{:.1} w:{:.1} h:{:.1}",
                obj.bbox.x, obj.bbox.y, obj.bbox.width, obj.bbox.height
            ));
        } else {
            lines.push("Selected  none".to_string());
        }

        // Log section
        lines.push("─── Log ─────────────────────────────────────────".to_string());
        for entry in self.debug_log.iter().rev().take(12) {
            lines.push(format!("  {}", entry));
        }

        lines
    }
}

// ── ICC helpers ───────────────────────────────────────────────────────────────

/// Try to locate and validate an sRGB ICC profile from the OS color management
/// system. On Windows this scans `System32\spool\drivers\color\`; on macOS
/// it checks the standard ColorSync directories; on Linux it tries common
/// FreeDesktop paths.
///
/// Uses [`rustybara_icc::profiles::IccProfile::from_user_bytes`] for validation
/// (same approach as rbara-gui's `load_persisted_profiles`). Only profiles
/// whose detected color space is `Rgb` are returned; any file that parses as a
/// different space (e.g. a stray `.icm` in the color directory) is skipped.
///
/// Returns `None` if no valid RGB profile was found, in which case the caller
/// should fall back to the bundled `AdobeRGB1998` profile.
fn find_system_srgb() -> Option<rustybara_icc::profiles::IccProfile> {
    use rustybara_icc::{profiles::IccProfile, ColorSpaceKind};

    #[cfg(target_os = "windows")]
    let candidates: &[&str] = &[
        "C:\\Windows\\System32\\spool\\drivers\\color\\sRGB Color Space Profile.icm",
        "C:\\Windows\\System32\\spool\\drivers\\color\\sRGB_IEC61966-2-1.icm",
        "C:\\Windows\\System32\\spool\\drivers\\color\\sRGB IEC61966-2-1.icm",
    ];
    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &[
        "/System/Library/ColorSync/Profiles/sRGB IEC61966-2.1.icc",
        "/Library/ColorSync/Profiles/sRGB.icc",
    ];
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let candidates: &[&str] = &[
        "/usr/share/color/icc/sRGB.icc",
        "/usr/share/colorhug/sRGB.icc",
        "/usr/share/color/icc/colord/sRGB.icc",
    ];

    for path in candidates {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sRGB")
            .to_string();
        if let Ok(profile) = IccProfile::from_user_bytes(stem.clone(), stem, bytes) {
            if profile.color_space == ColorSpaceKind::Rgb {
                return Some(profile);
            }
        }
    }
    None
}

/// Build the RGB → US Web Coated SWOP color transform used for pixel readout.
///
/// Priority:
///   1. sRGB from the OS ICC system (detected by [`find_system_srgb`]).
///   2. Bundled `AdobeRGB1998` (always available with the `bundled-profiles`
///      feature) as a fallback when no system sRGB profile is present.
///
/// Returns `None` only if transform construction fails (e.g. lcms2 error),
/// which should not happen with valid bundled profiles.
fn build_icc_transform() -> Option<ColorTransform> {
    use rustybara_icc::profiles;

    let dst = &profiles::US_WEB_COATED_SWOP;
    let intent = RenderingIntent::RelativeColorimetric;

    if let Some(srgb) = find_system_srgb() {
        match ColorTransform::from_bytes(&srgb.bytes, &dst.bytes, intent) {
            Ok(t) => return Some(t),
            Err(_) => {} // fall through to AdobeRGB fallback
        }
    }

    // Fall back to bundled AdobeRGB 1998.
    ColorTransform::new(&profiles::ADOBE_RGB_1998, dst, intent).ok()
}

// ── ApplicationHandler ────────────────────────────────────────────────────────

impl ApplicationHandler<ViewerEvent> for Viewer {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        use glutin::{
            config::ConfigTemplateBuilder,
            context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext},
            display::{Display, DisplayApiPreference},
            prelude::*,
            surface::SurfaceAttributesBuilder,
        };
        use glutin_winit::GlWindow;
        use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

        let title = format!("rbv \u{2014} {}/{}", self.page + 1, self.page_count);
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title(&title))
                .expect("create window"),
        );
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let raw_window = window.window_handle().expect("window handle").as_raw();
        let raw_display = event_loop
            .display_handle()
            .expect("display handle")
            .as_raw();

        let gl_display = unsafe {
            #[cfg(target_os = "windows")]
            let pref = DisplayApiPreference::EglThenWgl(Some(raw_window));
            #[cfg(target_os = "linux")]
            let pref = DisplayApiPreference::EglThenGlx(Box::new(|_| {}));
            #[cfg(target_os = "macos")]
            let pref = DisplayApiPreference::Cgl;
            Display::new(raw_display, pref).expect("GL display")
        };

        let config_template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_stencil_size(8)
            .build();

        let gl_config = unsafe {
            gl_display
                .find_configs(config_template)
                .expect("find GL configs")
                .next()
                .expect("no GL config found")
        };

        let surface_attrs = window
            .build_surface_attributes(SurfaceAttributesBuilder::new())
            .expect("surface attributes");
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &surface_attrs)
                .expect("GL window surface")
        };

        let gl_context = unsafe {
            let core = ContextAttributesBuilder::new()
                .with_context_api(ContextApi::OpenGl(None))
                .build(Some(raw_window));
            let gles = ContextAttributesBuilder::new()
                .with_context_api(ContextApi::Gles(None))
                .build(Some(raw_window));
            gl_display
                .create_context(&gl_config, &core)
                .or_else(|_| gl_display.create_context(&gl_config, &gles))
                .expect("GL context")
        }
        .make_current(&gl_surface)
        .expect("make context current");

        let renderer = SkiaRenderer::from_gl(gl_context, gl_surface, width, height);

        let egui_winit = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            None,
            None, // max_texture_side — None lets egui query it automatically
        );

        let page_image = self.pending_image.as_ref().map(image_to_skia);
        if page_image.is_some() {
            window.request_redraw();
        }
        self.state = Some(SkiaState {
            window,
            renderer,
            egui_winit,
            page_image,
            plate_image: None,
            tile_images: std::collections::HashMap::new(),
            width,
            height,
        });
        // Move pending_image into current_image for pixel sampling, clearing pending.
        self.current_image = self.pending_image.take();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.state.is_none() {
            return;
        }

        // Forward to egui before processing viewer input. Structural window
        // events (resize, redraw, close) are never consumed by egui.
        let egui_consumed = match &event {
            WindowEvent::RedrawRequested
            | WindowEvent::Resized(_)
            | WindowEvent::CloseRequested => false,
            _ => {
                let state = self.state.as_mut().unwrap();
                let resp = state.egui_winit.on_window_event(&state.window, &event);
                if resp.repaint {
                    state.window.request_redraw();
                }
                resp.consumed
            }
        };
        if egui_consumed {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                // ── Phase 1: take egui input ──────────────────────────────────
                let raw_egui_input = {
                    let state = self.state.as_mut().unwrap();
                    state.egui_winit.take_egui_input(&state.window)
                };

                // ── Phase 2: run egui frame ───────────────────────────────────
                // Clone is a cheap Arc bump; lets the closure borrow other fields
                // of `self` without conflicting with the egui_ctx borrow.
                let egui_ctx = self.egui_ctx.clone();
                let plate_before = self.active_plate.clone();
                let tinted_before = self.plate_tinted;
                let mut egui_over = false;
                let full_output = egui_ctx.run_ui(raw_egui_input, |ctx| {
                    egui_over = build_egui_ui(
                        ctx,
                        &mut self.active_plate,
                        &mut self.plate_tinted,
                        &self.plate_spot_names,
                        self.selected_object.as_ref(),
                        self.color_info.as_ref(),
                        &mut self.show_tools_panel,
                    );
                });
                // Store for next-frame use in mouse-event gating.
                self.egui_pointer_over_panel = egui_over;

                // ── Plate change detection ────────────────────────────────────
                // If the user changed the plate selection or the tint toggle,
                // clear the stale plate image and kick off a new separation.
                let plate_changed = self.active_plate != plate_before;
                let tint_changed = self.plate_tinted != tinted_before;
                if plate_changed || tint_changed {
                    if let Some(state) = self.state.as_mut() {
                        state.plate_image = None;
                    }
                    self.plate_image_for = PlateMode::All; // invalidate
                    if self.active_plate != PlateMode::All {
                        self.push_log(format!(
                            "Separation: {:?} tint={}",
                            self.active_plate.to_ink_selector(),
                            self.plate_tinted
                        ));
                        self.spawn_plate_separation();
                    }
                }

                let egui::FullOutput {
                    platform_output,
                    textures_delta,
                    shapes,
                    pixels_per_point,
                    ..
                } = full_output;

                // ── Phase 3: handle platform output (cursor, clipboard) ───────
                {
                    let state = self.state.as_mut().unwrap();
                    state
                        .egui_winit
                        .handle_platform_output(&state.window, platform_output);
                }

                // ── Phase 4: tessellate ───────────────────────────────────────
                let egui_primitives = self.egui_ctx.tessellate(shapes, pixels_per_point);

                // ── Phase 5: build debug/telemetry overlays (immutable borrows) ─
                let debug_lines: Option<Vec<String>> = if self.debug_mode {
                    Some(self.build_debug_lines())
                } else {
                    None
                };
                let debug_overlay = debug_lines.as_deref().map(|lines| DebugOverlay { lines });

                let telemetry_lines: Option<Vec<String>> = if self.telemetry_mode {
                    Some(self.build_telemetry_lines())
                } else {
                    None
                };
                let telemetry_overlay =
                    telemetry_lines.as_deref().map(|lines| TelemetryOverlay { lines });

                // ── Phase 6: project sample crosshair ────────────────────────
                // compute_page_rect borrows self.state immutably — must happen
                // before the mutable borrow taken in phase 7.
                let sample_screen_pos: Option<[f32; 2]> =
                    self.sampling_pdf_pos.and_then(|(pdf_x, pdf_y)| {
                        let page_rect = self.compute_page_rect()?;
                        let media = &self.page_boxes.as_ref()?.media_box;
                        let rel_x = ((pdf_x - media.x) / media.width) as f32;
                        let rel_y = ((media.top() - pdf_y) / media.height) as f32;
                        Some([
                            page_rect.left() + rel_x * page_rect.width(),
                            page_rect.top() + rel_y * page_rect.height(),
                        ])
                    });

                // ── Pre-Phase 7: compute tile overlay pairs ───────────────────
                // Must happen before `let state = self.state.as_mut()` because
                // `compute_page_rect()` borrows `self.state` immutably. The pairs
                // are owned (Images cloned cheaply via Arc) so no lifetime issue.
                let (tile_bucket, tile_dpi) = crate::tiles::zoom_bucket(self.zoom);
                let tile_pairs: Vec<(skia_safe::Rect, skia_safe::Image)> =
                    if tile_bucket > 0
                        && !self.show_wireframe
                        && self.active_plate == PlateMode::All
                    {
                        if let (Some(page_rect), Some(boxes)) =
                            (self.compute_page_rect(), self.page_boxes.as_ref())
                        {
                            let page_pts =
                                (boxes.media_box.width as f32, boxes.media_box.height as f32);
                            self.state.as_ref().map_or(vec![], |s| {
                                s.tile_images
                                    .iter()
                                    .filter(|(k, _)| {
                                        k.page == self.page && k.zoom_bucket == tile_bucket
                                    })
                                    .map(|(k, img)| {
                                        let r = crate::tiles::tile_rect_on_screen(
                                            k, &page_rect, page_pts, tile_dpi,
                                        );
                                        (r, img.clone())
                                    })
                                    .collect()
                            })
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    };

                // ── Phase 7: draw ─────────────────────────────────────────────
                let state = self.state.as_mut().unwrap();
                let overlays = if self.show_overlays {
                    self.page_boxes.as_ref().map(|b| OverlayData { boxes: b })
                } else {
                    None
                };

                // Plate filter — pre-compute before the wireframe block so the owned
                // Vec<PageObject> outlives the PageWireframe borrow below.
                // Only allocated when a non-All plate is active AND the tree is loaded.
                let filtered_for_plate: Option<Vec<PageObject>> =
                    self.active_plate.to_ink_selector().and_then(|sel| {
                        self.object_tree
                            .as_ref()
                            .map(|tree| filter_by_ink(tree, &sel).into_iter().cloned().collect())
                    });

                // Wireframe: Acrobat-style full-page mode (W key).
                // When a plate is active the wireframe only draws objects on that ink.
                // Borrows from self.object_tree / self.page_boxes / self.selected_object;
                // these are different fields from self.state so partial-field borrow is fine.
                let wireframe = if self.show_wireframe {
                    self.object_tree
                        .as_ref()
                        .zip(self.page_boxes.as_ref())
                        .map(|(tree, boxes)| PageWireframe {
                            objects: filtered_for_plate.as_deref().unwrap_or(&tree.objects),
                            media_box: &boxes.media_box,
                            selected: self.selected_object.as_ref(),
                            glyph_outlines: self.glyph_outlines.as_deref().unwrap_or(&[]),
                        })
                } else {
                    None
                };

                // Texture uploads must precede the egui draw call.
                state.renderer.update_egui_textures(&textures_delta);

                // When a plate is active, prefer plate_image; fall back to
                // page_image while the separation is still computing.
                let display_image = if self.active_plate != PlateMode::All {
                    state.plate_image.as_ref().or(state.page_image.as_ref())
                } else {
                    state.page_image.as_ref()
                };

                state.renderer.draw(
                    display_image,
                    self.zoom,
                    self.pan,
                    overlays.as_ref(),
                    wireframe.as_ref(),
                );

                // ── Phase 7b: tile overlay ────────────────────────────────────
                if !tile_pairs.is_empty() {
                    state.renderer.draw_tiles(&tile_pairs);
                }

                // ── Phase 7c: top-layer overlays (always above tiles) ─────────
                state.renderer.draw_top_layer(
                    sample_screen_pos,
                    debug_overlay.as_ref(),
                    telemetry_overlay.as_ref(),
                );

                state.renderer.draw_egui(&egui_primitives, pixels_per_point);

                // Free textures egui no longer references.
                state.renderer.free_egui_textures(&textures_delta);

                state.renderer.present();
            }

            WindowEvent::Resized(size) => {
                let state = self.state.as_mut().unwrap();
                state.width = size.width.max(1);
                state.height = size.height.max(1);
                state.renderer.resize(size.width, size.height);
                state.window.request_redraw();
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.ctrl_held = mods.state().contains(ModifiersState::CONTROL);
                self.shift_held = mods.state().contains(ModifiersState::SHIFT);
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                let (win_w, win_h) = {
                    let s = self.state.as_ref().unwrap();
                    (s.width as f32, s.height as f32)
                };
                match code {
                    KeyCode::Equal | KeyCode::NumpadAdd if self.ctrl_held => {
                        self.apply_zoom(1.1, None, win_w, win_h);
                    }
                    KeyCode::Minus | KeyCode::NumpadSubtract if self.ctrl_held => {
                        self.apply_zoom(1.0 / 1.1, None, win_w, win_h);
                    }
                    KeyCode::Digit0 | KeyCode::Numpad0 if self.ctrl_held => {
                        self.zoom = 1.0;
                        self.pan = [0.0, 0.0];
                    }
                    KeyCode::KeyO if !self.ctrl_held && !self.shift_held => {
                        self.show_overlays = !self.show_overlays;
                        self.push_log(format!(
                            "Overlays: {}",
                            if self.show_overlays { "ON" } else { "OFF" }
                        ));
                    }
                    KeyCode::KeyW if !self.ctrl_held && !self.shift_held => {
                        self.show_wireframe = !self.show_wireframe;
                        self.push_log(format!(
                            "Wireframe: {}",
                            if self.show_wireframe { "ON" } else { "OFF" }
                        ));
                    }
                    KeyCode::KeyD if self.ctrl_held && self.shift_held => {
                        // Ctrl+Shift+D — toggle debug overlay.
                        self.debug_mode = !self.debug_mode;
                        self.push_log(format!(
                            "Debug mode: {}",
                            if self.debug_mode { "ON" } else { "OFF" }
                        ));
                    }
                    KeyCode::KeyT if self.ctrl_held && self.shift_held => {
                        // Ctrl+Shift+T — toggle tile performance telemetry panel.
                        self.telemetry_mode = !self.telemetry_mode;
                        self.push_log(format!(
                            "Telemetry: {}",
                            if self.telemetry_mode { "ON" } else { "OFF" }
                        ));
                    }
                    KeyCode::KeyE if self.ctrl_held && self.shift_held => {
                        // Ctrl+Shift+E — export wireframe as a diagnostic PDF.
                        self.export_wireframe_pdf();
                    }
                    KeyCode::Escape => {
                        // Cancel an in-progress digit prefix first; exit on bare Escape.
                        if !self.digit_buf.is_empty() {
                            self.digit_buf.clear();
                            self.push_log("Jump cancelled".to_string());
                        } else {
                            std::process::exit(0);
                        }
                    }

                    // ── Page navigation ───────────────────────────────────────

                    // Previous page — ArrowLeft / h / k / ArrowUp
                    KeyCode::ArrowLeft | KeyCode::KeyH | KeyCode::KeyK | KeyCode::ArrowUp
                        if !self.ctrl_held && !self.shift_held =>
                    {
                        let step = self.take_digit_step().max(1);
                        let target = self.page.saturating_sub(step);
                        self.navigate_to_page(target);
                    }

                    // Next page — ArrowRight / l / j / ArrowDown
                    KeyCode::ArrowRight | KeyCode::KeyL | KeyCode::KeyJ | KeyCode::ArrowDown
                        if !self.ctrl_held && !self.shift_held =>
                    {
                        let step = self.take_digit_step().max(1);
                        let target = self.page.saturating_add(step);
                        self.navigate_to_page(target);
                    }

                    // g — go to first page, or jump to a digit-prefixed page (1-indexed)
                    // G (Shift+G) — go to last page
                    KeyCode::KeyG if !self.ctrl_held => {
                        if self.shift_held {
                            self.digit_buf.clear();
                            self.navigate_to_page(self.page_count.saturating_sub(1));
                        } else if !self.digit_buf.is_empty() {
                            // `5g` → page 5 (1-indexed → 0-indexed)
                            let n = self.digit_buf.parse::<u32>().unwrap_or(1);
                            self.digit_buf.clear();
                            self.navigate_to_page(n.saturating_sub(1));
                        } else {
                            // bare `g` → first page
                            self.navigate_to_page(0);
                        }
                    }

                    // ── Digit prefix accumulation ─────────────────────────────
                    // Digits (no modifier) accumulate in digit_buf for <N>g jumps
                    // and <N>l/<N>h multi-step moves.
                    // Digit0 without ctrl is safe here because the ctrl+0 zoom-reset
                    // arm above already consumed the ctrl case.
                    code @ (KeyCode::Digit0
                    | KeyCode::Digit1
                    | KeyCode::Digit2
                    | KeyCode::Digit3
                    | KeyCode::Digit4
                    | KeyCode::Digit5
                    | KeyCode::Digit6
                    | KeyCode::Digit7
                    | KeyCode::Digit8
                    | KeyCode::Digit9)
                        if !self.ctrl_held && !self.shift_held =>
                    {
                        let d = match code {
                            KeyCode::Digit0 => '0',
                            KeyCode::Digit1 => '1',
                            KeyCode::Digit2 => '2',
                            KeyCode::Digit3 => '3',
                            KeyCode::Digit4 => '4',
                            KeyCode::Digit5 => '5',
                            KeyCode::Digit6 => '6',
                            KeyCode::Digit7 => '7',
                            KeyCode::Digit8 => '8',
                            KeyCode::Digit9 => '9',
                            _ => unreachable!(),
                        };
                        self.digit_buf.push(d);
                        self.push_log(format!("Jump: {}▋", self.digit_buf));
                    }

                    _ => {}
                }
                self.enqueue_visible_tiles();
                self.state.as_ref().unwrap().window.request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } if self.ctrl_held => {
                let (win_w, win_h) = {
                    let s = self.state.as_ref().unwrap();
                    (s.width as f32, s.height as f32)
                };
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };
                let factor = if lines > 0.0 {
                    1.1_f32.powf(lines)
                } else {
                    (1.0 / 1.1_f32).powf(-lines)
                };
                let focal = self.cursor_pos;
                self.apply_zoom(factor, Some(focal), win_w, win_h);
                self.enqueue_visible_tiles();
                self.state.as_ref().unwrap().window.request_redraw();
            }

            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = [position.x as f32, position.y as f32];
                if let Some((cursor_start, pan_start)) = self.drag_origin {
                    self.pan[0] = pan_start[0] + new_pos[0] - cursor_start[0];
                    self.pan[1] = pan_start[1] + new_pos[1] - cursor_start[1];
                    self.enqueue_visible_tiles();
                    self.state.as_ref().unwrap().window.request_redraw();
                }
                self.cursor_pos = new_pos;

                // Refresh debug overlay continuously so cursor coords stay live.
                if self.debug_mode {
                    self.state.as_ref().unwrap().window.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: btn_state,
                ..
            } => {
                match btn_state {
                    ElementState::Pressed => {
                        // Don't start a PDF pan/selection when the pointer was over
                        // any egui surface last frame.  `egui_pointer_over_panel` is
                        // set from build_egui_ui's rect-contains check — more reliable
                        // than egui_wants_pointer_input() which only fires during active
                        // drags, not passive hover-over-panel.
                        if !self.egui_pointer_over_panel {
                            self.drag_origin = Some((self.cursor_pos, self.pan));
                        }
                    }
                    ElementState::Released => {
                        // Disambiguate pan drag vs click using a 4-pixel distance threshold.
                        if let Some((cursor_start, _)) = self.drag_origin {
                            let dx = self.cursor_pos[0] - cursor_start[0];
                            let dy = self.cursor_pos[1] - cursor_start[1];
                            if dx * dx + dy * dy < 16.0 && !self.egui_pointer_over_panel {
                                self.handle_selection_click();
                            }
                        }
                        self.drag_origin = None;
                    }
                }
                // Always redraw on mouse events so egui's run_ui sees both the press
                // and the release — without this, a panel click where drag_origin was
                // never set reaches no request_redraw() path, the release event never
                // enters run_ui, and radio/button widgets never fire clicked().
                self.state.as_ref().unwrap().window.request_redraw();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: ViewerEvent) {
        match event {
            ViewerEvent::PreviewReady { page, image } if page == self.page => {
                let skia_img = image_to_skia(&image);
                if let Some(state) = self.state.as_mut() {
                    state.page_image = Some(skia_img);
                    // Plate image is stale — clear it so the preview shows immediately.
                    state.plate_image = None;
                    self.plate_image_for = PlateMode::All;
                    state.window.request_redraw();
                    self.current_image = Some(image);
                } else {
                    self.pending_image = Some(image);
                }
                self.push_log(format!("Preview ready, page {page}"));
            }
            ViewerEvent::PageReady { page, image } if page == self.page => {
                let skia_img = image_to_skia(&image);
                if let Some(state) = self.state.as_mut() {
                    state.page_image = Some(skia_img);
                    state.plate_image = None;
                    self.plate_image_for = PlateMode::All;
                    state.window.request_redraw();
                    self.current_image = Some(image);
                } else {
                    self.pending_image = Some(image);
                }
                let obj_count = self.object_tree.as_ref().map_or(0, |t| t.objects.len());
                self.push_log(format!("Page ready: {obj_count} objects, page {page}"));
                // Re-trigger separation if a plate is already active when the full
                // render arrives (e.g. user selected a plate before the HQ render).
                if self.active_plate != PlateMode::All {
                    self.spawn_plate_separation();
                }
                // Start the tile worker now that we have a full-resolution page image.
                self.spawn_tile_worker();
                self.enqueue_visible_tiles();
            }
            ViewerEvent::PlateReady { page, plate, image }
                if page == self.page && plate == self.active_plate =>
            {
                let skia_img = image_to_skia(&image);
                self.plate_image_for = plate;
                if let Some(state) = self.state.as_mut() {
                    state.plate_image = Some(skia_img);
                    state.window.request_redraw();
                }
                self.push_log("Plate separation ready".to_string());
            }
            ViewerEvent::TileReady { key, image }
                if key.page == self.page
                    && key.zoom_bucket == crate::tiles::zoom_bucket(self.zoom).0 =>
            {
                let cached = if let Some(state) = self.state.as_mut() {
                    state.tile_images.insert(key, image_to_skia(&image));
                    state.window.request_redraw();
                    state.tile_images.len()
                } else {
                    0
                };
                self.push_log(format!(
                    "Tile ready: p{}b{} ({},{}) → {} cached",
                    key.page, key.zoom_bucket, key.col, key.row, cached
                ));
            }
            ViewerEvent::TileReady { key, .. } => {
                self.push_log(format!(
                    "Tile stale: p{}b{} (cur p{}b{})",
                    key.page,
                    key.zoom_bucket,
                    self.page,
                    crate::tiles::zoom_bucket(self.zoom).0
                ));
            }
            ViewerEvent::TileLog(msg) => {
                self.push_log(msg);
            }
            ViewerEvent::TileMetrics { dpi, render_ms, img_w, img_h } => {
                self.tile_perf.insert(dpi as u32, (render_ms, img_w, img_h));
                if self.telemetry_mode {
                    if let Some(state) = self.state.as_ref() {
                        state.window.request_redraw();
                    }
                }
            }
            ViewerEvent::FileChanged => {
                if let Ok(new_pipeline) = PdfPipeline::open(&self.file) {
                    // Compute page_id once; it's Copy so it can be used twice.
                    let page_id = new_pipeline
                        .doc()
                        .get_pages()
                        .values()
                        .nth(self.page as usize)
                        .copied();

                    self.page_boxes =
                        page_id.and_then(|id| PageBoxes::read(new_pipeline.doc(), id).ok());
                    self.object_tree =
                        page_id.and_then(|id| build_object_tree(new_pipeline.doc(), id).ok());
                    self.glyph_outlines =
                        page_id.and_then(|id| outline_page_text(new_pipeline.doc(), id).ok());
                    self.plate_spot_names = self
                        .object_tree
                        .as_ref()
                        .map(extract_spot_names)
                        .unwrap_or_default();

                    self.pipeline = Arc::new(new_pipeline);
                }
                // Page content changed — clear stale selection, plate data, and tiles.
                self.selected_object = None;
                self.color_info = None;
                self.sampling_pdf_pos = None;
                self.tile_sender = None; // drop → worker channel closes → worker exits
                self.tile_cache.clear();
                if let Some(state) = self.state.as_mut() {
                    state.plate_image = None;
                    state.tile_images.clear();
                }
                self.plate_image_for = PlateMode::All;
                self.push_log("File reloaded — selection cleared");
                self.spawn_render(self.page);
            }
            ViewerEvent::IpcCommand(cmd) => match cmd {
                IpcCmd::Open(new_file) => {
                    match PdfPipeline::open(&new_file) {
                        Err(e) => {
                            self.push_log(format!("IPC OPEN failed: {e}"));
                        }
                        Ok(new_pipeline) => {
                            // Reattach watcher to new file.
                            self.watcher.unwatch(&self.file).ok();
                            self.watcher
                                .watch(&new_file, RecursiveMode::NonRecursive)
                                .ok();

                            // Swap pipeline and file path.
                            self.file = new_file;
                            self.pipeline = Arc::new(new_pipeline);
                            self.page_count = self.pipeline.doc().get_pages().len() as u32;
                            self.page = 0;

                            // Rebuild page-0 metadata.
                            let page_id = self.pipeline.doc().get_pages().values().next().copied();
                            self.page_boxes = page_id
                                .and_then(|id| PageBoxes::read(self.pipeline.doc(), id).ok());
                            self.object_tree = page_id
                                .and_then(|id| build_object_tree(self.pipeline.doc(), id).ok());
                            self.glyph_outlines = page_id
                                .and_then(|id| outline_page_text(self.pipeline.doc(), id).ok());
                            self.plate_spot_names = self
                                .object_tree
                                .as_ref()
                                .map(extract_spot_names)
                                .unwrap_or_default();

                            // Clear stale display state.
                            self.selected_object = None;
                            self.color_info = None;
                            self.sampling_pdf_pos = None;
                            self.pending_image = None;
                            self.current_image = None;
                            self.tile_sender = None;
                            self.tile_cache.clear();
                            if let Some(state) = self.state.as_mut() {
                                state.page_image = None;
                                state.plate_image = None;
                                state.tile_images.clear();
                            }
                            self.plate_image_for = PlateMode::All;

                            // Update window title and kick off render.
                            let title = format!(
                                "rbv \u{2014} {}",
                                self.file.file_name().unwrap_or_default().to_string_lossy()
                            );
                            if let Some(state) = self.state.as_ref() {
                                state.window.set_title(&title);
                                state.window.request_redraw();
                            }
                            self.digit_buf.clear();
                            self.push_log(format!("IPC OPEN: {}", self.file.display()));
                            self.spawn_render(0);
                        }
                    }
                }
                IpcCmd::Page(p) => {
                    self.navigate_to_page(p);
                }
                IpcCmd::Quit => {
                    event_loop.exit();
                }
            },
            _ => {}
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(file: PathBuf, page: u32, config: RenderConfig, listen: bool) {
    let pipeline = Arc::new(PdfPipeline::open(&file).expect("open PDF"));

    let page_count = pipeline.doc().get_pages().len() as u32;
    // Clamp page to valid range in case the caller passed an out-of-bounds index.
    let page = page.min(page_count.saturating_sub(1));

    // Compute the page object ID once; reuse for both page_boxes and object_tree.
    let page_id = pipeline
        .doc()
        .get_pages()
        .values()
        .nth(page as usize)
        .copied();

    let page_boxes = page_id.and_then(|id| PageBoxes::read(pipeline.doc(), id).ok());
    let object_tree = page_id.and_then(|id| build_object_tree(pipeline.doc(), id).ok());
    let glyph_outlines = page_id.and_then(|id| outline_page_text(pipeline.doc(), id).ok());
    let plate_spot_names = object_tree
        .as_ref()
        .map(extract_spot_names)
        .unwrap_or_default();

    let event_loop = EventLoop::<ViewerEvent>::with_user_event()
        .build()
        .expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();

    {
        let pipeline = pipeline.clone();
        let proxy = proxy.clone();
        let preview = RenderConfig {
            dpi: 72,
            ..config.clone()
        };
        let full = config.clone();
        std::thread::spawn(move || {
            if let Ok(img) = pipeline.render_page(page, &preview) {
                let _ = proxy.send_event(ViewerEvent::PreviewReady { page, image: img });
            }
            if let Ok(img) = pipeline.render_page(page, &full) {
                let _ = proxy.send_event(ViewerEvent::PageReady { page, image: img });
            }
        });
    }

    let proxy_watch = proxy.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if matches!(
            res.map(|e| e.kind.is_modify() || e.kind.is_create()),
            Ok(true)
        ) {
            let _ = proxy_watch.send_event(ViewerEvent::FileChanged);
        }
    })
    .expect("watcher");
    watcher
        .watch(&file, RecursiveMode::NonRecursive)
        .expect("watch file");

    // ── IPC stdin reader (only when launched with --listen) ───────────────────
    if listen {
        let proxy_ipc = proxy.clone();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                let line = line.trim().to_owned();
                let cmd = if let Some(path) = line.strip_prefix("OPEN ") {
                    IpcCmd::Open(PathBuf::from(path))
                } else if let Some(n) = line.strip_prefix("PAGE ") {
                    if let Ok(p) = n.trim().parse::<u32>() {
                        IpcCmd::Page(p)
                    } else {
                        continue;
                    }
                } else if line == "QUIT" {
                    let _ = proxy_ipc.send_event(ViewerEvent::IpcCommand(IpcCmd::Quit));
                    break;
                } else {
                    continue; // unknown command — skip silently
                };
                if proxy_ipc.send_event(ViewerEvent::IpcCommand(cmd)).is_err() {
                    break; // event loop closed — stop reading
                }
            }
        });
    }

    let mut viewer = Viewer {
        file,
        pipeline,
        page,
        page_count,
        config,
        state: None,
        pending_image: None,
        current_image: None,
        page_boxes,
        object_tree,
        glyph_outlines,
        selected_object: None,
        color_info: None,
        sampling_pdf_pos: None,
        show_overlays: false,
        show_wireframe: false,
        zoom: 1.0,
        pan: [0.0, 0.0],
        ctrl_held: false,
        shift_held: false,
        cursor_pos: [0.0, 0.0],
        drag_origin: None,
        debug_mode: false,
        debug_log: VecDeque::with_capacity(DEBUG_LOG_CAP),
        icc_transform: None,
        watcher,
        proxy,
        digit_buf: String::new(),
        egui_ctx: egui::Context::default(),
        active_plate: PlateMode::All,
        plate_spot_names,
        show_tools_panel: false,
        plate_tinted: false,
        plate_image_for: PlateMode::All,
        egui_pointer_over_panel: false,
        tile_cache: crate::tiles::TileCache::new(),
        tile_sender: None,
        telemetry_mode: false,
        tile_perf: std::collections::HashMap::new(),
    };

    event_loop.run_app(&mut viewer).expect("run app");
}

// ── egui UI ───────────────────────────────────────────────────────────────────

/// Build the egui side panel for every frame.
///
/// Receives only the specific `Viewer` fields it needs, avoiding a `&mut Viewer`
/// borrow that would conflict with the `egui_ctx.run(...)` call in the caller.
// Panel::show(ctx, …) is the correct top-level call; show_inside() requires &mut Ui,
// so the deprecation message is inapplicable here. Suppress until egui stabilises the API.
#[allow(deprecated)]
fn build_egui_ui(
    ctx: &egui::Context,
    active_plate: &mut PlateMode,
    plate_tinted: &mut bool,
    spot_names: &[String],
    selected: Option<&PageObject>,
    color_info: Option<&ColorPanel>,
    show_panel: &mut bool,
) -> bool {
    // Capture current pointer position in egui's logical-pixel coordinate space.
    // Used to test whether the pointer is over any egui surface this frame so
    // the caller can gate PDF pan/selection next frame.
    let ptr = ctx.pointer_hover_pos();

    // ── Floating toggle button — always visible, does not affect PDF layout ───
    let btn_resp = egui::Area::new(egui::Id::new("tools_toggle"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
        .show(ctx, |ui| {
            // Use egui's own ▶/◀ geometric shapes (same block as CollapsingHeader arrows —
            // guaranteed to render in egui's bundled NotoSans font).
            let label = if *show_panel { "▶" } else { "◀" }; // Not a bug, actually works this way.
            if ui
                .button(egui::RichText::new(label).size(14.0))
                .on_hover_text(if *show_panel {
                    "Close Prepress Tools"
                } else {
                    "Open Prepress Tools"
                })
                .clicked()
            {
                *show_panel = !*show_panel;
            }
        });
    let over_btn = ptr.map_or(false, |p| btn_resp.response.rect.contains(p));

    // ── Side panel — only when the toggle is active ───────────────────────────
    if !*show_panel {
        return over_btn;
    }

    let panel_resp = egui::Panel::right("prepress_tools")
        .default_size(220.0)
        .min_size(180.0)
        .show(ctx, |ui| {
            // Wrap all panel content in a vertical scroll area so that
            // expanding sections (e.g. Keyboard Shortcuts) never push
            // content below the visible window.
            egui::ScrollArea::vertical()
                .id_salt("panel_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // ── Plate view ────────────────────────────────────────────────────
                    ui.heading("Plate View");
                    ui.separator();

                    ui.radio_value(active_plate, PlateMode::All, "All");

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Process")
                            .color(egui::Color32::GRAY)
                            .size(11.0),
                    );
                    for (label, ch) in [
                        ("Cyan", CmykChannel::Cyan),
                        ("Magenta", CmykChannel::Magenta),
                        ("Yellow", CmykChannel::Yellow),
                        ("Black", CmykChannel::Black),
                    ] {
                        ui.radio_value(active_plate, PlateMode::Cmyk(ch), label);
                    }

                    if !spot_names.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Spot Colors")
                                .color(egui::Color32::GRAY)
                                .size(11.0),
                        );
                        // No inner scroll area — the outer panel scroll handles overflow.
                        for name in spot_names {
                            ui.radio_value(
                                active_plate,
                                PlateMode::Spot(name.clone()),
                                name.as_str(),
                            );
                        }
                    }

                    // ── Plate display options ─────────────────────────────────────────
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Display")
                            .color(egui::Color32::GRAY)
                            .size(11.0),
                    );
                    ui.checkbox(plate_tinted, "Ink-tinted view");
                    if *plate_tinted && *active_plate != PlateMode::All {
                        ui.label(
                            egui::RichText::new("Preview uses ink approximation colors")
                                .color(egui::Color32::from_rgb(180, 180, 100))
                                .italics()
                                .size(10.0),
                        );
                    }

                    // ── Keyboard shortcuts reference ──────────────────────────────────
                    ui.separator();
                    egui::CollapsingHeader::new(
                        egui::RichText::new("Keyboard Shortcuts").size(12.0),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        egui::Grid::new("shortcuts_grid")
                            .num_columns(2)
                            .spacing([12.0, 3.0])
                            .striped(true)
                            .show(ui, |ui| {
                                let key = |ui: &mut egui::Ui, k: &str| {
                                    ui.label(
                                        egui::RichText::new(k)
                                            .monospace()
                                            .color(egui::Color32::from_rgb(220, 180, 80)),
                                    );
                                };
                                let desc = |ui: &mut egui::Ui, d: &str| {
                                    ui.label(egui::RichText::new(d).size(11.0));
                                    ui.end_row();
                                };

                                key(ui, "W");
                                desc(ui, "Toggle wireframe overlay");
                                key(ui, "O");
                                desc(ui, "Toggle trim/bleed box overlay");
                                key(ui, "Esc");
                                desc(ui, "Quit");
                                key(ui, "← / H / K / ↑");
                                desc(ui, "Previous page");
                                key(ui, "→ / L / J / ↓");
                                desc(ui, "Next page");
                                key(ui, "G");
                                desc(ui, "First page");
                                key(ui, "Shift+G");
                                desc(ui, "Last page");
                                key(ui, "<N>G");
                                desc(ui, "Jump to page N");
                                key(ui, "Ctrl+Scroll");
                                desc(ui, "Zoom in/out");
                                key(ui, "Ctrl + =");
                                desc(ui, "Zoom in");
                                key(ui, "Ctrl + -");
                                desc(ui, "Zoom out");
                                key(ui, "Ctrl + 0");
                                desc(ui, "Reset zoom");
                                key(ui, "Ctrl+Shift+D");
                                desc(ui, "Toggle debug overlay");
                                key(ui, "Ctrl+Shift+E");
                                desc(ui, "Export wireframe PDF");
                                key(ui, "Click");
                                desc(ui, "Inspect object");
                                key(ui, "Drag");
                                desc(ui, "Pan");
                            });
                    });

                    // ── Selection ─────────────────────────────────────────────────────
                    ui.separator();
                    ui.heading("Selection");

                    let Some(obj) = selected else {
                        ui.label(
                            egui::RichText::new("Click an object to inspect it")
                                .color(egui::Color32::GRAY)
                                .italics(),
                        );
                        return;
                    };

                    // Build a human-readable kind label.
                    // ObjectKind::Text stores raw PDF-decoded bytes as a Rust String;
                    // control characters (unit/group separators, etc.) are common PDF
                    // encoding artifacts and must be stripped before display so the UI
                    // doesn't show "\u{1f}\u{1d}" style escapes.
                    let kind_label = match &obj.kind {
                        ObjectKind::Fill => "Fill".to_string(),
                        ObjectKind::Stroke => "Stroke".to_string(),
                        ObjectKind::FillStroke => "FillStroke".to_string(),
                        ObjectKind::Image => "Image".to_string(),
                        ObjectKind::FormXObject => "FormXObject".to_string(),
                        ObjectKind::Text(s) => {
                            // Replace control characters with spaces; collapse runs; trim.
                            let clean: String = s
                                .chars()
                                .map(|c| if c.is_control() { ' ' } else { c })
                                .collect::<String>()
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");
                            if clean.is_empty() {
                                "Text".to_string()
                            } else {
                                format!("Text: \"{clean}\"")
                            }
                        }
                    };
                    ui.label(format!("Kind: {kind_label}"));

                    // Overprint state.
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Overprint").size(12.0).strong());
                    egui::Grid::new("overprint_grid")
                        .num_columns(2)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            ui.label("Fill:");
                            ui.label(format!("{}", obj.overprint.fill_overprint));
                            ui.end_row();
                            ui.label("Stroke:");
                            ui.label(format!("{}", obj.overprint.stroke_overprint));
                            ui.end_row();
                            ui.label("Mode:");
                            ui.label(format!("{}", obj.overprint.overprint_mode));
                            ui.end_row();
                        });

                    // PDF color values.
                    if let Some(fill) = &obj.fill_color {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Fill Color").size(12.0).strong());
                        show_pdf_color(ui, fill);
                    }
                    if let Some(stroke) = &obj.stroke_color {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Stroke Color").size(12.0).strong());
                        show_pdf_color(ui, stroke);
                    }

                    // ICC-sampled pixel readout.
                    if let Some(panel) = color_info {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Sampled Pixel").size(12.0).strong());
                        let [r, g, b, a] = panel.pixel_rgba;
                        ui.horizontal(|ui| {
                            // Use a painted rect — "██" (U+2588) is absent from egui's
                            // bundled font and shows as a missing-glyph box.
                            let swatch_color = egui::Color32::from_rgba_premultiplied(r, g, b, a);
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, swatch_color);
                            ui.label(format!("#{r:02X}{g:02X}{b:02X}"));
                        });
                        if let Some([c, m, y, k]) = panel.pixel_cmyk {
                            ui.label(format!(
                                "CMYK  {:.0}%  {:.0}%  {:.0}%  {:.0}%",
                                c * 100.0,
                                m * 100.0,
                                y * 100.0,
                                k * 100.0
                            ));
                        }
                    }
                    // Close the outer ScrollArea
                }); // end ScrollArea
        });

    // Return true if the pointer was inside the panel this frame.
    let over_panel = ptr.map_or(false, |p| panel_resp.response.rect.contains(p));
    over_btn || over_panel
}

fn show_pdf_color(ui: &mut egui::Ui, color: &PdfColor) {
    match color {
        PdfColor::DeviceGray(v) => {
            ui.label(format!("Gray {v:.3}"));
        }
        PdfColor::DeviceRgb(r, g, b) => {
            ui.horizontal(|ui| {
                let swatch_color = egui::Color32::from_rgb(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                );
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, swatch_color);
                ui.label(format!("RGB  {r:.3}  {g:.3}  {b:.3}"));
            });
        }
        PdfColor::DeviceCmyk(c, m, y, k) => {
            ui.label(format!(
                "CMYK  {:.0}%  {:.0}%  {:.0}%  {:.0}%",
                c * 100.0,
                m * 100.0,
                y * 100.0,
                k * 100.0
            ));
        }
        PdfColor::Separation { name, tint } => {
            ui.label(format!("Spot \"{name}\"  @{tint:.3}"));
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    // ── Navigation helpers (mirrors Viewer::navigate_to_page / take_digit_step) ─

    struct NavState {
        page: u32,
        page_count: u32,
        digit_buf: String,
    }

    impl NavState {
        fn new(page: u32, page_count: u32) -> Self {
            Self {
                page,
                page_count,
                digit_buf: String::new(),
            }
        }
        fn navigate(&mut self, target: u32) {
            let clamped = target.min(self.page_count.saturating_sub(1));
            self.page = clamped;
        }
        fn take_digit_step(&mut self) -> u32 {
            let n = self.digit_buf.parse::<u32>().unwrap_or(0);
            self.digit_buf.clear();
            n
        }
        fn push_digit(&mut self, d: char) {
            self.digit_buf.push(d);
        }
    }

    #[test]
    fn navigate_next_page() {
        let mut s = NavState::new(0, 5);
        s.navigate(1);
        assert_eq!(s.page, 1);
    }

    #[test]
    fn navigate_clamps_to_last_page() {
        let mut s = NavState::new(4, 5);
        s.navigate(99);
        assert_eq!(s.page, 4, "should clamp to page_count - 1");
    }

    #[test]
    fn navigate_clamps_at_zero() {
        let mut s = NavState::new(0, 5);
        // simulate ArrowLeft with saturating_sub
        let target = s.page.saturating_sub(1);
        s.navigate(target);
        assert_eq!(s.page, 0, "should stay at 0");
    }

    #[test]
    fn navigate_single_page_document() {
        let mut s = NavState::new(0, 1);
        s.navigate(0);
        assert_eq!(s.page, 0);
        // Any forward request still clamps to 0
        let target = s.page.saturating_add(1);
        s.navigate(target);
        assert_eq!(s.page, 0);
    }

    #[test]
    fn digit_buf_step_parsed_correctly() {
        let mut s = NavState::new(0, 100);
        s.push_digit('5');
        s.push_digit('3');
        let step = s.take_digit_step().max(1);
        assert_eq!(step, 53);
        assert!(s.digit_buf.is_empty(), "buf should be cleared after take");
    }

    #[test]
    fn digit_buf_empty_returns_zero() {
        let mut s = NavState::new(0, 10);
        assert_eq!(s.take_digit_step(), 0);
    }

    #[test]
    fn numbered_page_jump_is_one_indexed() {
        // User types "3g" → page index 2 (0-indexed)
        let mut s = NavState::new(0, 10);
        s.push_digit('3');
        let n = s.digit_buf.parse::<u32>().unwrap_or(1);
        s.digit_buf.clear();
        s.navigate(n.saturating_sub(1));
        assert_eq!(s.page, 2);
    }

    #[test]
    fn g_without_prefix_goes_to_first_page() {
        let mut s = NavState::new(4, 10);
        // bare `g` → first page
        s.navigate(0);
        assert_eq!(s.page, 0);
    }

    #[test]
    fn shift_g_goes_to_last_page() {
        let mut s = NavState::new(0, 10);
        s.navigate(s.page_count.saturating_sub(1));
        assert_eq!(s.page, 9);
    }

    #[test]
    fn multi_step_forward_with_digit_prefix() {
        // `3l` → move 3 pages forward from page 2 → page 5
        let mut s = NavState::new(2, 10);
        s.push_digit('3');
        let step = s.take_digit_step().max(1);
        s.navigate(s.page.saturating_add(step));
        assert_eq!(s.page, 5);
    }

    #[test]
    fn multi_step_backward_clamps_at_zero() {
        // `9h` from page 2 → saturates at 0
        let mut s = NavState::new(2, 10);
        s.push_digit('9');
        let step = s.take_digit_step().max(1);
        s.navigate(s.page.saturating_sub(step));
        assert_eq!(s.page, 0);
    }

    #[test]
    fn escape_clears_digit_buf() {
        let mut s = NavState::new(0, 10);
        s.push_digit('7');
        assert!(!s.digit_buf.is_empty());
        // simulate Escape cancelling the prefix
        s.digit_buf.clear();
        assert!(s.digit_buf.is_empty());
        // page unchanged
        assert_eq!(s.page, 0);
    }

    // ── ZoomState (mirrors Viewer::apply_zoom) ────────────────────────────────

    struct ZoomState {
        zoom: f32,
        pan: [f32; 2],
    }

    impl ZoomState {
        fn apply_zoom(&mut self, factor: f32, focal: Option<[f32; 2]>, win_w: f32, win_h: f32) {
            let old_zoom = self.zoom;
            self.zoom = (self.zoom * factor).clamp(0.05, 50.0);
            let r = self.zoom / old_zoom;
            let (cx, cy) = match focal {
                Some([x, y]) => (x, y),
                None => (win_w / 2.0, win_h / 2.0),
            };
            self.pan[0] = self.pan[0] * r + (cx - win_w / 2.0) * (1.0 - r);
            self.pan[1] = self.pan[1] * r + (cy - win_h / 2.0) * (1.0 - r);
        }
    }

    #[test]
    fn zoom_center_pan_unchanged() {
        let mut s = ZoomState {
            zoom: 1.0,
            pan: [0.0, 0.0],
        };
        s.apply_zoom(2.0, None, 800.0, 600.0);
        assert_eq!(s.zoom, 2.0);
        assert!(s.pan[0].abs() < 1e-4, "pan x={}", s.pan[0]);
        assert!(s.pan[1].abs() < 1e-4, "pan y={}", s.pan[1]);
    }

    #[test]
    fn zoom_corner_focal() {
        let mut s = ZoomState {
            zoom: 1.0,
            pan: [0.0, 0.0],
        };
        s.apply_zoom(2.0, Some([0.0, 0.0]), 800.0, 600.0);
        assert_eq!(s.zoom, 2.0);
        assert!((s.pan[0] - 400.0).abs() < 1e-3, "pan x={}", s.pan[0]);
        assert!((s.pan[1] - 300.0).abs() < 1e-3, "pan y={}", s.pan[1]);
    }

    #[test]
    fn zoom_in_out_roundtrip() {
        let mut s = ZoomState {
            zoom: 1.0,
            pan: [0.0, 0.0],
        };
        s.apply_zoom(1.1, None, 800.0, 600.0);
        s.apply_zoom(1.0 / 1.1, None, 800.0, 600.0);
        assert!((s.zoom - 1.0).abs() < 1e-5, "zoom={}", s.zoom);
        assert!(s.pan[0].abs() < 1e-4);
        assert!(s.pan[1].abs() < 1e-4);
    }

    #[test]
    fn zoom_clamps_minimum() {
        let mut s = ZoomState {
            zoom: 0.06,
            pan: [0.0, 0.0],
        };
        s.apply_zoom(0.1, None, 800.0, 600.0);
        assert_eq!(s.zoom, 0.05);
    }

    #[test]
    fn zoom_clamps_maximum() {
        let mut s = ZoomState {
            zoom: 49.0,
            pan: [0.0, 0.0],
        };
        s.apply_zoom(10.0, None, 800.0, 600.0);
        assert_eq!(s.zoom, 50.0);
    }

    #[test]
    fn zoom_factor_one_noop() {
        let mut s = ZoomState {
            zoom: 2.0,
            pan: [50.0, -30.0],
        };
        s.apply_zoom(1.0, Some([100.0, 200.0]), 800.0, 600.0);
        assert!((s.zoom - 2.0).abs() < 1e-5);
        assert!((s.pan[0] - 50.0).abs() < 1e-4);
        assert!((s.pan[1] - -30.0).abs() < 1e-4);
    }

    #[test]
    fn pan_drag_delta() {
        let pan_start = [10.0_f32, 20.0_f32];
        let cursor_start = [100.0_f32, 150.0_f32];
        let cursor_now = [130.0_f32, 160.0_f32];
        let new_pan = [
            pan_start[0] + cursor_now[0] - cursor_start[0],
            pan_start[1] + cursor_now[1] - cursor_start[1],
        ];
        assert_eq!(new_pan, [40.0, 30.0]);
    }

    // ── screen_to_pdf coordinate math ────────────────────────────────────────

    /// Mirrors `Viewer::screen_to_pdf` as a free function so the math can be
    /// tested without a live window or GL context.
    fn screen_to_pdf_math(
        screen: [f32; 2],
        page_left: f32,
        page_top: f32,
        page_w: f32,
        page_h: f32,
        media_x: f64,
        media_top: f64,
        media_w: f64,
        media_h: f64,
    ) -> (f64, f64) {
        let rel_x = (screen[0] - page_left) / page_w;
        let rel_y = (screen[1] - page_top) / page_h;
        let pdf_x = media_x + rel_x as f64 * media_w;
        let pdf_y = media_top - rel_y as f64 * media_h;
        (pdf_x, pdf_y)
    }

    // Screen top-left (0, 0) → PDF top-left (0, page_height) at 1:1 scale.
    #[test]
    fn screen_to_pdf_top_left_corner() {
        let (px, py) =
            screen_to_pdf_math([0.0, 0.0], 0.0, 0.0, 612.0, 792.0, 0.0, 792.0, 612.0, 792.0);
        assert!((px - 0.0).abs() < 0.1, "pdf_x={px}");
        assert!((py - 792.0).abs() < 0.1, "pdf_y={py}");
    }

    // Screen center → PDF center at 1:1 scale.
    #[test]
    fn screen_to_pdf_center() {
        let (px, py) = screen_to_pdf_math(
            [306.0, 396.0],
            0.0,
            0.0,
            612.0,
            792.0,
            0.0,
            792.0,
            612.0,
            792.0,
        );
        assert!((px - 306.0).abs() < 0.1, "pdf_x={px}");
        assert!((py - 396.0).abs() < 0.1, "pdf_y={py}");
    }

    // Screen bottom-left → PDF origin (0, 0).
    #[test]
    fn screen_to_pdf_bottom_left() {
        let (px, py) = screen_to_pdf_math(
            [0.0, 792.0],
            0.0,
            0.0,
            612.0,
            792.0,
            0.0,
            792.0,
            612.0,
            792.0,
        );
        assert!((px - 0.0).abs() < 0.1, "pdf_x={px}");
        assert!((py - 0.0).abs() < 0.1, "pdf_y={py}");
    }

    // Panned/scaled page rect: center of visible page → PDF center.
    #[test]
    fn screen_to_pdf_with_pan_and_scale() {
        // Page rect at (50, 100), size 306×396 (half of 612×792)
        // Center of page rect on screen = (50+153, 100+198) = (203, 298)
        let (px, py) = screen_to_pdf_math(
            [203.0, 298.0],
            50.0,
            100.0,
            306.0,
            396.0,
            0.0,
            792.0,
            612.0,
            792.0,
        );
        assert!((px - 306.0).abs() < 0.5, "pdf_x={px}");
        assert!((py - 396.0).abs() < 0.5, "pdf_y={py}");
    }

    // ── click vs drag threshold ───────────────────────────────────────────────

    // Mirrors the 4-pixel (16 squared) threshold in MouseInput Released.
    #[test]
    fn small_displacement_is_a_click() {
        let press = [100.0_f32, 100.0_f32];
        let release = [102.0_f32, 101.5_f32]; // |d| ≈ 2.5 px
        let dx = release[0] - press[0];
        let dy = release[1] - press[1];
        assert!(dx * dx + dy * dy < 16.0, "should classify as click");
    }

    #[test]
    fn large_displacement_is_a_drag() {
        let press = [100.0_f32, 100.0_f32];
        let release = [106.0_f32, 100.0_f32]; // 6 px
        let dx = release[0] - press[0];
        let dy = release[1] - press[1];
        assert!(dx * dx + dy * dy >= 16.0, "should classify as drag");
    }

    #[test]
    fn exactly_four_pixels_is_a_drag() {
        let press = [0.0_f32, 0.0_f32];
        let release = [4.0_f32, 0.0_f32]; // exactly 4 px
        let dx = release[0] - press[0];
        let dy = release[1] - press[1];
        // Threshold is strictly less than 16 (4² = 16 → drag, not click)
        assert!(dx * dx + dy * dy >= 16.0, "4 px should be a drag");
    }

    // ── debug log ring buffer ─────────────────────────────────────────────────

    // Mirrors `Viewer::push_log` / `DEBUG_LOG_CAP` logic.
    #[test]
    fn debug_log_caps_at_max() {
        let cap = super::DEBUG_LOG_CAP;
        let mut log: VecDeque<String> = VecDeque::with_capacity(cap);
        for i in 0..cap + 6 {
            if log.len() >= cap {
                log.pop_front();
            }
            log.push_back(format!("entry {i}"));
        }
        assert_eq!(log.len(), cap, "log must not exceed capacity");
    }

    #[test]
    fn debug_log_evicts_oldest_first() {
        let cap = 4usize;
        let mut log: VecDeque<String> = VecDeque::new();
        for i in 0..6usize {
            if log.len() >= cap {
                log.pop_front();
            }
            log.push_back(format!("entry {i}"));
        }
        // entries 0-1 should have been evicted
        assert_eq!(log.front().unwrap(), "entry 2");
        assert_eq!(log.back().unwrap(), "entry 5");
    }

    #[test]
    fn debug_log_newest_at_back() {
        let cap = 24usize;
        let mut log: VecDeque<String> = VecDeque::new();
        for i in 0..3usize {
            if log.len() >= cap {
                log.pop_front();
            }
            log.push_back(format!("msg {i}"));
        }
        // The overlay iterates rev() so newest (back) shows first.
        assert_eq!(log.back().unwrap(), "msg 2");
    }
}
