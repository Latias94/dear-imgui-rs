//! WGPU texture demo (single file): generate and update a texture on the CPU,
//! register it with the dear-imgui-wgpu backend, and show it via `Image`.

use ::image::ImageReader;
use dear_imgui_examples::animated_texture::animated_rgba_pixels;
use dear_imgui_rs::*;
use dear_imgui_wgpu::WgpuRenderer;
use dear_imgui_winit::WinitPlatform;
use pollster::block_on;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    last_frame: Instant,
    animation_started: Instant,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    imgui: ImguiState,
    // Texture demo state (managed by ImGui modern texture system)
    img_tex: dear_imgui_rs::ManagedTextureId,
    photo_tex: Option<(dear_imgui_rs::ManagedTextureId, (u32, u32))>,
    tex_size: (u32, u32),
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        let window = {
            let size = LogicalSize::new(1280.0, 720.0);
            Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Dear ImGui WGPU - Texture Demo")
                        .with_inner_size(size),
                )?,
            )
        };

        let surface = instance.create_surface(window.clone())?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))
        .expect("No suitable GPU adapters found on the system!");

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let size = LogicalSize::new(1280.0, 720.0);
        let caps = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .cloned()
            .find(|f| caps.formats.contains(f))
            .unwrap_or(caps.formats[0]);

        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width as u32,
            height: size.height as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_desc);

        // ImGui context
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        let mut platform = WinitPlatform::new(&mut context)?;
        platform.attach_window(
            Arc::clone(&window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        )?;

        // Renderer
        let init_info =
            dear_imgui_wgpu::WgpuInitInfo::new(device.clone(), queue.clone(), surface_desc.format);
        let mut renderer = WgpuRenderer::new(init_info, &mut context)?;
        renderer.set_gamma_mode(dear_imgui_wgpu::GammaMode::Auto);

        // Create a managed ImGui texture (CPU-side pixels; backend will create GPU texture)
        let tex_w: u32 = 128;
        let tex_h: u32 = 128;
        let mut img_tex = dear_imgui_rs::texture::OwnedTextureData::new();
        img_tex.create(dear_imgui_rs::texture::TextureFormat::RGBA32, tex_w, tex_h);

        let pixels = animated_rgba_pixels(tex_w, tex_h, Duration::ZERO);
        img_tex.set_data(&pixels);

        // Optionally, create a second managed texture from a user image
        let photo_tex = Self::maybe_load_photo_texture();

        let img_tex = context.register_texture(img_tex);
        let photo_tex = photo_tex.map(|photo| {
            let size = (photo.width(), photo.height());
            (context.register_texture(photo), size)
        });

        let now = Instant::now();
        let imgui = ImguiState {
            context,
            platform,
            renderer,
            last_frame: now,
            animation_started: now,
        };

        Ok(Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            imgui,
            img_tex,
            photo_tex,
            tex_size: (tex_w, tex_h),
        })
    }

    fn maybe_load_photo_texture() -> Option<dear_imgui_rs::texture::OwnedTextureData> {
        // Prefer a clean, low-frequency gradient test image; fall back to the
        // original JPEG if the gradient asset is missing.
        let asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let candidates = [
            asset_dir.join("texture_clean.ppm"),
            asset_dir.join("texture.jpg"),
        ];

        let path = match candidates.iter().find(|p| p.exists()) {
            Some(p) => p.clone(),
            None => {
                eprintln!(
                    "No demo image found in {:?}. Current dir: {:?}",
                    asset_dir,
                    std::env::current_dir().ok()
                );
                return None;
            }
        };

        match ImageReader::open(&path)
            .and_then(|r| r.with_guessed_format())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        {
            Ok(r) => match r.decode() {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let data = rgba.into_raw();
                    let mut t = dear_imgui_rs::texture::OwnedTextureData::new();
                    t.create(dear_imgui_rs::texture::TextureFormat::RGBA32, w, h);
                    t.set_data(&data);
                    println!("Loaded image for WGPU demo from {:?} ({}x{})", path, w, h);
                    Some(t)
                }
                Err(e) => {
                    eprintln!("Failed to decode WGPU demo image {:?}: {e}", path);
                    None
                }
            },
            Err(e) => {
                eprintln!("Failed to open WGPU demo image {:?}: {e}", path);
                None
            }
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_desc.width = new_size.width;
            self.surface_desc.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_desc);
        }
    }

    fn update_texture(&mut self) {
        let (w, h) = self.tex_size;
        let pixels = animated_rgba_pixels(w, h, self.imgui.animation_started.elapsed());

        self.imgui
            .context
            .with_texture_mut(self.img_tex, |mut texture| texture.set_data(&pixels))
            .expect("animated texture should remain active");
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        let delta_secs = delta_time.as_secs_f32();
        self.imgui.context.io_mut().set_delta_time(delta_secs);
        self.imgui.last_frame = now;

        // Update animated texture (marks WantUpdates)
        self.update_texture();

        let (frame, reconfigure_after_present) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_desc);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface acquisition failed with a WGPU validation error".into());
            }
        };
        self.imgui
            .platform
            .prepare_frame(&mut self.imgui.context, &self.window)?;
        let ui = self.imgui.context.frame();

        ui.window("WGPU Texture Demo (ImGui-managed)")
            .size([520.0, 420.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Wall-clock animation; pixels upload every rendered frame");
                ui.separator();
                Image::new(ui, self.img_tex, [256.0, 256.0]).build();

                if let Some((photo, (width, height))) = self.photo_tex {
                    ui.separator();
                    ui.text("Loaded Image (1:1):");
                    Image::new(ui, photo, [width as f32, height as f32]).build();
                } else {
                    ui.separator();
                    ui.text_wrapped(
                        "Tip: set DEAR_IMGUI_EXAMPLE_IMAGE or place examples/resources/statue.jpg to preview an actual image.",
                    );
                }
            });

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Finalize inputs on platform and build draw data
        self.imgui.platform.prepare_render(&ui, &self.window)?;
        let draw_data = self.imgui.context.render();
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
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
                multiview_mask: None,
            });

            self.imgui.renderer.new_frame()?;
            self.imgui.renderer.render(draw_data, &mut rpass)?;
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.surface_desc);
        }
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
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        if let Err(error) = window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        ) {
            eprintln!("Winit platform event error: {error}");
            event_loop.exit();
            return;
        }

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
