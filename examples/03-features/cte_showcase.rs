//! ImGuiColorTextEdit editors, diagnostics, autocomplete, diff, and notifications.

use dear_app::{
    AppConfig, Application, ApplicationStage, FrameContext, InitContext, RunError, ShutdownContext,
    run,
};
use dear_imgui_cte::{
    AutocompleteConfig, CteResult, CteUiExt, Language, NotificationType, Notifications, Palette,
    PaletteColor, Position, Selection, SquiggleKind, TextDiff, TextEditor, dejavu_font_source,
};
use dear_imgui_rs::Condition;
use std::{cell::Cell, rc::Rc, time::Duration};

const PRIMARY_SOURCE: &str = r#"#include <string>

std::string greeting(const std::string& name) {
    return "Hello, " + name;
}

int main() {
    return greeting("cimCTE").empty();
}
"#;

const TRIE_SOURCE: &str = r#"def format_message(name):
    message = f"Hello, {name}"
    return message

print(format_message("cimCTE"))
"#;

#[derive(Default)]
struct CteShowcase {
    state: Option<CteState>,
}

struct CteState {
    primary: TextEditor,
    trie: TextEditor,
    diff: TextDiff,
    notifications: Notifications,
    transaction_count: Rc<Cell<usize>>,
    word_wrap: bool,
    side_by_side: bool,
}

impl CteState {
    fn create(context: &dear_imgui_rs::Context) -> CteResult<Self> {
        let mut primary = TextEditor::try_create(context)?;
        primary.set_text(PRIMARY_SOURCE)?;
        primary.set_language(Some(Language::Cpp));
        primary.set_word_wrap_enabled(true);

        let mut palette = Palette::dark();
        palette.set(PaletteColor::Keyword, 0xFF_E0_9A_56);
        primary.set_palette(&palette)?;
        primary.add_marker(
            3,
            0xFF_64_B5_F6,
            0xFF_FF_FF_FF,
            "Return path",
            "Greeting result",
        )?;
        primary.add_squiggle(
            Selection::new(Position::new(7, 11), Position::new(7, 29)),
            SquiggleKind::new(1),
            0xFF_4F_7C_F7,
            "Try editing this expression",
        )?;

        let transaction_count: Rc<Cell<usize>> = Rc::new(Cell::new(0));
        let callback_count = Rc::clone(&transaction_count);
        primary.set_transaction_callback(move |_| {
            callback_count.set(callback_count.get().saturating_add(1));
        })?;
        let autocomplete = AutocompleteConfig::new()
            .trigger_delay(Duration::ZERO)
            .suggestion_width(32);
        primary.set_autocomplete(&autocomplete, |request| {
            let Ok(term) = request.search_term().map(str::to_owned) else {
                return;
            };
            let candidates = ["greeting", "std::string", "std::cout", "return"];
            let _ = request.set_suggestions(
                candidates
                    .into_iter()
                    .filter(|candidate| candidate.starts_with(&term)),
            );
        })?;

        let mut trie = TextEditor::try_create(context)?;
        trie.set_text(TRIE_SOURCE)?;
        trie.set_language(Some(Language::Python));
        trie.enable_trie_autocomplete()?;

        let mut diff = TextDiff::try_create(context)?;
        diff.set_text(
            "int answer() {\n    return 41;\n}\n",
            "int answer() {\n    return 42;\n}\n",
        )?;
        diff.set_language(Some(Language::Cpp));
        diff.set_side_by_side(true);

        let mut notifications = Notifications::try_create(context)?;
        notifications.add(
            NotificationType::Info,
            "CTE showcase initialized",
            Duration::from_secs(3),
        )?;

        Ok(Self {
            primary,
            trie,
            diff,
            notifications,
            transaction_count,
            word_wrap: true,
            side_by_side: true,
        })
    }
}

impl Application for CteShowcase {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let imgui = context.imgui();
        let font = dejavu_font_source(16.0)
            .map_err(|error| RunError::application(ApplicationStage::ConfigureImgui, error))?;
        imgui.font_atlas().add_font(&[font]);
        self.state = Some(
            CteState::create(imgui)
                .map_err(|error| RunError::application(ApplicationStage::ConfigureImgui, error))?,
        );
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let state = self.state.as_mut().ok_or_else(|| {
            RunError::application(
                ApplicationStage::Frame,
                std::io::Error::other("the CTE showcase was not initialized"),
            )
        })?;
        let ui = context.ui();

        let submitted = ui
            .window("CTE Showcase")
            .size([880.0, 680.0], Condition::FirstUseEver)
            .build(|| -> CteResult<()> {
                ui.text(format!(
                    "Transaction callback events: {}",
                    state.transaction_count.get()
                ));
                if ui.button("Undo") {
                    state.primary.undo();
                }
                ui.same_line();
                if ui.button("Redo") {
                    state.primary.redo();
                }
                ui.same_line();
                if ui.button("Notify") {
                    state.notifications.add(
                        NotificationType::Success,
                        "Notification emitted from Rust",
                        Duration::from_secs(4),
                    )?;
                }
                ui.same_line();
                if ui.checkbox("Word wrap", &mut state.word_wrap) {
                    state.primary.set_word_wrap_enabled(state.word_wrap);
                }

                if let Some(tab_bar) = ui.tab_bar("cte_showcase_tabs") {
                    if let Some(_tab) = ui.tab_item("Custom autocomplete") {
                        let available = ui.content_region_avail();
                        ui.text_editor(&mut state.primary, "C++##cte_primary")
                            .size([available[0].max(1.0), available[1].max(1.0)])
                            .build()?;
                    }
                    if let Some(_tab) = ui.tab_item("Trie autocomplete") {
                        let connected = state
                            .trie
                            .trie_autocomplete()
                            .map(|trie| trie.is_connected())
                            .transpose()?
                            .unwrap_or(false);
                        ui.text(format!("Trie connected: {connected}"));
                        let available = ui.content_region_avail();
                        ui.text_editor(&mut state.trie, "Python##cte_trie")
                            .size([available[0].max(1.0), available[1].max(1.0)])
                            .build()?;
                    }
                    if let Some(_tab) = ui.tab_item("Diff") {
                        if ui.checkbox("Side by side", &mut state.side_by_side) {
                            state.diff.set_side_by_side(state.side_by_side);
                        }
                        let available = ui.content_region_avail();
                        ui.text_diff(&mut state.diff, "Review##cte_diff")
                            .size([available[0].max(1.0), available[1].max(1.0)])
                            .build()?;
                    }
                    drop(tab_bar);
                }
                Ok(())
            });

        if let Some(result) = submitted {
            result.map_err(|error| RunError::application(ApplicationStage::Frame, error))?;
        }
        ui.notifications(&mut state.notifications)
            .build()
            .map_err(|error| RunError::application(ApplicationStage::Frame, error))?;
        Ok(())
    }

    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        drop(self.state.take());
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - CTE Showcase".to_owned(),
        window_size: (1100.0, 820.0),
        ..Default::default()
    };
    run(config, CteShowcase::default())
}
