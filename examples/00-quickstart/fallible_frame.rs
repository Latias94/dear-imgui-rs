//! Persistent, fallible frame closure with explicit non-error exit control.

use std::{error::Error, fmt};

use dear_app::{AppConfig, RunError, imgui::Condition, run_frame};

#[derive(Debug)]
struct DemoError;

impl fmt::Display for DemoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the example requested a frame failure")
    }
}

impl Error for DemoError {}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Fallible Frame".to_owned(),
        window_size: (720.0, 480.0),
        ..Default::default()
    };
    let mut clicks = 0_u64;
    let mut frames = 0_u64;

    run_frame(config, move |context| {
        frames = frames.saturating_add(1);
        let docking_flags = context.addons().docking().flags();
        let mut exit_requested = false;
        let mut failure_requested = false;
        let ui = context.ui();

        ui.window("Fallible Frame")
            .size([440.0, 240.0], Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("Persistent frame count: {frames}"));
                ui.text(format!("Configured docking flags: {docking_flags:?}"));

                if ui.button("Increment persistent state") {
                    clicks = clicks.saturating_add(1);
                }
                ui.same_line();
                ui.text(format!("Clicks: {clicks}"));

                ui.separator();
                exit_requested = ui.button("Exit successfully");
                failure_requested = ui.button("Return a user error");
            });

        if exit_requested {
            context.request_exit();
        }
        if failure_requested {
            return Err(DemoError);
        }
        Ok(())
    })
}
