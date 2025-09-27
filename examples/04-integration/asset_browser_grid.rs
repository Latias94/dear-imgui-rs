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

#[derive(Clone, Debug)]
struct AssetThumb {
    path: PathBuf,
    size_px: (u32, u32),
    tex: TextureId,
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
        // Cleanup existing GL textures before reloading
        if let Some(gl_rc) = renderer.gl_context().cloned() {
            let tex_map = renderer.texture_map_mut();
            for asset in self.assets.drain(..) {
                if let Some(gl_tex) = tex_map.remove(asset.tex) {
                    unsafe {
                        gl_rc.delete_texture(gl_tex);
                    }
                }
            }
        } else {
            self.assets.clear();
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
}

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
    browser: BrowserState,
    root: PathBuf,
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

        let mut app = Self {
            window,
            surface,
            context,
            imgui: ImguiState {
                context: imgui_context,
                platform,
                renderer,
                last_frame: Instant::now(),
            },
            browser: BrowserState::default(),
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        };
        app.browser
            .scan_and_load(&app.root, &mut app.imgui.renderer);
        Ok(app)
    }

    fn resize(&mut self, sz: winit::dpi::PhysicalSize<u32>) {
        if sz.width > 0 && sz.height > 0 {
            self.surface.resize(
                &self.context,
                NonZeroU32::new(sz.width).unwrap(),
                NonZeroU32::new(sz.height).unwrap(),
            );
        }
    }

    fn draw_browser(&mut self, ui: &Ui) {
        ui.text("Asset Browser (images)");
        ui.same_line();
        if ui.button("Refresh") {
            self.browser
                .scan_and_load(&self.root, &mut self.imgui.renderer);
        }
        ui.same_line();
        ui.text_disabled(&self.browser.status);

        ui.separator();
        let changed = ui
            .input_text("Filter", &mut self.browser.filter)
            .hint("substring...")
            .build();
        if changed { /* live filter */ }
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
                    Image::new(ui, it.tex, size).build();
                    let is_sel = self.browser.selected == Some(i);
                    if ui.selectable_config(name).selected(is_sel).build() {
                        self.browser.selected = Some(i);
                    }
                    if let Some(_p) = ui.begin_popup_context_item() {
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
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        // Inline browser UI to avoid &Ui + &mut self borrow conflict
        ui.window("Asset Browser")
            .size([980.0, 720.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Asset Browser (images)");
                ui.same_line();
                let mut want_refresh = false;
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
                            Image::new(ui, it.tex, size).build();
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

                // Apply deferred operations after building UI
                if want_refresh {
                    // safe: we are out of child_window closure; still within Ui, but not touching Ui internals
                    // It only uses renderer, not context.
                    // We still defer actual call outside the build closure to be extra safe.
                    // Record intent by writing to status; perform after window build.
                    // We'll use a flag in outer scope: set a temporary status and perform after build.
                    // However, we're inside closure; store intent in a field, then handle below.
                    // We reuse status as feedback, real refresh happens after window render.
                    self.browser.status = "Refreshing...".to_string();
                    // Mark refresh via a sentinel negative selection (not ideal but simple):
                    // We'll actually refresh just after rendering below.
                    // Instead, do nothing here; the frame ends immediately after build.
                }
            });

        // If user pressed Refresh, perform it here (check status message)
        if self.browser.status == "Refreshing..." {
            self.browser
                .scan_and_load(&self.root, &mut self.imgui.renderer);
        }

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();
        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
        self.imgui.renderer.new_frame()?;
        self.imgui.renderer.render(&draw_data)?;
        self.surface.swap_buffers(&self.context)?;
        Ok(())
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Some(gl_rc) = self.imgui.renderer.gl_context().cloned() {
            let tex_map = self.imgui.renderer.texture_map_mut();
            for asset in self.browser.assets.drain(..) {
                if let Some(gl_tex) = tex_map.remove(asset.tex) {
                    unsafe {
                        gl_rc.delete_texture(gl_tex);
                    }
                }
            }
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
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_mut() else {
            return;
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
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
