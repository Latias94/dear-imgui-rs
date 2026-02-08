use std::path::Path;

use crate::core::{ClickAction, SortMode};
use crate::dialog_core::EntryId;
use crate::dialog_state::{
    FileDialogState, FileListColumnsConfig, FileListViewMode, PasteConflictAction,
};
use crate::fs::FileSystem;
use dear_imgui_rs::Ui;

use super::file_table;

pub(super) fn draw_new_folder_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "New Folder";

    if !state.ui.new_folder_enabled {
        state.ui.new_folder_open_next = false;
        state.ui.new_folder_error = None;
        return;
    }

    if state.ui.new_folder_open_next {
        state.ui.new_folder_open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        ui.text("Create a new folder in:");
        ui.text_disabled(state.core.cwd.display().to_string());
        ui.separator();

        if state.ui.new_folder_focus_next {
            ui.set_keyboard_focus_here();
            state.ui.new_folder_focus_next = false;
        }
        ui.input_text("Name", &mut state.ui.new_folder_name).build();

        let create = ui.button("Create");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.new_folder_error = None;
            ui.close_current_popup();
        }

        if create {
            if try_create_new_folder_in_cwd(state, fs) {
                ui.close_current_popup();
            }
        }

        if let Some(err) = &state.ui.new_folder_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

pub(super) fn try_create_new_folder_in_cwd(
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
) -> bool {
    state.ui.new_folder_error = None;
    let name = state.ui.new_folder_name.trim();
    let invalid = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0');
    if invalid {
        state.ui.new_folder_error = Some("Invalid folder name".into());
        return false;
    }

    let name = name.to_string();
    let path = state.core.cwd.join(&name);
    match fs.create_dir(&path) {
        Ok(()) => {
            state.ui.new_folder_name.clear();
            let id = EntryId::from_path(&path);
            state.core.focus_and_select_by_id(id);
            state.ui.reveal_id_next = Some(id);
            state.core.invalidate_dir_cache();
            true
        }
        Err(e) => {
            state.ui.new_folder_error = Some(format!("Failed to create '{}': {}", name, e));
            false
        }
    }
}

pub(super) fn draw_columns_popup(ui: &Ui, state: &mut FileDialogState) {
    if let Some(_popup) = ui.begin_popup("##fb_columns_popup") {
        match state.ui.file_list_view {
            FileListViewMode::List => {
                let mut enabled = state.ui.thumbnails_enabled;
                if ui.checkbox("Enable thumbnails", &mut enabled) {
                    state.ui.thumbnails_enabled = enabled;
                }
                if state.ui.thumbnails_enabled {
                    ui.checkbox("Preview", &mut state.ui.file_list_columns.show_preview);
                } else {
                    ui.text_disabled("Preview (enable thumbnails)");
                }
            }
            FileListViewMode::ThumbnailsList => {
                ui.text_disabled("Preview (forced by Thumbs view)");
            }
            FileListViewMode::Grid => {}
        }
        ui.checkbox(
            file_table::extension_ui_label(state),
            &mut state.ui.file_list_columns.show_extension,
        );
        ui.checkbox("Size", &mut state.ui.file_list_columns.show_size);
        ui.checkbox("Modified", &mut state.ui.file_list_columns.show_modified);

        ui.separator();
        if ui.small_button("Compact") {
            if matches!(state.ui.file_list_view, FileListViewMode::ThumbnailsList) {
                file_table::apply_compact_column_layout_keep_preview(
                    &mut state.ui.file_list_columns,
                );
            } else {
                file_table::apply_compact_column_layout(&mut state.ui.file_list_columns);
            }
        }
        ui.same_line();
        if ui.small_button("Balanced") {
            if matches!(state.ui.file_list_view, FileListViewMode::ThumbnailsList) {
                file_table::apply_balanced_column_layout_keep_preview(
                    &mut state.ui.file_list_columns,
                );
            } else {
                file_table::apply_balanced_column_layout(&mut state.ui.file_list_columns);
            }
        }

        ui.separator();
        ui.text("Order:");
        let mut order = state.ui.file_list_columns.normalized_order();
        let mut changed = false;
        for index in 0..order.len() {
            let column = order[index];
            let mut label = file_table::data_column_label_for_state(state, column).to_string();
            if !file_table::is_data_column_visible(&state.ui.file_list_columns, column) {
                label.push_str(" (hidden)");
            }
            ui.text(label);
            ui.same_line();
            if ui.small_button(format!("Up##col_order_up_{index}")) {
                changed |= file_table::move_column_order_up(&mut order, index);
            }
            ui.same_line();
            if ui.small_button(format!("Down##col_order_down_{index}")) {
                changed |= file_table::move_column_order_down(&mut order, index);
            }
        }
        if changed {
            state.ui.file_list_columns.order = order;
        }

        if ui.small_button("Reset columns") {
            state.ui.file_list_columns = FileListColumnsConfig::default();
        }

        ui.separator();
        let mut natural_sort = matches!(state.core.sort_mode, SortMode::Natural);
        if ui.checkbox("Natural sort", &mut natural_sort) {
            state.core.sort_mode = if natural_sort {
                SortMode::Natural
            } else {
                SortMode::Lexicographic
            };
        }
    }
}

