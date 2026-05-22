use crate::renderer::{image_to_skia, SkiaRenderer};
use image::DynamicImage;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rustybara::raster::RenderConfig;
use rustybara::PdfPipeline;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

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
    zoom: f32,
    pan: [f32; 2],
    ctrl_held: bool,
    cursor_pos: [f32; 2],
    drag_origin: Option<([f32; 2], [f32; 2])>,
    _watcher: RecommendedWatcher,
    proxy: EventLoopProxy<ViewerEvent>,
}

impl Viewer {
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
}

impl ApplicationHandler<ViewerEvent> for Viewer {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("rbv"))
                .expect("create window"),
        );
        let size = window.inner_size();
        let renderer = SkiaRenderer::new(window.clone());
        let page_image = self.pending_image.as_ref().map(image_to_skia);
        if page_image.is_some() {
            window.request_redraw();
        }
        self.state = Some(SkiaState {
            window,
            renderer,
            page_image,
            width: size.width.max(1),
            height: size.height.max(1),
        });
        self.pending_image = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.state.is_none() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                let state = self.state.as_mut().unwrap();
                state
                    .renderer
                    .draw(state.page_image.as_ref(), self.zoom, self.pan);
                state.renderer.present();
            }

            WindowEvent::Resized(size) => {
                let state = self.state.as_mut().unwrap();
                state.width = size.width.max(1);
                state.height = size.height.max(1);
                state.renderer.resize(state.width, state.height);
                state.window.request_redraw();
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.ctrl_held = mods.state().contains(ModifiersState::CONTROL);
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
                    self.drag_origin = None;
                }
            },
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ViewerEvent) {
        match event {
            ViewerEvent::PreviewReady { page, image } | ViewerEvent::PageReady { page, image }
                if page == self.page =>
            {
                match self.state.as_mut() {
                    Some(state) => {
                        state.page_image = Some(image_to_skia(&image));
                        state.window.request_redraw();
                    }
                    None => {
                        self.pending_image = Some(image);
                    }
                }
            }
            ViewerEvent::FileChanged => {
                if let Ok(new_pipeline) = PdfPipeline::open(&self.file) {
                    self.pipeline = Arc::new(new_pipeline);
                }
                self.spawn_render(self.page);
            }
            _ => {}
        }
    }
}

pub fn run(file: PathBuf, page: u32, config: RenderConfig) {
    let pipeline = Arc::new(PdfPipeline::open(&file).expect("open PDF"));

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
        zoom: 1.0,
        pan: [0.0, 0.0],
        ctrl_held: false,
        cursor_pos: [0.0, 0.0],
        drag_origin: None,
        _watcher: watcher,
        proxy,
    };

    event_loop.run_app(&mut viewer).expect("run app");
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a Viewer-like zoom/pan state for unit testing the math
    struct ZoomState {
        zoom: f32,
        pan: [f32; 2],
    }

    impl ZoomState {
        fn apply_zoom(
            &mut self,
            factor: f32,
            focal: Option<[f32; 2]>,
            win_w: f32,
            win_h: f32,
        ) {
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

    // Zoom from identity at window centre leaves pan unchanged
    #[test]
    fn zoom_center_pan_unchanged() {
        let mut s = ZoomState { zoom: 1.0, pan: [0.0, 0.0] };
        s.apply_zoom(2.0, None, 800.0, 600.0);
        assert_eq!(s.zoom, 2.0);
        assert!( s.pan[0].abs() < 1e-4, "pan x should be 0, got {}", s.pan[0]);
        assert!(s.pan[1].abs() < 1e-4, "pan y should be 0, got {}", s.pan[1]);
    }

    // Zoom at top-left corner (0,0) shifts pan toward centre
    #[test]
    fn zoom_corner_focal() {
        let mut s = ZoomState { zoom: 1.0, pan: [0.0, 0.0] };
        s.apply_zoom(2.0, Some([0.0, 0.0]), 800.0, 600.0);
        assert_eq!(s.zoom, 2.0);
        // focal (0,0) is (-400,-300) from centre; (1-r) = -1, so pan shifts +400/+300
        assert!((s.pan[0] - 400.0).abs() < 1e-3, "pan x={}", s.pan[0]);
        assert!((s.pan[1] - 300.0).abs() < 1e-3, "pan y={}", s.pan[1]);
    }

    // Zoom in then zoom out returns to original zoom (within float tolerance)
    #[test]
    fn zoom_in_out_roundtrip() {
        let mut s = ZoomState { zoom: 1.0, pan: [0.0, 0.0] };
        s.apply_zoom(1.1, None, 800.0, 600.0);
        s.apply_zoom(1.0 / 1.1, None, 800.0, 600.0);
        assert!((s.zoom - 1.0).abs() < 1e-5, "zoom={}", s.zoom);
        assert!(s.pan[0].abs() < 1e-4);
        assert!(s.pan[1].abs() < 1e-4);
    }

    // Zoom clamps at minimum (0.05)
    #[test]
    fn zoom_clamps_minimum() {
        let mut s = ZoomState { zoom: 0.06, pan: [0.0, 0.0] };
        s.apply_zoom(0.1, None, 800.0, 600.0);
        assert_eq!(s.zoom, 0.05);
    }

    // Zoom clamps at maximum (50.0)
    #[test]
    fn zoom_clamps_maximum() {
        let mut s = ZoomState { zoom: 49.0, pan: [0.0, 0.0] };
        s.apply_zoom(10.0, None, 800.0, 600.0);
        assert_eq!(s.zoom, 50.0);
    }

    // Zoom factor 1.0 is a no-op on both zoom and pan
    #[test]
    fn zoom_factor_one_noop() {
        let mut s = ZoomState { zoom: 2.0, pan: [50.0, -30.0] };
        s.apply_zoom(1.0, Some([100.0, 200.0]), 800.0, 600.0);
        assert!((s.zoom - 2.0).abs() < 1e-5);
        assert!((s.pan[0] - 50.0).abs() < 1e-4);
        assert!((s.pan[1] - -30.0).abs() < 1e-4);
    }

    // Pan drag: delta applied correctly
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
}