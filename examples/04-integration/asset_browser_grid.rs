//! Integration: Asset Browser Grid with thumbnails (single file)
//! - Scans `examples/assets/` for images (png/jpg/jpeg)
//! - Shows a responsive grid with thumbnails, selection, filter
//! - Context actions: Refresh, Reveal (logs path)

use ::image::ImageReader;
use std::{
    fs,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
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
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[derive(Clone, Debug)]
struct AssetThumb {
    path: PathBuf,
    size_px: (u32, u32),
    tex: RendererTextureId,
}

struct BrowserState {
    assets: Vec<AssetThumb>,
    selected: Option<usize>,
    filter: String,
    thumb_size: f32,
    status: String,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            assets: Vec::new(),
            selected: None,
            filter: String::new(),
            thumb_size: 128.0,
            status: String::new(),
        }
    }
}

impl BrowserState {
    fn is_supported(path: &Path) -> bool {
        match path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
        {
            Some(ext) if matches!(ext.as_str(), "png" | "jpg" | "jpeg") => true,
            _ => false,
        }
    }

    fn scan_and_load(&mut self, root: &Path, renderer: &mut GlowRenderer) {
        for error in self.release_textures(renderer) {
            eprintln!("[asset_browser] texture cleanup failed: {error}");
        }
        let mut count = 0usize;
        let mut ok = 0usize;
        let entries = match fs::read_dir(root) {
            Ok(e) => e,
            Err(e) => {
                self.status = format!("[ERR] ReadDir failed: {e}");
                return;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !Self::is_supported(&path) {
                continue;
            }
            count += 1;
            match ImageReader::open(&path) {
                Ok(reader) => match reader.decode() {
                    Ok(img) => {
                        let rgba = img.to_rgba8();
                        let (w, h) = (rgba.width(), rgba.height());
                        let data = rgba.as_raw();
                        match renderer.register_texture(w, h, TextureFormat::RGBA32, data) {
                            Ok(tex) => {
                                ok += 1;
                                self.assets.push(AssetThumb {
                                    path: path.clone(),
                                    size_px: (w, h),
                                    tex,
                                });
                            }
                            Err(e) => {
                                eprintln!("[asset_browser] register_texture failed: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[asset_browser] decode failed for {:?}: {e}", path);
                    }
                },
                Err(e) => {
                    eprintln!("[asset_browser] open failed for {:?}: {e}", path);
                }
            }
        }
        self.status = format!("Loaded {ok}/{count} images from {}", root.display());
    }

    fn release_textures(&mut self, renderer: &mut GlowRenderer) -> Vec<String> {
        let mut errors = Vec::new();
        let mut retained = Vec::new();
        for asset in self.assets.drain(..) {
            if let Err(error) = renderer.unregister_texture(asset.tex) {
                errors.push(error.to_string());
                retained.push(asset);
            }
        }
        self.assets = retained;
        errors
    }
}

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
            eprintln!("Asset browser fallback context unbind failed: {error}");
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
    let mut errors = vec![format!("Asset browser initialization failed: {cause}")];
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
    browser: BrowserState,
    root: PathBuf,
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
            .with_title("Dear ImGui - Asset Browser (Integration)")
            .with_inner_size(LogicalSize::new(1200.0, 800.0));
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
                NonZeroU32::new(1200).unwrap(),
                NonZeroU32::new(800).unwrap(),
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

        let mut app = Self {
            imgui: ImguiState {
                renderer,
                platform,
                last_frame: Instant::now(),
                renderer_shutdown_complete: false,
                platform_shutdown_complete: false,
                context: imgui_context,
            },
            browser: BrowserState::default(),
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
            gl_context,
            surface,
            window,
        };
        app.browser
            .scan_and_load(&app.root, &mut app.imgui.renderer);
        Ok(app)
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.context.end_frame();
        let mut errors = self.browser.release_textures(&mut self.imgui.renderer);

        if !self.imgui.renderer_shutdown_complete {
            match self.imgui.renderer.shutdown(&mut self.imgui.context) {
                Ok(()) => {
                    self.imgui.renderer_shutdown_complete = true;
                    self.browser.assets.clear();
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

    fn resize(&mut self, sz: winit::dpi::PhysicalSize<u32>) {
        if sz.width > 0 && sz.height > 0 {
            self.surface.resize(
                self.gl_context.get(),
                NonZeroU32::new(sz.width).unwrap(),
                NonZeroU32::new(sz.height).unwrap(),
            );
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&mut self.imgui.context, &self.window)?;
        let ui = self.imgui.context.frame();

        let mut want_refresh = false;
        ui.window("Asset Browser")
            .size([980.0, 720.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Asset Browser (images)");
                ui.same_line();
                if ui.button("Refresh") {
                    want_refresh = true;
                }
                ui.same_line();
                ui.text_disabled(&self.browser.status);

                ui.separator();
                let _ = ui
                    .input_text("Filter", &mut self.browser.filter)
                    .hint("substring...")
                    .build();
                ui.slider("Thumb Size", 64.0, 256.0, &mut self.browser.thumb_size);

                ui.separator();
                let avail = ui.content_region_avail();
                let pad = 12.0f32;
                let cell_w = self.browser.thumb_size + pad;
                let cols = (avail[0] / cell_w).max(1.0).floor() as i32;
                let mut cur_col = 0i32;

                ui.child_window("grid").size([0.0, 0.0]).build(&ui, || {
                    let filter = self.browser.filter.to_lowercase();
                    for (i, it) in self.browser.assets.iter().enumerate() {
                        let name = it.path.file_name().and_then(|s| s.to_str()).unwrap_or("?");
                        if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                            continue;
                        }

                        if cur_col > 0 {
                            ui.same_line();
                        }
                        ui.group(|| {
                            let aspect = it.size_px.1 as f32 / it.size_px.0 as f32;
                            let size = [
                                self.browser.thumb_size,
                                (self.browser.thumb_size * aspect).max(1.0),
                            ];
                            Image::new(ui, it.tex.texture_id(), size).build();
                            let is_sel = self.browser.selected == Some(i);
                            if ui.selectable_config(name).selected(is_sel).build() {
                                self.browser.selected = Some(i);
                            }
                            if let Some(_popup) = ui.begin_popup_context_item() {
                                if ui.menu_item("Reveal in log") {
                                    self.browser.status = format!("{}", it.path.display());
                                }
                            }
                        });

                        cur_col += 1;
                        if cur_col >= cols {
                            cur_col = 0;
                        }
                    }
                });
            });

        if want_refresh {
            self.browser
                .scan_and_load(&self.root, &mut self.imgui.renderer);
        }

        self.imgui.platform.prepare_render(&ui, &self.window)?;
        let pending_frame = self
            .imgui
            .context
            .render(self.imgui.renderer.renderer_consumer()?);
        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
        self.imgui.renderer.render(pending_frame)?;
        self.surface.swap_buffers(self.gl_context.get())?;
        Ok(())
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Asset browser fallback shutdown failed: {error}");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match AppWindow::new(event_loop) {
                Ok(w) => {
                    self.window = Some(w);
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
        let Some(window) = self.window.as_mut() else {
            return;
        };
        if let Err(error) = window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        ) {
            eprintln!("Winit platform error: {error}");
            event_loop.exit();
            return;
        }
        match event {
            WindowEvent::Resized(sz) => {
                window.resize(sz);
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
        if let Some(w) = &self.window {
            w.window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_mut()
            && let Err(error) = window.shutdown()
        {
            eprintln!("Asset browser shutdown failed: {error}");
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
