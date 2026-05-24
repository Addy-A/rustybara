use crate::export::export_wireframe;
use crate::renderer::{ColorPanel, DebugOverlay, OverlayData, PageWireframe, SkiaRenderer, image_to_skia};
use image::{DynamicImage, GenericImageView};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rustybara::objects::{ObjectKind, ObjectTree, PageObject, PdfColor, build_object_tree, hit_test};
use rustybara::pages::PageBoxes;
use rustybara::raster::RenderConfig;
use rustybara::PdfPipeline;
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

pub enum ViewerEvent {
    PreviewReady { page: u32, image: DynamicImage },
    PageReady { page: u32, image: DynamicImage },
    FileChanged,
}

struct SkiaState {
    window: Arc<Window>,
    renderer: SkiaRenderer,
    page_image: Option<skia_safe::Image>,
    width: u32,
    height: u32,
}

struct Viewer {
    file: PathBuf,
    pipeline: Arc<PdfPipeline>,
    page: u32,
    config: RenderConfig,
    state: Option<SkiaState>,
    pending_image: Option<DynamicImage>,
    /// Retained rasterized page image for pixel sampling on click.
    current_image: Option<DynamicImage>,
    page_boxes: Option<PageBoxes>,
    /// Cached object tree for the current page, used for hit-testing.
    object_tree: Option<ObjectTree>,
    /// The topmost object under the last selection click (owned clone).
    selected_object: Option<PageObject>,
    /// Color diagnostic data captured at the last selection click.
    color_info: Option<ColorPanel>,
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
    _watcher: RecommendedWatcher,
    proxy: EventLoopProxy<ViewerEvent>,
}

impl Viewer {
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
        let pdf_color = self
            .selected_object
            .as_ref()
            .and_then(|obj| obj.fill_color.or(obj.stroke_color));

        self.color_info = Some(ColorPanel {
            pixel_rgba,
            pdf_color,
        });

        // Debug log.
        let kind_str = self.selected_object.as_ref().map_or("none", |obj| {
            match &obj.kind {
                ObjectKind::Fill => "Fill",
                ObjectKind::Stroke => "Stroke",
                ObjectKind::FillStroke => "FillStroke",
                ObjectKind::Text(_) => "Text",
                ObjectKind::Image => "Image",
                ObjectKind::FormXObject => "FormXObject",
            }
        });
        self.push_log(format!(
            "Click ({pdf_x:.1}, {pdf_y:.1}) → {kind_str}"
        ));
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
        let (Some(tree), Some(boxes)) =
            (self.object_tree.as_ref(), self.page_boxes.as_ref())
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
            let dir = self
                .file
                .parent()
                .unwrap_or(std::path::Path::new("."));
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

