use dear_app::{AppConfig, RunError, imgui::Condition, run_ui};

fn main() -> Result<(), RunError> {
    let mut clicks = 0;

    run_ui(AppConfig::default(), move |ui| {
        ui.window("Hello")
            .size([360.0, 160.0], Condition::FirstUseEver)
            .build(|| {
                if ui.button("Click me") {
                    clicks += 1;
                }
                ui.same_line();
                ui.text(format!("Clicks: {clicks}"));
            });
    })
}
