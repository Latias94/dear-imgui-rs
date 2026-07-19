//! String and `ImString` input, hints, capacity, callbacks, and multiline editing.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::{
    Condition, HistoryDirection, ImString, InputTextCallback, InputTextCallbackHandler,
    TextCallbackData,
};

#[derive(Default)]
struct DemoHandler;

impl InputTextCallbackHandler for DemoHandler {
    fn char_filter(&mut self, character: char) -> Option<char> {
        (!matches!(character, 'x' | 'X')).then_some(character)
    }

    fn on_completion(&mut self, mut data: TextCallbackData<'_>) {
        data.push_str(" [Tab]");
    }

    fn on_history(&mut self, direction: HistoryDirection, mut data: TextCallbackData<'_>) {
        match direction {
            HistoryDirection::Up => data.push_str(" [Up]"),
            HistoryDirection::Down => data.push_str(" [Down]"),
        }
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Input Text".to_owned(),
        window_size: (900.0, 640.0),
        ..Default::default()
    };
    let mut title = String::with_capacity(64);
    let mut name = ImString::new("");
    let mut notes = ImString::new("");
    let mut callback_notes = String::new();

    run_ui(config, move |ui| {
        ui.window("Input Text")
            .size([700.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("String");
                ui.input_text("Title", &mut title)
                    .hint("Enter a title...")
                    .capacity_hint(64)
                    .enter_returns_true(true)
                    .build();

                ui.separator();
                ui.text("ImString");
                ui.input_text_imstr("Name", &mut name)
                    .hint("Your name")
                    .build();
                ui.input_text_multiline_imstr("Notes", &mut notes, [500.0, 160.0])
                    .build();

                ui.separator();
                ui.text("String with callbacks (the letter x is filtered)");
                let callbacks = InputTextCallback::CHAR_FILTER
                    | InputTextCallback::EDIT
                    | InputTextCallback::ALWAYS;
                ui.input_text_multiline("Callback notes", &mut callback_notes, [500.0, 120.0])
                    .callback(callbacks, DemoHandler)
                    .build();

                ui.separator();
                ui.text(format!(
                    "Title={} bytes, name={} bytes, notes={} bytes",
                    title.len(),
                    name.to_str().len(),
                    notes.to_str().len()
                ));
            });
    })
}
