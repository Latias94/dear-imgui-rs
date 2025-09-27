//! DrawList minimal example (single file).
//! Shows basic primitives drawn inside a window using `get_window_draw_list`.

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
    last_frame: Instant,
}

struct AppWindow {
    window: Arc<Window>,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    imgui: ImguiState,
    thickness: f32,
    aa_lines: bool,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - DrawList Minimal")
            .with_inner_size(LogicalSize::new(1000.0, 640.0));

        let (window, cfg) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().unwrap()
            })?;

        let window = Arc::new(window.unwrap());

        let context_attribs =
            ContextAttributesBuilder::new().build(Some(window.window_handle()?.as_raw()));
        let context = unsafe { cfg.display().create_context(&cfg, &context_attribs)? };

        let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(false))
            .build(
                window.window_handle()?.as_raw(),
                NonZeroU32::new(1000).unwrap(),
                NonZeroU32::new(640).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let context = context.make_current(&surface)?;

        let mut imgui_context = Context::create();
        imgui_context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut imgui_context);
        platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        );

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                context.display().get_proc_address(s).cast()
            })
        };
        let mut renderer = GlowRenderer::new(gl, &mut imgui_context)?;
        renderer.set_framebuffer_srgb_enabled(false);
        renderer.new_frame()?;

        let imgui = ImguiState {
            context: imgui_context,
            platform,
            renderer,
            last_frame: Instant::now(),
        };

        Ok(Self {
            window,
            surface,
            context,
            imgui,
            thickness: 2.0,
            aa_lines: true,
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

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        ui.window("DrawList Minimal")
            .size([720.0, 500.0], Condition::FirstUseEver)
            .build(|| {
                // Controls
                ui.slider("Thickness", 1.0, 10.0, &mut self.thickness);
                ui.checkbox("Antialiased Lines", &mut self.aa_lines);
                ui.separator();

                let content = ui.content_region_avail();
                if content[0] > 0.0 && content[1] > 0.0 {
                    let draw_list = ui.get_window_draw_list();
                    let canvas_pos = ui.cursor_screen_pos();
                    let w = content[0];
                    let h = content[1];

                    // Background
                    draw_list
                        .add_rect(
                            canvas_pos,
                            [canvas_pos[0] + w, canvas_pos[1] + h],
                            [0.15, 0.16, 0.19, 1.0],
                        )
                        .filled(true)
                        .build();

                    // A few primitives
                    let col_line = [0.9, 0.7, 0.2, 1.0];
                    draw_list
                        .add_line(
                            [canvas_pos[0] + 20.0, canvas_pos[1] + 20.0],
                            [canvas_pos[0] + w - 20.0, canvas_pos[1] + 20.0],
                            col_line,
                        )
                        .thickness(self.thickness)
                        .build();

                    draw_list
                        .add_rect(
                            [canvas_pos[0] + 40.0, canvas_pos[1] + 60.0],
                            [canvas_pos[0] + 200.0, canvas_pos[1] + 160.0],
                            [0.2, 0.7, 0.9, 1.0],
                        )
                        .filled(false)
                        .rounding(8.0)
                        .thickness(self.thickness)
                        .build();

                    draw_list
                        .add_rect(
                            [canvas_pos[0] + 220.0, canvas_pos[1] + 60.0],
                            [canvas_pos[0] + 380.0, canvas_pos[1] + 160.0],
                            [0.2, 0.9, 0.5, 1.0],
                        )
                        .filled(true)
                        .rounding(8.0)
                        .build();

                    draw_list
                        .add_circle(
                            [canvas_pos[0] + 500.0, canvas_pos[1] + 110.0],
                            50.0,
                            [0.95, 0.4, 0.3, 1.0],
                        )
                        .thickness(self.thickness)
                        .build();

                    draw_list
                        .add_circle(
                            [canvas_pos[0] + 620.0, canvas_pos[1] + 110.0],
                            50.0,
                            [0.55, 0.8, 0.2, 1.0],
                        )
                        .filled(true)
                        .build();

                    // Text overlay
                    draw_list.add_text(
                        [canvas_pos[0] + 20.0, canvas_pos[1] + h - 30.0],
                        [1.0, 1.0, 1.0, 1.0],
                        "DrawList primitives",
                    );
                }
            });

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
                    self.window.as_ref().unwrap().window.request_redraw();
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
            WindowEvent::Resized(size) => {
                window.resize(size);
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => event_loop.exit(),
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
