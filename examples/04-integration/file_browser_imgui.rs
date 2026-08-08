//! ImGui-embedded File Browser example
//! - Uses dear-file-browser `FileDialogState` and `Ui` extension
//! - Works on desktop and WASM without native dialogs

use std::{collections::HashMap, num::NonZeroU32, sync::Arc, time::Instant};

use dear_file_browser::{
    DialogMode, FileDialogExt, FileDialogState, FileListViewMode, ImageThumbnailProvider,
    ThumbnailBackend, ThumbnailRenderer, ToolbarDensity, ToolbarIconMode,
};
use dear_imgui_glow::{GlowRenderer, RendererTextureId};
use dear_imgui_rs::*;
use dear_imgui_winit::WinitPlatform;
use glow::HasContext;
use glutin::{
    config::ConfigTemplateBuilder,
    context::{
        ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext,
        PossiblyCurrentGlContext,
    },
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
    renderer: GlowRenderer,
    platform: WinitPlatform,
    last_frame: Instant,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    // Context must outlive every attachment, including fallback field drops after failed shutdown.
    context: Context,
}

struct CurrentGlContext {
    context: PossiblyCurrentContext,
    bound: bool,
}

impl CurrentGlContext {
    fn new(context: PossiblyCurrentContext) -> Self {
        Self {
            context,
            bound: true,
        }
    }

    fn get(&self) -> &PossiblyCurrentContext {
        &self.context
    }

    fn unbind(&mut self) -> glutin::error::Result<()> {
        if self.bound {
            self.context.make_not_current_in_place()?;
            self.bound = false;
        }
        Ok(())
    }
}

impl Drop for CurrentGlContext {
    fn drop(&mut self) {
        if let Err(error) = self.unbind() {
            eprintln!("File browser fallback context unbind failed: {error}");
        }
    }
}

fn initialization_failure(
    cause: impl std::fmt::Display,
    context: &mut Context,
    platform: Option<&mut WinitPlatform>,
    renderer: Option<&mut GlowRenderer>,
    gl_context: &mut CurrentGlContext,
) -> Box<dyn std::error::Error> {
    context.end_frame();
    let mut errors = vec![format!("File browser initialization failed: {cause}")];
    let mut attachments_shutdown = true;

    if let Some(renderer) = renderer
        && let Err(error) = renderer.shutdown(context)
    {
        errors.push(format!("Glow renderer rollback failed: {error}"));
        attachments_shutdown = false;
    }
    if let Some(platform) = platform
        && let Err(error) = platform.shutdown(context)
    {
        errors.push(format!("Winit platform rollback failed: {error}"));
        attachments_shutdown = false;
    }
    if attachments_shutdown && let Err(error) = gl_context.unbind() {
        errors.push(format!("OpenGL context rollback failed: {error}"));
    }

    errors.join("; ").into()
}

struct AppWindow {
    imgui: ImguiState,
    browser: FileDialogState,
    thumbnails_provider: ImageThumbnailProvider,
    thumbnail_textures: HashMap<TextureId, RendererTextureId>,
    status: String,
    gl_context: CurrentGlContext,
    surface: Surface<WindowSurface>,
    window: Arc<Window>,
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
        let mut gl_context = CurrentGlContext::new(context.make_current(&surface)?);

