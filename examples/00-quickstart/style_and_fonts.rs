//! Style and Fonts (single file, quickstart)
//! - Theme switching: Dark / Light / Classic + styled presets (modern dark, Catppuccin Mocha, etc.)
//! - StyleVar push/pop demo (temporary overrides)
//! - Font loading and merging (Chinese/Emoji) — optional assets
//! - Global font scaling (FontScaleMain) and rounding sliders

use std::ffi::CStr;
use std::{fs, num::NonZeroU32, path::PathBuf, sync::Arc, time::Instant};

use dear_imgui_glow::GlowRenderer;
use dear_imgui_rs::*;
use dear_imgui_rs::{ColorOverride, FontConfig, FontLoaderFlags, FontSource, ThemePreset};
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
enum AppTheme {
    Dark,
    Light,
    Classic,
    CorporateBlue,
    ModernDark,      // Based on an ImGui styling snippet (blue-accent dark theme)
    CatppuccinMocha, // Based on the Catppuccin Mocha ImGui theme
    Darcula,         // Darcula-style theme (JetBrains-like)
    Cherry,          // Cherry red theme (classic ImGui "Cherry" style)
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
    theme: AppTheme,
    style_demo_alpha: f32,
    style_demo_rounding: f32,
    font_scale: f32,
    cjk_loaded: bool,
    emoji_loaded: bool,
    status: String,
    pending_theme: Option<AppTheme>,
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
            theme: AppTheme::Dark,
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

