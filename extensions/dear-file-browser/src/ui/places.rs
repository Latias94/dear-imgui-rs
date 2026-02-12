use std::path::{Path, PathBuf};

use crate::core::LayoutStyle;
use crate::dialog_core::CoreEvent;
use crate::dialog_state::FileDialogState;
use crate::fs::FileSystem;
use crate::places::{Place, PlaceOrigin, Places};
use dear_imgui_rs::input::{Key, MouseButton};
use dear_imgui_rs::{
    SelectableFlags, StyleColor, TableColumnFlags, TableColumnSetup, TableFlags, TreeNodeFlags, Ui,
};

fn subtle_separator(ui: &Ui) {
    let style = ui.clone_style();
    let mut col = style.color(StyleColor::Separator);
    col[3] *= 0.35;
    let _col = ui.push_style_color(StyleColor::Separator, col);
    ui.separator();
}

pub(super) fn draw_minimal_places_popup(ui: &Ui, state: &mut FileDialogState) {
    if !matches!(state.ui.layout, LayoutStyle::Minimal) {
        return;
    }
    if let Some(_popup) = ui.begin_popup("##fb_places_popup") {
        ui.text_disabled("Places");
        ui.separator();
        let mut new_cwd: Option<PathBuf> = None;
        ui.child_window("##fb_places_popup_child")
            .size([280.0, 380.0])
            .border(true)
            .build(ui, || {
                new_cwd = draw_places_pane(ui, state);
            });
        if let Some(p) = new_cwd {
            let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
            ui.close_current_popup();
        }
    }
}

