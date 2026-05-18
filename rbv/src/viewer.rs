use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{WindowAttributes, WindowId};

const SHADER: &str = r#"
struct Aspect {
    image:  f32,
    window: f32,
    _pad0:  f32,
    _pad1:  f32,
}

@group(1) @binding(0) var<uniform> u_aspect: Aspect;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0)       uv:  vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2( 1.0, -1.0), vec2(-1.0,  1.0),
        vec2( 1.0, -1.0), vec2( 1.0,  1.0), vec2(-1.0,  1.0),
    );
    var uv = array<vec2<f32>, 6>(
        vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(0.0, 0.0),
        vec2(1.0, 1.0), vec2(1.0, 0.0), vec2(0.0, 0.0),
    );
    let ia = u_aspect.image;
    let wa = u_aspect.window;
    var scale: vec2<f32>;
    if ia > wa {
        scale = vec2(1.0, wa / ia);
    } else {
        scale = vec2(ia / wa, 1.0);
    }
    var out: VsOut;
    out.pos = vec4(pos[vi] * scale, 0.0, 1.0);
    out.uv  = uv[vi];
    return out;
}

@group(0) @binding(0) var t_pdf: texture_2d<f32>;
@group(0) @binding(1) var s_pdf: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_pdf, s_pdf, in.uv);
}
"#;

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
struct AspectUniform {
    image:  f32,
    window: f32,
    _pad:   [f32; 2],
}

struct Viewer {
    path:       PathBuf,
    page:       u32,
    page_count: u32,
    config:     rustybara::raster::RenderConfig,
    image:      image::DynamicImage,
    gpu:        Option<GpuState>,
    digit_buf:  String,
}

struct GpuState {
    window:       Arc<winit::window::Window>,
    surface:      wgpu::Surface<'static>,
    device:       wgpu::Device,
    queue:        wgpu::Queue,
    config:       wgpu::SurfaceConfiguration,
    pipeline:     wgpu::RenderPipeline,
    bgl:          wgpu::BindGroupLayout,
    sampler:      wgpu::Sampler,
    bind_group:   wgpu::BindGroup,
    aspect_buf:   wgpu::Buffer,
    aspect_bg:    wgpu::BindGroup,
    image_aspect: f32,
}

impl GpuState {
    fn write_aspect(&self, win_w: u32, win_h: u32) {
        let data = AspectUniform {
            image:  self.image_aspect,
            window: win_w as f32 / win_h as f32,
            _pad:   [0.0; 2],
        };
        self.queue.write_buffer(&self.aspect_buf, 0, bytemuck::bytes_of(&data));
    }

    fn reload_texture(&mut self, image: &image::DynamicImage) {
        let texture = crate::texture::upload(&self.device, &self.queue, image);
        let tex_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   None,
            layout:  &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.image_aspect = image.width() as f32 / image.height() as f32;
    }
}

impl Viewer {
    fn title(&self) -> String {
        let name = self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        format!("{name}  [{}/{}]", self.page + 1, self.page_count)
    }

    fn reload_current_page(&mut self) {
        if let Ok(new_image) = rustybara::PdfPipeline::open(&self.path)
            .and_then(|p| p.render_page(self.page, &self.config))
        {
            self.image = new_image;
            let title = self.title();
            if let Some(gpu) = &mut self.gpu {
                gpu.reload_texture(&self.image);
                let (w, h) = (gpu.config.width, gpu.config.height);
                gpu.write_aspect(w, h);
                gpu.window.set_title(&title);
                gpu.window.request_redraw();
            }
        }
    }

    fn go_to_page(&mut self, page: u32) {
        if page < self.page_count && page != self.page {
            self.page = page;
            self.reload_current_page();
        }
    }

    fn consume_digit_buf(&mut self) -> Option<u32> {
        if self.digit_buf.is_empty() {
            return None;
        }
        let n = self.digit_buf.parse::<u32>().ok();
        self.digit_buf.clear();
        n
    }
}