    fn apply_theme_now(&mut self, t: AppTheme) {
        // Use the high-level Theme API to apply presets + overrides.
        let mut cfg = dear_imgui_rs::Theme::default();
        match t {
            AppTheme::Dark => {
                cfg.preset = ThemePreset::Dark;
            }
            AppTheme::Light => {
                cfg.preset = ThemePreset::Light;
            }
            AppTheme::Classic => {
                cfg.preset = ThemePreset::Classic;
            }
            AppTheme::CorporateBlue => {
                // Base dark preset, then override a few accent colors.
                cfg.preset = ThemePreset::Dark;
                cfg.colors = vec![
                    ColorOverride {
                        id: StyleColor::Header,
                        rgba: [0.2, 0.48, 0.78, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::HeaderHovered,
                        rgba: [0.26, 0.56, 0.86, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::HeaderActive,
                        rgba: [0.18, 0.42, 0.72, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::Button,
                        rgba: [0.2, 0.48, 0.78, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::ButtonHovered,
                        rgba: [0.26, 0.56, 0.86, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::ButtonActive,
                        rgba: [0.18, 0.42, 0.72, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrab,
                        rgba: [0.2, 0.48, 0.78, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrabActive,
                        rgba: [0.26, 0.56, 0.86, 1.0],
                    },
                ];
                cfg.style.tab_rounding = Some(4.0);
            }
            AppTheme::ModernDark => {
                // Modern dark theme with blue accents, inspired by a snippet from
                // https://github.com/ocornut/imgui/issues/707
                cfg.preset = ThemePreset::None;
                cfg.colors = vec![
                    ColorOverride {
                        id: StyleColor::Text,
                        rgba: [0.92, 0.93, 0.94, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TextDisabled,
                        rgba: [0.50, 0.52, 0.54, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::WindowBg,
                        rgba: [0.14, 0.14, 0.16, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ChildBg,
                        rgba: [0.16, 0.16, 0.18, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::PopupBg,
                        rgba: [0.18, 0.18, 0.20, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::Border,
                        rgba: [0.28, 0.29, 0.30, 0.60],
                    },
                    ColorOverride {
                        id: StyleColor::BorderShadow,
                        rgba: [0.00, 0.00, 0.00, 0.00],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBg,
                        rgba: [0.20, 0.22, 0.24, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgHovered,
                        rgba: [0.22, 0.24, 0.26, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgActive,
                        rgba: [0.24, 0.26, 0.28, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TitleBg,
                        rgba: [0.14, 0.14, 0.16, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgActive,
                        rgba: [0.16, 0.16, 0.18, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgCollapsed,
                        rgba: [0.14, 0.14, 0.16, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::MenuBarBg,
                        rgba: [0.20, 0.20, 0.22, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarBg,
                        rgba: [0.16, 0.16, 0.18, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrab,
                        rgba: [0.24, 0.26, 0.28, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabHovered,
                        rgba: [0.28, 0.30, 0.32, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabActive,
                        rgba: [0.32, 0.34, 0.36, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::CheckMark,
                        rgba: [0.46, 0.56, 0.66, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrab,
                        rgba: [0.36, 0.46, 0.56, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrabActive,
                        rgba: [0.40, 0.50, 0.60, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::Button,
                        rgba: [0.24, 0.34, 0.44, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ButtonHovered,
                        rgba: [0.28, 0.38, 0.48, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ButtonActive,
                        rgba: [0.32, 0.42, 0.52, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::Header,
                        rgba: [0.24, 0.34, 0.44, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::HeaderHovered,
                        rgba: [0.28, 0.38, 0.48, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::HeaderActive,
                        rgba: [0.32, 0.42, 0.52, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::Separator,
                        rgba: [0.28, 0.29, 0.30, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorHovered,
                        rgba: [0.46, 0.56, 0.66, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorActive,
                        rgba: [0.46, 0.56, 0.66, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGrip,
                        rgba: [0.36, 0.46, 0.56, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripHovered,
                        rgba: [0.40, 0.50, 0.60, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripActive,
                        rgba: [0.44, 0.54, 0.64, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::Tab,
                        rgba: [0.20, 0.22, 0.24, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TabHovered,
                        rgba: [0.28, 0.38, 0.48, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TabSelected,
                        rgba: [0.24, 0.34, 0.44, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TabDimmed,
                        rgba: [0.20, 0.22, 0.24, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TabDimmedSelected,
                        rgba: [0.24, 0.34, 0.44, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::PlotLines,
                        rgba: [0.46, 0.56, 0.66, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::PlotLinesHovered,
                        rgba: [0.46, 0.56, 0.66, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogram,
                        rgba: [0.36, 0.46, 0.56, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogramHovered,
                        rgba: [0.40, 0.50, 0.60, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TableHeaderBg,
                        rgba: [0.20, 0.22, 0.24, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TableBorderStrong,
                        rgba: [0.28, 0.29, 0.30, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TableBorderLight,
                        rgba: [0.24, 0.25, 0.26, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TableRowBg,
                        rgba: [0.20, 0.22, 0.24, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TableRowBgAlt,
                        rgba: [0.22, 0.24, 0.26, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::TextSelectedBg,
                        rgba: [0.24, 0.34, 0.44, 0.35],
                    },
                    ColorOverride {
                        id: StyleColor::DragDropTarget,
                        rgba: [0.46, 0.56, 0.66, 0.90],
                    },
                    ColorOverride {
                        id: StyleColor::NavCursor,
                        rgba: [0.46, 0.56, 0.66, 1.00],
                    },
                    ColorOverride {
                        id: StyleColor::NavWindowingHighlight,
                        rgba: [1.00, 1.00, 1.00, 0.70],
                    },
                    ColorOverride {
                        id: StyleColor::NavWindowingDimBg,
                        rgba: [0.80, 0.80, 0.80, 0.20],
                    },
                    ColorOverride {
                        id: StyleColor::ModalWindowDimBg,
                        rgba: [0.80, 0.80, 0.80, 0.35],
                    },
                ];
                cfg.style.window_padding = Some([8.0, 8.0]);
                cfg.style.frame_padding = Some([5.0, 2.0]);
                cfg.style.cell_padding = Some([6.0, 6.0]);
                cfg.style.item_spacing = Some([6.0, 6.0]);
                cfg.style.item_inner_spacing = Some([6.0, 6.0]);
                cfg.style.indent_spacing = Some(25.0);
                cfg.style.scrollbar_size = Some(11.0);
                cfg.style.grab_min_size = Some(10.0);
                cfg.style.window_border_size = Some(1.0);
                cfg.style.child_border_size = Some(1.0);
                cfg.style.popup_border_size = Some(1.0);
                cfg.style.frame_border_size = Some(1.0);
                cfg.style.tab_border_size = Some(1.0);
                cfg.style.window_rounding = Some(7.0);
                cfg.style.child_rounding = Some(4.0);
                cfg.style.frame_rounding = Some(3.0);
                cfg.style.popup_rounding = Some(4.0);
                cfg.style.scrollbar_rounding = Some(9.0);
                cfg.style.grab_rounding = Some(3.0);
                cfg.style.tab_rounding = Some(4.0);
            }
            AppTheme::CatppuccinMocha => {
                // Catppuccin Mocha palette, ported from:
                // https://github.com/catppuccin/catppuccin (community ImGui theme snippets)
                cfg.preset = ThemePreset::None;
                let base = [0.117, 0.117, 0.172, 1.0];
                let mantle = [0.109, 0.109, 0.156, 1.0];
                let surface0 = [0.200, 0.207, 0.286, 1.0];
                let surface1 = [0.247, 0.254, 0.337, 1.0];
                let surface2 = [0.290, 0.301, 0.388, 1.0];
                let overlay0 = [0.396, 0.403, 0.486, 1.0];
                let overlay2 = [0.576, 0.584, 0.654, 1.0];
                let text = [0.803, 0.815, 0.878, 1.0];
                let subtext0 = [0.639, 0.658, 0.764, 1.0];
                let mauve = [0.796, 0.698, 0.972, 1.0];
                let peach = [0.980, 0.709, 0.572, 1.0];
                let yellow = [0.980, 0.913, 0.596, 1.0];
                let green = [0.650, 0.890, 0.631, 1.0];
                let teal = [0.580, 0.886, 0.819, 1.0];
                let sapphire = [0.458, 0.784, 0.878, 1.0];
                let blue = [0.533, 0.698, 0.976, 1.0];
                let _lavender = [0.709, 0.764, 0.980, 1.0];

                cfg.colors = vec![
                    ColorOverride {
                        id: StyleColor::WindowBg,
                        rgba: base,
                    },
                    ColorOverride {
                        id: StyleColor::ChildBg,
                        rgba: base,
                    },
                    ColorOverride {
                        id: StyleColor::PopupBg,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::Border,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::BorderShadow,
                        rgba: [0.0, 0.0, 0.0, 0.0],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBg,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgHovered,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgActive,
                        rgba: surface2,
                    },
                    ColorOverride {
                        id: StyleColor::TitleBg,
                        rgba: mantle,
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgActive,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgCollapsed,
                        rgba: mantle,
                    },
                    ColorOverride {
                        id: StyleColor::MenuBarBg,
                        rgba: mantle,
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarBg,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrab,
                        rgba: surface2,
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabHovered,
                        rgba: overlay0,
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabActive,
                        rgba: overlay2,
                    },
                    ColorOverride {
                        id: StyleColor::CheckMark,
                        rgba: green,
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrab,
                        rgba: sapphire,
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrabActive,
                        rgba: blue,
                    },
                    ColorOverride {
                        id: StyleColor::Button,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::ButtonHovered,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::ButtonActive,
                        rgba: surface2,
                    },
                    ColorOverride {
                        id: StyleColor::Header,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::HeaderHovered,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::HeaderActive,
                        rgba: surface2,
                    },
                    ColorOverride {
                        id: StyleColor::Separator,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorHovered,
                        rgba: mauve,
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorActive,
                        rgba: mauve,
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGrip,
                        rgba: surface2,
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripHovered,
                        rgba: mauve,
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripActive,
                        rgba: mauve,
                    },
                    ColorOverride {
                        id: StyleColor::Tab,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::TabHovered,
                        rgba: surface2,
                    },
                    ColorOverride {
                        id: StyleColor::TabSelected,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::TabDimmed,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::TabDimmedSelected,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::DockingPreview,
                        rgba: sapphire,
                    },
                    ColorOverride {
                        id: StyleColor::DockingEmptyBg,
                        rgba: base,
                    },
                    ColorOverride {
                        id: StyleColor::PlotLines,
                        rgba: blue,
                    },
                    ColorOverride {
                        id: StyleColor::PlotLinesHovered,
                        rgba: peach,
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogram,
                        rgba: teal,
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogramHovered,
                        rgba: green,
                    },
                    ColorOverride {
                        id: StyleColor::TableHeaderBg,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::TableBorderStrong,
                        rgba: surface1,
                    },
                    ColorOverride {
                        id: StyleColor::TableBorderLight,
                        rgba: surface0,
                    },
                    ColorOverride {
                        id: StyleColor::TableRowBg,
                        rgba: [0.0, 0.0, 0.0, 0.0],
                    },
                    ColorOverride {
                        id: StyleColor::TableRowBgAlt,
                        rgba: [1.0, 1.0, 1.0, 0.06],
                    },
                    ColorOverride {
                        id: StyleColor::TextSelectedBg,
                        rgba: surface2,
                    },
                    ColorOverride {
                        id: StyleColor::DragDropTarget,
                        rgba: yellow,
                    },
                    ColorOverride {
                        id: StyleColor::NavWindowingHighlight,
                        rgba: [1.0, 1.0, 1.0, 0.7],
                    },
                    ColorOverride {
                        id: StyleColor::NavWindowingDimBg,
                        rgba: [0.8, 0.8, 0.8, 0.2],
                    },
                    ColorOverride {
                        id: StyleColor::ModalWindowDimBg,
                        rgba: [0.0, 0.0, 0.0, 0.35],
                    },
                    ColorOverride {
                        id: StyleColor::Text,
                        rgba: text,
                    },
                    ColorOverride {
                        id: StyleColor::TextDisabled,
                        rgba: subtext0,
                    },
                ];

                cfg.style.window_rounding = Some(6.0);
                cfg.style.child_rounding = Some(6.0);
                cfg.style.frame_rounding = Some(4.0);
                cfg.style.popup_rounding = Some(4.0);
                cfg.style.scrollbar_rounding = Some(9.0);
                cfg.style.grab_rounding = Some(4.0);
                cfg.style.tab_rounding = Some(4.0);

                cfg.style.window_padding = Some([8.0, 8.0]);
                cfg.style.frame_padding = Some([5.0, 3.0]);
                cfg.style.item_spacing = Some([8.0, 4.0]);
                cfg.style.item_inner_spacing = Some([4.0, 4.0]);
                cfg.style.indent_spacing = Some(21.0);
                cfg.style.scrollbar_size = Some(14.0);
                cfg.style.grab_min_size = Some(10.0);

                cfg.style.window_border_size = Some(1.0);
                cfg.style.child_border_size = Some(1.0);
                cfg.style.popup_border_size = Some(1.0);
                cfg.style.frame_border_size = Some(0.0);
                cfg.style.tab_border_size = Some(0.0);
            }
            AppTheme::Darcula => {
                // Darcula-style theme, adapted from common ImGui Darcula snippets.
                cfg.preset = ThemePreset::None;
                cfg.colors = vec![
                    ColorOverride {
                        id: StyleColor::Text,
                        rgba: [0.73333335, 0.73333335, 0.73333335, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::TextDisabled,
                        rgba: [0.34509805, 0.34509805, 0.34509805, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::WindowBg,
                        rgba: [0.23529413, 0.24705884, 0.25490198, 0.94],
                    },
                    ColorOverride {
                        id: StyleColor::ChildBg,
                        rgba: [0.23529413, 0.24705884, 0.25490198, 0.0],
                    },
                    ColorOverride {
                        id: StyleColor::PopupBg,
                        rgba: [0.23529413, 0.24705884, 0.25490198, 0.94],
                    },
                    ColorOverride {
                        id: StyleColor::Border,
                        rgba: [0.33333334, 0.33333334, 0.33333334, 0.50],
                    },
                    ColorOverride {
                        id: StyleColor::BorderShadow,
                        rgba: [0.15686275, 0.15686275, 0.15686275, 0.0],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBg,
                        rgba: [0.16862746, 0.16862746, 0.16862746, 0.54],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgHovered,
                        rgba: [0.453125, 0.67578125, 0.99609375, 0.67],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgActive,
                        rgba: [0.47058827, 0.47058827, 0.47058827, 0.67],
                    },
                    ColorOverride {
                        id: StyleColor::TitleBg,
                        rgba: [0.04, 0.04, 0.04, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgCollapsed,
                        rgba: [0.16, 0.29, 0.48, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgActive,
                        rgba: [0.0, 0.0, 0.0, 0.51],
                    },
                    ColorOverride {
                        id: StyleColor::MenuBarBg,
                        rgba: [0.27058825, 0.28627452, 0.2901961, 0.80],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarBg,
                        rgba: [0.27058825, 0.28627452, 0.2901961, 0.60],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrab,
                        rgba: [0.21960786, 0.30980393, 0.41960788, 0.51],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabHovered,
                        rgba: [0.21960786, 0.30980393, 0.41960788, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabActive,
                        rgba: [0.13725491, 0.19215688, 0.2627451, 0.91],
                    },
                    ColorOverride {
                        id: StyleColor::CheckMark,
                        rgba: [0.90, 0.90, 0.90, 0.83],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrab,
                        rgba: [0.70, 0.70, 0.70, 0.62],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrabActive,
                        rgba: [0.30, 0.30, 0.30, 0.84],
                    },
                    ColorOverride {
                        id: StyleColor::Button,
                        rgba: [0.33333334, 0.3529412, 0.36078432, 0.49],
                    },
                    ColorOverride {
                        id: StyleColor::ButtonHovered,
                        rgba: [0.21960786, 0.30980393, 0.41960788, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::ButtonActive,
                        rgba: [0.13725491, 0.19215688, 0.2627451, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::Header,
                        rgba: [0.33333334, 0.3529412, 0.36078432, 0.53],
                    },
                    ColorOverride {
                        id: StyleColor::HeaderHovered,
                        rgba: [0.453125, 0.67578125, 0.99609375, 0.67],
                    },
                    ColorOverride {
                        id: StyleColor::HeaderActive,
                        rgba: [0.47058827, 0.47058827, 0.47058827, 0.67],
                    },
                    ColorOverride {
                        id: StyleColor::Separator,
                        rgba: [0.31640625, 0.31640625, 0.31640625, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorHovered,
                        rgba: [0.31640625, 0.31640625, 0.31640625, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorActive,
                        rgba: [0.31640625, 0.31640625, 0.31640625, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGrip,
                        rgba: [1.0, 1.0, 1.0, 0.85],
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripHovered,
                        rgba: [1.0, 1.0, 1.0, 0.60],
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripActive,
                        rgba: [1.0, 1.0, 1.0, 0.90],
                    },
                    ColorOverride {
                        id: StyleColor::PlotLines,
                        rgba: [0.61, 0.61, 0.61, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::PlotLinesHovered,
                        rgba: [1.0, 0.43, 0.35, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogram,
                        rgba: [0.90, 0.70, 0.00, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogramHovered,
                        rgba: [1.0, 0.60, 0.00, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::TextSelectedBg,
                        rgba: [0.18431373, 0.39607847, 0.79215693, 0.90],
                    },
                ];

                cfg.style.window_rounding = Some(5.3);
                cfg.style.grab_rounding = Some(2.3);
                cfg.style.frame_rounding = Some(2.3);
                cfg.style.scrollbar_rounding = Some(5.0);
                cfg.style.frame_border_size = Some(1.0);
                cfg.style.item_spacing = Some([8.0, 6.5]);
            }
            AppTheme::Cherry => {
                // Cherry red theme, ported from the classic ImGui "Cherry" style example.
                cfg.preset = ThemePreset::None;
                let hi = |v: f32| [0.502, 0.075, 0.256, v];
                let med = |v: f32| [0.455, 0.198, 0.301, v];
                let low = |v: f32| [0.232, 0.201, 0.271, v];
                let bg = |v: f32| [0.200, 0.220, 0.270, v];
                let text = |v: f32| [0.860, 0.930, 0.890, v];

                cfg.colors = vec![
                    ColorOverride {
                        id: StyleColor::Text,
                        rgba: text(0.78),
                    },
                    ColorOverride {
                        id: StyleColor::TextDisabled,
                        rgba: text(0.28),
                    },
                    ColorOverride {
                        id: StyleColor::WindowBg,
                        rgba: [0.13, 0.14, 0.17, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::ChildBg,
                        rgba: bg(0.58),
                    },
                    ColorOverride {
                        id: StyleColor::PopupBg,
                        rgba: bg(0.9),
                    },
                    ColorOverride {
                        id: StyleColor::BorderShadow,
                        rgba: [0.0, 0.0, 0.0, 0.0],
                    },
                    ColorOverride {
                        id: StyleColor::FrameBg,
                        rgba: bg(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgHovered,
                        rgba: med(0.78),
                    },
                    ColorOverride {
                        id: StyleColor::FrameBgActive,
                        rgba: med(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::TitleBg,
                        rgba: low(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgActive,
                        rgba: hi(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::TitleBgCollapsed,
                        rgba: bg(0.75),
                    },
                    ColorOverride {
                        id: StyleColor::MenuBarBg,
                        rgba: bg(0.47),
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarBg,
                        rgba: bg(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrab,
                        rgba: [0.09, 0.15, 0.16, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabHovered,
                        rgba: med(0.78),
                    },
                    ColorOverride {
                        id: StyleColor::ScrollbarGrabActive,
                        rgba: med(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::CheckMark,
                        rgba: [0.71, 0.22, 0.27, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrab,
                        rgba: [0.47, 0.77, 0.83, 0.14],
                    },
                    ColorOverride {
                        id: StyleColor::SliderGrabActive,
                        rgba: [0.71, 0.22, 0.27, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::Button,
                        rgba: [0.47, 0.77, 0.83, 0.14],
                    },
                    ColorOverride {
                        id: StyleColor::ButtonHovered,
                        rgba: med(0.86),
                    },
                    ColorOverride {
                        id: StyleColor::ButtonActive,
                        rgba: med(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::Header,
                        rgba: med(0.76),
                    },
                    ColorOverride {
                        id: StyleColor::HeaderHovered,
                        rgba: med(0.86),
                    },
                    ColorOverride {
                        id: StyleColor::HeaderActive,
                        rgba: hi(1.0),
                    },
                    // Legacy column colors map well to modern separator colors.
                    ColorOverride {
                        id: StyleColor::Separator,
                        rgba: [0.14, 0.16, 0.19, 1.0],
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorHovered,
                        rgba: med(0.78),
                    },
                    ColorOverride {
                        id: StyleColor::SeparatorActive,
                        rgba: med(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGrip,
                        rgba: [0.47, 0.77, 0.83, 0.04],
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripHovered,
                        rgba: med(0.78),
                    },
                    ColorOverride {
                        id: StyleColor::ResizeGripActive,
                        rgba: med(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::PlotLines,
                        rgba: text(0.63),
                    },
                    ColorOverride {
                        id: StyleColor::PlotLinesHovered,
                        rgba: med(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogram,
                        rgba: text(0.63),
                    },
                    ColorOverride {
                        id: StyleColor::PlotHistogramHovered,
                        rgba: med(1.0),
                    },
                    ColorOverride {
                        id: StyleColor::TextSelectedBg,
                        rgba: med(0.43),
                    },
                    ColorOverride {
                        id: StyleColor::ModalWindowDimBg,
                        rgba: bg(0.73),
                    },
                    // Final border color tweak from the original snippet.
                    ColorOverride {
                        id: StyleColor::Border,
                        rgba: [0.539, 0.479, 0.255, 0.162],
                    },
                ];

                cfg.style.window_padding = Some([6.0, 4.0]);
                cfg.style.window_rounding = Some(0.0);
                cfg.style.frame_padding = Some([5.0, 2.0]);
                cfg.style.frame_rounding = Some(3.0);
                cfg.style.item_spacing = Some([7.0, 1.0]);
                cfg.style.item_inner_spacing = Some([1.0, 1.0]);
                cfg.style.indent_spacing = Some(6.0);
                cfg.style.scrollbar_size = Some(12.0);
                cfg.style.scrollbar_rounding = Some(16.0);
                cfg.style.grab_min_size = Some(20.0);
                cfg.style.grab_rounding = Some(2.0);
                cfg.style.frame_border_size = Some(0.0);
                cfg.style.window_border_size = Some(1.0);
            }
        }
        cfg.apply_to_context(&mut self.imgui.context);
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

        let mut theme_change: Option<AppTheme> = None;
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
                let themes = [
                    "Dark",
                    "Light",
                    "Classic",
                    "Corporate Blue",
                    "Modern Dark",
                    "Catppuccin Mocha",
                    "Darcula",
                    "Cherry",
                ];
                let mut current = match self.theme {
                    AppTheme::Dark => 0,
                    AppTheme::Light => 1,
                    AppTheme::Classic => 2,
                    AppTheme::CorporateBlue => 3,
                    AppTheme::ModernDark => 4,
                    AppTheme::CatppuccinMocha => 5,
                    AppTheme::Darcula => 6,
                    AppTheme::Cherry => 7,
                };
                if let Some(_c) = ui.begin_combo("Theme##demo", themes[current]) {
                    for (i, &name) in themes.iter().enumerate() {
                        if ui.selectable_config(&format!("{}##demo", name)).selected(i==current).build() { current = i; }
                    }
                }
                let new_theme = match current {
                    0 => AppTheme::Dark,
                    1 => AppTheme::Light,
                    2 => AppTheme::Classic,
                    3 => AppTheme::CorporateBlue,
                    4 => AppTheme::ModernDark,
                    5 => AppTheme::CatppuccinMocha,
                    6 => AppTheme::Darcula,
                    _ => AppTheme::Cherry,
                };
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
