//! Dear ImGui Glow example demonstrating modern texture management
//!
//! This example shows how to use the modern ImGui 1.92+ texture management system
//! with the dear-imgui-glow backend, including texture registration, updates, and
//! accessing texture data.

use ::image::ImageReader;
use std::{num::NonZeroU32, path::PathBuf, sync::Arc, time::Instant};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::*;
use dear_imgui_winit::WinitPlatform;
use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext},
    display::{GetGlDisplay, GlDisplay},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::HasWindowHandle;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct TextureDemo {
    generated_texture: Option<TextureId>,
    checkerboard_texture: Option<TextureId>,
    animated_texture: Option<TextureId>,
    user_image_texture: Option<TextureId>,
    user_image_size: Option<(u32, u32)>,
    frame_count: u32,
}

impl TextureDemo {
    fn new() -> Self {
        Self {
            generated_texture: None,
            checkerboard_texture: None,
            animated_texture: None,
            user_image_texture: None,
            user_image_size: None,
            frame_count: 0,
        }
    }

    fn initialize(
        &mut self,
        renderer: &mut GlowRenderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Generate a simple gradient texture
        self.generated_texture = Some(self.create_gradient_texture(renderer)?);

        // Create a checkerboard pattern
        self.checkerboard_texture = Some(self.create_checkerboard_texture(renderer)?);

        // Create an animated texture (will be updated each frame)
        self.animated_texture = Some(self.create_animated_texture(renderer)?);

        // Try to load a user image from disk (optional)
        if let Some(tex) = self.try_load_image_texture(renderer) {
            self.user_image_texture = Some(tex);
            // We can infer size from the registered texture using the backend's inspection API
            if let Some(t) = renderer.get_texture_data(tex) {
                self.user_image_size = Some((t.width() as u32, t.height() as u32));
            }
        }

        Ok(())
    }

    fn create_gradient_texture(
        &self,
        renderer: &mut GlowRenderer,
    ) -> Result<TextureId, Box<dyn std::error::Error>> {
        const WIDTH: u32 = 256;
        const HEIGHT: u32 = 256;

        let mut data = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let r = (x as f32 / WIDTH as f32 * 255.0) as u8;
                let g = (y as f32 / HEIGHT as f32 * 255.0) as u8;
                let b = ((x + y) as f32 / (WIDTH + HEIGHT) as f32 * 255.0) as u8;
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }

        let texture_id = renderer.register_texture(WIDTH, HEIGHT, TextureFormat::RGBA32, &data)?;

        println!("Created gradient texture with ID: {:?}", texture_id);
        if let Some(texture_data) = renderer.get_texture_data(texture_id) {
            println!("Texture format: {:?}", texture_data.format());
            println!(
                "Texture size: {}x{}",
                texture_data.width(),
                texture_data.height()
            );
        }

