use std::path::PathBuf;

use dear_imgui_rs::Ui;

use crate::dialog_state::FileDialogState;
use crate::fs::FileSystem;
use crate::places::{Place, PlaceOrigin, Places};

pub(in crate::ui) fn draw_places_edit_modal(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
) {
    const POPUP_ID: &str = "Edit Places";
    if state.ui.operations.places.edit.open_next {
        ui.open_popup(POPUP_ID);
        state.ui.operations.places.edit.open_next = false;
    }

    let Some(_popup) = ui.begin_modal_popup(POPUP_ID) else {
        return;
    };

    use crate::dialog_state::PlacesEditMode;
    let mode = state.ui.operations.places.edit.mode;
    match mode {
        PlacesEditMode::AddGroup => {
            ui.text("Create a new places group:");
            ui.separator();
            if state.ui.operations.places.edit.focus_next {
                ui.set_keyboard_focus_here();
                state.ui.operations.places.edit.focus_next = false;
            }
            ui.input_text("Group", &mut state.ui.operations.places.edit.group)
                .build();

            let create = ui.button("Create");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.operations.places.edit.error = None;
                ui.close_current_popup();
                return;
            }
            if create {
                state.ui.operations.places.edit.error = None;
                let label = state.ui.operations.places.edit.group.trim();
                if label.is_empty() {
                    state.ui.operations.places.edit.error = Some("Group name is empty".into());
                } else if label == Places::SYSTEM_GROUP || label == Places::BOOKMARKS_GROUP {
                    state.ui.operations.places.edit.error = Some("Group name is reserved".into());
                } else if state.core.places.groups.iter().any(|g| g.label == label) {
                    state.ui.operations.places.edit.error = Some("Group already exists".into());
                } else {
                    state.core.places.add_group(label.to_string());
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::RenameGroup => {
            let Some(from) = state.ui.operations.places.edit.group_from.clone() else {
                ui.text_disabled("Missing source group.");
                if ui.button("Close") {
                    ui.close_current_popup();
                }
                return;
            };
            ui.text("Rename group:");
            ui.text_disabled(&from);
            ui.separator();
            if state.ui.operations.places.edit.focus_next {
                ui.set_keyboard_focus_here();
                state.ui.operations.places.edit.focus_next = false;
            }
            ui.input_text("To", &mut state.ui.operations.places.edit.group)
                .build();

            let rename = ui.button("Rename");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.operations.places.edit.error = None;
                ui.close_current_popup();
                return;
            }
            if rename {
                state.ui.operations.places.edit.error = None;
                let to = state.ui.operations.places.edit.group.trim();
                if to.is_empty() {
                    state.ui.operations.places.edit.error =
                        Some("Target group name is empty".into());
                } else if to == Places::SYSTEM_GROUP || to == Places::BOOKMARKS_GROUP {
                    state.ui.operations.places.edit.error =
                        Some("Target group name is reserved".into());
                } else if to == from.as_str() {
                    state.ui.operations.places.edit.error =
                        Some("Target group name is unchanged".into());
                } else if state.core.places.groups.iter().any(|g| g.label == to) {
                    state.ui.operations.places.edit.error =
                        Some("Target group already exists".into());
                } else if !state.core.places.rename_group(&from, to.to_string()) {
                    state.ui.operations.places.edit.error = Some("Group not found".into());
                } else {
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::RemoveGroupConfirm => {
            let Some(group) = state.ui.operations.places.edit.group_from.clone() else {
                ui.text_disabled("Missing group.");
                if ui.button("Close") {
                    ui.close_current_popup();
                }
                return;
            };

            let places_count = state
                .core
                .places
                .groups
                .iter()
                .find(|g| g.label == group)
                .map(|g| g.places.len())
                .unwrap_or(0);

            ui.text("Remove group?");
            ui.separator();
            ui.text(format!("Group: {group}"));
            ui.text_disabled(format!("Places: {places_count}"));
            ui.separator();
            let remove = ui.button("Remove");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.operations.places.edit.error = None;
                ui.close_current_popup();
                return;
            }
            if remove {
                state.ui.operations.places.edit.error = None;
                if group == Places::SYSTEM_GROUP || group == Places::BOOKMARKS_GROUP {
                    state.ui.operations.places.edit.error =
                        Some("Cannot remove reserved group".into());
                } else if !state.core.places.remove_group(&group) {
                    state.ui.operations.places.edit.error = Some("Group not found".into());
                } else {
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::AddPlace | PlacesEditMode::EditPlace => {
            let is_add = mode == PlacesEditMode::AddPlace;
            let group = state.ui.operations.places.edit.group.clone();
            ui.text(if is_add { "Add place:" } else { "Edit place:" });
            ui.text_disabled(&group);
            ui.separator();

            if state.ui.operations.places.edit.focus_next {
                ui.set_keyboard_focus_here();
                state.ui.operations.places.edit.focus_next = false;
            }
            ui.input_text("Label", &mut state.ui.operations.places.edit.place_label)
                .build();
            ui.input_text("Path", &mut state.ui.operations.places.edit.place_path)
                .build();

            let ok_label = if is_add { "Add" } else { "Save" };
            let ok = ui.button(ok_label);
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.operations.places.edit.error = None;
                ui.close_current_popup();
                return;
            }

            if ok {
                state.ui.operations.places.edit.error = None;
                let path_s = state.ui.operations.places.edit.place_path.trim();
                if path_s.is_empty() {
                    state.ui.operations.places.edit.error = Some("Path is empty".into());
                } else {
                    let raw = PathBuf::from(path_s);
                    let p = fs.canonicalize(&raw).unwrap_or(raw);
                    let is_dir = fs.metadata(&p).map(|m| m.is_dir).unwrap_or(false);
                    if !is_dir {
                        state.ui.operations.places.edit.error =
                            Some("Path does not exist or is not a directory".into());
                    } else {
                        let mut label = state
                            .ui
                            .operations
                            .places
                            .edit
                            .place_label
                            .trim()
                            .to_string();
                        if label.is_empty() {
                            label = p
                                .file_name()
                                .and_then(|s| s.to_str())
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| p.display().to_string());
                        }

                        let group_places = state
                            .core
                            .places
                            .groups
                            .iter()
                            .find(|g| g.label == group)
                            .map(|g| g.places.clone())
                            .unwrap_or_default();

                        let from_path = state.ui.operations.places.edit.place_from_path.clone();
                        let is_duplicate = group_places.iter().any(|x| {
                            if let Some(from) = &from_path {
                                if x.path == *from {
                                    return false;
                                }
                            }
                            x.path == p
                        });
                        if is_duplicate {
                            state.ui.operations.places.edit.error =
                                Some("Place already exists in group".into());
                        } else if is_add {
                            state
                                .core
                                .places
                                .add_place(group, Place::new(label, p, PlaceOrigin::User));
                            ui.close_current_popup();
                        } else {
                            let Some(from_path) = from_path else {
                                state.ui.operations.places.edit.error =
                                    Some("Missing source place".into());
                                return;
                            };
                            if !state
                                .core
                                .places
                                .edit_place_by_path(&group, &from_path, label, p)
                            {
                                state.ui.operations.places.edit.error =
                                    Some("Place not found".into());
                            } else {
                                ui.close_current_popup();
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(err) = &state.ui.operations.places.edit.error {
        ui.separator();
        ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
    }
}