pub(super) fn draw_places_pane(ui: &Ui, state: &mut FileDialogState) -> Option<PathBuf> {
    let mut out: Option<PathBuf> = None;

    if let Some(_popup) = ui.begin_popup_context_window() {
        ui.text_disabled("Places");
        ui.separator();
        if ui.menu_item("+ Bookmark") {
            state.core.places.add_bookmark_path(state.core.cwd.clone());
            ui.close_current_popup();
        }
        if ui.menu_item("+ Group...") {
            state.ui.places_edit_mode = crate::dialog_state::PlacesEditMode::AddGroup;
            state.ui.places_edit_group.clear();
            state.ui.places_edit_group_from = None;
            state.ui.places_edit_error = None;
            state.ui.places_edit_open_next = true;
            state.ui.places_edit_focus_next = true;
            ui.close_current_popup();
        }
        ui.separator();
        if ui.menu_item("Refresh system") {
            state.core.places.refresh_system_places();
            ui.close_current_popup();
        }
        ui.separator();
        if ui.menu_item("Export...") {
            state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Export;
            state.ui.places_io_buffer =
                state
                    .core
                    .places
                    .serialize_compact(crate::PlacesSerializeOptions {
                        include_code_places: state.ui.places_io_include_code,
                    });
            state.ui.places_io_error = None;
            state.ui.places_io_open_next = true;
            ui.close_current_popup();
        }
        if ui.menu_item("Import...") {
            state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Import;
            state.ui.places_io_buffer.clear();
            state.ui.places_io_error = None;
            state.ui.places_io_open_next = true;
            ui.close_current_popup();
        }
    }

    let mut groups = state.core.places.groups.clone();
    groups.sort_by(|a, b| {
        a.display_order
            .cmp(&b.display_order)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    let mut remove_place: Option<(String, PathBuf)> = None;
    let mut edit_req: Option<PlacesEditRequest> = None;
    for (gi, g) in groups.iter().enumerate() {
        let _gid = ui.push_id(&g.label);
        let is_system = g.label == Places::SYSTEM_GROUP;
        let is_reserved = is_system || g.label == Places::BOOKMARKS_GROUP;

        let selected_path = state.ui.places_selected.as_ref().and_then(|(group, path)| {
            if group == &g.label {
                Some(path.clone())
            } else {
                None
            }
        });

        let is_editing_this_group = state
            .ui
            .places_inline_edit
            .as_ref()
            .is_some_and(|(group, _)| group == &g.label);
        let editing_path = state
            .ui
            .places_inline_edit
            .as_ref()
            .and_then(|(group, path)| {
                if group == &g.label {
                    Some(path.clone())
                } else {
                    None
                }
            });

        let can_remove = selected_path.as_ref().is_some_and(|sel| {
            g.places
                .iter()
                .find(|p| !p.is_separator() && &p.path == sel)
                .is_some_and(|p| p.origin == PlaceOrigin::User)
        });

        let style = ui.clone_style();
        let spacing_x = style.item_spacing()[0];
        let btn = ui.frame_height();
        let actions_w = if is_system {
            0.0
        } else {
            btn * 2.0 + spacing_x
        };

        let mut open = false;
        let col_setups = vec![
            TableColumnSetup::new("##places_group_label")
                .flags(TableColumnFlags::NO_SORT | TableColumnFlags::NO_RESIZE)
                .init_width_or_weight(1.0),
            TableColumnSetup::new("##places_group_actions")
                .flags(TableColumnFlags::NO_SORT | TableColumnFlags::NO_RESIZE)
                .init_width_or_weight(actions_w),
        ];
        ui.table("##places_group_header")
            .flags(TableFlags::SIZING_STRETCH_PROP | TableFlags::NO_PAD_OUTER_X)
            .columns(col_setups)
            .headers(false)
            .build(|ui| {
                ui.table_next_row();
                ui.table_next_column();

                let flags = if g.default_opened {
                    TreeNodeFlags::DEFAULT_OPEN
                } else {
                    TreeNodeFlags::NONE
                };
                open = ui.collapsing_header(&g.label, flags);

                if let Some(_popup) = ui.begin_popup_context_item() {
                    if ui.menu_item_enabled_selected(
                        "Add place...",
                        None::<&str>,
                        false,
                        !is_system,
                    ) {
                        edit_req = Some(PlacesEditRequest::add_place(&g.label, &state.core.cwd));
                        ui.close_current_popup();
                    }
                    if ui.menu_item_enabled_selected(
                        "Rename group...",
                        None::<&str>,
                        false,
                        !is_reserved,
                    ) {
                        edit_req = Some(PlacesEditRequest::rename_group(&g.label));
                        ui.close_current_popup();
                    }
                    if ui.menu_item_enabled_selected(
                        "Remove group...",
                        None::<&str>,
                        false,
                        !is_reserved,
                    ) {
                        edit_req = Some(PlacesEditRequest::remove_group_confirm(&g.label));
                        ui.close_current_popup();
                    }
                }

                ui.table_next_column();
                if !is_system {
                    if ui
                        .button_config("+##places_group_add")
                        .size([btn, btn])
                        .build()
                    {
                        let label = state
                            .core
                            .cwd
                            .file_name()
                            .and_then(|s| s.to_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| state.core.cwd.display().to_string());
                        state
                            .core
                            .places
                            .add_place(&g.label, Place::user(label, state.core.cwd.clone()));
                    }

                    ui.same_line();
                    {
                        let _disabled = ui.begin_disabled_with_cond(!can_remove);
                        if ui
                            .button_config("-##places_group_remove")
                            .size([btn, btn])
                            .build()
                        {
                            if let Some(sel) = selected_path.clone() {
                                remove_place = Some((g.label.clone(), sel.clone()));
                                if state
                                    .ui
                                    .places_inline_edit
                                    .as_ref()
                                    .is_some_and(|(group, path)| group == &g.label && *path == sel)
                                {
                                    state.ui.places_inline_edit = None;
                                    state.ui.places_inline_edit_buffer.clear();
                                    state.ui.places_inline_edit_focus_next = false;
                                }
                                state.ui.places_selected = None;
                            }
                        }
                    }
                }
            });

        if is_editing_this_group {
            let esc = ui.is_key_pressed(Key::Escape);
            ui.new_line();
            let can_commit =
                editing_path.is_some() && !state.ui.places_inline_edit_buffer.trim().is_empty();
            let btn = ui.frame_height();
            let spacing_x = ui.clone_style().item_spacing()[0];
            let avail_w = ui.content_region_avail_width().max(0.0);
            let input_w = (avail_w - btn * 2.0 - spacing_x * 2.0).max(80.0);

            let ok_clicked = {
                let _disabled = ui.begin_disabled_with_cond(!can_commit);
                ui.button_config("OK##places_edit_ok")
                    .size([btn, btn])
                    .build()
            };
            if ui.is_item_hovered() && !can_commit && editing_path.is_some() {
                ui.tooltip_text("Label cannot be empty");
            }

            ui.same_line();
            if state.ui.places_inline_edit_focus_next {
                ui.set_keyboard_focus_here();
                state.ui.places_inline_edit_focus_next = false;
            }
            ui.set_next_item_width(input_w);
            let submitted = ui
                .input_text(
                    "##places_inline_edit",
                    &mut state.ui.places_inline_edit_buffer,
                )
                .auto_select_all(true)
                .enter_returns_true(true)
                .build();
            ui.same_line();
            let cancel_clicked = ui
                .button_config("X##places_edit_cancel")
                .size([btn, btn])
                .build()
                || esc;

            if cancel_clicked {
                state.ui.places_inline_edit = None;
                state.ui.places_inline_edit_buffer.clear();
                state.ui.places_inline_edit_focus_next = false;
            } else if (ok_clicked || submitted) && can_commit {
                if let Some(from_path) = editing_path.as_ref() {
                    let _ = state.core.places.edit_place_by_path(
                        &g.label,
                        from_path,
                        state.ui.places_inline_edit_buffer.clone(),
                        from_path.clone(),
                    );
                }
                state.ui.places_inline_edit = None;
                state.ui.places_inline_edit_buffer.clear();
                state.ui.places_inline_edit_focus_next = false;
            }
        }

        if !open {
            continue;
        }

        if !is_system {
            subtle_separator(ui);
        }

        if g.places.is_empty() {
            ui.text_disabled("Empty");
            continue;
        }

        let can_clip = g
            .places
            .iter()
            .all(|p| p.separator_thickness.unwrap_or(0) <= 1);
        let use_clipper = can_clip && g.places.len() > 200;

        let draw_place_row = |ui: &Ui,
                              state: &mut FileDialogState,
                              edit_req: &mut Option<PlacesEditRequest>,
                              remove_place: &mut Option<(String, PathBuf)>,
                              out: &mut Option<PathBuf>,
                              gi: usize,
                              pi: usize,
                              p: &Place| {
            let _id = ui.push_id((gi * 10_000 + pi) as i32);
            if let Some(thickness) = p.separator_thickness {
                if thickness > 1 {
                    ui.dummy([0.0, (thickness - 1) as f32]);
                }
                subtle_separator(ui);
                if thickness > 1 {
                    ui.dummy([0.0, (thickness - 1) as f32]);
                }
                return;
            }

            let editable = !p.is_separator()
                && p.origin == PlaceOrigin::User
                && g.label != Places::SYSTEM_GROUP;

            let selected = state.core.cwd == p.path
                || state
                    .ui
                    .places_selected
                    .as_ref()
                    .is_some_and(|(group, path)| group == &g.label && path == &p.path);

            let display_label = state
                .ui
                .places_inline_edit
                .as_ref()
                .is_some_and(|(group, path)| group == &g.label && path == &p.path)
                .then(|| state.ui.places_inline_edit_buffer.as_str())
                .unwrap_or(p.label.as_str());

            // Render the row label as a full-width selectable, then overlay a compact edit
            // button on the right (IGFD-like). This avoids left-side jitter and keeps the
            // place list visually aligned.
            let row_w = ui.content_region_avail_width().max(0.0);
            let clicked = ui
                .selectable_config(display_label)
                .selected(selected)
                .flags(SelectableFlags::ALLOW_DOUBLE_CLICK | SelectableFlags::ALLOW_OVERLAP)
                .size([row_w, 0.0])
                .build();
            let row_min = ui.item_rect_min();
            let row_max = ui.item_rect_max();
            let row_hovered = ui.is_mouse_hovering_rect(row_min, row_max);
            let after = ui.cursor_pos();

            if editable {
                let is_editing = state
                    .ui
                    .places_inline_edit
                    .as_ref()
                    .is_some_and(|(group, path)| group == &g.label && path == &p.path);
                let show_edit = row_hovered || selected || is_editing;
                if show_edit {
                    let frame_h = ui.frame_height();
                    let row_h = (row_max[1] - row_min[1]).max(0.0);
                    let y = row_min[1] + ((row_h - frame_h) * 0.5).max(0.0);
                    let w = frame_h;
                    let x = (row_max[0] - w).max(row_min[0]);
                    ui.set_cursor_screen_pos([x, y]);
                    ui.set_next_item_allow_overlap();
                    if ui
                        .button_config("E##places_inline_edit_btn")
                        .size([w, frame_h])
                        .build()
                    {
                        state.ui.places_selected = Some((g.label.clone(), p.path.clone()));
                        state.ui.places_inline_edit = Some((g.label.clone(), p.path.clone()));
                        state.ui.places_inline_edit_buffer = p.label.clone();
                        state.ui.places_inline_edit_focus_next = true;
                    }
                }
            }
            ui.set_cursor_pos(after);

            if clicked {
                state.ui.places_selected = Some((g.label.clone(), p.path.clone()));
                if state
                    .ui
                    .places_inline_edit
                    .as_ref()
                    .is_some_and(|(group, path)| group == &g.label && path != &p.path)
                {
                    state.ui.places_inline_edit = None;
                    state.ui.places_inline_edit_buffer.clear();
                    state.ui.places_inline_edit_focus_next = false;
                }
            }
            if row_hovered && ui.is_mouse_double_clicked(MouseButton::Left) {
                *out = Some(p.path.clone());
            }
            if row_hovered {
                ui.tooltip_text(p.path.display().to_string());
            }
            if let Some(_popup) = ui.begin_popup_context_item() {
                ui.text_disabled(p.path.display().to_string());
                ui.separator();
                if ui.menu_item_enabled_selected("Open", Some("Enter"), false, true) {
                    *out = Some(p.path.clone());
                    ui.close_current_popup();
                }
                ui.separator();
                if ui.menu_item_enabled_selected("Edit...", None::<&str>, false, editable) {
                    *edit_req = Some(PlacesEditRequest::edit_place(&g.label, p));
                    ui.close_current_popup();
                }
                if ui.menu_item_enabled_selected("Remove", None::<&str>, false, editable) {
                    *remove_place = Some((g.label.clone(), p.path.clone()));
                }
            }
        };

        if use_clipper {
            let items_count = i32::try_from(g.places.len()).unwrap_or(i32::MAX);
            let clipper = dear_imgui_rs::ListClipper::new(items_count)
                .items_height(ui.text_line_height_with_spacing())
                .begin(ui);
            for i in clipper.iter() {
                let pi = i as usize;
                if pi >= g.places.len() {
                    continue;
                }
                let p = &g.places[pi];
                draw_place_row(
                    ui,
                    state,
                    &mut edit_req,
                    &mut remove_place,
                    &mut out,
                    gi,
                    pi,
                    p,
                );
            }
        } else {
            for (pi, p) in g.places.iter().enumerate() {
                draw_place_row(
                    ui,
                    state,
                    &mut edit_req,
                    &mut remove_place,
                    &mut out,
                    gi,
                    pi,
                    p,
                );
            }
        }
    }
    if let Some((g, p)) = remove_place {
        state.core.places.remove_place_path(&g, &p);
    }
    if let Some(req) = edit_req {
        req.apply_to_state(&mut state.ui);
    }
    out
}

#[derive(Clone, Debug)]
struct PlacesEditRequest {
    mode: crate::dialog_state::PlacesEditMode,
    group: String,
    group_from: Option<String>,
    place_from_path: Option<PathBuf>,
    place_label: String,
    place_path: String,
    focus: bool,
}

impl PlacesEditRequest {
    fn add_place(group: &str, cwd: &Path) -> Self {
        let label = cwd
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| cwd.display().to_string());
        Self {
            mode: crate::dialog_state::PlacesEditMode::AddPlace,
            group: group.to_string(),
            group_from: None,
            place_from_path: None,
            place_label: label,
            place_path: cwd.display().to_string(),
            focus: true,
        }
    }

    fn edit_place(group: &str, p: &Place) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::EditPlace,
            group: group.to_string(),
            group_from: None,
            place_from_path: Some(p.path.clone()),
            place_label: p.label.clone(),
            place_path: p.path.display().to_string(),
            focus: true,
        }
    }

    fn rename_group(group: &str) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::RenameGroup,
            group: group.to_string(),
            group_from: Some(group.to_string()),
            place_from_path: None,
            place_label: String::new(),
            place_path: String::new(),
            focus: true,
        }
    }

    fn remove_group_confirm(group: &str) -> Self {
        Self {
            mode: crate::dialog_state::PlacesEditMode::RemoveGroupConfirm,
            group: group.to_string(),
            group_from: Some(group.to_string()),
            place_from_path: None,
            place_label: String::new(),
            place_path: String::new(),
            focus: false,
        }
    }

    fn apply_to_state(self, ui: &mut crate::FileDialogUiState) {
        ui.places_edit_mode = self.mode;
        ui.places_edit_group = self.group;
        ui.places_edit_group_from = self.group_from;
        ui.places_edit_place_from_path = self.place_from_path;
        ui.places_edit_place_label = self.place_label;
        ui.places_edit_place_path = self.place_path;
        ui.places_edit_error = None;
        ui.places_edit_open_next = true;
        ui.places_edit_focus_next = self.focus;
    }
}

