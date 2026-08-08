//! Minimal ImGuIZMO.quat integration with the high-level `dear-app` runtime.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::Condition;
use dear_imguizmo_quat::{GizmoQuatExt, Mode};

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - ImGuIZMO.quat Minimal".to_owned(),
        window_size: (640.0, 520.0),
        ..Default::default()
    };

    let mut rotation = [0.0, 0.0, 0.0, 1.0];

    run_ui(config, move |ui| {
        ui.window("ImGuIZMO.quat Minimal")
            .size([420.0, 390.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Drag the gizmo to rotate the quaternion.");
                ui.gizmo_quat()
                    .builder()
                    .size(240.0)
                    .mode(Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN)
                    .quat("##rotation", &mut rotation);

                ui.text(format!(
                    "x: {:.3}  y: {:.3}  z: {:.3}  w: {:.3}",
                    rotation[0], rotation[1], rotation[2], rotation[3]
                ));
                if ui.button("Reset") {
                    rotation = [0.0, 0.0, 0.0, 1.0];
                }
            });
    })
}