pub(super) fn draw_options_popup(
    ui: &Ui,
    state: &mut FileDialogState,
    has_thumbnail_backend: bool,
) {
    if let Some(_popup) = ui.begin_popup("##fb_options") {
        let mut nav_on_click = matches!(state.core.click_action, ClickAction::Navigate);
        if ui.checkbox("Navigate on click", &mut nav_on_click) {
            state.core.click_action = if nav_on_click {
                ClickAction::Navigate
            } else {
                ClickAction::Select
            };
        }
        let mut dbl = state.core.double_click;
        if ui.checkbox("DblClick confirm", &mut dbl) {
            state.core.double_click = dbl;
        }
        let mut quick = state.ui.breadcrumbs_quick_select;
        if ui.checkbox("Quick path select", &mut quick) {
            state.ui.breadcrumbs_quick_select = quick;
        }
        let mut show_hidden = state.core.show_hidden;
        if ui.checkbox("Show hidden", &mut show_hidden) {
            state.core.show_hidden = show_hidden;
        }
        ui.separator();
        ui.text_disabled("Thumbnails:");
        ui.text("Size:");
        ui.same_line();
        if ui.small_button("S##thumb_size") {
            state.ui.thumbnail_size = [20.0, 20.0];
        }
        ui.same_line();
        if ui.small_button("M##thumb_size") {
            state.ui.thumbnail_size = [32.0, 32.0];
        }
        ui.same_line();
        if ui.small_button("L##thumb_size") {
            state.ui.thumbnail_size = [48.0, 48.0];
        }
        if !has_thumbnail_backend {
            ui.same_line();
            ui.text_disabled("No thumbnail backend");
        }
        ui.separator();
        ui.text_disabled("Shortcuts:");
        ui.bullet_text("Ctrl+L: focus Path");
        ui.bullet_text("Ctrl+F: focus Search");
        ui.bullet_text("Alt+Left/Right: back/forward");
        ui.bullet_text("Backspace: up");
        ui.bullet_text("F5: refresh");
        ui.bullet_text("Tab: path completion");
        ui.bullet_text("Up/Down: path history");
    }
}

