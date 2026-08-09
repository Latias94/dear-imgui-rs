//! Global themes, scoped style overrides, and safe dynamic font configuration.
//!
//! This example uses `dear-app`, whose WGPU renderer owns the font atlas through a managed
//! renderer consumer. Font sources may be configured in `configure_imgui` and mutated later in
//! `prepare_frame`; the renderer consumes the resulting texture requests automatically. Do not
//! claim `LegacyFontAtlas` or call `LegacyFontAtlas::build` on this route. A custom legacy renderer
//! must explicitly claim that capability, build the CPU atlas, upload it, and retire the upload at
//! the correct renderer lifetime boundary.

use std::io;

use dear_app::{
    AppConfig, Application, ApplicationStage, FrameContext, InitContext, PrepareFrameContext,
    RunError, run,
};
use dear_imgui_rs::{
    ColorOverride, Condition, FontConfig, FontId, FontSource, StbTrueTypeFontData, StyleColor,
    StyleTweaks, StyleVar, Theme, ThemePreset, Ui,
};

const ROBOTO_MEDIUM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
));
const PREVIEW_TEXT: &str = "Sphinx of black quartz, judge my vow. 0123456789";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThemeChoice {
    Dark,
    Light,
    Classic,
    Graphite,
}

impl ThemeChoice {
    const VALUES: [Self; 4] = [Self::Dark, Self::Light, Self::Classic, Self::Graphite];
    const LABELS: [&'static str; 4] = ["Dark", "Light", "Classic", "Graphite accent"];

    fn from_index(index: usize) -> Self {
        Self::VALUES.get(index).copied().unwrap_or(Self::Graphite)
    }

    fn theme(self) -> Theme {
        let preset = match self {
            Self::Dark | Self::Graphite => ThemePreset::Dark,
            Self::Light => ThemePreset::Light,
            Self::Classic => ThemePreset::Classic,
        };

        if self != Self::Graphite {
            return Theme {
                preset,
                ..Default::default()
            };
        }

        Theme {
            preset,
            colors: vec![
                ColorOverride {
                    id: StyleColor::WindowBg,
                    rgba: [0.075, 0.082, 0.102, 1.0],
                },
                ColorOverride {
                    id: StyleColor::FrameBg,
                    rgba: [0.13, 0.145, 0.18, 1.0],
                },
                ColorOverride {
                    id: StyleColor::Button,
                    rgba: [0.18, 0.38, 0.68, 0.78],
                },
                ColorOverride {
                    id: StyleColor::ButtonHovered,
                    rgba: [0.24, 0.49, 0.86, 1.0],
                },
                ColorOverride {
                    id: StyleColor::Header,
                    rgba: [0.18, 0.38, 0.68, 0.65],
                },
                ColorOverride {
                    id: StyleColor::CheckMark,
                    rgba: [0.38, 0.7, 1.0, 1.0],
                },
            ],
            style: StyleTweaks {
                window_rounding: Some(7.0),
                frame_rounding: Some(5.0),
                tab_rounding: Some(5.0),
                frame_padding: Some([8.0, 5.0]),
                item_spacing: Some([8.0, 6.0]),
                grab_rounding: Some(5.0),
                scrollbar_rounding: Some(7.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Classic => "Classic",
            Self::Graphite => "Graphite accent",
        }
    }
}

#[derive(Clone, Copy)]
struct AppFonts {
    vector: FontId,
    roboto: FontId,
    compact: Option<FontId>,
}

impl AppFonts {
    const BASE_LABELS: [&'static str; 2] = ["Embedded vector", "Roboto Medium"];
    const ALL_LABELS: [&'static str; 3] = [
        "Embedded vector",
        "Roboto Medium",
        "Compact bitmap (runtime)",
    ];

    fn labels(self) -> &'static [&'static str] {
        if self.compact.is_some() {
            &Self::ALL_LABELS
        } else {
            &Self::BASE_LABELS
        }
    }

    fn selected(self, index: usize) -> FontId {
        match index {
            1 => self.roboto,
            2 => self.compact.unwrap_or(self.vector),
            _ => self.vector,
        }
    }
}

struct StyleAndFontsApp {
    fonts: Option<AppFonts>,
    theme_index: usize,
    font_index: usize,
    global_font_scale: f32,
    preview_font_size: f32,
    preview_rounding: f32,
    accent: [f32; 4],
    pending_theme: Option<ThemeChoice>,
    pending_global_font_scale: Option<f32>,
    pending_compact_font: bool,
    status: String,
}

impl Default for StyleAndFontsApp {
    fn default() -> Self {
        Self {
            fonts: None,
            theme_index: 3,
            font_index: 1,
            global_font_scale: 1.0,
            preview_font_size: 24.0,
            preview_rounding: 8.0,
            accent: [0.22, 0.55, 0.92, 1.0],
            pending_theme: None,
            pending_global_font_scale: None,
            pending_compact_font: false,
            status: "Startup fonts are validated and ready.".to_owned(),
        }
    }
}

impl StyleAndFontsApp {
    fn missing_fonts(stage: ApplicationStage) -> RunError {
        RunError::application(
            stage,
            io::Error::other("style_and_fonts font setup did not complete"),
        )
    }

    fn install_startup_fonts(
        context: &mut dear_imgui_rs::Context,
    ) -> Result<AppFonts, dear_imgui_rs::StbTrueTypeFontError> {
        let roboto_data = StbTrueTypeFontData::from_slice(ROBOTO_MEDIUM)?;
        let atlas = context.font_atlas();
        let vector = atlas.add_font(&[FontSource::default_vector_with_size(16.0)
            .with_config(FontConfig::new().name("Embedded vector UI"))]);
        let roboto = atlas.add_font(&[FontSource::stb_truetype_with_size(roboto_data, 18.0)
            .with_config(FontConfig::new().name("Roboto Medium"))]);

        Ok(AppFonts {
            vector,
            roboto,
            compact: None,
        })
    }

    fn apply_pending_context_changes(
        &mut self,
        context: &mut PrepareFrameContext<'_>,
    ) -> Result<(), RunError> {
        if let Some(theme) = self.pending_theme.take() {
            theme.theme().apply_to_context(context.imgui());
            self.status = format!("Applied {} before opening this frame.", theme.label());
        }

        if let Some(scale) = self.pending_global_font_scale.take() {
            context.imgui().style_mut().set_font_scale_main(scale);
            self.status = format!("Global font scale is now {scale:.2}x.");
        }

        if self.pending_compact_font {
            self.pending_compact_font = false;
            let fonts = self
                .fonts
                .as_mut()
                .ok_or_else(|| Self::missing_fonts(ApplicationStage::PrepareFrame))?;

            if fonts.compact.is_none() {
                let atlas = context.imgui().font_atlas();
                let compact = atlas.add_font(&[FontSource::default_bitmap_with_size(13.0)
                    .with_config(FontConfig::new().name("Compact bitmap UI"))]);

                // Clear cached misses before the managed renderer processes the atlas update.
                // There is no manual build or GPU upload on the managed route.
                atlas.discard_bakes(0);
                fonts.compact = Some(compact);
                self.status =
                    "Installed a font at runtime; the managed renderer owns the upload.".to_owned();
            }
        }

        Ok(())
    }

    fn draw_theme_controls(&mut self, ui: &Ui) {
        ui.text("Global theme");
        if ui.combo_simple_string("Preset", &mut self.theme_index, &ThemeChoice::LABELS) {
            let theme = ThemeChoice::from_index(self.theme_index);
            self.pending_theme = Some(theme);
            self.status = format!("Queued {} for the next frame.", theme.label());
        }

        if ui.slider("Global font scale", 0.75, 1.75, &mut self.global_font_scale) {
            self.pending_global_font_scale = Some(self.global_font_scale);
        }

        if ui.button("Reset global style") {
            self.theme_index = 3;
            self.global_font_scale = 1.0;
            self.pending_theme = Some(ThemeChoice::Graphite);
            self.pending_global_font_scale = Some(1.0);
            self.status = "Queued the Graphite theme and a 1.00x font scale.".to_owned();
        }
        ui.text_disabled("Context-wide changes are committed in prepare_frame.");
    }

    fn draw_font_controls(&mut self, ui: &Ui, fonts: AppFonts) {
        ui.text("Font atlas");
        let labels = fonts.labels();
        self.font_index = self.font_index.min(labels.len().saturating_sub(1));
        ui.combo_simple_string("Font", &mut self.font_index, labels);
        ui.slider(
            "Preview runtime size",
            12.0,
            42.0,
            &mut self.preview_font_size,
        );

        if fonts.compact.is_none() {
            if self.pending_compact_font {
                ui.text_disabled("Compact font installation is queued.");
            } else if ui.button("Install compact bitmap font") {
                self.pending_compact_font = true;
                self.status = "Queued a managed font-atlas mutation.".to_owned();
            }
        } else {
            ui.text_disabled("The compact font was added after renderer initialization.");
        }

        let selected = fonts.selected(self.font_index);
        ui.text_disabled(format!(
            "{} | reference size: {} | sources: {} | loaded: {}",
            selected.debug_name(),
            selected
                .reference_size()
                .map_or_else(|| "dynamic".to_owned(), |size| format!("{size:.1}px")),
            selected.source_count(),
            selected.is_loaded(),
        ));

        {
            let _font = ui.push_font(selected);
            ui.text("push_font uses the source's reference size:");
            ui.text(PREVIEW_TEXT);
        }
        {
            let _font = ui.push_font_with_size(Some(selected), self.preview_font_size);
            ui.text(format!(
                "push_font_with_size requests {:.1}px at runtime:",
                self.preview_font_size
            ));
            ui.text(PREVIEW_TEXT);
        }
    }

    fn draw_scoped_style_controls(&mut self, ui: &Ui) {
        ui.text("Scoped style tokens");
        ui.slider("Preview rounding", 0.0, 16.0, &mut self.preview_rounding);
        ui.color_edit4("Preview accent", &mut self.accent);

        {
            let _rounding = ui.push_style_var(StyleVar::FrameRounding(self.preview_rounding));
            let _padding = ui.push_style_var(StyleVar::FramePadding([12.0, 7.0]));
            let _button = ui.push_style_color(StyleColor::Button, self.accent);
            let _button_hovered =
                ui.push_style_color(StyleColor::ButtonHovered, brighten(self.accent, 0.14));
            let _button_active =
                ui.push_style_color(StyleColor::ButtonActive, brighten(self.accent, 0.25));
            ui.button("Scoped accent button");
        }

        ui.same_line();
        ui.button("Global style button");
        ui.text_disabled("Dropping the tokens restores the global style automatically.");
    }

    fn draw_capability_notes(ui: &Ui) {
        ui.text("Renderer capability boundary");
        ui.bullet_text("dear-app attaches a managed renderer consumer.");
        ui.bullet_text("Configure or mutate fonts before a Dear ImGui frame is opened.");
        ui.bullet_text("Managed renderers consume texture requests; they do not call build().");
        ui.bullet_text(
            "Legacy renderers must claim LegacyFontAtlas and own build, upload, and retirement.",
        );
    }

    fn draw(&mut self, ui: &Ui, fonts: AppFonts) {
        ui.window("Style and Fonts")
            .size([760.0, 700.0], Condition::FirstUseEver)
            .build(|| {
                ui.text_wrapped(
                    "Use persistent Context mutations for global style and atlas changes, then use \
                     scoped tokens for local presentation. The embedded Roboto source is validated \
                     before it crosses FFI, so routine TTF loading requires no unsafe block.",
                );

                ui.separator();
                self.draw_theme_controls(ui);
                ui.separator();
                self.draw_font_controls(ui, fonts);
                ui.separator();
                self.draw_scoped_style_controls(ui);
                ui.separator();
                Self::draw_capability_notes(ui);
                ui.separator();
                ui.text_disabled(&self.status);
            });
    }
}

impl Application for StyleAndFontsApp {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        ThemeChoice::Graphite
            .theme()
            .apply_to_context(context.imgui());
        context
            .imgui()
            .style_mut()
            .set_font_scale_main(self.global_font_scale);
        self.fonts = Some(
            Self::install_startup_fonts(context.imgui())
                .map_err(|error| RunError::application(ApplicationStage::ConfigureImgui, error))?,
        );
        Ok(())
    }

    fn prepare_frame(&mut self, context: &mut PrepareFrameContext<'_>) -> Result<(), RunError> {
        self.apply_pending_context_changes(context)
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let fonts = self
            .fonts
            .ok_or_else(|| Self::missing_fonts(ApplicationStage::Frame))?;
        self.draw(context.ui(), fonts);
        Ok(())
    }
}

fn brighten(mut color: [f32; 4], amount: f32) -> [f32; 4] {
    for channel in &mut color[..3] {
        *channel = (*channel + amount).min(1.0);
    }
    color
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Style and Fonts".to_owned(),
        window_size: (980.0, 760.0),
        ..Default::default()
    };

    run(config, StyleAndFontsApp::default())
}
