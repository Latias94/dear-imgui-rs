//! Style and Fonts (single file, quickstart)
//! - Theme switching: Dark / Light / Classic / Corporate Blue
//! - StyleVar push/pop demo (temporary overrides)
//! - Font loading and merging (Chinese/Emoji) — optional assets
//! - Global font scaling (FontScaleMain) and rounding sliders

use std::ffi::CStr;
use std::{fs, num::NonZeroU32, path::PathBuf, sync::Arc, time::Instant};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::internal::RawWrapper;
use dear_imgui_rs::*;
use dear_imgui_rs::{FontConfig, FontLoaderFlags, FontSource};
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Theme {
    Dark,
    Light,
    Classic,
    CorporateBlue,
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
    theme: Theme,
    style_demo_alpha: f32,
    style_demo_rounding: f32,
    font_scale: f32,
    cjk_loaded: bool,
    emoji_loaded: bool,
    status: String,
    pending_theme: Option<Theme>,
    pending_load_cjk: bool,
    pending_load_emoji: bool,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn freetype_active() -> bool {
        unsafe {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            if io.is_null() {
                return false;
            }
            let atlas = (*io).Fonts;
            if atlas.is_null() {
                return false;
            }
            let name_ptr = (*atlas).FontLoaderName;
            if name_ptr.is_null() {
                return false;
            }
            match CStr::from_ptr(name_ptr).to_str() {
                Ok(n) => n.eq_ignore_ascii_case("FreeType"),
                Err(_) => false,
            }
        }
    }
    // Heuristic: check whether a font buffer looks like a stb_truetype-compatible TrueType
    // - Accept: sfntVersion 0x00010000 and presence of 'glyf' + 'loca' tables
    // - Reject: 'OTTO' (CFF OTF), 'ttcf' (TrueType Collection), missing glyf/loca
    fn is_ttf_stb_compatible(data: &[u8]) -> bool {
        if Self::freetype_active() {
            return true;
        }
        if data.len() < 12 {
            return false;
        }
        let tag_u32 = |b: &[u8]| -> u32 { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) };
        let sfnt = tag_u32(&data[0..4]);
        const TAG_OTTO: u32 = 0x4F54544Fu32; // 'OTTO'
        const TAG_TTCF: u32 = 0x74746366u32; // 'ttcf'
        const TAG_TRUE: u32 = 0x74727565u32; // 'true' (old Macintosh TrueType)
        // Accept only classic TrueType (0x00010000) or 'true'. Reject CFF and TTC here.
        if !(sfnt == 0x00010000 || sfnt == TAG_TRUE) {
            return false;
        }
        if sfnt == TAG_OTTO || sfnt == TAG_TTCF {
            return false;
        }

        let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
        let table_dir_offset = 12usize;
        let DirEntrySize = 16usize;
        if data.len() < table_dir_offset + num_tables * DirEntrySize {
            return false;
        }
        let mut has_glyf = false;
        let mut has_loca = false;
        for i in 0..num_tables {
            let off = table_dir_offset + i * DirEntrySize;
            let tag = tag_u32(&data[off..off + 4]);
            match tag {
                0x676C7966u32 => has_glyf = true, // 'glyf'
                0x6C6F6361u32 => has_loca = true, // 'loca'
                _ => {}
            }
        }
        has_glyf && has_loca
    }
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Window + GL
        let window_attributes = winit::window::Window::default_attributes()
            .with_title("Dear ImGui - Style & Fonts")
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

        // ImGui
        let mut context_imgui = Context::create();
        context_imgui.set_ini_filename(None::<String>).unwrap();
        let mut platform = WinitPlatform::new(&mut context_imgui);
        platform.attach_window(
            &window,
            dear_imgui_winit::HiDpiMode::Default,
            &mut context_imgui,
        );

        // Fonts: select FreeType loader if available, then add default font
        {
            // If FreeType is compiled and linked on the C++ side, select it now
            // so the atlas uses it (enables color emoji, OTF/CFF, etc.).
            // Note: We detect FreeType at runtime and enable color glyph flags when merging
            // emoji, but we don't switch loaders automatically here because some builds of
            // dear-imgui-sys ship prebuilt cimgui without the FreeType loader symbol.

            let mut fonts = context_imgui.fonts();
            let _id = fonts.add_font(&[FontSource::DefaultFontData {
                size_pixels: Some(16.0),
                config: None,
            }]);
            // build atlas; renderer.new_frame() will pick up changes too
            let mut fonts = context_imgui.fonts();
            fonts.build();
        }

        // Renderer
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                context.display().get_proc_address(s).cast()
            })
        };
        let mut renderer = GlowRenderer::new(gl, &mut context_imgui)?;
        renderer.set_framebuffer_srgb_enabled(false);
        renderer.new_frame()?;

        let imgui = ImguiState {
            context: context_imgui,
            platform,
            renderer,
            last_frame: Instant::now(),
        };
        Ok(Self {
            window,
            surface,
            context,
            imgui,
            theme: Theme::Dark,
            style_demo_alpha: 1.0,
            style_demo_rounding: 5.0,
            font_scale: 1.0,
            cjk_loaded: false,
            emoji_loaded: false,
            status: String::new(),
            pending_theme: None,
            pending_load_cjk: false,
            pending_load_emoji: false,
        })
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

    fn apply_theme_now(&mut self, t: Theme) {
        use dear_imgui_rs::sys;
        let st = self.imgui.context.style_mut();
        unsafe {
            let raw = st.raw_mut() as *mut _ as *mut sys::ImGuiStyle;
            match t {
                Theme::Dark => sys::igStyleColorsDark(raw),
                Theme::Light => sys::igStyleColorsLight(raw),
                Theme::Classic => sys::igStyleColorsClassic(raw),
                Theme::CorporateBlue => {
                    // Base dark then tweak key colors
                    sys::igStyleColorsDark(raw);
                    let st2 = self.imgui.context.style_mut();
                    st2.set_color(StyleColor::Header, [0.2, 0.48, 0.78, 1.0]);
                    st2.set_color(StyleColor::HeaderHovered, [0.26, 0.56, 0.86, 1.0]);
                    st2.set_color(StyleColor::HeaderActive, [0.18, 0.42, 0.72, 1.0]);
                    st2.set_color(StyleColor::Button, [0.2, 0.48, 0.78, 1.0]);
                    st2.set_color(StyleColor::ButtonHovered, [0.26, 0.56, 0.86, 1.0]);
                    st2.set_color(StyleColor::ButtonActive, [0.18, 0.42, 0.72, 1.0]);
                    st2.set_color(StyleColor::SliderGrab, [0.2, 0.48, 0.78, 1.0]);
                    st2.set_color(StyleColor::SliderGrabActive, [0.26, 0.56, 0.86, 1.0]);
                }
            }
        }
        self.theme = t;
    }

    fn try_load_font_file(&mut self, path: &str, size: f32, merge: bool) -> bool {
        let mut p = PathBuf::from(path);
        if !p.exists() {
            return false;
        }
        let data = match fs::read(&p) {
            Ok(d) => d,
            Err(_) => return false,
        };
        // Validate before calling ImGui to avoid assert on unsupported formats (e.g., OTF/CFF, color-only emoji)
        if !Self::is_ttf_stb_compatible(&data) {
            self.status = format!(
                "[WARN] Unsupported font for stb_truetype: {} (need TTF with glyf/loca; try NotoSansSC-Regular.ttf or OpenMoji-Black.ttf; or enable FreeType)",
                p.display()
            );
            return false;
        }
        let cfg = FontConfig::new().size_pixels(size).merge_mode(merge);
        let mut fonts = self.imgui.context.fonts();
        let _id = fonts.add_font(&[FontSource::TtfData {
            data: &data,
            size_pixels: Some(size),
            config: Some(cfg),
        }]);
        fonts.build();
        true
    }

    fn load_cjk_font(&mut self) {
        // Attempt several common paths under examples/assets
        let candidates: Vec<&str> = if Self::freetype_active() {
            vec![
                // FreeType path: allow OTF/TTF
                "examples/assets/NotoSansSC-Regular.ttf",
                "examples/assets/NotoSansCJKsc-Regular.otf",
            ]
        } else {
            vec![
                // STB path: prefer TTF
                "examples/assets/NotoSansSC-Regular.ttf",
            ]
        };
        let ok = candidates
            .iter()
            .any(|p| self.try_load_font_file(p, 18.0, true));
        self.cjk_loaded = ok;
        self.status = if ok {
            "[OK] CJK font merged".to_string()
        } else {
            "[WARN] CJK font not found. Place NotoSansSC-Regular.ttf under examples/assets/"
                .to_string()
        };
    }

    fn load_emoji_font(&mut self) {
        // Try common emoji font files if present
        let candidates: Vec<&str> = if Self::freetype_active() {
            vec![
                // FreeType supports color emoji fonts
                "examples/assets/emoji/NotoColorEmoji.ttf",
                "examples/assets/emoji/OpenMoji.ttf",
                "examples/assets/emoji/OpenMoji-Black.ttf",
            ]
        } else {
            vec![
                // STB path: prefer monochrome TTF variants
                "examples/assets/emoji/OpenMoji-Black.ttf",
            ]
        };
        // If FreeType is active, enable color glyph loading
        if Self::freetype_active() {
            let mut fonts = self.imgui.context.fonts();
            let cur = fonts.font_loader_flags();
            fonts.set_font_loader_flags(cur | FontLoaderFlags::LOAD_COLOR);
        }
        let ok = candidates
            .iter()
            .any(|p| self.try_load_font_file(p, 20.0, true));
        self.emoji_loaded = ok;
        self.status = if ok {
            "[OK] Emoji font merged".to_string()
        } else {
            // Better diagnostics for common cases
            let noto = PathBuf::from("examples/assets/emoji/NotoColorEmoji.ttf");
            if !Self::freetype_active() && noto.exists() {
                "[WARN] Color emoji requires FreeType loader; current loader is stb_truetype (see examples/README.md to install/link FreeType)".to_string()
            } else {
                "[WARN] Emoji font not found. Put NotoColorEmoji.ttf under examples/assets/emoji/"
                    .to_string()
            }
        };
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        // Apply deferred actions from previous frame before building Ui
        if let Some(t) = self.pending_theme.take() {
            self.apply_theme_now(t);
        }
        if self.pending_load_cjk {
            self.load_cjk_font();
            self.pending_load_cjk = false;
        }
        if self.pending_load_emoji {
            self.load_emoji_font();
            self.pending_load_emoji = false;
        }
        // Apply font scale to style before frame to avoid borrowing during UI building
        {
            let st = self.imgui.context.style_mut();
            st.set_font_scale_main(self.font_scale);
        }
        let ui = self.imgui.context.frame();
        // (No style/context mutations here; only record intents from UI.)

        let mut theme_change: Option<Theme> = None;
        let mut want_cjk = false;
        let mut want_emoji = false;

        ui.window("Style & Fonts")
                .size([840.0, 620.0], Condition::FirstUseEver)
                .build(|| {
                // Avoid ID clashes with the built-in style editor controls
                let _idscope = ui.push_id("style_demo_scope");
                // Theme
                ui.text("Theme");
                ui.separator();
                let themes = ["Dark", "Light", "Classic", "Corporate Blue"];
                let mut current = match self.theme { Theme::Dark=>0, Theme::Light=>1, Theme::Classic=>2, Theme::CorporateBlue=>3 };
                if let Some(_c) = ui.begin_combo("Theme##demo", themes[current]) {
                    for (i, &name) in themes.iter().enumerate() {
                        if ui.selectable_config(&format!("{}##demo", name)).selected(i==current).build() { current = i; }
                    }
                }
                let new_theme = match current { 0=>Theme::Dark, 1=>Theme::Light, 2=>Theme::Classic, _=>Theme::CorporateBlue };
                if new_theme != self.theme { theme_change = Some(new_theme); }

                ui.spacing();
                ui.separator();

                // StyleVar demo: temporary overrides
                ui.text("Temporary StyleVar overrides");
                ui.slider("Alpha##demo", 0.3, 1.0, &mut self.style_demo_alpha);
                ui.slider("FrameRounding##demo", 0.0, 12.0, &mut self.style_demo_rounding);
                let a = ui.push_style_var(StyleVar::Alpha(self.style_demo_alpha));
                let r = ui.push_style_var(StyleVar::FrameRounding(self.style_demo_rounding));
                ui.button("Rounded Button##demo"); ui.same_line(); ui.text("This text respects Alpha");
                r.pop(); a.pop();

                ui.spacing();
                ui.separator();

                // Global scaling (FontScaleMain)
                ui.text("Scaling");
                ui.slider("Font scale##demo", 0.8, 1.6, &mut self.font_scale);

                ui.spacing();
                ui.separator();

                // Fonts
                ui.text("Fonts (optional merge)");
                let loader = if Self::freetype_active() { "FreeType" } else { "stb_truetype" };
                ui.text_disabled(&format!("Font Loader: {}", loader));
                if ui.button("Load + Merge CJK (NotoSansSC)") { want_cjk = true; }
                ui.same_line();
                if ui.button("Load + Merge Emoji") { want_emoji = true; }
                ui.same_line();
                ui.text_disabled(&self.status);

                ui.separator();
                ui.text("Preview: 你好, 世界! こんにちは! Hello! 🙂🚀");
                ui.text("如果看不到中文或 Emoji，请点击上面的按钮加载字体，或把字体放到 examples/assets 下。");

                ui.separator();
                ui.text("Built-in Style Editor");
                let mut style_copy = ui.clone_style();
                ui.show_style_editor(&mut style_copy);
            });

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        // Clear + render
        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
        self.imgui.renderer.new_frame()?;
        self.imgui.renderer.render(&draw_data)?;
        self.surface.swap_buffers(&self.context)?;

        // Defer actions to next frame to avoid borrowing conflicts during this frame
        if let Some(t) = theme_change {
            self.pending_theme = Some(t);
        }
        if want_cjk {
            self.pending_load_cjk = true;
        }
        if want_emoji {
            self.pending_load_emoji = true;
        }
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
            Some(w) => w,
            None => return,
        };
        window.imgui.platform.handle_window_event(
            &mut window.imgui.context,
            &window.window,
            &event,
        );
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
