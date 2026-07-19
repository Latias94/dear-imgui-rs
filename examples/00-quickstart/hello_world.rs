//! Minimal stateful application using the high-level `dear-app` runtime.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::Condition;

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Hello World".to_owned(),
        window_size: (720.0, 480.0),
        ..Default::default()
    };
    let mut counter = 0;
    let mut show_hello = true;

    run_ui(config, move |ui| {
        if show_hello {
            ui.window("Hello")
                .opened(&mut show_hello)
                .size([360.0, 180.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello, world!");
                    if ui.button("Click me") {
                        counter += 1;
                    }
                    ui.same_line();
                    ui.text(format!("Counter: {counter}"));
                });
        } else {
            ui.window("Main")
                .size([280.0, 120.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Hello window is closed.");
                    if ui.button("Reopen Hello") {
                        show_hello = true;
                    }
                });
        }
    })
}
