//! Minimal Docking example (single file).
//! Shows how to enable docking and create a fullscreen DockSpace.

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
    first_layout_applied: bool,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Create window with OpenGL context
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Dockspace Minimal")
            .with_inner_size(LogicalSize::new(1200.0, 720.0));

        let (window, cfg) = glutin_winit::DisplayBuilder::new()
            .with_window_attributes(Some(window_attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs.next().unwrap()
            })?;

        let window = Arc::new(window.unwrap());

        // OpenGL context
        let context_attribs =
            ContextAttributesBuilder::new().build(Some(window.window_handle()?.as_raw()));
        let context = unsafe { cfg.display().create_context(&cfg, &context_attribs)? };

        // Linear framebuffer for simplicity
        let surface_attribs = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(false))
            .build(
                window.window_handle()?.as_raw(),
                NonZeroU32::new(1200).unwrap(),
                NonZeroU32::new(720).unwrap(),
            );
        let surface = unsafe {
            cfg.display()
                .create_window_surface(&cfg, &surface_attribs)?
        };
        let context = context.make_current(&surface)?;

        // Dear ImGui
        let mut imgui_context = Context::create();
        // Deterministic layout for a minimal sample
        imgui_context.set_ini_filename(None::<String>).unwrap();

        // Enable docking
        let io = imgui_context.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::DOCKING_ENABLE);
        io.set_config_flags(flags);

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
            first_layout_applied: false,
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

        // 1) Host a fullscreen window for the DockSpace (mirrors minimal C++ docking example)
        use dear_imgui_rs::{
            DockBuilder, DockNodeFlags, Id, SplitDirection, StyleColor, StyleVar, WindowFlags,
        };

        let viewport = ui.main_viewport();
        // Ensure this window is associated with the main viewport (safe wrapper)
        ui.set_next_window_viewport(viewport.id().into());
        let pos = viewport.pos();
        let size = viewport.size();

        let mut window_flags = WindowFlags::MENU_BAR | WindowFlags::NO_DOCKING;
        window_flags |= WindowFlags::NO_TITLE_BAR
            | WindowFlags::NO_COLLAPSE
            | WindowFlags::NO_RESIZE
            | WindowFlags::NO_MOVE
            | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
            | WindowFlags::NO_NAV_FOCUS;

        // Zero rounding/border and remove padding for a clean host window
        let rounding = ui.push_style_var(StyleVar::WindowRounding(0.0));
        let border = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
        let padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));

        ui.window("DockSpace Demo")
            .flags(window_flags)
            .position(pos, Condition::Always)
            .size(size, Condition::Always)
            .build(|| {
                // Pop padding/border/rounding to restore defaults
                padding.pop();
                border.pop();
                rounding.pop();

                let dockspace_id = ui.get_id("MyDockspace");
                // Configure DockBuilder only once (if node doesn't exist yet)
                if !DockBuilder::node_exists(&ui, dockspace_id) {
                    DockBuilder::remove_node(dockspace_id);
                    DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
                    DockBuilder::set_node_size(dockspace_id, size);

                    let mut dock_main_id = dockspace_id;
                    let (dock_id_left, new_main) =
                        DockBuilder::split_node(dock_main_id, SplitDirection::Left, 0.20);
                    dock_main_id = new_main;

                    let (dock_id_right, new_main) =
                        DockBuilder::split_node(dock_main_id, SplitDirection::Right, 0.20);
                    dock_main_id = new_main;

                    let (dock_id_bottom, new_main) =
                        DockBuilder::split_node(dock_main_id, SplitDirection::Down, 0.20);
                    dock_main_id = new_main;

                    DockBuilder::dock_window("James_1", dock_id_left);
                    DockBuilder::dock_window("James_2", dock_main_id);
                    DockBuilder::dock_window("James_3", dock_id_right);
                    DockBuilder::dock_window("James_4", dock_id_bottom);
                    DockBuilder::finish(dockspace_id);
                }

                // Render DockSpace inside the host window
                let color = ui.push_style_color(StyleColor::DockingEmptyBg, [1.0, 0.0, 0.0, 1.0]);
                let _ = ui.dock_space(dockspace_id, [0.0, 0.0]);
                color.pop();
            });

        // 2) Create docked windows
        ui.window("James_1").build(|| ui.text("Text 1"));
        ui.window("James_2").build(|| ui.text("Text 2"));
        ui.window("James_3").build(|| ui.text("Text 3"));
        ui.window("James_4").build(|| ui.text("Text 4"));

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

        // Pass to ImGui
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
