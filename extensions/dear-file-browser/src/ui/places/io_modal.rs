use dear_imgui_rs::Ui;

use crate::dialog_state::FileDialogState;
use crate::places::Places;

pub(in crate::ui) fn draw_places_io_modal(ui: &Ui, state: &mut FileDialogState) {
    if state.ui.operations.places.io.open_next {
        ui.open_popup("Places");
        state.ui.operations.places.io.open_next = false;
    }

    if let Some(_popup) = ui.begin_modal_popup("Places") {
        let is_export =
            state.ui.operations.places.io.mode == crate::dialog_state::PlacesIoMode::Export;

        ui.text("Places persistence (compact format)");
        ui.separator();

        if ui.button("Export") {
            state.ui.operations.places.io.mode = crate::dialog_state::PlacesIoMode::Export;
            state.ui.operations.places.io.buffer =
                state
                    .core
                    .places
                    .serialize_compact(crate::PlacesSerializeOptions {
                        include_code_places: state.ui.operations.places.io.include_code,
                    });
            state.ui.operations.places.io.error = None;
        }
        ui.same_line();
        if ui.button("Import") {
            state.ui.operations.places.io.mode = crate::dialog_state::PlacesIoMode::Import;
            state.ui.operations.places.io.error = None;
        }
        ui.same_line();
        if ui.button("Close") {
            ui.close_current_popup();
            state.ui.operations.places.io.error = None;
        }

        ui.separator();

        if is_export {
            let mut include_code = state.ui.operations.places.io.include_code;
            if ui.checkbox("Include code places", &mut include_code) {
                state.ui.operations.places.io.include_code = include_code;
                state.ui.operations.places.io.buffer =
                    state
                        .core
                        .places
                        .serialize_compact(crate::PlacesSerializeOptions {
                            include_code_places: state.ui.operations.places.io.include_code,
                        });
            }
        }

        let avail = ui.content_region_avail();
        let size = [avail[0].max(200.0), (avail[1] - 95.0).max(120.0)];
        if is_export {
            ui.input_text_multiline(
                "##places_export",
                &mut state.ui.operations.places.io.buffer,
                size,
            )
            .read_only(true)
            .build();
        } else {
            ui.input_text_multiline(
                "##places_import",
                &mut state.ui.operations.places.io.buffer,
                size,
            )
            .build();

            if ui.button("Replace") {
                match Places::deserialize_compact(&state.ui.operations.places.io.buffer) {
                    Ok(p) => {
                        state.core.places = p;
                        state.ui.operations.places.io.error = None;
                    }
                    Err(e) => {
                        state.ui.operations.places.io.error = Some(e.to_string());
                    }
                }
            }
            ui.same_line();
            if ui.button("Merge") {
                match Places::deserialize_compact(&state.ui.operations.places.io.buffer) {
                    Ok(p) => {
                        state.core.places.merge_from(
                            p,
                            crate::places::PlacesMergeOptions {
                                overwrite_group_metadata: true,
                            },
                        );
                        state.ui.operations.places.io.error = None;
                    }
                    Err(e) => {
                        state.ui.operations.places.io.error = Some(e.to_string());
                    }
                }
            }
        }

        if let Some(err) = &state.ui.operations.places.io.error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}
