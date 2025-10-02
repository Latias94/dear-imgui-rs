//! WGPU Render-to-Texture integration example.
//! Renders to an offscreen wgpu::Texture each frame and shows it in an ImGui window.

use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use pollster::block_on;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;
use wgpu::TextureUsages;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[path = "../support/wgpu_init.rs"]
mod wgpu_init;

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
}

struct OffscreenRtt {
    size: (u32, u32),
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    texture_id: dear_imgui_rs::TextureId,
}

impl OffscreenRtt {
    fn create(
        device: &wgpu::Device,
        renderer: &mut WgpuRenderer,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen-rtt"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let id64 =
            renderer
                .texture_manager_mut()
                .register_texture(dear_imgui_wgpu::WgpuTexture::new(
                    texture.clone(),
                    view.clone(),
                ));
        let texture_id = dear_imgui_rs::TextureId::from(id64);

        Self {
            size,
            texture,
            view,
            texture_id,
        }
    }

    fn recreate(
        &mut self,
        device: &wgpu::Device,
        renderer: &mut WgpuRenderer,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) {
        // Destroy old registration
        renderer
            .texture_manager_mut()
            .destroy_texture(self.texture_id);

        let new_self = OffscreenRtt::create(device, renderer, size, format);
        *self = new_self;
    }
}

// WGSL shader that draws a procedural grid and a filled triangle
const OFFSCREEN_WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0,  1.0)
    );
    let p = positions[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    // Map clip-space to UV; flip Y to have UV origin at top-left
    out.uv = vec2<f32>(p.x, -p.y) * 0.5 + vec2<f32>(0.5, 0.5);
    return out;
}

fn in_triangle(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, p: vec2<f32>) -> bool {
    // Barycentric technique
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = dot(v0, v0);
    let d01 = dot(v0, v1);
    let d11 = dot(v1, v1);
    let d20 = dot(v2, v0);
    let d21 = dot(v2, v1);
    let denom = d00 * d11 - d01 * d01;
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;
    return u >= 0.0 && v >= 0.0 && w >= 0.0;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var color = vec3<f32>(0.05, 0.07, 0.10);

    // Grid lines in UV space
    let uv = clamp(in.uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    let grid_scale = 16.0; // number of cells
    let t = 0.015; // line thickness (in UV units)
    let fu = fract(uv.x * grid_scale);
    let fv = fract(uv.y * grid_scale);
    let near_line = fu < t || fv < t || fu > (1.0 - t) || fv > (1.0 - t);
    if (near_line) {
        color = mix(color, vec3<f32>(0.20, 0.22, 0.24), 1.0);
    }

    // Filled triangle overlay
    // Base near bottom, apex towards top (UV origin at top-left)
    let A = vec2<f32>(0.20, 0.80);
    let B = vec2<f32>(0.80, 0.80);
    let C = vec2<f32>(0.50, 0.20);
    if (in_triangle(A, B, C, uv)) {
        color = vec3<f32>(0.85, 0.30, 0.25);
    }

    return vec4<f32>(color, 1.0);
}
"#;

fn create_offscreen_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Offscreen WGSL"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(OFFSCREEN_WGSL)),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Offscreen Pipeline Layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Offscreen Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview: None,
        cache: None,
    })
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
    rtt: OffscreenRtt,
    offscreen_pipeline: wgpu::RenderPipeline,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window = {
            let size = LogicalSize::new(1280.0, 720.0);
            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Dear ImGui WGPU - Render to Texture (Game View)")
                        .with_inner_size(size),
                )?,
            )
        };

        // Init WGPU
        let (device, queue, surface, mut surface_desc) = wgpu_init::init_wgpu_for_window(&window)?;

        // ImGui context
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        // Renderer
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer = WgpuRenderer::new(init_info, &mut context)?;
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        // Create offscreen RTT at initial size
        let rtt = OffscreenRtt::create(&device, &mut renderer, (640, 360), surface_desc.format);

        // Create a simple pipeline that draws a procedural grid and a triangle
        let offscreen_pipeline = create_offscreen_pipeline(&device, surface_desc.format);

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            last_frame: Instant::now(),
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
            rtt,
            offscreen_pipeline,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta.as_secs_f32());

        // Ensure RTT size fits a fraction of window
        let logical = self.window.inner_size();
        let target = (
            (logical.width as f32 * 0.5).max(1.0) as u32,
            (logical.height as f32 * 0.5).max(1.0) as u32,
        );
        if target != self.rtt.size {
            self.rtt.recreate(
                &self.device,
                &mut self.imgui.renderer,
                target,
                self.surface_desc.format,
            );
        }

        // Render pass to offscreen RTT (grid + triangle)
        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Offscreen Encoder"),
                });
            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Offscreen Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.rtt.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                // Fullscreen triangle with procedural grid + filled triangle in FS
                rpass.set_pipeline(&self.offscreen_pipeline);
                rpass.draw(0..3, 0..1);
                drop(rpass);
            }
            self.queue.submit(Some(encoder.finish()));
        }

        // Begin frame
        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        ui.window("Game View (RTT)")
            .size([640.0, 400.0], Condition::FirstUseEver)
            .build(|| {
                let avail = ui.content_region_avail();
                let avail_w = avail[0].max(1.0);
                let avail_h = avail[1].max(1.0);

                // Aspect-fit scaling for the RTT texture
                let tex_w = self.rtt.size.0 as f32;
                let tex_h = self.rtt.size.1 as f32;
                let tex_aspect = tex_w / tex_h.max(1.0);
                let avail_aspect = avail_w / avail_h;

                let (disp_w, disp_h) = if avail_aspect > tex_aspect {
                    // Limited by height
                    let h = avail_h;
                    let w = h * tex_aspect;
                    (w, h)
                } else {
                    // Limited by width
                    let w = avail_w;
                    let h = w / tex_aspect;
                    (w, h)
                };

                // Center the image in the available region
                let cur = ui.cursor_pos();
                let off_x = (avail_w - disp_w) * 0.5;
                let off_y = (avail_h - disp_h) * 0.5;
                ui.set_cursor_pos([cur[0] + off_x, cur[1] + off_y]);

                ui.image(self.rtt.texture_id, [disp_w, disp_h]);
            });

        // Render to swapchain
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Encoder"),
            });

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.imgui.renderer.new_frame()?;
            self.imgui
                .renderer
                .render_draw_data(&draw_data, &mut rpass)?;
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                }
                Err(e) => {
                    eprintln!("Failed to create window: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        window
            .imgui
            .platform
            .handle_event(&mut window.imgui.context, &window.window, &full_event);

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    eprintln!("Render error: {e}");
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
