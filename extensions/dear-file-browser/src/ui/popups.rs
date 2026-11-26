use std::path::Path;

use crate::core::{ClickAction, SortMode};
use crate::dialog_core::{EntryId, ScanStatus};
use crate::dialog_state::{
    FileDialogState, FileListColumnsConfig, FileListViewMode, PasteConflictAction,
};
use dear_imgui_rs::Ui;

use super::file_table;

fn is_entry_name(name: &str) -> bool {
    !name.contains(['/', '\\', '\0'])
        && !(cfg!(windows) && name.contains(':'))
        && Path::new(name).file_name() == Some(std::ffi::OsStr::new(name))
}

pub(super) fn draw_new_folder_modal(ui: &Ui, state: &mut FileDialogState) {
    const POPUP_ID: &str = "New Folder";

    if !state.ui.config.new_folder_enabled {
        state.ui.operations.new_folder.open_next = false;
        state.ui.operations.new_folder.error = None;
        return;
    }

    if state.ui.operations.new_folder.open_next {
        state.ui.operations.new_folder.open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        ui.text("Create a new folder in:");
        ui.text_disabled(crate::fs::display_path(&state.core.cwd).as_ref());
        ui.separator();

        if state.ui.operations.new_folder.focus_next {
            ui.set_keyboard_focus_here();
            state.ui.operations.new_folder.focus_next = false;
        }
        ui.input_text("Name", &mut state.ui.operations.new_folder.name)
            .build();

        let create = ui.button("Create");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.operations.new_folder.error = None;
            ui.close_current_popup();
        }

        if create && try_create_new_folder_in_cwd(state) {
            ui.close_current_popup();
        }

        if let Some(err) = &state.ui.operations.new_folder.error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

pub(super) fn try_create_new_folder_in_cwd(state: &mut FileDialogState) -> bool {
    if super::ops::file_operation_in_progress(state) {
        state.ui.operations.new_folder.error = Some("Another file operation is in progress".into());
        return false;
    }
    state.ui.operations.new_folder.error = None;
    let name = state.ui.operations.new_folder.name.trim();
    if !is_entry_name(name) {
        state.ui.operations.new_folder.error = Some("Invalid folder name".into());
        return false;
    }

    let name = name.to_string();
    let path = state.core.cwd.join(&name);
    let create_result = state.filesystem.as_file_system().create_dir(&path);
    match create_result {
        Ok(()) => {
            state.ui.operations.new_folder.name.clear();
            let id = EntryId::from_path(&path);
            state.core.focus_and_select_by_id(id);
            state.ui.operations.reveal_id_next = Some(id);
            state.core.invalidate_dir_cache();
            true
        }
        Err(e) => {
            state.ui.operations.new_folder.error =
                Some(format!("Failed to create '{}': {}", name, e));
            false
        }
    }
}

pub(super) fn draw_columns_popup(
    ui: &Ui,
    state: &mut FileDialogState,
    has_thumbnail_backend: bool,
) {
    if let Some(_popup) = ui.begin_popup("##fb_columns_popup") {
        match state.ui.config.file_list_view {
            FileListViewMode::List => {
                let mut enabled = state.ui.config.thumbnails_enabled;
                {
                    let _disabled = ui.begin_disabled_with_cond(!has_thumbnail_backend);
                    if ui.checkbox("Enable thumbnails", &mut enabled) {
                        state.ui.config.thumbnails_enabled = enabled;
                    }
                }
                if !has_thumbnail_backend {
                    ui.same_line();
                    ui.text_disabled("No thumbnail backend");
                }
                if state.ui.config.thumbnails_enabled && has_thumbnail_backend {
                    ui.checkbox(
                        "Preview",
                        &mut state.ui.config.file_list_columns.show_preview,
                    );
                } else if has_thumbnail_backend {
                    ui.text_disabled("Preview (enable thumbnails)");
                } else {
                    ui.text_disabled("Preview (no thumbnail backend)");
                }
            }
            FileListViewMode::ThumbnailsList => {
                ui.text_disabled("Preview (forced by Thumbs view)");
            }
            FileListViewMode::Grid => {}
        }
        ui.checkbox(
            file_table::extension_ui_label(state),
            &mut state.ui.config.file_list_columns.show_extension,
        );
        ui.checkbox("Size", &mut state.ui.config.file_list_columns.show_size);
        ui.checkbox(
            "Modified",
            &mut state.ui.config.file_list_columns.show_modified,
        );

        ui.separator();
        if ui.small_button("Compact") {
            if matches!(
                state.ui.config.file_list_view,
                FileListViewMode::ThumbnailsList
            ) {
                file_table::apply_compact_column_layout_keep_preview(
                    &mut state.ui.config.file_list_columns,
                );
            } else {
                file_table::apply_compact_column_layout(&mut state.ui.config.file_list_columns);
            }
        }
        ui.same_line();
        if ui.small_button("Balanced") {
            if matches!(
                state.ui.config.file_list_view,
                FileListViewMode::ThumbnailsList
            ) {
                file_table::apply_balanced_column_layout_keep_preview(
                    &mut state.ui.config.file_list_columns,
                );
            } else {
                file_table::apply_balanced_column_layout(&mut state.ui.config.file_list_columns);
            }
        }

        ui.separator();
        ui.text("Order:");
        let mut order = state.ui.config.file_list_columns.normalized_order();
        let mut changed = false;
        for index in 0..order.len() {
            let column = order[index];
            let mut label = file_table::data_column_label_for_state(state, column).to_string();
            if !file_table::is_data_column_visible(&state.ui.config.file_list_columns, column) {
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
            state.ui.config.file_list_columns.order = order;
        }

        if ui.small_button("Reset columns") {
            state.ui.config.file_list_columns = FileListColumnsConfig::default();
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
        let mut quick = state.ui.config.breadcrumbs_quick_select;
        if ui.checkbox("Quick path select", &mut quick) {
            state.ui.config.breadcrumbs_quick_select = quick;
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
            state.ui.config.thumbnail_size = [20.0, 20.0];
        }
        ui.same_line();
        if ui.small_button("M##thumb_size") {
            state.ui.config.thumbnail_size = [32.0, 32.0];
        }
        ui.same_line();
        if ui.small_button("L##thumb_size") {
            state.ui.config.thumbnail_size = [48.0, 48.0];
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

pub(super) fn draw_rename_modal(ui: &Ui, state: &mut FileDialogState) {
    const POPUP_ID: &str = "Rename";

    if state.ui.operations.rename.open_next {
        state.ui.operations.rename.open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        let Some(target_id) = state.ui.operations.rename.target_id else {
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
                state.ui.operations.rename.target_id = None;
                ui.close_current_popup();
            }
            return;
        };

        let from_name = from_path
            .file_name()
            .filter(|name| !name.is_empty())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| from_path.display().to_string());

        ui.text("Rename in:");
        ui.text_disabled(crate::fs::display_path(&state.core.cwd).as_ref());
        ui.separator();
        ui.text(format!("From: {from_name}"));

        if state.ui.operations.rename.focus_next {
            ui.set_keyboard_focus_here();
            state.ui.operations.rename.focus_next = false;
        }
        ui.input_text("To", &mut state.ui.operations.rename.to)
            .build();

        let rename = {
            let _disabled =
                ui.begin_disabled_with_cond(super::ops::file_operation_in_progress(state));
            ui.button("Rename")
        };
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.operations.rename.error = None;
            state.ui.operations.rename.target_id = None;
            ui.close_current_popup();
        }

        if rename {
            state.ui.operations.rename.error = None;
            let to_name = state.ui.operations.rename.to.trim();
            if !is_entry_name(to_name) {
                state.ui.operations.rename.error = Some("Invalid target name".into());
            } else if state.ui.operations.rename.to == from_name || to_name == from_name.as_str() {
                state.ui.operations.rename.error = Some("Target name is unchanged".into());
            } else {
                let to_name = to_name.to_string();
                let to_path = from_path.with_file_name(&to_name);

                match state
                    .filesystem
                    .as_file_system()
                    .rename_noreplace(&from_path, &to_path)
                {
                    Ok(()) => {
                        let id = EntryId::from_path(&to_path);
                        state.core.focus_and_select_by_id(id);
                        state.ui.operations.reveal_id_next = Some(id);
                        state.core.invalidate_dir_cache();
                        state.ui.operations.rename.target_id = None;
                        state.ui.operations.rename.to.clear();
                        ui.close_current_popup();
                    }
                    Err(e) => {
                        state.ui.operations.rename.error = Some(format!(
                            "Failed to rename '{from_name}' to '{to_name}': {e}"
                        ));
                    }
                }
            }
        }

        if let Some(err) = &state.ui.operations.rename.error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

pub(super) fn draw_delete_confirm_modal(ui: &Ui, state: &mut FileDialogState) {
    const POPUP_ID: &str = "Delete";

    if state.ui.operations.delete.open_next {
        let delete = &state.ui.operations.delete;
        if delete.error.is_some()
            && matches!(
                state.scan_status(),
                ScanStatus::Scanning { .. } | ScanStatus::Partial { .. }
            )
            && delete
                .target_ids
                .iter()
                .any(|&id| state.core.entry_by_id(id).is_none())
        {
            return;
        }
        state.ui.operations.delete.open_next = false;
        state.ui.operations.delete.recursive = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        if state.ui.operations.delete.target_ids.is_empty() {
            ui.text_disabled("No entries selected for deletion.");
            if ui.button("Close") {
                ui.close_current_popup();
            }
            return;
        }

        let mut any_dir = false;
        let delete_targets = state
            .ui
            .operations
            .delete
            .target_ids
            .iter()
            .copied()
            .filter_map(|id| {
                let entry = state.core.entry_by_id(id)?;
                any_dir |= entry.is_dir && !entry.is_symlink;
                Some(entry.path.clone())
            })
            .collect::<Vec<_>>();

        if delete_targets.len() != state.ui.operations.delete.target_ids.len() {
            ui.text_disabled("Some selected entries are no longer available.");
            if let Some(err) = &state.ui.operations.delete.error {
                ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
            }
            if ui.button("Close") {
                state.ui.operations.delete.error = None;
                state.ui.operations.delete.target_ids.clear();
                state.ui.operations.delete.recursive = false;
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
        ui.text_disabled(crate::fs::display_path(&state.core.cwd).as_ref());
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

        if any_dir {
            ui.checkbox("Recursive", &mut state.ui.operations.delete.recursive);
            ui.same_line();
            ui.text_disabled("Delete directories with contents");
        } else {
            state.ui.operations.delete.recursive = false;
        }

        ui.separator();
        let del = ui.button("Delete");
        ui.same_line();
        let cancel = ui.button("Cancel");
        if cancel {
            state.ui.operations.delete.error = None;
            state.ui.operations.delete.target_ids.clear();
            state.ui.operations.delete.recursive = false;
            ui.close_current_popup();
        }

        if del && super::ops::delete_targets(state, &delete_targets) {
            ui.close_current_popup();
        }

        if let Some(err) = &state.ui.operations.delete.error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

pub(super) fn draw_paste_conflict_modal(ui: &Ui, state: &mut FileDialogState) {
    const POPUP_ID: &str = "Paste Conflict";

    if state.ui.operations.paste.conflict_open_next {
        state.ui.operations.paste.conflict_open_next = false;
        if !ui.is_popup_open(POPUP_ID) {
            ui.open_popup(POPUP_ID);
        }
    }

    if let Some(_popup) = ui.begin_modal_popup(POPUP_ID) {
        let prompt = state
            .ui
            .operations
            .paste
            .job
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
        ui.text_disabled(format!(
            "Source: {}",
            crate::fs::display_path(&prompt.source)
        ));
        ui.text_disabled(format!("Target: {}", crate::fs::display_path(&prompt.dest)));
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
            super::ops::cancel_paste_job(state);
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
            if let Some(job) = state.ui.operations.paste.job.as_mut() {
                if apply_to_all {
                    job.apply_all_conflicts = Some(action);
                }
                job.pending_conflict_action = Some(action);
                job.conflict = None;
            }
            ui.close_current_popup();
            state.ui.runtime.error = super::ops::run_paste_job_until_wait_or_done(state).err();
        } else if let Some(job) = state.ui.operations.paste.job.as_mut() {
            if let Some(conflict) = job.conflict.as_mut() {
                conflict.apply_to_all = apply_to_all;
            }
        }
    }
}

// Places helpers live in `places.rs`.

#[cfg(windows)]
#[test]
fn rename_and_new_folder_names_stay_in_the_current_directory() {
    assert!(is_entry_name("report.txt"));
    assert!(!is_entry_name("D:report.txt"));
    assert!(!is_entry_name("report.txt:stream"));
}
