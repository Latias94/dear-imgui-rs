//! ImNodeFlow Basic Example with Glow Backend
//!
//! This example demonstrates the basic usage of ImNodeFlow with Dear ImGui.
//! It creates a simple node editor interface and shows how to integrate
//! ImNodeFlow with the Dear ImGui Glow (OpenGL) backend.

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui::*;
use dear_imgui_glow::AutoRenderer;
use dear_imgui_winit::WinitPlatform;
use dear_imnodeflow::*;
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

#[derive(Debug)]
enum AppError {
    Glow(Box<dyn std::error::Error>),
    NodeFlow(dear_imnodeflow::NodeFlowError),
    Other(Box<dyn std::error::Error>),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Glow(e) => write!(f, "Glow error: {}", e),
            AppError::NodeFlow(e) => write!(f, "NodeFlow error: {}", e),
            AppError::Other(e) => write!(f, "Other error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<dear_imnodeflow::NodeFlowError> for AppError {
    fn from(e: dear_imnodeflow::NodeFlowError) -> Self {
        AppError::NodeFlow(e)
    }
}

impl From<Box<dyn std::error::Error>> for AppError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        AppError::Other(e)
    }
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: AutoRenderer,
    clear_color: [f32; 4],
    last_frame: Instant,
    demo_open: bool,
}

struct AppWindow {
    window: Arc<Window>,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    imgui: ImguiState,
    node_editor: Option<NodeEditorWrapper>,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        // Create window with OpenGL context
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("ImNodeFlow Basic Example with Glow Backend")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));

        let (window, cfg) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().unwrap()
            })?;

        let window = Arc::new(window.unwrap());

        // Create OpenGL context
        let window_handle = window
            .window_handle()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        let context_attribs = ContextAttributesBuilder::new().build(Some(window_handle.as_raw()));
        let context = unsafe {
            cfg.display()
                .create_context(&cfg, &context_attribs)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
        };

        // Create surface
        let window_handle2 = window
            .window_handle()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window_handle2.as_raw(),
            NonZeroU32::new(1280).unwrap(),
            NonZeroU32::new(720).unwrap(),
        );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?
        };

        let context = context
            .make_current(&surface)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        // Setup Dear ImGui
        let mut imgui_context = Context::create_or_panic();
        imgui_context.set_ini_filename_or_panic(None::<String>);

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

        let mut renderer = AutoRenderer::new(gl, &mut imgui_context)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        renderer
            .new_frame()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        let imgui = ImguiState {
            context: imgui_context,
            platform,
            renderer,
            clear_color: [0.1, 0.2, 0.3, 1.0],
            demo_open: true,
            last_frame: Instant::now(),
        };

        // Initialize node editor
        let node_editor = match NodeEditorWrapper::new("ImNodeFlow Basic Example") {
            Ok(editor) => {
                println!("✅ NodeEditor initialized successfully with example nodes");
                Some(editor)
            }
            Err(e) => {
                eprintln!("Failed to initialize NodeEditor: {}", e);
                None
            }
        };

        Ok(Self {
            window,
            surface,
            context,
            imgui,
            node_editor,
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

    fn render(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
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

        // Main window content
        ui.window("ImNodeFlow Basic Example with Glow")
            .size([400.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Welcome to ImNodeFlow with Glow backend!");
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

                ui.text("Node Editor Status:");
                if let Some(ref mut editor) = self.node_editor {
                    ui.text_colored([0.0, 1.0, 0.0, 1.0], "✅ Node Editor Active");
                    // Update the node editor
                    editor.update(&ui);
                } else {
                    ui.text_colored([1.0, 0.0, 0.0, 1.0], "❌ Node Editor Failed");
                }
            });

        // Show demo window if requested
        if self.imgui.demo_open {
            ui.show_demo_window(&mut self.imgui.demo_open);
        }

        // Render
        let gl = self.imgui.renderer.gl_context();
        unsafe {
            gl.clear_color(
                self.imgui.clear_color[0],
                self.imgui.clear_color[1],
                self.imgui.clear_color[2],
                self.imgui.clear_color[3],
            );
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        self.imgui
            .platform
            .prepare_render(&mut self.imgui.context, &self.window);
        let draw_data = self.imgui.context.render();

        self.imgui
            .renderer
            .new_frame()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        self.imgui
            .renderer
            .render(&draw_data)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        self.surface
            .swap_buffers(&self.context)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // For compatibility with older winit versions and mobile platforms
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(window) => {
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
        // Request redraw if we have a window
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