    // ── Debug overlay lines ───────────────────────────────────────────────────

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
        lines.push(format!("Page  #{}   Objects: {}", self.page, obj_count));

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
            let color_str = match obj.fill_color.or(obj.stroke_color) {
                Some(PdfColor::DeviceGray(v)) => format!("Gray({v:.3})"),
                Some(PdfColor::DeviceRgb(r, g, b)) => {
                    format!("RGB({r:.2} {g:.2} {b:.2})")
                }
                Some(PdfColor::DeviceCmyk(c, m, y, k)) => {
                    format!("CMYK({c:.2} {m:.2} {y:.2} {k:.2})")
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

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("rbv"))
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

        let page_image = self.pending_image.as_ref().map(image_to_skia);
        if page_image.is_some() {
            window.request_redraw();
        }
        self.state = Some(SkiaState {
            window,
            renderer,
            page_image,
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

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                // Build debug lines first — this borrows `self` immutably (reads self.state),
                // so it must complete before we take the mutable `state` borrow below.
                let debug_lines: Option<Vec<String>> = if self.debug_mode {
                    Some(self.build_debug_lines())
                } else {
                    None
                };
                let debug_overlay = debug_lines
                    .as_deref()
                    .map(|lines| DebugOverlay { lines });

                let state = self.state.as_mut().unwrap();
                let overlays = if self.show_overlays {
                    self.page_boxes.as_ref().map(|b| OverlayData { boxes: b })
                } else {
                    None
                };

                // Wireframe: Acrobat-style full-page mode (W key).
                // Passes the full object tree + optional selected object for highlighting.
                // Borrows from self.object_tree / self.page_boxes / self.selected_object;
                // these are different fields from self.state, so partial-field borrow is fine.
                let wireframe = if self.show_wireframe {
                    self.object_tree
                        .as_ref()
                        .zip(self.page_boxes.as_ref())
                        .map(|(tree, boxes)| PageWireframe {
                            objects: &tree.objects,
                            media_box: &boxes.media_box,
                            selected: self.selected_object.as_ref(),
                        })
                } else {
                    None
                };

                state.renderer.draw(
                    state.page_image.as_ref(),
                    self.zoom,
                    self.pan,
                    overlays.as_ref(),
                    wireframe.as_ref(),
                    self.color_info.as_ref(),
                    debug_overlay.as_ref(),
                );
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
                    KeyCode::KeyE if self.ctrl_held && self.shift_held => {
                        // Ctrl+Shift+E — export wireframe as a diagnostic PDF.
                        self.export_wireframe_pdf();
                    }
                    KeyCode::Escape => {
                        std::process::exit(0);
                    }
                    _ => {}
                }
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
                self.state.as_ref().unwrap().window.request_redraw();
            }

            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = [position.x as f32, position.y as f32];
                if let Some((cursor_start, pan_start)) = self.drag_origin {
                    self.pan[0] = pan_start[0] + new_pos[0] - cursor_start[0];
                    self.pan[1] = pan_start[1] + new_pos[1] - cursor_start[1];
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
            } => match btn_state {
                ElementState::Pressed => {
                    self.drag_origin = Some((self.cursor_pos, self.pan));
                }
                ElementState::Released => {
                    // Disambiguate pan drag vs click using a 4-pixel distance threshold.
                    // If the cursor moved less than 4px from press to release it's a click;
                    // larger displacement means the user was panning.
                    if let Some((cursor_start, _)) = self.drag_origin {
                        let dx = self.cursor_pos[0] - cursor_start[0];
                        let dy = self.cursor_pos[1] - cursor_start[1];
                        if dx * dx + dy * dy < 16.0 {
                            self.handle_selection_click();
                            self.state.as_ref().unwrap().window.request_redraw();
                        }
                    }
                    self.drag_origin = None;
                }
            },
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ViewerEvent) {
        match event {
            ViewerEvent::PreviewReady { page, image } if page == self.page => {
                let skia_img = image_to_skia(&image);
                if let Some(state) = self.state.as_mut() {
                    state.page_image = Some(skia_img);
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
                    state.window.request_redraw();
                    self.current_image = Some(image);
                } else {
                    self.pending_image = Some(image);
                }
                let obj_count = self.object_tree.as_ref().map_or(0, |t| t.objects.len());
                self.push_log(format!("Page ready: {obj_count} objects, page {page}"));
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

                    self.pipeline = Arc::new(new_pipeline);
                }
                // Page content changed — clear stale selection data.
                self.selected_object = None;
                self.color_info = None;
                self.push_log("File reloaded — selection cleared");
                self.spawn_render(self.page);
            }
            _ => {}
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(file: PathBuf, page: u32, config: RenderConfig) {
    let pipeline = Arc::new(PdfPipeline::open(&file).expect("open PDF"));

    // Compute the page object ID once; reuse for both page_boxes and object_tree.
    let page_id = pipeline
        .doc()
        .get_pages()
        .values()
        .nth(page as usize)
        .copied();

    let page_boxes = page_id.and_then(|id| PageBoxes::read(pipeline.doc(), id).ok());
    let object_tree = page_id.and_then(|id| build_object_tree(pipeline.doc(), id).ok());

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

    let mut viewer = Viewer {
        file,
        pipeline,
        page,
        config,
        state: None,
        pending_image: None,
        current_image: None,
        page_boxes,
        object_tree,
        selected_object: None,
        color_info: None,
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
        _watcher: watcher,
        proxy,
    };

    event_loop.run_app(&mut viewer).expect("run app");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

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
        let (px, py) = screen_to_pdf_math(
            [0.0, 0.0],
            0.0, 0.0, 612.0, 792.0,
            0.0, 792.0, 612.0, 792.0,
        );
        assert!((px - 0.0).abs() < 0.1, "pdf_x={px}");
        assert!((py - 792.0).abs() < 0.1, "pdf_y={py}");
    }

    // Screen center → PDF center at 1:1 scale.
    #[test]
    fn screen_to_pdf_center() {
        let (px, py) = screen_to_pdf_math(
            [306.0, 396.0],
            0.0, 0.0, 612.0, 792.0,
            0.0, 792.0, 612.0, 792.0,
        );
        assert!((px - 306.0).abs() < 0.1, "pdf_x={px}");
        assert!((py - 396.0).abs() < 0.1, "pdf_y={py}");
    }

    // Screen bottom-left → PDF origin (0, 0).
    #[test]
    fn screen_to_pdf_bottom_left() {
        let (px, py) = screen_to_pdf_math(
            [0.0, 792.0],
            0.0, 0.0, 612.0, 792.0,
            0.0, 792.0, 612.0, 792.0,
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
            50.0, 100.0, 306.0, 396.0,
            0.0, 792.0, 612.0, 792.0,
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

    // Overlay toggle is bool negation — verified by inspection, no state machine test needed.
}