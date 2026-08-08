//! Minimal ImGuizmo integration with the high-level `dear-app` runtime.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::Condition;
use dear_imguizmo::{GuizmoExt, Mode, Operation};
use glam::{Mat4, Vec3};

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - ImGuizmo Minimal".to_owned(),
        window_size: (900.0, 680.0),
        ..Default::default()
    };

    let mut model = Mat4::IDENTITY;
    let view = Mat4::look_at_rh(Vec3::new(4.0, 3.0, 6.0), Vec3::ZERO, Vec3::Y);

    run_ui(config, move |ui| {
        ui.window("ImGuizmo Minimal")
            .size([720.0, 560.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Drag an axis to translate the cube.");
                ui.same_line();
                if ui.button("Reset") {
                    model = Mat4::IDENTITY;
                }
                ui.separator();

                let canvas_position = ui.cursor_screen_pos();
                let available = ui.content_region_avail();
                let canvas_size = [available[0].max(1.0), available[1].max(1.0)];
                let projection = Mat4::perspective_rh_gl(
                    45.0_f32.to_radians(),
                    canvas_size[0] / canvas_size[1],
                    0.1,
                    100.0,
                );

                let gizmo = ui.guizmo();
                gizmo.set_drawlist_window();
                gizmo.set_rect(
                    canvas_position[0],
                    canvas_position[1],
                    canvas_size[0],
                    canvas_size[1],
                );
                gizmo.set_orthographic(false);
                gizmo.draw_grid(&view, &projection, &Mat4::IDENTITY, 10.0);
                gizmo
                    .manipulate_config(&view, &projection, &mut model)
                    .operation(Operation::TRANSLATE)
                    .mode(Mode::World)
                    .build();
                gizmo.draw_cubes(&view, &projection, std::slice::from_ref(&model));

                ui.dummy(canvas_size);
            });
    })
}
