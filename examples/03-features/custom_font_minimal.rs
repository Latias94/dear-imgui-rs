//! Minimal custom font setup with the high-level `dear-app` runtime.

use dear_app::{AppConfig, Application, FrameContext, InitContext, RunError, run};
use dear_imgui_rs::{Condition, FontId, FontSource};

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

        // SAFETY: the vendored Roboto file is a complete TTF accepted by Dear ImGui's loaders.
        let source = unsafe { FontSource::ttf_data_with_size(ROBOTO_MEDIUM, 20.0) };
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
                ui.text("This line uses Dear ImGui's default font.");
                ui.separator();
                {
                    let _font = ui.push_font(roboto);
                    ui.text("This line uses the bundled Roboto Medium font.");
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