pub(super) fn draw_rename_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Rename";

    if state.ui.rename_open_next {
        state.ui.rename_open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        let Some(target_id) = state.ui.rename_target_id else {
            ui.text_disabled("No entry selected for rename.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        };

        let Some(from_path) = state
            .core
            .entry_path_by_id(target_id)
            .map(Path::to_path_buf)
        else {
            ui.text_disabled("Selected entry is no longer available.");
            if ui.button("Close") {
                state.ui.rename_target_id = None;
                ui.close_current_popup();
            }
            return;
        };

        let from_name = from_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| from_path.display().to_string());

        ui.text("Rename in:");
        ui.text_disabled(state.core.cwd.display().to_string());
        ui.separator();
        ui.text(format!("From: {from_name}"));

        if state.ui.rename_focus_next {
            ui.set_keyboard_focus_here();
            state.ui.rename_focus_next = false;
        }
        ui.input_text("To", &mut state.ui.rename_to).build();

        let rename = ui.button("Rename");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.rename_error = None;
            state.ui.rename_target_id = None;
            ui.close_current_popup();
        }

        if rename {
            state.ui.rename_error = None;
            let to_name = state.ui.rename_to.trim();
            let invalid = to_name.is_empty()
                || to_name == "."
                || to_name == ".."
                || to_name.contains('/')
                || to_name.contains('\\')
                || to_name.contains('\0');
            if invalid {
                state.ui.rename_error = Some("Invalid target name".into());
            } else if to_name == from_name.as_str() {
                state.ui.rename_error = Some("Target name is unchanged".into());
            } else {
                let to_name = to_name.to_string();
                let to_path = from_path.with_file_name(&to_name);

                if fs.metadata(&to_path).is_ok() {
                    state.ui.rename_error = Some("Target already exists".into());
                } else {
                    match fs.rename(&from_path, &to_path) {
                        Ok(()) => {
                            let id = EntryId::from_path(&to_path);
                            state.core.focus_and_select_by_id(id);
                            state.ui.reveal_id_next = Some(id);
                            state.core.invalidate_dir_cache();
                            state.ui.rename_target_id = None;
                            state.ui.rename_to.clear();
                            ui.close_current_popup();
                        }
                        Err(e) => {
                            state.ui.rename_error =
                                Some(format!("Failed to rename '{from_name}': {e}"));
                        }
                    }
                }
            }
        }

        if let Some(err) = &state.ui.rename_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

pub(super) fn draw_delete_confirm_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Delete";

    if state.ui.delete_open_next {
        state.ui.delete_open_next = false;
        state.ui.delete_recursive = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        if state.ui.delete_target_ids.is_empty() {
            ui.text_disabled("No entries selected for deletion.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        }

        let delete_targets = state
            .ui
            .delete_target_ids
            .iter()
            .copied()
            .filter_map(|id| state.core.entry_path_by_id(id).map(Path::to_path_buf))
            .collect::<Vec<_>>();

        if delete_targets.len() != state.ui.delete_target_ids.len() {
            ui.text_disabled("Some selected entries are no longer available.");
            if ui.button("Close") {
                state.ui.delete_error = None;
                state.ui.delete_target_ids.clear();
                state.ui.delete_recursive = false;
                ui.close_current_popup();
            }
            return;
        }

        let delete_target_names = delete_targets
            .iter()
            .map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| path.display().to_string())
            })
            .collect::<Vec<_>>();

        ui.text(format!(
            "Delete {} entr{} in:",
            delete_target_names.len(),
            if delete_target_names.len() == 1 {
                "y"
            } else {
                "ies"
            }
        ));
        ui.text_disabled(state.core.cwd.display().to_string());
        ui.separator();

        let preview_n = 6usize.min(delete_target_names.len());
        for name in delete_target_names.iter().take(preview_n) {
            ui.text(name);
        }
        if delete_target_names.len() > preview_n {
            ui.text_disabled(format!(
                "... and {} more",
                delete_target_names.len() - preview_n
            ));
        }

        ui.separator();

        let any_dir = delete_targets
            .iter()
            .any(|path| fs.metadata(path).map(|m| m.is_dir).unwrap_or(false));
        if any_dir {
            ui.checkbox("Recursive", &mut state.ui.delete_recursive);
            ui.same_line();
            ui.text_disabled("Delete directories with contents");
        } else {
            state.ui.delete_recursive = false;
        }

        ui.separator();
        let del = ui.button("Delete");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.delete_error = None;
            state.ui.delete_target_ids.clear();
            state.ui.delete_recursive = false;
            ui.close_current_popup();
        }

        if del {
            state.ui.delete_error = None;
            let recursive = state.ui.delete_recursive;
            for (path, name) in delete_targets.iter().zip(delete_target_names.iter()) {
                let is_dir = fs.metadata(path).map(|m| m.is_dir).unwrap_or(false);
                let result = if is_dir {
                    if recursive {
                        fs.remove_dir_all(path)
                    } else {
                        fs.remove_dir(path)
                    }
                } else {
                    fs.remove_file(path)
                };
                if let Err(e) = result {
                    state.ui.delete_error = Some(format!("Failed to delete '{name}': {e}"));
                    break;
                }
            }

            if state.ui.delete_error.is_none() {
                state.core.clear_selection();
                state.core.invalidate_dir_cache();
                state.ui.delete_target_ids.clear();
                state.ui.delete_recursive = false;
                ui.close_current_popup();
            }
        }

        if let Some(err) = &state.ui.delete_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

pub(super) fn draw_paste_conflict_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Paste Conflict";

    if state.ui.paste_conflict_open_next {
        state.ui.paste_conflict_open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        let prompt = state
            .ui
            .paste_job
            .as_ref()
            .and_then(|j| j.conflict.as_ref())
            .cloned();

        let Some(prompt) = prompt else {
            ui.text_disabled("No pending paste conflict.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        };

        let src_name = prompt
            .source
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| prompt.source.display().to_string());

        ui.text(format!("Target already exists: {src_name}"));
        ui.text_disabled(format!("Source: {}", prompt.source.display()));
        ui.text_disabled(format!("Target: {}", prompt.dest.display()));
        ui.separator();

        let mut apply_to_all = prompt.apply_to_all;
        ui.checkbox("Apply to all conflicts", &mut apply_to_all);

        ui.separator();
        let overwrite = ui.button("Overwrite");
        ui.same_line();
        let keep_both = ui.button("Keep Both");
        ui.same_line();
        let skip = ui.button("Skip");
        ui.same_line();
        let cancel = ui.button("Cancel Paste");

        if cancel {
            state.ui.paste_job = None;
            ui.close_current_popup();
            return;
        }

        let selected = if overwrite {
            Some(PasteConflictAction::Overwrite)
        } else if keep_both {
            Some(PasteConflictAction::KeepBoth)
        } else if skip {
            Some(PasteConflictAction::Skip)
        } else {
            None
        };

        if let Some(action) = selected {
            if let Some(job) = state.ui.paste_job.as_mut() {
                if apply_to_all {
                    job.apply_all_conflicts = Some(action);
                }
                job.pending_conflict_action = Some(action);
                job.conflict = None;
            }
            ui.close_current_popup();
            state.ui.ui_error = None;
            if let Err(e) = super::ops::run_paste_job_until_wait_or_done(state, fs) {
                state.ui.ui_error = Some(e);
                state.ui.paste_job = None;
            }
        } else if let Some(job) = state.ui.paste_job.as_mut() {
            if let Some(conflict) = job.conflict.as_mut() {
                conflict.apply_to_all = apply_to_all;
            }
        }
    }
}

// Places helpers live in `places.rs`.