pub(super) fn draw_places_io_modal(ui: &Ui, state: &mut FileDialogState) {
    if state.ui.places_io_open_next {
        ui.open_popup("Places");
        state.ui.places_io_open_next = false;
    }

    if let Some(_popup) = ui.begin_modal_popup("Places") {
        let is_export = state.ui.places_io_mode == crate::dialog_state::PlacesIoMode::Export;

        ui.text("Places persistence (compact format)");
        ui.separator();

        if ui.button("Export") {
            state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Export;
            state.ui.places_io_buffer =
                state
                    .core
                    .places
                    .serialize_compact(crate::PlacesSerializeOptions {
                        include_code_places: state.ui.places_io_include_code,
                    });
            state.ui.places_io_error = None;
        }
        ui.same_line();
        if ui.button("Import") {
            state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Import;
            state.ui.places_io_error = None;
        }
        ui.same_line();
        if ui.button("Close") {
            ui.close_current_popup();
            state.ui.places_io_error = None;
        }

        ui.separator();

        if is_export {
            let mut include_code = state.ui.places_io_include_code;
            if ui.checkbox("Include code places", &mut include_code) {
                state.ui.places_io_include_code = include_code;
                state.ui.places_io_buffer =
                    state
                        .core
                        .places
                        .serialize_compact(crate::PlacesSerializeOptions {
                            include_code_places: state.ui.places_io_include_code,
                        });
            }
        }

        let avail = ui.content_region_avail();
        let size = [avail[0].max(200.0), (avail[1] - 95.0).max(120.0)];
        if is_export {
            ui.input_text_multiline("##places_export", &mut state.ui.places_io_buffer, size)
                .read_only(true)
                .build();
        } else {
            ui.input_text_multiline("##places_import", &mut state.ui.places_io_buffer, size)
                .build();

            if ui.button("Replace") {
                match Places::deserialize_compact(&state.ui.places_io_buffer) {
                    Ok(p) => {
                        state.core.places = p;
                        state.ui.places_io_error = None;
                    }
                    Err(e) => {
                        state.ui.places_io_error = Some(e.to_string());
                    }
                }
            }
            ui.same_line();
            if ui.button("Merge") {
                match Places::deserialize_compact(&state.ui.places_io_buffer) {
                    Ok(p) => {
                        state.core.places.merge_from(
                            p,
                            crate::places::PlacesMergeOptions {
                                overwrite_group_metadata: true,
                            },
                        );
                        state.ui.places_io_error = None;
                    }
                    Err(e) => {
                        state.ui.places_io_error = Some(e.to_string());
                    }
                }
            }
        }

        if let Some(err) = &state.ui.places_io_error {
            ui.separator();
            ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
        }
    }
}

