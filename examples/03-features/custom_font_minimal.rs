//! Minimal custom font setup with the high-level `dear-app` runtime.

use dear_app::{AppConfig, Application, FrameContext, InitContext, RunError, run};
use dear_imgui_rs::{Condition, FontId, FontSource, StbTrueTypeFontData};

const ROBOTO_MEDIUM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
));

#[derive(Default)]
struct CustomFontApp {
    roboto: Option<FontId>,
}

impl Application for CustomFontApp {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let fonts = context.imgui().font_atlas();
        fonts.add_font(&[FontSource::default_font_with_size(16.0)]);

        let roboto = StbTrueTypeFontData::from_slice(ROBOTO_MEDIUM).map_err(|error| {
            RunError::application(
                "configure_imgui",
                format!("invalid bundled Roboto font: {error}"),
            )
        })?;
        let source = FontSource::stb_truetype_with_size(roboto, 20.0);
        self.roboto = Some(fonts.add_font(&[source]));
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let roboto = self
            .roboto
            .ok_or_else(|| RunError::application("frame", "the custom font was not initialized"))?;
        let ui = context.ui();

        ui.window("Custom Font")
            .size([440.0, 190.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Default font with a 16 px reference size.");
                ui.separator();
                {
                    let _font = ui.push_font(roboto);
                    ui.text("Roboto Medium with a 20 px reference size.");
                    ui.text("Sphinx of black quartz, judge my vow.");
                }
                ui.separator();
                ui.text("Dropping the token restores the previous font.");
            });

        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Custom Font".to_owned(),
        window_size: (720.0, 480.0),
        ..Default::default()
    };

    run(config, CustomFontApp::default())
}