        Ok(texture_id)
    }

    fn try_load_image_texture(&self, renderer: &mut GlowRenderer) -> Option<TextureId> {
        // Prefer the gradient test image we ship with the examples; fall back
        // to the original JPEG if needed. This keeps behavior consistent with
        // the WGPU texture demos.
        let asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let candidates = [
            asset_dir.join("texture_clean.ppm"),
            asset_dir.join("texture.jpg"),
        ];

        let path = match candidates.iter().find(|p| p.exists()) {
            Some(p) => p.clone(),
            None => {
                eprintln!(
                    "Image not found in {:?}. Current dir: {:?}",
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
            Ok(mut r) => match r.decode() {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let data = rgba.into_raw();
                    match renderer.register_texture(w, h, TextureFormat::RGBA32, &data) {
                        Ok(id) => {
                            println!("Loaded image texture from {:?} ({}x{})", path, w, h);
                            // store size via caller
                            // caller will set user_image_size
                            Some(id)
                        }
                        Err(e) => {
                            eprintln!("Failed to register image texture: {e}");
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to decode image {:?}: {e}", path);
                    None
                }
            },
            Err(e) => {
                eprintln!("Failed to open image {:?}: {e}", path);
                None
            }
        }
    }

    fn create_checkerboard_texture(
        &self,
        renderer: &mut GlowRenderer,
    ) -> Result<TextureId, Box<dyn std::error::Error>> {
        const WIDTH: u32 = 128;
        const HEIGHT: u32 = 128;
        const CHECKER_SIZE: u32 = 16;

        let mut data = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let checker_x = (x / CHECKER_SIZE) % 2;
                let checker_y = (y / CHECKER_SIZE) % 2;
                let is_white = (checker_x + checker_y) % 2 == 0;

                let color = if is_white { 255 } else { 64 };
                data.extend_from_slice(&[color, color, color, 255]);
            }
        }

        renderer
            .register_texture(WIDTH, HEIGHT, TextureFormat::RGBA32, &data)
            .map_err(Into::into)
    }

    fn create_animated_texture(
        &self,
        renderer: &mut GlowRenderer,
    ) -> Result<TextureId, Box<dyn std::error::Error>> {
        const WIDTH: u32 = 64;
        const HEIGHT: u32 = 64;

        let mut data = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                // Initial pattern
                let r = ((x + y) % 256) as u8;
                let g = (x % 256) as u8;
                let b = (y % 256) as u8;
                data.extend_from_slice(&[r, g, b, 255]);
            }
        }

        renderer
            .register_texture(WIDTH, HEIGHT, TextureFormat::RGBA32, &data)
            .map_err(Into::into)
    }

    fn update_animated_texture(
        &mut self,
        renderer: &mut GlowRenderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(texture_id) = self.animated_texture {
            const WIDTH: u32 = 64;
            const HEIGHT: u32 = 64;

            let mut data = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
            let time = self.frame_count as f32 * 0.1;

            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let fx = x as f32 / WIDTH as f32;
                    let fy = y as f32 / HEIGHT as f32;

                    // Create animated pattern
                    let r = ((fx * 255.0 + time.sin() * 128.0).max(0.0).min(255.0)) as u8;
                    let g = ((fy * 255.0 + (time * 1.5).cos() * 128.0)
                        .max(0.0)
                        .min(255.0)) as u8;
                    let b = (((fx + fy) * 255.0 + time * 50.0).sin().abs() * 255.0) as u8;

                    data.extend_from_slice(&[r, g, b, 255]);
                }
            }

            renderer.update_texture(texture_id, WIDTH, HEIGHT, &data)?;
        }
        Ok(())
    }

    fn show_ui(&self, ui: &dear_imgui_rs::Ui) {
        ui.window("Modern Texture Management Demo")
            .size([500.0, 600.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui Glow - Modern Texture System");
                ui.separator();

                ui.text("Features demonstrated:");
                ui.bullet_text("Texture registration with TextureFormat");
                ui.bullet_text("Texture data access and inspection");
                ui.bullet_text("Dynamic texture updates");
                ui.bullet_text("RENDERER_HAS_TEXTURES backend flag");

                ui.separator();

                if let Some(texture_id) = self.generated_texture {
                    ui.text("Gradient Texture:");
                    Image::new(ui, texture_id, [128.0, 128.0]).build();
                }

                ui.same_line();

                if let Some(texture_id) = self.checkerboard_texture {
                    ui.text("Checkerboard Texture:");
                    Image::new(ui, texture_id, [128.0, 128.0]).build();
                }

                ui.separator();

                if let Some(texture_id) = self.animated_texture {
                    ui.text("Animated Texture (updates each frame):");
                    Image::new(ui, texture_id, [128.0, 128.0]).build();

                    ui.text(&format!("Frame: {}", self.frame_count));
                }

                if let Some(texture_id) = self.user_image_texture {
                    ui.separator();
                    ui.text("Loaded Image:");
                    // Display at a reasonable size while preserving aspect ratio
                    if let Some((w_u, h_u)) = self.user_image_size {
                        let w = w_u as f32;
                        let h = h_u as f32;
                        let max_dim = 256.0;
                        let scale = if w > h { max_dim / w } else { max_dim / h };
                        Image::new(ui, texture_id, [w * scale, h * scale]).build();
                    } else {
                        Image::new(ui, texture_id, [256.0, 256.0]).build();
                    }
                } else {
                    ui.separator();
                    ui.text_wrapped(
                        "Tip: set DEAR_IMGUI_EXAMPLE_IMAGE to a file path or place examples/resources/statue.jpg to preview a real image.",
                    );
                }

                ui.separator();
                ui.text("Texture Management Info:");
                ui.text(" All textures use the modern ImTextureData system");
                ui.text(" Backend properly declares RENDERER_HAS_TEXTURES");
                ui.text(" Textures are automatically managed by Dear ImGui");
            });
    }

    fn update(&mut self) {
        self.frame_count += 1;
    }
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: GlowRenderer,
    texture_demo: TextureDemo,
    last_frame: Instant,
}

struct AppWindow {
    window: Arc<Window>,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    imgui: ImguiState,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Create window with OpenGL context
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui Glow - Modern Texture Management")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));

        let (window, cfg) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().unwrap()
            })?;

        let window = Arc::new(window.unwrap());

        // Create OpenGL context
        let context_attribs =
            ContextAttributesBuilder::new().build(Some(window.window_handle()?.as_raw()));
        let context = unsafe { cfg.display().create_context(&cfg, &context_attribs)? };

        // Create surface (linear framebuffer for predictable hex -> output mapping)
        let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(false))
            .build(
                window.window_handle()?.as_raw(),
                NonZeroU32::new(1280).unwrap(),
                NonZeroU32::new(720).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };

        let context = context.make_current(&surface)?;

        // Setup Dear ImGui
        let mut imgui_context = Context::create();
        imgui_context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut imgui_context);
        platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        );

        // Create Glow context and renderer
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                context.display().get_proc_address(s).cast()
            })
        };

        let mut renderer = GlowRenderer::new(gl, &mut imgui_context)?;
        // Use linear framebuffer (no sRGB conversion for ImGui rendering)
        renderer.set_framebuffer_srgb_enabled(false);
        renderer.new_frame()?;

        // Initialize texture demo
        let mut texture_demo = TextureDemo::new();
        texture_demo.initialize(&mut renderer)?;

        let imgui = ImguiState {
            context: imgui_context,
            platform,
            renderer,
            texture_demo,
            last_frame: Instant::now(),
        };

        Ok(Self {
            window,
            surface,
            context,
            imgui,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface.resize(
                &self.context,
                NonZeroU32::new(new_size.width).unwrap(),
                NonZeroU32::new(new_size.height).unwrap(),
            );
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        self.imgui.last_frame = now;

        // Update animated texture
        self.imgui.texture_demo.update();
        self.imgui
            .texture_demo
            .update_animated_texture(&mut self.imgui.renderer)?;

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Show texture demo UI
        self.imgui.texture_demo.show_ui(ui);

        // Show demo window
        ui.show_demo_window(&mut true);

        // Render
        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        self.imgui.renderer.new_frame()?;
        self.imgui.renderer.render(&draw_data)?;

        self.surface.swap_buffers(&self.context)?;
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
                    self.window = Some(window);
                    println!("Window created successfully with texture demo");
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

        // Handle the event with ImGui first
        window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        );

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                println!("Close requested");
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
