use std::path::PathBuf;

use dear_imgui_rs::input::{Key, MouseButton};
use dear_imgui_rs::{
    SelectableFlags, TableColumnFlags, TableColumnSetup, TableFlags, TableSizingPolicy,
    TreeNodeFlags, Ui,
};

use crate::core::LayoutStyle;
use crate::dialog_core::CoreEvent;
use crate::dialog_state::FileDialogState;
use crate::places::{Place, PlaceOrigin, Places};

use super::edit_request::PlacesEditRequest;
use super::subtle_separator;

pub(in crate::ui) fn draw_minimal_places_popup(ui: &Ui, state: &mut FileDialogState) {
    if !matches!(state.ui.config.layout, LayoutStyle::Minimal) {
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

pub(in crate::ui) fn draw_places_pane(ui: &Ui, state: &mut FileDialogState) -> Option<PathBuf> {
    let mut out: Option<PathBuf> = None;

    if let Some(_popup) = ui.begin_popup_context_window() {
        ui.text_disabled("Places");
        ui.separator();
        if ui.menu_item("+ Bookmark") {
            state.core.places.add_bookmark_path(state.core.cwd.clone());
            ui.close_current_popup();
        }
        if ui.menu_item("+ Group...") {
            state.ui.operations.places.edit.mode = crate::dialog_state::PlacesEditMode::AddGroup;
            state.ui.operations.places.edit.group.clear();
            state.ui.operations.places.edit.group_from = None;
            state.ui.operations.places.edit.error = None;
            state.ui.operations.places.edit.open_next = true;
            state.ui.operations.places.edit.focus_next = true;
            ui.close_current_popup();
        }
        ui.separator();
        if ui.menu_item("Refresh system") {
            state.core.places.refresh_system_places();
            ui.close_current_popup();
        }
        ui.separator();
        if ui.menu_item("Export...") {
            state.ui.operations.places.io.mode = crate::dialog_state::PlacesIoMode::Export;
            state.ui.operations.places.io.buffer =
                state
                    .core
                    .places
                    .serialize_compact(crate::PlacesSerializeOptions {
                        include_code_places: state.ui.operations.places.io.include_code,
                    });
            state.ui.operations.places.io.error = None;
            state.ui.operations.places.io.open_next = true;
            ui.close_current_popup();
        }
        if ui.menu_item("Import...") {
            state.ui.operations.places.io.mode = crate::dialog_state::PlacesIoMode::Import;
            state.ui.operations.places.io.buffer.clear();
            state.ui.operations.places.io.error = None;
            state.ui.operations.places.io.open_next = true;
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

        let selected_path =
            state
                .ui
                .operations
                .places
                .selected
                .as_ref()
                .and_then(|(group, path)| {
                    if group == &g.label {
                        Some(path.clone())
                    } else {
                        None
                    }
                });

        let is_editing_this_group = state
            .ui
            .operations
            .places
            .inline_edit
            .target
            .as_ref()
            .is_some_and(|(group, _)| group == &g.label);
        let editing_path = state
            .ui
            .operations
            .places
            .inline_edit
            .target
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
                .stretch_weight(1.0),
            TableColumnSetup::new("##places_group_actions")
                .flags(TableColumnFlags::NO_SORT | TableColumnFlags::NO_RESIZE)
                .fixed_width(actions_w),
        ];
        ui.table("##places_group_header")
            .flags(TableFlags::NO_PAD_OUTER_X)
            .sizing_policy(TableSizingPolicy::StretchProp)
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
                                    .operations
                                    .places
                                    .inline_edit
                                    .target
                                    .as_ref()
                                    .is_some_and(|(group, path)| group == &g.label && *path == sel)
                                {
                                    state.ui.operations.places.inline_edit.target = None;
                                    state.ui.operations.places.inline_edit.buffer.clear();
                                    state.ui.operations.places.inline_edit.focus_next = false;
                                }
                                state.ui.operations.places.selected = None;
                            }
                        }
                    }
                }
            });

        if is_editing_this_group {
            let esc = ui.is_key_pressed(Key::Escape);
            ui.new_line();
            let can_commit = editing_path.is_some()
                && !state
                    .ui
                    .operations
                    .places
                    .inline_edit
                    .buffer
                    .trim()
                    .is_empty();
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
            if state.ui.operations.places.inline_edit.focus_next {
                ui.set_keyboard_focus_here();
                state.ui.operations.places.inline_edit.focus_next = false;
            }
            ui.set_next_item_width(input_w);
            let submitted = ui
                .input_text(
                    "##places_inline_edit",
                    &mut state.ui.operations.places.inline_edit.buffer,
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
                state.ui.operations.places.inline_edit.target = None;
                state.ui.operations.places.inline_edit.buffer.clear();
                state.ui.operations.places.inline_edit.focus_next = false;
            } else if (ok_clicked || submitted) && can_commit {
                if let Some(from_path) = editing_path.as_ref() {
                    let _ = state.core.places.edit_place_by_path(
                        &g.label,
                        from_path,
                        state.ui.operations.places.inline_edit.buffer.clone(),
                        from_path.to_path_buf(),
                    );
                }
                state.ui.operations.places.inline_edit.target = None;
                state.ui.operations.places.inline_edit.buffer.clear();
                state.ui.operations.places.inline_edit.focus_next = false;
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
                    .operations
                    .places
                    .selected
                    .as_ref()
                    .is_some_and(|(group, path)| group == &g.label && path == &p.path);

            let display_label = state
                .ui
                .operations
                .places
                .inline_edit
                .target
                .as_ref()
                .is_some_and(|(group, path)| group == &g.label && path == &p.path)
                .then(|| state.ui.operations.places.inline_edit.buffer.as_str())
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
                    .operations
                    .places
                    .inline_edit
                    .target
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
                        state.ui.operations.places.selected =
                            Some((g.label.clone(), p.path.clone()));
                        state.ui.operations.places.inline_edit.target =
                            Some((g.label.clone(), p.path.clone()));
                        state.ui.operations.places.inline_edit.buffer = p.label.clone();
                        state.ui.operations.places.inline_edit.focus_next = true;
                    }
                }
            }
            ui.set_cursor_pos(after);

            if clicked {
                state.ui.operations.places.selected = Some((g.label.clone(), p.path.clone()));
                if state
                    .ui
                    .operations
                    .places
                    .inline_edit
                    .target
                    .as_ref()
                    .is_some_and(|(group, path)| group == &g.label && path != &p.path)
                {
                    state.ui.operations.places.inline_edit.target = None;
                    state.ui.operations.places.inline_edit.buffer.clear();
                    state.ui.operations.places.inline_edit.focus_next = false;
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
            let clipper = dear_imgui_rs::ListClipper::new(g.places.len())
                .items_height(ui.text_line_height_with_spacing())
                .begin(ui);
            for pi in clipper.iter() {
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
