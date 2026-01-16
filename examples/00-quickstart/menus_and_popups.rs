//! Menus and Popups (single file, quickstart)
//! - Main menu bar with sub-menus and shortcut labels
//! - Window menu bar
//! - Context menu (right-click)
//! - Modal popup (Preferences)

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::WindowFlags;
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
    // UI state
    status_bar: bool,
    show_about: bool,
    prefs_open: bool,
    theme_dark: bool,
    rename_buf: String,
    // Document content/state
    sections: Vec<String>,
    selected: Option<usize>,
    confirm_delete_open: bool,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Create window + OpenGL context
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Menus and Popups")
            .with_inner_size(LogicalSize::new(1100.0, 720.0));

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
                NonZeroU32::new(720).unwrap(),
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
            status_bar: true,
            show_about: false,
            prefs_open: false,
            theme_dark: true,
            rename_buf: String::from("Untitled"),
            sections: vec!["Introduction".to_string(), "Details".to_string()],
            selected: Some(0),
            confirm_delete_open: false,
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
        // Delta time
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        // New frame
        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Main menu bar
        if let Some(_bar) = ui.begin_main_menu_bar() {
            ui.menu("File", || {
                if ui.menu_item_config("New").shortcut("Ctrl+N").build() {}
                if ui.menu_item_config("Open...").shortcut("Ctrl+O").build() {}
                if ui.menu_item_config("Save").shortcut("Ctrl+S").build() {}
                ui.separator();
                if ui.menu_item("Preferences...") {
                    self.prefs_open = true;
                    ui.open_popup("Preferences");
                }
                ui.separator();
                if ui.menu_item("Exit") {
                    // handled in event loop (Esc)
                }
            });

            ui.menu("Edit", || {
                ui.menu_item_config("Undo")
                    .shortcut("Ctrl+Z")
                    .enabled(false)
                    .build();
                ui.menu_item_config("Redo")
                    .shortcut("Ctrl+Y")
                    .enabled(false)
                    .build();
                ui.separator();
                ui.menu_item("Cut");
                ui.menu_item("Copy");
                ui.menu_item("Paste");
            });

            ui.menu("View", || {
                ui.menu_item_config("Status Bar")
                    .build_with_ref(&mut self.status_bar);
                ui.menu_item_config("Dark Theme")
                    .build_with_ref(&mut self.theme_dark);
            });

            ui.menu("Help", || {
                if ui.menu_item("About Dear ImGui...") {
                    self.show_about = true;
                }
            });
        }

        // A window with its own menu bar
        ui.window("Document")
            .size([700.0, 420.0], Condition::FirstUseEver)
            .flags(WindowFlags::MENU_BAR)
            .build(|| {
                if let Some(_m) = ui.begin_menu_bar() {
                    ui.menu("Document", || {
                        if ui.menu_item("Rename...") {
                            if let Some(i) = self.selected {
                                self.rename_buf = self.sections[i].clone();
                            }
                            ui.open_popup("RenameDoc");
                        }
                        let has_sel = self.selected.is_some();
                        if ui
                            .menu_item_config("Delete...")
                            .enabled(false)
                            .selected(has_sel)
                            .build()
                        {
                            self.confirm_delete_open = true;
                            ui.open_popup("ConfirmDelete");
                        }
                    });
                }

                // Content area with context menu
                ui.text_disabled("Right-click anywhere in this window for context menu");
                ui.separator();
                ui.child_window("doc_area")
                    .size([0.0, 260.0])
                    .build(&ui, || {
                        // List of sections with selection
                        for (i, title) in self.sections.iter().enumerate() {
                            let selected = self.selected == Some(i);
                            if ui
                                .selectable_config(format!("{}##sec{}", title, i))
                                .selected(selected)
                                .build()
                            {
                                self.selected = Some(i);
                            }
                        }
                    });

                // Window-level context menu (right-click anywhere inside "Document")
                if let Some(_popup) = ui.begin_popup_context_window() {
                    if ui.menu_item("Add Section") {
                        let name = format!("New Section {}", self.sections.len() + 1);
                        self.sections.push(name);
                        self.selected = Some(self.sections.len() - 1);
                        ui.close_current_popup();
                    }
                    let has_sel = self.selected.is_some();
                    if ui
                        .menu_item_config("Duplicate")
                        .enabled(false)
                        .selected(has_sel)
                        .build()
                    {
                        if let Some(i) = self.selected {
                            let name = format!("{} Copy", self.sections[i]);
                            self.sections.push(name);
                            self.selected = Some(self.sections.len() - 1);
                        }
                        ui.close_current_popup();
                    }
                    ui.separator();
                    if ui
                        .menu_item_config("Delete")
                        .enabled(false)
                        .selected(has_sel)
                        .build()
                    {
                        if let Some(i) = self.selected {
                            self.sections.remove(i);
                            if self.sections.is_empty() {
                                self.selected = None;
                            } else {
                                let new_i = i.min(self.sections.len() - 1);
                                self.selected = Some(new_i);
                            }
                        }
                        ui.close_current_popup();
                    }
                }

                // Rename popup
                if let Some(_popup) = ui.begin_popup("RenameDoc") {
                    ui.text("Rename document");
                    ui.input_text("Name", &mut self.rename_buf).build();
                    if ui.button("OK") {
                        if let Some(i) = self.selected {
                            self.sections[i] = self.rename_buf.clone();
                        }
                        ui.close_current_popup();
                    }
                    ui.same_line();
                    if ui.button("Cancel") {
                        ui.close_current_popup();
                    }
                }

                // Confirm delete modal
                if self.confirm_delete_open {
                    if let Some(_modal) = ui.begin_modal_popup("ConfirmDelete") {
                        ui.text("Delete this document? This action cannot be undone.");
                        let has_sel = self.selected.is_some();
                        let _ = has_sel; // informational
                        if ui.button("Delete") {
                            if let Some(i) = self.selected {
                                self.sections.remove(i);
                                if self.sections.is_empty() {
                                    self.selected = None;
                                } else {
                                    let new_i = i.min(self.sections.len() - 1);
                                    self.selected = Some(new_i);
                                }
                            }
                            self.confirm_delete_open = false;
                            ui.close_current_popup();
                        }
                        ui.same_line();
                        if ui.button("Cancel") {
                            self.confirm_delete_open = false;
                            ui.close_current_popup();
                        }
                    }
                }
            });

        // Preferences modal (from main menu)
        if self.prefs_open {
            if let Some(_modal) = ui.begin_modal_popup("Preferences") {
                ui.checkbox("Dark theme", &mut self.theme_dark);
                ui.checkbox("Show status bar", &mut self.status_bar);
                if ui.button("Close") {
                    self.prefs_open = false;
                    ui.close_current_popup();
                }
            }
        }

        // About window
        if self.show_about {
            ui.show_about_window(&mut self.show_about);
        }

        // Clear + render
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
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        );

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