        let mut imgui_context = Context::create();
        if let Err(error) = imgui_context.set_ini_filename(None::<String>) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                None,
                None,
                &mut gl_context,
            ));
        }
        let mut platform = match WinitPlatform::new(&mut imgui_context) {
            Ok(platform) => platform,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut imgui_context,
                    None,
                    None,
                    &mut gl_context,
                ));
            }
        };
        if let Err(error) = platform.attach_window(
            Arc::clone(&window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut imgui_context,
        ) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                Some(&mut platform),
                None,
                &mut gl_context,
            ));
        }

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                gl_context.get().display().get_proc_address(s).cast()
            })
        };
        let mut renderer = match GlowRenderer::new(gl, &mut imgui_context) {
            Ok(renderer) => renderer,
            Err(error) => {
                return Err(initialization_failure(
                    error,
                    &mut imgui_context,
                    Some(&mut platform),
                    None,
                    &mut gl_context,
                ));
            }
        };
        if let Err(error) = renderer.set_framebuffer_srgb_enabled(false) {
            return Err(initialization_failure(
                error,
                &mut imgui_context,
                Some(&mut platform),
                Some(&mut renderer),
                &mut gl_context,
            ));
        }

        let browser = {
            let mut st = FileDialogState::new(DialogMode::OpenFiles);
            st.apply_igfd_classic_preset();
            let filter =
                dear_file_browser::FileFilter::from(("Images", &["png", "jpg", "jpeg"][..]));
            st.core.set_filters(vec![filter]);
            st.ui.config.file_list_view = FileListViewMode::ThumbnailsList;
            st.ui.config.thumbnails_enabled = true;
            st.ui.config.file_list_columns.show_preview = true;
            st.ui.config.toolbar.density = ToolbarDensity::Compact;
            st.ui.config.toolbar.icons.mode = ToolbarIconMode::IconAndText;
            st.ui.config.toolbar.icons.refresh = Some("⟳".to_string());
            st.ui.config.toolbar.icons.new_folder = Some("+".to_string());
            st.ui.config.toolbar.icons.columns = Some("≡".to_string());
            st.ui.config.toolbar.icons.options = Some("⚙".to_string());

            // Curated places: keep System, add a few handy bookmarks.
            if let Ok(pwd) = std::env::current_dir() {
                st.core.places.add_bookmark("Repo", pwd);
            }
            st.core.places.add_bookmark("Temp", std::env::temp_dir());

            // For thumbnails demo: prefer a directory that contains images in this repo.
            if let Ok(pwd) = std::env::current_dir() {
                let screenshots = pwd.join("screenshots");
                if screenshots.is_dir() {
                    st.core.set_cwd(screenshots);
                }
            }
            st
        };

        Ok(Self {
            imgui: ImguiState {
                renderer,
                platform,
                last_frame: Instant::now(),
                renderer_shutdown_complete: false,
                platform_shutdown_complete: false,
                context: imgui_context,
            },
            browser,
            thumbnails_provider: ImageThumbnailProvider::default(),
            thumbnail_textures: HashMap::new(),
            status: String::new(),
            gl_context,
            surface,
            window,
        })
    }

    fn release_thumbnail_textures(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let textures = std::mem::take(&mut self.thumbnail_textures);
        for (texture_id, texture) in textures {
            if let Err(error) = self.imgui.renderer.unregister_texture(texture) {
                errors.push(format!(
                    "Failed to release thumbnail texture {texture_id:?}: {error}"
                ));
                self.thumbnail_textures.insert(texture_id, texture);
            }
        }
        errors
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.context.end_frame();
        let mut errors = self.release_thumbnail_textures();

        if !self.imgui.renderer_shutdown_complete {
            match self.imgui.renderer.shutdown(&mut self.imgui.context) {
                Ok(()) => {
                    self.imgui.renderer_shutdown_complete = true;
                    self.thumbnail_textures.clear();
                }
                Err(error) => errors.push(format!("Glow renderer shutdown failed: {error}")),
            }
        }
        if !self.imgui.platform_shutdown_complete {
            match self.imgui.platform.shutdown(&mut self.imgui.context) {
                Ok(()) => self.imgui.platform_shutdown_complete = true,
                Err(error) => errors.push(format!("Winit platform shutdown failed: {error}")),
            }
        }
        if self.imgui.renderer_shutdown_complete
            && self.imgui.platform_shutdown_complete
            && let Err(error) = self.gl_context.unbind()
        {
            errors.push(format!("OpenGL context unbind failed: {error}"));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(std::io::Error::other(errors.join("; ")).into())
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface.resize(
                self.gl_context.get(),
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
            .prepare_frame(&mut self.imgui.context, &self.window)?;
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
                    let mut renderer = GlowThumbnailRenderer {
                        renderer: &mut self.imgui.renderer,
                        textures: &mut self.thumbnail_textures,
                    };
                    let mut backend = ThumbnailBackend {
                        provider: &mut self.thumbnails_provider,
                        renderer: &mut renderer,
                    };
                    if let Some(res) = ui.file_browser().draw_contents_with(
                        &mut self.browser,
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
        self.imgui.platform.prepare_render(&ui, &self.window)?;
        let pending_frame = self
            .imgui
            .context
            .render(self.imgui.renderer.renderer_consumer()?);
        self.imgui.renderer.render(pending_frame)?;
        self.surface.swap_buffers(self.gl_context.get())?;
        Ok(())
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("File browser fallback shutdown failed: {error}");
        }
    }
}

struct GlowThumbnailRenderer<'a> {
    renderer: &'a mut GlowRenderer,
    textures: &'a mut HashMap<TextureId, RendererTextureId>,
}

impl ThumbnailRenderer for GlowThumbnailRenderer<'_> {
    fn upload_rgba8(
        &mut self,
        image: &dear_file_browser::DecodedRgbaImage,
    ) -> Result<TextureId, String> {
        let texture = self
            .renderer
            .register_texture(
                image.width,
                image.height,
                dear_imgui_rs::TextureFormat::RGBA32,
                &image.rgba,
            )
            .map_err(|error| error.to_string())?;
        let texture_id = texture.texture_id();
        let previous = self.textures.insert(texture_id, texture);
        debug_assert!(previous.is_none(), "Glow texture ID was reused");
        Ok(texture_id)
    }

    fn destroy(&mut self, texture_id: TextureId) {
        let Some(texture) = self.textures.remove(&texture_id) else {
            eprintln!("Unknown thumbnail texture {texture_id:?}");
            return;
        };
        if let Err(error) = self.renderer.unregister_texture(texture) {
            eprintln!("Failed to release thumbnail texture {texture_id:?}: {error}");
            self.textures.insert(texture_id, texture);
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
        if let Err(error) =
            w.imgui
                .platform
                .handle_window_event(&mut w.imgui.context, &w.window, &event)
        {
            eprintln!("Winit platform error: {error}");
            event_loop.exit();
            return;
        }
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

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_mut()
            && let Err(error) = window.shutdown()
        {
            eprintln!("File browser shutdown failed: {error}");
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