pub(super) fn draw_places_edit_modal(ui: &Ui, state: &mut FileDialogState, fs: &dyn FileSystem) {
    const POPUP_ID: &str = "Edit Places";
    if state.ui.places_edit_open_next {
        ui.open_popup(POPUP_ID);
        state.ui.places_edit_open_next = false;
    }

    let Some(_popup) = ui.begin_modal_popup(POPUP_ID) else {
        return;
    };

    use crate::dialog_state::PlacesEditMode;
    let mode = state.ui.places_edit_mode;
    match mode {
        PlacesEditMode::AddGroup => {
            ui.text("Create a new places group:");
            ui.separator();
            if state.ui.places_edit_focus_next {
                ui.set_keyboard_focus_here();
                state.ui.places_edit_focus_next = false;
            }
            ui.input_text("Group", &mut state.ui.places_edit_group)
                .build();

            let create = ui.button("Create");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }
            if create {
                state.ui.places_edit_error = None;
                let label = state.ui.places_edit_group.trim();
                if label.is_empty() {
                    state.ui.places_edit_error = Some("Group name is empty".into());
                } else if label == Places::SYSTEM_GROUP || label == Places::BOOKMARKS_GROUP {
                    state.ui.places_edit_error = Some("Group name is reserved".into());
                } else if state.core.places.groups.iter().any(|g| g.label == label) {
                    state.ui.places_edit_error = Some("Group already exists".into());
                } else {
                    state.core.places.add_group(label.to_string());
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::RenameGroup => {
            let Some(from) = state.ui.places_edit_group_from.clone() else {
                ui.text_disabled("Missing source group.");
                if ui.button("Close") {
                    ui.close_current_popup();
                }
                return;
            };
            ui.text("Rename group:");
            ui.text_disabled(&from);
            ui.separator();
            if state.ui.places_edit_focus_next {
                ui.set_keyboard_focus_here();
                state.ui.places_edit_focus_next = false;
            }
            ui.input_text("To", &mut state.ui.places_edit_group).build();

            let rename = ui.button("Rename");
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }
            if rename {
                state.ui.places_edit_error = None;
                let to = state.ui.places_edit_group.trim();
                if to.is_empty() {
                    state.ui.places_edit_error = Some("Target group name is empty".into());
                } else if to == Places::SYSTEM_GROUP || to == Places::BOOKMARKS_GROUP {
                    state.ui.places_edit_error = Some("Target group name is reserved".into());
                } else if to == from.as_str() {
                    state.ui.places_edit_error = Some("Target group name is unchanged".into());
                } else if state.core.places.groups.iter().any(|g| g.label == to) {
                    state.ui.places_edit_error = Some("Target group already exists".into());
                } else if !state.core.places.rename_group(&from, to.to_string()) {
                    state.ui.places_edit_error = Some("Group not found".into());
                } else {
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::RemoveGroupConfirm => {
            let Some(group) = state.ui.places_edit_group_from.clone() else {
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
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }
            if remove {
                state.ui.places_edit_error = None;
                if group == Places::SYSTEM_GROUP || group == Places::BOOKMARKS_GROUP {
                    state.ui.places_edit_error = Some("Cannot remove reserved group".into());
                } else if !state.core.places.remove_group(&group) {
                    state.ui.places_edit_error = Some("Group not found".into());
                } else {
                    ui.close_current_popup();
                }
            }
        }
        PlacesEditMode::AddPlace | PlacesEditMode::EditPlace => {
            let is_add = mode == PlacesEditMode::AddPlace;
            let group = state.ui.places_edit_group.clone();
            ui.text(if is_add { "Add place:" } else { "Edit place:" });
            ui.text_disabled(&group);
            ui.separator();

            if state.ui.places_edit_focus_next {
                ui.set_keyboard_focus_here();
                state.ui.places_edit_focus_next = false;
            }
            ui.input_text("Label", &mut state.ui.places_edit_place_label)
                .build();
            ui.input_text("Path", &mut state.ui.places_edit_place_path)
                .build();

            let ok_label = if is_add { "Add" } else { "Save" };
            let ok = ui.button(ok_label);
            ui.same_line();
            let cancel = ui.button("Cancel");
            if cancel {
                state.ui.places_edit_error = None;
                ui.close_current_popup();
                return;
            }

            if ok {
                state.ui.places_edit_error = None;
                let path_s = state.ui.places_edit_place_path.trim();
                if path_s.is_empty() {
                    state.ui.places_edit_error = Some("Path is empty".into());
                } else {
                    let raw = PathBuf::from(path_s);
                    let p = fs.canonicalize(&raw).unwrap_or(raw);
                    let is_dir = fs.metadata(&p).map(|m| m.is_dir).unwrap_or(false);
                    if !is_dir {
                        state.ui.places_edit_error =
                            Some("Path does not exist or is not a directory".into());
                    } else {
                        let mut label = state.ui.places_edit_place_label.trim().to_string();
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

                        let from_path = state.ui.places_edit_place_from_path.clone();
                        let is_duplicate = group_places.iter().any(|x| {
                            if let Some(from) = &from_path {
                                if x.path == *from {
                                    return false;
                                }
                            }
                            x.path == p
                        });
                        if is_duplicate {
                            state.ui.places_edit_error =
                                Some("Place already exists in group".into());
                        } else if is_add {
                            state
                                .core
                                .places
                                .add_place(group, Place::new(label, p, PlaceOrigin::User));
                            ui.close_current_popup();
                        } else {
                            let Some(from_path) = from_path else {
                                state.ui.places_edit_error = Some("Missing source place".into());
                                return;
                            };
                            if !state
                                .core
                                .places
                                .edit_place_by_path(&group, &from_path, label, p)
                            {
                                state.ui.places_edit_error = Some("Place not found".into());
                            } else {
                                ui.close_current_popup();
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(err) = &state.ui.places_edit_error {
        ui.separator();
        ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
    }
}
