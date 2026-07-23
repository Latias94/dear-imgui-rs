//! Style and Fonts (single file, quickstart)
//! - Theme switching: Dark / Light / Classic + styled presets (modern dark, Catppuccin Mocha, etc.)
//! - StyleVar push/pop demo (temporary overrides)
//! - Font loading and merging (Chinese/Emoji) — optional assets
//! - Global font scaling (FontScaleMain) and rounding sliders

use std::ffi::CStr;
use std::{
    fs,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

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

#[path = "../support/font_validation.rs"]
mod font_validation;

const BUNDLED_ROBOTO_RELATIVE_PATH: &str =
    "../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf";
const CHECKER_SIZE: [u16; 2] = [16, 16];
const CHECKER_CELL_SIZE: usize = 4;
const ROBOTO_PREVIEW: &str = "Sphinx of black quartz, judge my vow.";

fn example_path(path: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn bundled_roboto_path() -> PathBuf {
    example_path(BUNDLED_ROBOTO_RELATIVE_PATH)
}

fn push_unique(candidates: &mut Vec<PathBuf>, path: impl Into<PathBuf>) {
    let path = path.into();
    if !candidates.contains(&path) {
        candidates.push(path);
    }
}

fn cjk_font_candidates(freetype: bool) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    push_unique(
        &mut candidates,
        example_path("assets/NotoSansSC-Regular.ttf"),
    );
    if freetype {
        push_unique(
            &mut candidates,
            example_path("assets/NotoSansCJKsc-Regular.otf"),
        );
    }

    #[cfg(target_os = "windows")]
    {
        let windows = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let fonts = windows.join("Fonts");
        for name in ["NotoSansSC-VF.ttf", "msyh.ttc", "simhei.ttf"] {
            push_unique(&mut candidates, fonts.join(name));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if freetype {
            for path in [
                "/System/Library/Fonts/PingFang.ttc",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
                "/Library/Fonts/NotoSansCJKsc-Regular.otf",
            ] {
                push_unique(&mut candidates, path);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if freetype {
            for path in [
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            ] {
                push_unique(&mut candidates, path);
            }
        }
        for path in [
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/wenquanyi/wqy-zenhei/wqy-zenhei.ttc",
        ] {
            push_unique(&mut candidates, path);
        }
    }

    candidates
}

fn emoji_font_candidates(freetype: bool) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if freetype {
        push_unique(
            &mut candidates,
            example_path("assets/emoji/NotoColorEmoji.ttf"),
        );
        push_unique(&mut candidates, example_path("assets/emoji/OpenMoji.ttf"));
    }
    push_unique(
        &mut candidates,
        example_path("assets/emoji/OpenMoji-Black.ttf"),
    );

    #[cfg(target_os = "windows")]
    {
        let windows = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let fonts = windows.join("Fonts");
        if freetype {
            push_unique(&mut candidates, fonts.join("seguiemj.ttf"));
        }
        push_unique(&mut candidates, fonts.join("seguisym.ttf"));
    }

    #[cfg(target_os = "macos")]
    {
        if freetype {
            push_unique(
                &mut candidates,
                "/System/Library/Fonts/Apple Color Emoji.ttc",
            );
        }
        push_unique(&mut candidates, "/System/Library/Fonts/Apple Symbols.ttf");
    }

    #[cfg(target_os = "linux")]
    {
        if freetype {
            for path in [
                "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
                "/usr/share/fonts/noto/NotoColorEmoji.ttf",
                "/usr/share/fonts/google-noto-color-emoji-fonts/NotoColorEmoji.ttf",
            ] {
                push_unique(&mut candidates, path);
            }
        }
        for path in [
            "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf",
            "/usr/share/fonts/truetype/ancient-scripts/Symbola_hint.ttf",
        ] {
            push_unique(&mut candidates, path);
        }
    }

    candidates
}

fn checker_pixels(inverted: bool) -> Vec<u8> {
    let width = usize::from(CHECKER_SIZE[0]);
    let height = usize::from(CHECKER_SIZE[1]);
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let alternate = ((x / CHECKER_CELL_SIZE) + (y / CHECKER_CELL_SIZE)).is_multiple_of(2);
            let color = if alternate ^ inverted {
                [40, 44, 52, 255]
            } else {
                [230, 90, 72, 255]
            };
            pixels.extend_from_slice(&color);
        }
    }
    pixels
}

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
    roboto_font: Option<FontId>,
    roboto_source: Option<PathBuf>,
    cjk_loaded: bool,
    cjk_source: Option<PathBuf>,
    emoji_loaded: bool,
    emoji_source: Option<PathBuf>,
    checker_rect: CustomRectId,
    checker_inverted: bool,
    status: String,
    pending_theme: Option<AppTheme>,
    pending_load_roboto: bool,
    pending_load_cjk: bool,
    pending_load_emoji: bool,
    pending_update_checker: bool,
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
        let mut platform = WinitPlatform::new(&mut context_imgui)?;
        platform.attach_window(
            Arc::clone(&window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut context_imgui,
        )?;

        // Fonts: select FreeType loader if available, then add default font
        {
            // If FreeType is compiled and linked on the C++ side, select it now
            // so the atlas uses it (enables color emoji, OTF/CFF, etc.).
            // Note: We detect FreeType at runtime and enable color glyph flags when merging
            // emoji, but we don't switch loaders automatically here because some builds of
            // dear-imgui-sys ship prebuilt cimgui without the FreeType loader symbol.

            let fonts = context_imgui.font_atlas();
            let _id = fonts.add_font(&[FontSource::default_font_with_size(16.0)]);
            // Do not call `fonts.build()` here: it must not be called before the renderer
            // sets `ImGuiBackendFlags_RendererHasTextures` on the IO (ImGui 1.92+).
        }

        // Renderer
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                context.display().get_proc_address(s).cast()
            })
        };
        let mut renderer = GlowRenderer::new(gl, &mut context_imgui)?;
        renderer.set_framebuffer_srgb_enabled(false);
        let checker = checker_pixels(false);
        let checker_rect = context_imgui
            .font_atlas()
            .add_custom_rect(CustomRectData::rgba32(CHECKER_SIZE, &checker))
            .ok_or_else(|| std::io::Error::other("font atlas could not allocate checker rect"))?;
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
            roboto_font: None,
            roboto_source: None,
            cjk_loaded: false,
            cjk_source: None,
            emoji_loaded: false,
            emoji_source: None,
            checker_rect,
            checker_inverted: false,
            status: String::new(),
            pending_theme: None,
            pending_load_roboto: false,
            pending_load_cjk: false,
            pending_load_emoji: false,
            pending_update_checker: false,
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

    /// Load a font from one of this example's fixed repository or system paths.
    ///
    /// # Safety
    ///
    /// `path` must identify trusted application data that remains valid for the selected native
    /// loader. Structural validation rejects malformed containers before FFI, but cannot make an
    /// untrusted asset safe to load.
    unsafe fn try_load_trusted_font_file(
        &mut self,
        path: &Path,
        size: f32,
        merge: bool,
    ) -> Option<FontId> {
        if !path.exists() {
            return None;
        }
        let data = match fs::read(path) {
            Ok(d) => d,
            Err(error) => {
                self.status = format!("[WARN] Could not read font {}: {error}", path.display());
                return None;
            }
        };
        let loader = if Self::freetype_active() {
            font_validation::LoaderKind::FreeType
        } else {
            font_validation::LoaderKind::StbTrueType
        };
        if let Err(error) = font_validation::validate_font_data(&data, loader) {
            self.status = format!(
                "[WARN] Unsupported or malformed font {}: {error}",
                path.display(),
            );
            return None;
        }
        let cfg = FontConfig::new().size_pixels(size).merge_mode(merge);
        let fonts = self.imgui.context.font_atlas();
        // SAFETY: upheld by this function's explicit trusted-font precondition after structural
        // validation confirmed a representation accepted by the selected native loader.
        let source = unsafe { FontSource::ttf_data_with_size(&data, size) }.with_config(cfg);
        Some(fonts.add_font(&[source]))
    }

    /// # Safety
    ///
    /// The repository-owned Roboto file must remain a trusted, complete font asset.
    unsafe fn ensure_roboto_font(&mut self) -> Option<FontId> {
        if let Some(font) = self.roboto_font {
            return Some(font);
        }

        let path = bundled_roboto_path();
        // SAFETY: this exact file is vendored with Dear ImGui and validated before FFI.
        let font = unsafe { self.try_load_trusted_font_file(&path, 18.0, false) }?;
        self.roboto_font = Some(font);
        self.roboto_source = Some(path);
        Some(font)
    }

    /// # Safety
    ///
    /// The repository-owned Roboto file must remain a trusted, complete font asset.
    unsafe fn load_bundled_roboto(&mut self) {
        let already_loaded = self.roboto_font.is_some();
        // SAFETY: forwarded from this function's repository-asset precondition.
        if unsafe { self.ensure_roboto_font() }.is_some() {
            let source = self.roboto_source.as_ref().map_or_else(
                || "bundled asset".to_owned(),
                |path| path.display().to_string(),
            );
            self.status = if already_loaded {
                format!("[OK] Roboto is already loaded from {source}")
            } else {
                format!("[OK] Loaded bundled Roboto from {source}")
            };
        }
    }

    /// # Safety
    ///
    /// Any existing candidate file must be a trusted, complete repository or system font asset.
    unsafe fn load_cjk_font(&mut self) {
        if self.cjk_loaded {
            self.status = "[OK] CJK font is already merged".to_owned();
            return;
        }
        // SAFETY: forwarded from this function's repository-asset precondition.
        if unsafe { self.ensure_roboto_font() }.is_none() {
            return;
        }

        let candidates = cjk_font_candidates(Self::freetype_active());
        let had_existing_candidate = candidates.iter().any(|path| path.exists());
        for path in candidates {
            // SAFETY: candidates are fixed repository asset or OS font locations and are
            // structurally validated before reaching the native loader.
            if let Some(font) = unsafe { self.try_load_trusted_font_file(&path, 18.0, true) } {
                self.imgui.context.font_atlas().discard_bakes(0);
                self.roboto_font = Some(font);
                self.cjk_loaded = true;
                self.cjk_source = Some(path.clone());
                self.status = format!("[OK] Merged CJK font from {}", path.display());
                return;
            }
        }

        if !had_existing_candidate {
            self.status =
                "[WARN] CJK font not found in examples/assets or known system font directories"
                    .to_owned();
        }
    }

    /// # Safety
    ///
    /// Any existing candidate file must be a trusted, complete repository or system font asset.
    unsafe fn load_emoji_font(&mut self) {
        if self.emoji_loaded {
            self.status = "[OK] Emoji font is already merged".to_owned();
            return;
        }
        // SAFETY: forwarded from this function's repository-asset precondition.
        if unsafe { self.ensure_roboto_font() }.is_none() {
            return;
        }

        let freetype = Self::freetype_active();
        if freetype {
            let fonts = self.imgui.context.font_atlas();
            let cur = fonts.font_loader_flags();
            fonts.set_font_loader_flags(cur | FontLoaderFlags::LOAD_COLOR);
        }

        let candidates = emoji_font_candidates(freetype);
        let had_existing_candidate = candidates.iter().any(|path| path.exists());
        for path in candidates {
            // SAFETY: candidates are fixed repository asset or OS font locations and are
            // structurally validated before reaching the native loader.
            if let Some(font) = unsafe { self.try_load_trusted_font_file(&path, 20.0, true) } {
                self.imgui.context.font_atlas().discard_bakes(0);
                self.roboto_font = Some(font);
                self.emoji_loaded = true;
                self.emoji_source = Some(path.clone());
                self.status = format!("[OK] Merged Emoji font from {}", path.display());
                return;
            }
        }

        if !had_existing_candidate {
            let color_asset = example_path("assets/emoji/NotoColorEmoji.ttf");
            self.status = if !freetype && color_asset.exists() {
                "[WARN] NotoColorEmoji requires the FreeType loader; the active loader is stb_truetype"
                    .to_owned()
            } else {
                "[WARN] Emoji font not found in examples/assets or known system font directories"
                    .to_owned()
            };
        }
    }

    fn update_checker_rect(&mut self) {
        let inverted = !self.checker_inverted;
        let pixels = checker_pixels(inverted);
        if self.imgui.context.font_atlas().write_custom_rect(
            self.checker_rect,
            CustomRectData::rgba32(CHECKER_SIZE, &pixels),
        ) {
            self.checker_inverted = inverted;
            self.status = "[OK] Updated the checker with a partial atlas texture upload".to_owned();
        } else {
            self.status = "[WARN] The checker custom rect is no longer available".to_owned();
        }
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();
        let dt = now - self.imgui.last_frame;
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt.as_secs_f32());

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context)?;
        // Apply deferred actions from previous frame before building Ui
        if let Some(t) = self.pending_theme.take() {
            self.apply_theme_now(t);
        }
        if self.pending_load_roboto {
            // SAFETY: the example loads the exact Roboto file vendored with Dear ImGui.
            unsafe { self.load_bundled_roboto() };
            self.pending_load_roboto = false;
        }
        if self.pending_load_cjk {
            // SAFETY: the example loads only documented repository or OS font paths and
            // validates the complete bytes before FFI.
            unsafe { self.load_cjk_font() };
            self.pending_load_cjk = false;
        }
        if self.pending_load_emoji {
            // SAFETY: the example loads only documented repository or OS font paths and
            // validates the complete bytes before FFI.
            unsafe { self.load_emoji_font() };
            self.pending_load_emoji = false;
        }
        if self.pending_update_checker {
            self.update_checker_rect();
            self.pending_update_checker = false;
        }
        // Apply font scale to style before frame to avoid borrowing during UI building
        {
            let st = self.imgui.context.style_mut();
            st.set_font_scale_main(self.font_scale);
        }
        let ui = self.imgui.context.frame();
        // (No style/context mutations here; only record intents from UI.)

        let mut theme_change: Option<AppTheme> = None;
        let mut want_roboto = false;
        let mut want_cjk = false;
        let mut want_emoji = false;
        let mut want_checker_update = false;

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
                ui.text("Fonts");
                let loader = if Self::freetype_active() { "FreeType" } else { "stb_truetype" };
                ui.text_disabled(format!("Font Loader: {}", loader));
                {
                    let _disabled = ui.begin_disabled_with_cond(self.roboto_font.is_some());
                    if ui.button("Load bundled Roboto") { want_roboto = true; }
                }
                ui.same_line();
                {
                    let _disabled = ui.begin_disabled_with_cond(self.cjk_loaded);
                    if ui.button("Load + Merge CJK") { want_cjk = true; }
                }
                ui.same_line();
                {
                    let _disabled = ui.begin_disabled_with_cond(self.emoji_loaded);
                    if ui.button("Load + Merge Emoji") { want_emoji = true; }
                }
                if !self.status.is_empty() {
                    ui.text_wrapped(&self.status);
                }
                if let Some(path) = &self.roboto_source {
                    ui.text_wrapped(format!("Roboto source: {}", path.display()));
                }
                if let Some(path) = &self.cjk_source {
                    ui.text_wrapped(format!("CJK source: {}", path.display()));
                }
                if let Some(path) = &self.emoji_source {
                    ui.text_wrapped(format!("Emoji source: {}", path.display()));
                }

                ui.separator();
                // Keep the selected font scoped so measurements and baked metrics exercise the
                // same persistent FontId that the preview renders with.
                {
                    let _preview_font = self.roboto_font.map(|font| ui.push_font(font));
                    ui.text(ROBOTO_PREVIEW);
                    ui.text("你好, 世界! こんにちは! Hello! 🙂🚀");

                    let wchar_bytes = std::mem::size_of::<dear_imgui_rs::sys::ImWchar>();
                    let font = ui.current_font();
                    let mut baked = ui.current_baked_font();
                    let measured = ui.calc_text_size(ROBOTO_PREVIEW);
                    let glyph_r_advance = baked.glyph('R').map(|glyph| glyph.advance_x());
                    let font_sources = font.source_count();
                    let in_ni = font.is_glyph_in_font('你');
                    let loaded_ni = baked.is_glyph_loaded('你');
                    let in_shi = font.is_glyph_in_font('世');
                    let loaded_shi = baked.is_glyph_loaded('世');
                    let in_ko = font.is_glyph_in_font('こ');
                    let loaded_ko = baked.is_glyph_loaded('こ');
                    let (backend_flags, atlas_locked, atlas_fonts, atlas_sources) = unsafe {
                        let io = dear_imgui_rs::sys::igGetIO_Nil();
                        if io.is_null() || (*io).Fonts.is_null() {
                            (0, false, 0, 0)
                        } else {
                            let atlas = (*io).Fonts;
                            (
                                (*io).BackendFlags,
                                (*atlas).Locked,
                                usize::try_from((*atlas).Fonts.Size).unwrap_or(0),
                                usize::try_from((*atlas).Sources.Size).unwrap_or(0),
                            )
                        }
                    };
                    ui.text_disabled(format!(
                        "FontId: {} | calc_text_size={:.1} x {:.1} | BakedFont size={:.1} density={:.1} surface={} | glyph R advance={}",
                        font.debug_name(),
                        measured[0],
                        measured[1],
                        baked.size(),
                        baked.rasterizer_density(),
                        baked.metrics_total_surface(),
                        glyph_r_advance.map_or_else(|| "missing".to_owned(), |advance| format!("{advance:.1}")),
                    ));
                    ui.text_disabled(format!(
                        "Diagnostics: BackendFlags=0x{:X} Locked={} | atlas fonts={} sources={} | font sources={} | U+4F60(in={}, loaded={}) U+4E16(in={}, loaded={}) U+3053(in={}, loaded={}) | ImWchar={} bytes",
                        backend_flags,
                        atlas_locked,
                        atlas_fonts,
                        atlas_sources,
                        font_sources,
                        in_ni,
                        loaded_ni,
                        in_shi,
                        loaded_shi,
                        in_ko,
                        loaded_ko,
                        wchar_bytes,
                    ));
                    if wchar_bytes == 2 {
                        ui.text_disabled("Note: most emoji (e.g. 🙂) require IMGUI_USE_WCHAR32 (not enabled on this target).");
                    }
                }
                if let Some(font) = self.roboto_font {
                    let _large_font = ui.push_font_with_size(Some(font), 26.0);
                    ui.text("Roboto rendered through push_font_with_size at 26 px");
                }

                ui.separator();
                ui.text("Managed atlas custom rect");
                let checker_drawn = ui.image_custom_rect(self.checker_rect, [64.0, 64.0]);
                ui.same_line();
                if ui.button("Invert checker") { want_checker_update = true; }
                if !checker_drawn {
                    ui.text_disabled("The checker custom rect is unavailable");
                }

                ui.separator();
                ui.text("Built-in Style Editor");
                let mut style_copy = ui.clone_style();
                ui.show_style_editor(&mut style_copy);
            });

        self.imgui
            .platform
            .prepare_render_with_ui(ui, &self.window)?;
        let draw_data = self.imgui.context.render();

        // Clear + render
        if let Some(gl) = self.imgui.renderer.gl_context() {
            unsafe {
                gl.clear_color(0.1, 0.2, 0.3, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
        self.imgui.renderer.new_frame()?;
        self.imgui.renderer.render(draw_data)?;
        self.surface.swap_buffers(&self.context)?;

        // Defer actions to next frame to avoid borrowing conflicts during this frame
        if let Some(t) = theme_change {
            self.pending_theme = Some(t);
        }
        if want_roboto {
            self.pending_load_roboto = true;
        }
        if want_cjk {
            self.pending_load_cjk = true;
        }
        if want_emoji {
            self.pending_load_emoji = true;
        }
        if want_checker_update {
            self.pending_update_checker = true;
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
            WindowEvent::KeyboardInput { event, .. }
                if event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                event_loop.exit();
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn bundled_roboto_is_present_and_supported_by_both_loaders() {
        let path = bundled_roboto_path();
        let data = fs::read(&path).unwrap_or_else(|error| {
            panic!("failed to read bundled font {}: {error}", path.display())
        });

        for loader in [
            font_validation::LoaderKind::StbTrueType,
            font_validation::LoaderKind::FreeType,
        ] {
            assert!(
                font_validation::validate_font_data(&data, loader).is_ok(),
                "bundled Roboto must remain valid for {loader:?}"
            );
        }
    }

    #[test]
    fn platform_font_candidates_are_stable_and_unique() {
        for freetype in [false, true] {
            let cjk = cjk_font_candidates(freetype);
            assert_eq!(
                cjk.first(),
                Some(&example_path("assets/NotoSansSC-Regular.ttf"))
            );

            let emoji = emoji_font_candidates(freetype);
            let expected_emoji = if freetype {
                example_path("assets/emoji/NotoColorEmoji.ttf")
            } else {
                example_path("assets/emoji/OpenMoji-Black.ttf")
            };
            assert_eq!(emoji.first(), Some(&expected_emoji));

            for candidates in [&cjk, &emoji] {
                let unique: HashSet<_> = candidates.iter().collect();
                assert_eq!(
                    unique.len(),
                    candidates.len(),
                    "candidate paths must not be retried"
                );
            }
        }
    }

    #[test]
    fn checker_pixels_have_exact_rgba_shape_and_distinct_phases() {
        let normal = checker_pixels(false);
        let inverted = checker_pixels(true);
        let expected_len = usize::from(CHECKER_SIZE[0]) * usize::from(CHECKER_SIZE[1]) * 4;

        assert_eq!(normal.len(), expected_len);
        assert_eq!(inverted.len(), expected_len);
        assert_ne!(normal, inverted);
        assert!(normal.chunks_exact(4).all(|pixel| pixel[3] == 255));

        let data = CustomRectData::rgba32(CHECKER_SIZE, &normal);
        assert_eq!(data.size(), CHECKER_SIZE);
        assert_eq!(data.format(), TextureFormat::RGBA32);
    }

    #[test]
    fn runtime_font_and_custom_rect_flow_runs_headless() {
        let mut context = Context::create();
        context.io_mut().set_display_size([320.0, 200.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);
        let _consumer = context
            .create_renderer_consumer()
            .expect("the headless renderer consumer should attach");
        context
            .font_atlas()
            .add_font(&[FontSource::default_font_with_size(16.0)]);

        let initial_checker = checker_pixels(false);
        let checker = context
            .font_atlas()
            .add_custom_rect(CustomRectData::rgba32(CHECKER_SIZE, &initial_checker))
            .expect("the initial checker should fit in the managed atlas");
        {
            let ui = context.frame();
            assert!(ui.image_custom_rect(checker, [32.0, 32.0]));
        }
        let mut rendered = context.render();
        assert_eq!(rendered.texture_requests().len(), 1);
        assert_eq!(
            rendered.texture_requests()[0].kind(),
            TextureRequestKind::Create
        );
        let feedback = rendered.texture_requests()[0]
            .uploaded(TextureId::new(1))
            .expect("the atlas create request should accept upload feedback");
        rendered
            .reconcile_texture_feedback([feedback])
            .expect("the atlas create feedback should complete the frame");
        drop(rendered);

        let path = bundled_roboto_path();
        let font_bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!("failed to read bundled font {}: {error}", path.display())
        });
        font_validation::validate_font_data(&font_bytes, font_validation::LoaderKind::StbTrueType)
            .expect("bundled Roboto should pass structural validation");
        // SAFETY: these are the complete, structurally validated bytes of the vendored font.
        let source = unsafe { FontSource::ttf_data_with_size(&font_bytes, 18.0) };
        let roboto = context.font_atlas().add_font(&[source]);

        let inverted_checker = checker_pixels(true);
        assert!(context.font_atlas().write_custom_rect(
            checker,
            CustomRectData::rgba32(CHECKER_SIZE, &inverted_checker),
        ));

        {
            let ui = context.frame();
            let _font = ui.push_font(roboto);
            assert_eq!(ui.current_font(), roboto);
            assert!(ui.calc_text_size(ROBOTO_PREVIEW)[0] > 0.0);
            let mut baked = ui.current_baked_font();
            assert!(baked.glyph('R').is_some());
            assert!(ui.image_custom_rect(checker, [32.0, 32.0]));
        }
        let mut rendered = context.render();
        assert_eq!(rendered.texture_requests().len(), 1);
        assert_eq!(
            rendered.texture_requests()[0].kind(),
            TextureRequestKind::Update
        );
        let feedback = rendered.texture_requests()[0]
            .uploaded(TextureId::new(1))
            .expect("the atlas update request should accept upload feedback");
        rendered
            .reconcile_texture_feedback([feedback])
            .expect("the atlas update feedback should complete the frame");
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
