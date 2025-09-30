//! Input Text minimal example (single file).
//! Demonstrates String vs ImString, hints, capacity_hint and multiline.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

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
    // demo state
    title_string: String,
    name_imstr: ImString,
    notes_imstr: ImString,
    notes_string_cb: String,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Input Text Minimal")
            .with_inner_size(LogicalSize::new(1100.0, 700.0));

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
                NonZeroU32::new(1100).unwrap(),
                NonZeroU32::new(700).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let context = context.make_current(&surface)?;

        // Dear ImGui
        let mut imgui_context = Context::create();
        imgui_context.set_ini_filename(None::<String>).unwrap();

        let mut platform = WinitPlatform::new(&mut imgui_context);
        platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        );

        // OpenGL renderer
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
            title_string: String::with_capacity(64),
            name_imstr: ImString::new(""),
            notes_imstr: ImString::new(""),
            notes_string_cb: String::new(),
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

        ui.window("Input Text Minimal")
            .size([700.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("String (heap-allocated)");
                if ui
                    .input_text("Title (String)", &mut self.title_string)
                    .hint("Enter a title...")
                    .capacity_hint(64)
                    .enter_returns_true(true)
                    .build()
                {
                    // pressed Enter (EnterReturnsTrue)
                }

                ui.separator();

                ui.text("ImString (zero-copy, internal grow)");
                ui.input_text_imstr("Name (ImString)", &mut self.name_imstr)
                    .hint("Your name")
                    .build();

                ui.input_text_multiline_imstr(
                    "Notes (ImString)",
                    &mut self.notes_imstr,
                    [500.0, 160.0],
                )
                .build();

                ui.separator();
                ui.text("String (with callbacks)");
                // Define a simple callback handler demonstrating features
                #[derive(Default)]
                struct DemoHandler;
                impl dear_imgui_rs::InputTextCallbackHandler for DemoHandler {
                    fn char_filter(&mut self, c: char) -> Option<char> {
                        // Filter out 'x' or 'X'
                        if c == 'x' || c == 'X' { None } else { Some(c) }
                    }
                    fn on_completion(&mut self, mut data: dear_imgui_rs::TextCallbackData) {
                        data.push_str(" [Tab]");
                    }
                    fn on_history(
                        &mut self,
                        dir: dear_imgui_rs::HistoryDirection,
                        mut data: dear_imgui_rs::TextCallbackData,
                    ) {
                        match dir {
                            dear_imgui_rs::HistoryDirection::Up => data.push_str(" [Up]"),
                            dear_imgui_rs::HistoryDirection::Down => data.push_str(" [Down]"),
                        }
                    }
                    fn on_edit(&mut self, _data: dear_imgui_rs::TextCallbackData) {}
                    fn on_always(&mut self, _data: dear_imgui_rs::TextCallbackData) {}
                }

                // For multiline, ImGui forbids HISTORY/COMPLETION callbacks.
                let callbacks = dear_imgui_rs::InputTextCallback::CHAR_FILTER
                    | dear_imgui_rs::InputTextCallback::EDIT
                    | dear_imgui_rs::InputTextCallback::ALWAYS;

                // Multiline String with callbacks (Tab/Up/Down/CharFilter)
                ui.input_text_multiline(
                    "Notes (String + Callbacks)",
                    &mut self.notes_string_cb,
                    [500.0, 120.0],
                )
                .callback(callbacks, DemoHandler::default())
                .build();

                ui.separator();
                ui.text(format!(
                    "Title.len={} | Name.len={} | Notes.len={}",
                    self.title_string.len(),
                    self.name_imstr.to_str().len(),
                    self.notes_imstr.to_str().len()
                ));
                ui.text(format!(
                    "Notes(String+CB).len={}",
                    self.notes_string_cb.len()
                ));
            });

        // Clear and render
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

        // Forward to ImGui platform
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
