//! A two-column property grid built with the declarative table API.

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::{Condition, TableColumnSetup, TableFlags, TableSizingPolicy, Ui};

struct Properties {
    name: String,
    visible: bool,
    speed: f32,
    size: [f32; 2],
    color: [f32; 4],
    mode: usize,
    notes: String,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            name: "Player".to_owned(),
            visible: true,
            speed: 3.5,
            size: [1.0, 2.0],
            color: [0.15, 0.65, 0.95, 1.0],
            mode: 0,
            notes: "Supports multi-line text.\nUseful for descriptions.".to_owned(),
        }
    }
}

fn property_row(ui: &Ui, label: &str, editor: impl FnOnce()) {
    ui.table_next_row();
    ui.table_set_column_index(0);
    ui.text(label);
    ui.table_set_column_index(1);
    let _width = ui.push_item_width(-1.0);
    editor();
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Property Grid".to_owned(),
        window_size: (900.0, 640.0),
        ..Default::default()
    };
    let mut properties = Properties::default();

    run_ui(config, move |ui| {
        ui.window("Property Grid")
            .size([760.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                let columns = [
                    TableColumnSetup::new("Property").fixed_width(180.0),
                    TableColumnSetup::new("Value").stretch_weight(1.0),
                ];
                ui.table("properties")
                    .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::ROW_BG)
                    .sizing_policy(TableSizingPolicy::StretchProp)
                    .columns(columns)
                    .freeze(1, 0)
                    .headers(true)
                    .build(|ui| {
                        property_row(ui, "Name", || {
                            ui.input_text("##name", &mut properties.name).build();
                        });
                        property_row(ui, "Visible", || {
                            ui.checkbox("##visible", &mut properties.visible);
                        });
                        property_row(ui, "Speed", || {
                            ui.drag_float_config("##speed")
                                .range(0.0, 20.0)
                                .speed(0.1)
                                .try_display_format("%.2f")
                                .expect("static numeric format is valid")
                                .build(ui, &mut properties.speed);
                        });
                        property_row(ui, "Size", || {
                            ui.input_float2("##size", &mut properties.size).build();
                        });
                        property_row(ui, "Color", || {
                            ui.color_edit4("##color", &mut properties.color);
                        });
                        property_row(ui, "Mode", || {
                            const MODES: [&str; 3] = ["Idle", "Walking", "Running"];
                            if let Some(_combo) = ui.begin_combo("##mode", MODES[properties.mode]) {
                                for (index, mode) in MODES.iter().enumerate() {
                                    if ui
                                        .selectable_config(mode)
                                        .selected(properties.mode == index)
                                        .build()
                                    {
                                        properties.mode = index;
                                    }
                                }
                            }
                        });
                        property_row(ui, "Notes", || {
                            ui.input_text_multiline("##notes", &mut properties.notes, [0.0, 80.0])
                                .build();
                        });
                    });
            });
    })
}