impl ApplicationHandler for Viewer {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default().with_title(&self.title()))
                .unwrap(),
        );

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference:   wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
        }))
        .expect("no compatible GPU adapter found");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("failed to create wgpu device");

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let size = window.inner_size();
        let surf_config = wgpu::SurfaceConfiguration {
            usage:    wgpu::TextureUsages::RENDER_ATTACHMENT,
            format:   surface_format,
            width:    size.width.max(1),
            height:   size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode:   caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surf_config);

        // --- group 0: texture + sampler ---
        let texture  = crate::texture::upload(&device, &queue, &self.image);
        let tex_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler  = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   None,
            layout:  &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // --- group 1: aspect uniform ---
        let image_aspect = self.image.width() as f32 / self.image.height() as f32;
        let aspect_data  = AspectUniform {
            image:  image_aspect,
            window: size.width as f32 / size.height as f32,
            _pad:   [0.0; 2],
        };

        use wgpu::util::DeviceExt;
        let aspect_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    None,
            contents: bytemuck::bytes_of(&aspect_data),
            usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let aspect_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty:                 wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let aspect_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   None,
            layout:  &aspect_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: aspect_buf.as_entire_binding(),
            }],
        });

        // --- pipeline ---
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:              None,
            bind_group_layouts: &[Some(&bgl), Some(&aspect_bgl)],
            immediate_size:     0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &shader,
                entry_point:         Some("vs_main"),
                buffers:             &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     surface_format,
                    blend:      Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil:  None,
            multisample:    wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache:          None,
        });

        window.request_redraw();
        self.gpu = Some(GpuState {
            window,
            surface,
            device,
            queue,
            config: surf_config,
            pipeline,
            bgl,
            sampler,
            bind_group,
            aspect_buf,
            aspect_bg,
            image_aspect,
        });
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        self.reload_current_page();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(code),
                    state: ElementState::Pressed,
                    text,
                    ..
                },
                ..
            } => match code {
                KeyCode::Escape | KeyCode::KeyQ => {
                    self.digit_buf.clear();
                    event_loop.exit();
                }
                KeyCode::ArrowRight | KeyCode::ArrowDown | KeyCode::KeyL | KeyCode::KeyJ => {
                    self.digit_buf.clear();
                    self.go_to_page(self.page + 1);
                }
                KeyCode::ArrowLeft | KeyCode::ArrowUp | KeyCode::KeyH | KeyCode::KeyK => {
                    self.digit_buf.clear();
                    self.go_to_page(self.page.saturating_sub(1));
                }
                KeyCode::KeyG => {
                    if let Some(n) = self.consume_digit_buf() {
                        self.go_to_page(n.saturating_sub(1));
                    }
                }
                _ => {
                    if let Some(ch) = text.as_deref()
                        .and_then(|t| t.chars().next())
                        .filter(|c| c.is_ascii_digit())
                    {
                        self.digit_buf.push(ch);
                    } else {
                        self.digit_buf.clear();
                    }
                }
            },
            WindowEvent::RedrawRequested => {
                let Some(gpu) = &mut self.gpu else { return };
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    _ => return,
                };
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view:           &view,
                            resolve_target: None,
                            depth_slice:    None,
                            ops: wgpu::Operations {
                                load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set:      None,
                        timestamp_writes:         None,
                        multiview_mask:           None,
                    });
                    rpass.set_pipeline(&gpu.pipeline);
                    rpass.set_bind_group(0, &gpu.bind_group, &[]);
                    rpass.set_bind_group(1, &gpu.aspect_bg, &[]);
                    rpass.draw(0..6, 0..1);
                }
                gpu.queue.submit(std::iter::once(encoder.finish()));
                frame.present();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width  = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                    gpu.write_aspect(gpu.config.width, gpu.config.height);
                    gpu.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

pub fn run(path: PathBuf, page: u32, config: rustybara::raster::RenderConfig) {
    let pipeline = rustybara::PdfPipeline::open(&path).unwrap_or_else(|e| {
        eprintln!("Cannot open PDF: {e}");
        std::process::exit(1);
    });
    let page_count = pipeline.page_count() as u32;
    let page = page.min(page_count.saturating_sub(1));
    let image = pipeline.render_page(page, &config).unwrap_or_else(|e| {
        eprintln!("Cannot render page: {e}");
        std::process::exit(1);
    });

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let proxy = event_loop.create_proxy();

    let watch_path = path.clone();
    std::thread::spawn(move || {
        use notify::{RecursiveMode, Watcher};
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(e) = res {
                    if e.kind.is_modify() || e.kind.is_create() {
                        let _ = tx.send(());
                    }
                }
            },
            notify::Config::default(),
        )
        .expect("failed to create file watcher");
        watcher
            .watch(&watch_path, RecursiveMode::NonRecursive)
            .expect("failed to watch file");
        loop {
            if rx.recv().is_ok() {
                while rx.recv_timeout(std::time::Duration::from_millis(300)).is_ok() {}
                std::thread::sleep(std::time::Duration::from_millis(150));
                let _ = proxy.send_event(());
            }
        }
    });

    let mut app = Viewer { path, page, page_count, config, image, gpu: None, digit_buf: String::new() };
    event_loop.run_app(&mut app).unwrap();
}
