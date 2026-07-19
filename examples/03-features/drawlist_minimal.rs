//! Basic primitives drawn inside a window with its draw list.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::Condition;

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Draw Lists".to_owned(),
        window_size: (900.0, 600.0),
        ..Default::default()
    };
    let mut thickness = 2.0;

    run_ui(config, move |ui| {
        ui.window("Draw List")
            .size([720.0, 500.0], Condition::FirstUseEver)
            .build(|| {
                ui.slider("Thickness", 1.0, 10.0, &mut thickness);
                ui.separator();

                let size = ui.content_region_avail();
                if size[0] <= 0.0 || size[1] <= 0.0 {
                    return;
                }

                let draw_list = ui.get_window_draw_list();
                let origin = ui.cursor_screen_pos();
                let [width, height] = size;
                draw_list
                    .add_rect(
                        origin,
                        [origin[0] + width, origin[1] + height],
                        [0.15, 0.16, 0.19, 1.0],
                    )
                    .filled(true)
                    .build();
                draw_list
                    .add_line(
                        [origin[0] + 20.0, origin[1] + 20.0],
                        [origin[0] + width - 20.0, origin[1] + 20.0],
                        [0.9, 0.7, 0.2, 1.0],
                    )
                    .thickness(thickness)
                    .build();
                draw_list
                    .add_rect(
                        [origin[0] + 40.0, origin[1] + 60.0],
                        [origin[0] + 200.0, origin[1] + 160.0],
                        [0.2, 0.7, 0.9, 1.0],
                    )
                    .rounding(8.0)
                    .thickness(thickness)
                    .build();
                draw_list
                    .add_rect(
                        [origin[0] + 220.0, origin[1] + 60.0],
                        [origin[0] + 380.0, origin[1] + 160.0],
                        [0.2, 0.9, 0.5, 1.0],
                    )
                    .filled(true)
                    .rounding(8.0)
                    .build();
                draw_list
                    .add_circle(
                        [origin[0] + 500.0, origin[1] + 110.0],
                        50.0,
                        [0.95, 0.4, 0.3, 1.0],
                    )
                    .thickness(thickness)
                    .build();
                draw_list.add_text(
                    [origin[0] + 20.0, origin[1] + height - 30.0],
                    [1.0, 1.0, 1.0, 1.0],
                    "Draw-list primitives",
                );
            });
    })
}
