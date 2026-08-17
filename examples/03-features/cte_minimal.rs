//! Minimal ImGuiColorTextEdit integration with the high-level `dear-app` runtime.

use dear_app::{
    AppConfig, Application, ApplicationStage, FrameContext, InitContext, RunError, ShutdownContext,
    run,
};
use dear_imgui_cte::{CteUiExt, Language, TextEditor, dejavu_font_source};
use dear_imgui_rs::Condition;

const SOURCE: &str = r#"#include <iostream>

int main() {
    std::cout << "Hello from cimCTE!\n";
    return 0;
}
"#;

#[derive(Default)]
struct CteMinimal {
    editor: Option<TextEditor>,
    changed_last_frame: bool,
}

impl Application for CteMinimal {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let imgui = context.imgui();
        let font = dejavu_font_source(16.0)
            .map_err(|error| RunError::application(ApplicationStage::ConfigureImgui, error))?;
        imgui.font_atlas().add_font(&[font]);

        let mut editor = TextEditor::try_create(imgui)
            .map_err(|error| RunError::application(ApplicationStage::ConfigureImgui, error))?;
        editor
            .set_text(SOURCE)
            .map_err(|error| RunError::application(ApplicationStage::ConfigureImgui, error))?;
        editor.set_language(Some(Language::Cpp));
        self.editor = Some(editor);
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let editor = self.editor.as_mut().ok_or_else(|| {
            RunError::application(
                ApplicationStage::Frame,
                std::io::Error::other("the CTE editor was not initialized"),
            )
        })?;
        let ui = context.ui();

        let rendered = ui
            .window("CTE Minimal")
            .size([760.0, 560.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(if self.changed_last_frame {
                    "Document changed on the previous frame"
                } else {
                    "Edit the C++ source below"
                });
                ui.separator();
                let available = ui.content_region_avail();
                ui.text_editor(editor, "Source##cte_minimal")
                    .size([available[0].max(1.0), available[1].max(1.0)])
                    .build()
            });

        if let Some(result) = rendered {
            self.changed_last_frame =
                result.map_err(|error| RunError::application(ApplicationStage::Frame, error))?;
        }
        Ok(())
    }

    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        drop(self.editor.take());
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - CTE Minimal".to_owned(),
        window_size: (960.0, 720.0),
        ..Default::default()
    };
    run(config, CteMinimal::default())
}
