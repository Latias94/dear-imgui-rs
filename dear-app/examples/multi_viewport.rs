use dear_app::{AppConfig, DockingConfig, RedrawMode, RunError, Theme};

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear App multi-window editor".to_owned(),
        multi_viewport: true,
        docking: DockingConfig::full_viewport(),
        redraw: RedrawMode::Wait,
        theme: Some(Theme::Dark),
        ..Default::default()
    };
    let mut selected = 0;
    let mut assets_open = true;
    let mut inspector_open = true;
    dear_app::run_ui(config, move |ui| {
        ui.set_next_window_viewport(ui.main_viewport().id());
        ui.window("Editor").build(|| {
            ui.checkbox("Assets", &mut assets_open);
            ui.checkbox("Inspector", &mut inspector_open);
        });
        if assets_open {
            ui.window("Assets").opened(&mut assets_open).build(|| {
                ui.text("Drag this tab outside the main window to create a native window.");
                for index in 0..5 {
                    if ui.selectable(format!("Scene {index}")) {
                        selected = index;
                    }
                }
            });
        }
        if inspector_open {
            ui.window("Inspector")
                .opened(&mut inspector_open)
                .build(|| {
                    ui.text(format!("Selected scene: {selected}"));
                    ui.text("Closing a secondary window keeps the editor running.");
                });
        }
    })
}
