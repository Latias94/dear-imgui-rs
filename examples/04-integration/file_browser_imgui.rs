//! ImGui-embedded File Browser example
//! - Uses dear-file-browser `FileDialogState` and `Ui` extension
//! - Works on desktop and WASM without native dialogs

use std::{num::NonZeroU32, sync::Arc, time::Instant};

use dear_file_browser::{
    DialogMode, FileDialogExt, FileDialogState, FileListViewMode, ImageThumbnailProvider,
    ThumbnailBackend, ThumbnailRenderer, ToolbarDensity, ToolbarIconMode,
};
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
    browser: FileDialogState,
    thumbnails_provider: ImageThumbnailProvider,
    status: String,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - File Browser (ImGui)")
            .with_inner_size(LogicalSize::new(1000.0, 680.0));
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
                NonZeroU32::new(680).unwrap(),
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

        let browser = {
            let mut st = FileDialogState::new(DialogMode::OpenFiles);
            st.apply_igfd_classic_preset();
            let filter =
                dear_file_browser::FileFilter::from(("Images", &["png", "jpg", "jpeg"][..]));
            st.core.set_filters(vec![filter]);
            st.ui.file_list_view = FileListViewMode::ThumbnailsList;
            st.ui.thumbnails_enabled = true;
            st.ui.file_list_columns.show_preview = true;
            st.ui.toolbar.density = ToolbarDensity::Compact;
            st.ui.toolbar.icons.mode = ToolbarIconMode::IconOnly;
            st.ui.toolbar.icons.refresh = Some("⟳".to_string());
            st.ui.toolbar.icons.new_folder = Some("+".to_string());
            st.ui.toolbar.icons.columns = Some("≡".to_string());
            st.ui.toolbar.icons.options = Some("⚙".to_string());

            // Curated places: keep System, add a few handy bookmarks.
            if let Ok(pwd) = std::env::current_dir() {
                st.core.places.add_bookmark("Repo", pwd);
            }
            st.core.places.add_bookmark("Temp", std::env::temp_dir());
            st
        };

        Ok(Self {
            window,
            surface,
            context,
            imgui: ImguiState {
                context: imgui_context,
                platform,
                renderer,
                last_frame: Instant::now(),
            },
            browser,
            thumbnails_provider: ImageThumbnailProvider::default(),
            status: String::new(),
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
        let delta = now - self.imgui.last_frame;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta.as_secs_f32());
        self.imgui.last_frame = now;

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        ui.window("File Browser (ImGui)")
            .size([680.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                let browser_open = self.browser.is_open();
                if ui.button(if browser_open {
                    "Hide Browser"
                } else {
                    "Show Browser"
                }) {
                    if browser_open {
                        self.browser.close();
                    } else {
                        self.browser.open();
                    }
                }
                ui.same_line();
                ui.text(&self.status);

                ui.separator();
                if self.browser.is_open() {
                    let gl = self
                        .imgui
                        .renderer
                        .gl_context()
                        .cloned()
                        .expect("GlowRenderer missing gl_context");
                    let mut renderer = GlowThumbnailRenderer {
                        gl: &gl,
                        texture_map: self.imgui.renderer.texture_map_mut(),
                    };
                    let mut backend = ThumbnailBackend {
                        provider: &mut self.thumbnails_provider,
                        renderer: &mut renderer,
                    };
                    if let Some(res) = ui.file_browser().draw_contents_with(
                        &mut self.browser,
                        &dear_file_browser::StdFileSystem,
                        None,
                        Some(&mut backend),
                    ) {
                        match res {
                            Ok(sel) => {
                                self.status = format!("Selected {} path(s)", sel.paths.len());
                                for p in &sel.paths {
                                    eprintln!("[selected] {}", p.display());
                                }
                            }
                            Err(e) => {
                                self.status = format!("{e}");
                            }
                        }
                    }
                }
            });

        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.06, 0.07, 0.09, 1.0);
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

struct GlowThumbnailRenderer<'a> {
    gl: &'a glow::Context,
    texture_map: &'a mut dyn dear_imgui_glow::TextureMap,
}

impl ThumbnailRenderer for GlowThumbnailRenderer<'_> {
    fn upload_rgba8(
        &mut self,
        image: &dear_file_browser::DecodedRgbaImage,
    ) -> Result<TextureId, String> {
        let gl_tex = dear_imgui_glow::create_texture_from_rgba(
            self.gl,
            image.width,
            image.height,
            &image.rgba,
        )
        .map_err(|e| format!("{e}"))?;
        Ok(self.texture_map.register_texture(
            gl_tex,
            image.width as i32,
            image.height as i32,
            dear_imgui_rs::TextureFormat::RGBA32,
        ))
    }

    fn destroy(&mut self, texture_id: TextureId) {
        let Some(gl_tex) = self.texture_map.remove(texture_id) else {
            return;
        };
        unsafe {
            self.gl.delete_texture(gl_tex);
        }
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
        let Some(w) = &mut self.window else {
            return;
        };
        if w.window.id() != window_id {
            return;
        }
        // Feed to ImGui platform first (window-local path)
        w.imgui
            .platform
            .handle_window_event(&mut w.imgui.context, &w.window, &event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => w.resize(size),
            WindowEvent::RedrawRequested => {
                if let Err(e) = w.render() {
                    eprintln!("render error: {e}");
                    event_loop.exit();
                    return;
                }
                w.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
