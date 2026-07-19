//! Sortable and resizable table using the declarative table builder.

use std::cmp::Ordering;

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::{Condition, SortDirection, TableColumnFlags, TableFlags, Ui};

struct Row {
    name: &'static str,
    quantity: i32,
    price: f32,
}

fn apply_sort(ui: &Ui, rows: &mut [Row]) {
    let Some(mut specs) = ui.table_get_sort_specs() else {
        return;
    };
    if !specs.is_dirty() {
        return;
    }

    if let Some(spec) = specs.iter().next() {
        let reverse = match spec.sort_direction {
            SortDirection::Ascending => false,
            SortDirection::Descending => true,
            SortDirection::None => return,
        };
        rows.sort_by(|left, right| {
            let ordering = match spec.column_index.get() {
                0 => left.name.cmp(right.name),
                1 => left.quantity.cmp(&right.quantity),
                2 => left
                    .price
                    .partial_cmp(&right.price)
                    .unwrap_or(Ordering::Equal),
                _ => Ordering::Equal,
            };
            if reverse {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }
    specs.clear_dirty();
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Tables".to_owned(),
        window_size: (900.0, 600.0),
        ..Default::default()
    };
    let mut rows = [
        Row {
            name: "Apples",
            quantity: 12,
            price: 1.2,
        },
        Row {
            name: "Bananas",
            quantity: 8,
            price: 0.9,
        },
        Row {
            name: "Cherries",
            quantity: 24,
            price: 2.6,
        },
        Row {
            name: "Dates",
            quantity: 5,
            price: 3.1,
        },
        Row {
            name: "Elderberry",
            quantity: 13,
            price: 4.2,
        },
    ];

    run_ui(config, move |ui| {
        ui.window("Inventory")
            .size([640.0, 400.0], Condition::FirstUseEver)
            .build(|| {
                ui.table("inventory")
                    .flags(
                        TableFlags::RESIZABLE
                            | TableFlags::REORDERABLE
                            | TableFlags::ROW_BG
                            | TableFlags::BORDERS
                            | TableFlags::SORTABLE,
                    )
                    .column("Name")
                    .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
                    .done()
                    .column("Quantity")
                    .done()
                    .column("Price")
                    .done()
                    .headers(true)
                    .build(|ui| {
                        apply_sort(ui, &mut rows);
                        for row in &rows {
                            ui.table_next_row();
                            ui.table_next_column();
                            ui.text(row.name);
                            ui.table_next_column();
                            ui.text(row.quantity.to_string());
                            ui.table_next_column();
                            ui.text(format!("${:.2}", row.price));
                        }
                    });
            });
    })
}
