//! Basic Dear ImGui example using the Glow OpenGL backend
//!
//! This example demonstrates the basic usage of dear-imgui-glow with winit v0.30.
//! It shows a simple Dear ImGui window with some basic widgets and the demo window.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui::*;
use dear_imgui_glow::GlowRenderer;
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

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: GlowRenderer,
    clear_color: [f32; 4],
    demo_open: bool,
    software_cursor: bool,
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
            .with_title("Dear ImGui Glow Basic Example")
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

        // Create surface (request sRGB-capable framebuffer for consistent visuals)
        let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(true))
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
        // Use sRGB framebuffer: enable FRAMEBUFFER_SRGB during ImGui rendering
        renderer.set_framebuffer_srgb_enabled(true);
        renderer.new_frame()?;

        let imgui = ImguiState {
            context: imgui_context,
            platform,
            renderer,
            clear_color: [0.1, 0.2, 0.3, 1.0],
            demo_open: true,
            software_cursor: false,
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

        // Apply pending software cursor change before starting the frame
        let want_sw = self.imgui.software_cursor;
        if self.imgui.context.io().mouse_draw_cursor() != want_sw {
            self.imgui
                .platform
                .set_software_cursor_enabled(&mut self.imgui.context, want_sw);
        }

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Main window content
        ui.window("Hello, Dear ImGui Glow!")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Welcome to Dear ImGui with Glow backend!");
                ui.separator();

                ui.text(&format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));

                if ui.color_edit4("Clear color", &mut self.imgui.clear_color) {
                    // Color updated
                }

                if ui.button("Show Demo Window") {
                    self.imgui.demo_open = true;
                }

                // Toggle software cursor (ImGui-drawn cursor)
                let mut sw = self.imgui.software_cursor;
                if ui.checkbox("Software cursor (drawn by ImGui)", &mut sw) {
                    // Defer IO change to next frame start to avoid borrow conflicts
                    self.imgui.software_cursor = sw;
                }

                ui.text("Modern texture management features:");
                ui.bullet_text("RENDERER_HAS_TEXTURES backend flag");
                ui.bullet_text("Complete ImTextureData system");
                ui.bullet_text("Texture registration and updates");
            });

        // Show demo window if requested
        if self.imgui.demo_open {
            ui.show_demo_window(&mut self.imgui.demo_open);
        }

        // Render
        let gl = self.imgui.renderer.gl_context().unwrap();
        unsafe {
            // Enable sRGB write for clear on sRGB-capable surface
            gl.enable(glow::FRAMEBUFFER_SRGB);
            gl.clear_color(
                self.imgui.clear_color[0],
                self.imgui.clear_color[1],
                self.imgui.clear_color[2],
                self.imgui.clear_color[3],
            );
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.disable(glow::FRAMEBUFFER_SRGB);
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
                    // Request initial redraw to start the render loop
                    window.window.request_redraw();
                    self.window = Some(window);
                    println!("Window created successfully");
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

        // Handle the event with ImGui first
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
