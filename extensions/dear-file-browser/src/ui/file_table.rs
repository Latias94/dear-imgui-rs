use std::time::{Duration, Instant};

use crate::core::{DialogMode, LayoutStyle, SortBy};
use crate::dialog_core::{CoreEvent, CoreEventOutcome, DirEntry, Modifiers};
use crate::dialog_state::{
    ClipboardOp, FileDialogState, FileListDataColumn, FileListViewMode, HeaderStyle,
};
use crate::fs::FileSystem;
use crate::thumbnails::ThumbnailBackend;
use dear_imgui_rs::Ui;
use dear_imgui_rs::input::{Key, MouseButton};

use super::ops::{
    clipboard_set_from_selection, open_delete_modal_from_selection,
    open_rename_modal_from_selection, run_paste_job_until_wait_or_done, start_paste_into_cwd,
};

mod columns;
mod format;
mod style;

pub(in crate::ui) use columns::{
    apply_balanced_column_layout, apply_balanced_column_layout_keep_preview,
    apply_compact_column_layout, apply_compact_column_layout_keep_preview,
    data_column_label_for_state, extension_ui_label, is_data_column_visible,
    move_column_order_down, move_column_order_up,
};

use columns::list_column_layout as list_column_layout_impl;
use columns::{
    igfd_type_dots_to_extract, resolved_data_column_weight, resolved_preview_column_weight,
    sync_runtime_column_order_from_table, sync_runtime_column_weights_from_table,
    type_extension_by_dot_count,
};
use format::{format_modified_ago, format_size};
use style::{TextColorToken, style_visual_for_entry};

#[cfg(test)]
pub(in crate::ui) use columns::{ListColumnLayout, list_column_layout, merged_order_with_current};

pub(super) fn draw_file_table(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    fs: &dyn FileSystem,
    request_confirm: &mut bool,
    thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
) {
    match state.ui.file_list_view {
        FileListViewMode::List => draw_file_table_view(
            ui,
            state,
            size,
            fs,
            request_confirm,
            thumbnails_backend,
            false,
        ),
        FileListViewMode::ThumbnailsList => {
            state.ui.thumbnails_enabled = true;
            draw_file_table_view(
                ui,
                state,
                size,
                fs,
                request_confirm,
                thumbnails_backend,
                true,
            )
        }
        FileListViewMode::Grid => {
            draw_file_grid_view(ui, state, size, fs, request_confirm, thumbnails_backend)
        }
    }
}

fn draw_file_table_view(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    fs: &dyn FileSystem,
    request_confirm: &mut bool,
    thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
    force_preview: bool,
) {
    state.core.rescan_if_needed(fs);
    if state.ui.thumbnails_enabled {
        state.ui.thumbnails.advance_frame();
    }

    // Table
    use dear_imgui_rs::{SortDirection, TableColumnFlags, TableFlags};
    let flags = TableFlags::RESIZABLE
        | TableFlags::REORDERABLE
        | TableFlags::ROW_BG
        | TableFlags::BORDERS_V
        | TableFlags::BORDERS_OUTER
        | TableFlags::SCROLL_Y
        | TableFlags::SIZING_STRETCH_PROP
        | TableFlags::SORTABLE; // enable built-in header sorting
    let columns_config = &state.ui.file_list_columns;
    let show_preview =
        state.ui.thumbnails_enabled && (columns_config.show_preview || force_preview);
    let show_size = columns_config.show_size;
    let show_modified = columns_config.show_modified;
    let layout = list_column_layout_impl(show_preview, columns_config);
    let type_dots_to_extract = if matches!(state.ui.header_style, HeaderStyle::IgfdClassic) {
        igfd_type_dots_to_extract(state.core.active_filter())
    } else {
        1
    };

    let mut table = ui.table("file_table").flags(flags).outer_size(size);
    if show_preview {
        table = table
            .column("Preview")
            .flags(
                TableColumnFlags::NO_SORT
                    | TableColumnFlags::NO_RESIZE
                    | TableColumnFlags::NO_REORDER,
            )
            .weight(resolved_preview_column_weight(columns_config))
            .done();
    }

    for column in &layout.data_columns {
        let ext_label = extension_ui_label(state);
        table = match column {
            FileListDataColumn::Name => table
                .column("Name")
                .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
                .user_id(0)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
            FileListDataColumn::Extension => table
                .column(ext_label)
                .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
                .user_id(1)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
            FileListDataColumn::Size => table
                .column("Size")
                .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
                .user_id(2)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
            FileListDataColumn::Modified => table
                .column("Modified")
                .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
                .user_id(3)
                .weight(resolved_data_column_weight(
                    columns_config,
                    *column,
                    show_preview,
                    show_size,
                    show_modified,
                ))
                .done(),
        };
    }

    table = table.headers(true);

    table.build(|ui| {
        // Apply ImGui sort specs (single primary sort)
        if let Some(mut specs) = ui.table_get_sort_specs() {
            if specs.is_dirty() {
                let extension_sort_by = if matches!(state.ui.header_style, HeaderStyle::IgfdClassic)
                {
                    SortBy::Type
                } else {
                    SortBy::Extension
                };
                if let Some(s) = specs.iter().next() {
                    let (by, asc) = match (s.column_index, s.sort_direction) {
                        (i, SortDirection::Ascending) if i == layout.name => (SortBy::Name, true),
                        (i, SortDirection::Descending) if i == layout.name => (SortBy::Name, false),
                        (i, SortDirection::Ascending) if layout.extension == Some(i) => {
                            (extension_sort_by, true)
                        }
                        (i, SortDirection::Descending) if layout.extension == Some(i) => {
                            (extension_sort_by, false)
                        }
                        (i, SortDirection::Ascending) if layout.size == Some(i) => {
                            (SortBy::Size, true)
                        }
                        (i, SortDirection::Descending) if layout.size == Some(i) => {
                            (SortBy::Size, false)
                        }
                        (i, SortDirection::Ascending) if layout.modified == Some(i) => {
                            (SortBy::Modified, true)
                        }
                        (i, SortDirection::Descending) if layout.modified == Some(i) => {
                            (SortBy::Modified, false)
                        }
                        _ => (state.core.sort_by, state.core.sort_ascending),
                    };
                    state.core.sort_by = by;
                    state.core.sort_ascending = asc;
                    state.core.rescan_if_needed(fs);
                }
                specs.clear_dirty();
            }
        }

        if ui.is_window_focused() && !ui.io().want_text_input() {
            let modifiers = Modifiers {
                ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                shift: ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift),
            };

            if modifiers.ctrl && ui.is_key_pressed(Key::A) && !modifiers.shift {
                let _ = state.core.handle_event(CoreEvent::SelectAll);
            }
            if modifiers.ctrl && ui.is_key_pressed(Key::C) && !modifiers.shift {
                clipboard_set_from_selection(state, ClipboardOp::Copy);
            }
            if modifiers.ctrl && ui.is_key_pressed(Key::X) && !modifiers.shift {
                clipboard_set_from_selection(state, ClipboardOp::Cut);
            }
            if modifiers.ctrl && ui.is_key_pressed(Key::V) && !modifiers.shift {
                state.ui.ui_error = None;
                start_paste_into_cwd(state);
                if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                    state.ui.ui_error = Some(e);
                    state.ui.paste_job = None;
                }
            }
            if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                let _ = state.core.handle_event(CoreEvent::MoveFocus {
                    delta: -1,
                    modifiers,
                });
            }
            if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                let _ = state.core.handle_event(CoreEvent::MoveFocus {
                    delta: 1,
                    modifiers,
                });
            }
            if state.ui.type_select_enabled && !modifiers.ctrl && !modifiers.shift {
                handle_type_select(ui, state);
            }
        }

        // Clone the entry list so we can mutate `state.core` while iterating (selection, navigation).
        let entries: Vec<DirEntry> = state.core.entries().to_vec();
        if entries.is_empty() {
            if state.ui.empty_hint_enabled {
                ui.table_next_row();
                if show_preview {
                    ui.table_next_column();
                    ui.text("");
                }
                ui.table_next_column();
                let msg = if let Some(custom) = &state.ui.empty_hint_static_message {
                    custom.clone()
                } else {
                    let filter_label = state
                        .core
                        .active_filter()
                        .map(|f| f.name.as_str())
                        .unwrap_or("All files");
                    let hidden_label = if state.core.show_hidden { "on" } else { "off" };
                    if state.core.search.is_empty() {
                        format!(
                            "No matching entries. Filter: {}, Hidden: {}",
                            filter_label, hidden_label
                        )
                    } else {
                        format!(
                            "No matching entries. Filter: {}, Search: '{}', Hidden: {}",
                            filter_label, state.core.search, hidden_label
                        )
                    }
                };
                ui.text_colored(state.ui.empty_hint_color, msg);
            }
            return;
        }

        for e in &entries {
            ui.table_next_row();
            if show_preview {
                ui.table_next_column();
                draw_thumbnail_cell(ui, state, e);
            }

            let selected = state.core.is_selected_id(e.id);
            let visual = style_visual_for_entry(state, e);

            let mut label = e.display_name();
            if let Some(icon) = visual.icon.as_deref() {
                label = format!("{icon} {label}");
            }

            for column in &layout.data_columns {
                ui.table_next_column();
                match column {
                    FileListDataColumn::Name => {
                        let _font = visual.font_id.map(|id| ui.push_font(id));
                        let _color = visual
                            .text_color
                            .map(TextColorToken::push)
                            .unwrap_or_else(TextColorToken::none);
                        {
                            if ui
                                .selectable_config(label.as_str())
                                .selected(selected)
                                .span_all_columns(false)
                                .build()
                            {
                                let modifiers = Modifiers {
                                    ctrl: ui.is_key_down(Key::LeftCtrl)
                                        || ui.is_key_down(Key::RightCtrl),
                                    shift: ui.is_key_down(Key::LeftShift)
                                        || ui.is_key_down(Key::RightShift),
                                };
                                let _ = state.core.handle_event(CoreEvent::ClickEntry {
                                    id: e.id,
                                    modifiers,
                                });
                                if matches!(state.core.mode, DialogMode::SaveFile) && !e.is_dir {
                                    state.core.save_name = e.name.clone();
                                }
                            }
                        }

                        if ui.is_item_hovered() {
                            if let Some(t) = visual.tooltip.as_deref() {
                                ui.tooltip_text(t);
                            }
                        }

                        if let Some(_popup) = ui.begin_popup_context_item() {
                            if !selected {
                                let _ =
                                    state.core.handle_event(CoreEvent::FocusAndSelectById(e.id));
                            }
                            let has_selection = state.core.has_selection();
                            let can_paste = state
                                .ui
                                .clipboard
                                .as_ref()
                                .map(|c| !c.sources.is_empty())
                                .unwrap_or(false);

                            if ui.menu_item_enabled_selected("Open", Some("Enter"), false, true) {
                                state.ui.ui_error = None;
                                *request_confirm |= matches!(
                                    state
                                        .core
                                        .handle_event(CoreEvent::DoubleClickEntry { id: e.id }),
                                    CoreEventOutcome::RequestConfirm
                                );
                                ui.close_current_popup();
                            }
                            ui.separator();
                            if ui.menu_item_enabled_selected(
                                "Copy",
                                Some("Ctrl+C"),
                                false,
                                has_selection,
                            ) {
                                clipboard_set_from_selection(state, ClipboardOp::Copy);
                                ui.close_current_popup();
                            }
                            if ui.menu_item_enabled_selected(
                                "Cut",
                                Some("Ctrl+X"),
                                false,
                                has_selection,
                            ) {
                                clipboard_set_from_selection(state, ClipboardOp::Cut);
                                ui.close_current_popup();
                            }
                            if ui.menu_item_enabled_selected(
                                "Paste",
                                Some("Ctrl+V"),
                                false,
                                can_paste,
                            ) {
                                state.ui.ui_error = None;
                                start_paste_into_cwd(state);
                                if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                                    state.ui.ui_error = Some(e);
                                    state.ui.paste_job = None;
                                }
                                ui.close_current_popup();
                            }

                            ui.separator();
                            let can_rename = state.core.selected_len() == 1;
                            if ui.menu_item_enabled_selected(
                                "Rename",
                                Some("F2"),
                                false,
                                can_rename,
                            ) {
                                open_rename_modal_from_selection(state);
                                ui.close_current_popup();
                            }
                            if ui.menu_item_enabled_selected("Delete", Some("Del"), false, true) {
                                open_delete_modal_from_selection(state);
                                ui.close_current_popup();
                            }
                        }

                        if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                            state.ui.ui_error = None;
                            *request_confirm |= matches!(
                                state
                                    .core
                                    .handle_event(CoreEvent::DoubleClickEntry { id: e.id }),
                                CoreEventOutcome::RequestConfirm
                            );
                        }
                    }
                    FileListDataColumn::Extension => {
                        if e.is_dir {
                            ui.text("");
                        } else if matches!(state.ui.header_style, HeaderStyle::IgfdClassic) {
                            ui.text(type_extension_by_dot_count(&e.name, type_dots_to_extract));
                        } else if let Some(i) = e.name.find('.') {
                            ui.text(&e.name[i..]);
                        } else {
                            ui.text("");
                        }
                    }
                    FileListDataColumn::Size => {
                        ui.text(match e.size {
                            Some(s) => format_size(s),
                            None => String::new(),
                        });
                    }
                    FileListDataColumn::Modified => {
                        let modified_str = format_modified_ago(e.modified);
                        ui.text(&modified_str);
                        if ui.is_item_hovered() {
                            if let Some(m) = e.modified {
                                use chrono::{DateTime, Local};
                                let dt: DateTime<Local> = DateTime::<Local>::from(m);
                                ui.tooltip_text(dt.format("%Y-%m-%d %H:%M:%S").to_string());
                            }
                        }
                    }
                }
            }

            if state.ui.reveal_id_next == Some(e.id) {
                ui.set_scroll_here_y(0.5);
                state.ui.reveal_id_next = None;
            }
        }

        if let Some(_popup) = ui.begin_popup_context_window() {
            let can_paste = state
                .ui
                .clipboard
                .as_ref()
                .map(|c| !c.sources.is_empty())
                .unwrap_or(false);
            if ui.menu_item("Refresh") {
                let _ = state.core.handle_event(CoreEvent::Refresh);
                ui.close_current_popup();
            }
            if state.ui.new_folder_enabled {
                if ui.menu_item("New Folder") {
                    match state.ui.layout {
                        LayoutStyle::Standard => {
                            state.ui.new_folder_inline_active = true;
                        }
                        LayoutStyle::Minimal => {
                            state.ui.new_folder_open_next = true;
                        }
                    }
                    state.ui.new_folder_name.clear();
                    state.ui.new_folder_error = None;
                    state.ui.new_folder_focus_next = true;
                    ui.close_current_popup();
                }
            }
            if ui.menu_item("Options...") {
                ui.open_popup("##fb_options");
                ui.close_current_popup();
            }
            if matches!(
                state.ui.file_list_view,
                FileListViewMode::List | FileListViewMode::ThumbnailsList
            ) {
                if ui.menu_item("Columns...") {
                    ui.open_popup("##fb_columns_popup");
                    ui.close_current_popup();
                }
            }
            ui.separator();
            if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste) {
                state.ui.ui_error = None;
                start_paste_into_cwd(state);
                if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                    state.ui.ui_error = Some(e);
                    state.ui.paste_job = None;
                }
                ui.close_current_popup();
            }
        }

        sync_runtime_column_order_from_table(&layout, &mut state.ui.file_list_columns);

        sync_runtime_column_weights_from_table(
            show_preview,
            &layout,
            &mut state.ui.file_list_columns,
        );
    });

    let mut thumbnails_backend = thumbnails_backend;
    if state.ui.thumbnails_enabled {
        if let Some(backend) = thumbnails_backend.as_deref_mut() {
            state.ui.thumbnails.maintain(backend);
        }
    }
}

fn draw_file_grid_view(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    fs: &dyn FileSystem,
    request_confirm: &mut bool,
    thumbnails_backend: Option<&mut ThumbnailBackend<'_>>,
) {
    state.core.rescan_if_needed(fs);
    if state.ui.thumbnails_enabled {
        state.ui.thumbnails.advance_frame();
    }

    use dear_imgui_rs::{SelectableFlags, TableColumnFlags, TableColumnSetup, TableFlags};

    let entries: Vec<DirEntry> = state.core.entries().to_vec();
    if entries.is_empty() {
        if state.ui.empty_hint_enabled {
            let msg = state
                .ui
                .empty_hint_static_message
                .clone()
                .unwrap_or_else(|| "No matching entries.".to_string());
            ui.text_colored(state.ui.empty_hint_color, msg);
        }
        return;
    }

    let thumb = state.ui.thumbnail_size;
    let pad = 6.0f32;
    let text_h = ui.text_line_height_with_spacing();
    let cell_w = (thumb[0] + pad * 2.0).max(64.0);
    let cell_h = thumb[1] + text_h + pad * 3.0;
    let cols = ((size[0].max(1.0)) / cell_w).floor() as usize;
    let cols = cols.clamp(1, 16);

    let flags = TableFlags::SCROLL_Y
        | TableFlags::SIZING_FIXED_FIT
        | TableFlags::NO_PAD_OUTER_X
        | TableFlags::NO_PAD_INNER_X;
    let mut col_setups = Vec::with_capacity(cols);
    for i in 0..cols {
        col_setups.push(
            TableColumnSetup::new(format!("##grid_col_{i}"))
                .flags(TableColumnFlags::NO_SORT | TableColumnFlags::NO_RESIZE)
                .init_width_or_weight(cell_w),
        );
    }

    ui.table("file_grid")
        .flags(flags)
        .outer_size(size)
        .columns(col_setups)
        .headers(false)
        .build(|ui| {
            let dl = ui.get_window_draw_list();

            if ui.is_window_focused() && !ui.io().want_text_input() {
                let modifiers = Modifiers {
                    ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                    shift: ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift),
                };

                if modifiers.ctrl && ui.is_key_pressed(Key::A) && !modifiers.shift {
                    let _ = state.core.handle_event(CoreEvent::SelectAll);
                }
                if modifiers.ctrl && ui.is_key_pressed(Key::C) && !modifiers.shift {
                    clipboard_set_from_selection(state, ClipboardOp::Copy);
                }
                if modifiers.ctrl && ui.is_key_pressed(Key::X) && !modifiers.shift {
                    clipboard_set_from_selection(state, ClipboardOp::Cut);
                }
                if modifiers.ctrl && ui.is_key_pressed(Key::V) && !modifiers.shift {
                    state.ui.ui_error = None;
                    start_paste_into_cwd(state);
                    if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                        state.ui.ui_error = Some(e);
                        state.ui.paste_job = None;
                    }
                }
                if ui.is_key_pressed_with_repeat(Key::LeftArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: -1,
                        modifiers,
                    });
                }
                if ui.is_key_pressed_with_repeat(Key::RightArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: 1,
                        modifiers,
                    });
                }
                if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: -(cols as i32),
                        modifiers,
                    });
                }
                if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                    let _ = state.core.handle_event(CoreEvent::MoveFocus {
                        delta: cols as i32,
                        modifiers,
                    });
                }
                if state.ui.type_select_enabled && !modifiers.ctrl && !modifiers.shift {
                    handle_type_select(ui, state);
                }
            }

            let mut idx = 0usize;
            while idx < entries.len() {
                ui.table_next_row();
                for _ in 0..cols {
                    ui.table_next_column();
                    if idx >= entries.len() {
                        break;
                    }
                    let item_idx = idx;
                    let e = &entries[item_idx];
                    idx += 1;

                    let selected = state.core.is_selected_id(e.id);
                    let visual = style_visual_for_entry(state, e);

                    let mut label = e.display_name();
                    if let Some(icon) = visual.icon.as_deref() {
                        label = format!("{icon} {label}");
                    }

                    let _id = ui.push_id(item_idx as i32);
                    let clicked = ui
                        .selectable_config("##grid_item")
                        .selected(selected)
                        .flags(SelectableFlags::ALLOW_OVERLAP)
                        .size([cell_w, cell_h])
                        .build();

                    let item_min = ui.item_rect_min();
                    let item_max = ui.item_rect_max();
                    let img_min = [item_min[0] + pad, item_min[1] + pad];
                    let img_max = [img_min[0] + thumb[0], img_min[1] + thumb[1]];

                    if state.ui.reveal_id_next == Some(e.id) {
                        ui.set_scroll_here_y(0.5);
                        state.ui.reveal_id_next = None;
                    }

                    if state.ui.thumbnails_enabled && !e.is_dir {
                        let max_size_u32 = [thumb[0].max(1.0) as u32, thumb[1].max(1.0) as u32];
                        if let Some(tex) = state.ui.thumbnails.texture_id(&e.path) {
                            dl.add_image(
                                tex,
                                img_min,
                                img_max,
                                [0.0, 0.0],
                                [1.0, 1.0],
                                dear_imgui_rs::Color::rgb(1.0, 1.0, 1.0),
                            );
                        } else {
                            dl.add_rect(
                                img_min,
                                img_max,
                                dear_imgui_rs::Color::new(0.2, 0.2, 0.2, 1.0),
                            )
                            .filled(true)
                            .build();
                            if ui.is_item_visible() {
                                state.ui.thumbnails.request_visible(&e.path, max_size_u32);
                            }
                        }
                    } else {
                        dl.add_rect(
                            img_min,
                            img_max,
                            dear_imgui_rs::Color::new(0.2, 0.2, 0.2, 1.0),
                        )
                        .filled(true)
                        .build();
                    }

                    let text_pos = [item_min[0] + pad, img_max[1] + pad];
                    let col = visual
                        .text_color
                        .map(|c| dear_imgui_rs::Color::from_array(c))
                        .unwrap_or_else(|| dear_imgui_rs::Color::rgb(1.0, 1.0, 1.0));
                    let _font = visual.font_id.map(|id| ui.push_font(id));
                    {
                        dl.with_clip_rect(item_min, item_max, || {
                            dl.add_text(text_pos, col, &label);
                        });
                    }

                    if clicked {
                        let modifiers = Modifiers {
                            ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                            shift: ui.is_key_down(Key::LeftShift)
                                || ui.is_key_down(Key::RightShift),
                        };
                        let _ = state.core.handle_event(CoreEvent::ClickEntry {
                            id: e.id,
                            modifiers,
                        });
                        if matches!(state.core.mode, DialogMode::SaveFile) && !e.is_dir {
                            state.core.save_name = e.name.clone();
                        }
                    }

                    if ui.is_item_hovered() {
                        if let Some(t) = visual.tooltip.as_deref() {
                            ui.tooltip_text(t);
                        }
                    }

                    if let Some(_popup) = ui.begin_popup_context_item() {
                        if !selected {
                            let _ = state.core.handle_event(CoreEvent::FocusAndSelectById(e.id));
                        }
                        let has_selection = state.core.has_selection();
                        let can_paste = state
                            .ui
                            .clipboard
                            .as_ref()
                            .map(|c| !c.sources.is_empty())
                            .unwrap_or(false);

                        if ui.menu_item_enabled_selected(
                            "Copy",
                            Some("Ctrl+C"),
                            false,
                            has_selection,
                        ) {
                            clipboard_set_from_selection(state, ClipboardOp::Copy);
                            ui.close_current_popup();
                        }
                        if ui.menu_item_enabled_selected(
                            "Cut",
                            Some("Ctrl+X"),
                            false,
                            has_selection,
                        ) {
                            clipboard_set_from_selection(state, ClipboardOp::Cut);
                            ui.close_current_popup();
                        }
                        if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste)
                        {
                            state.ui.ui_error = None;
                            start_paste_into_cwd(state);
                            if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                                state.ui.ui_error = Some(e);
                                state.ui.paste_job = None;
                            }
                            ui.close_current_popup();
                        }

                        ui.separator();
                        let can_rename = state.core.selected_len() == 1;
                        if ui.menu_item_enabled_selected("Rename", Some("F2"), false, can_rename) {
                            open_rename_modal_from_selection(state);
                            ui.close_current_popup();
                        }
                        if ui.menu_item_enabled_selected("Delete", Some("Del"), false, true) {
                            open_delete_modal_from_selection(state);
                            ui.close_current_popup();
                        }
                    }

                    if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                        state.ui.ui_error = None;
                        *request_confirm |= matches!(
                            state
                                .core
                                .handle_event(CoreEvent::DoubleClickEntry { id: e.id }),
                            CoreEventOutcome::RequestConfirm
                        );
                    }
                }
            }
            if let Some(_popup) = ui.begin_popup_context_window() {
                let can_paste = state
                    .ui
                    .clipboard
                    .as_ref()
                    .map(|c| !c.sources.is_empty())
                    .unwrap_or(false);
                if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste) {
                    state.ui.ui_error = None;
                    start_paste_into_cwd(state);
                    if let Err(e) = run_paste_job_until_wait_or_done(state, fs) {
                        state.ui.ui_error = Some(e);
                        state.ui.paste_job = None;
                    }
                    ui.close_current_popup();
                }
            }
        });

    let mut thumbnails_backend = thumbnails_backend;
    if state.ui.thumbnails_enabled {
        if let Some(backend) = thumbnails_backend.as_deref_mut() {
            state.ui.thumbnails.maintain(backend);
        }
    }
}

fn draw_thumbnail_cell(ui: &Ui, state: &mut FileDialogState, e: &DirEntry) {
    if e.is_dir {
        ui.text("");
        return;
    }

    let max_size_u32 = [
        state.ui.thumbnail_size[0].max(1.0) as u32,
        state.ui.thumbnail_size[1].max(1.0) as u32,
    ];
    let size = state.ui.thumbnail_size;

    if let Some(tex) = state.ui.thumbnails.texture_id(&e.path) {
        ui.image(tex, size);
        return;
    }

    ui.text_disabled("...");
    if ui.is_item_visible() {
        state.ui.thumbnails.request_visible(&e.path, max_size_u32);
    }
}

fn handle_type_select(ui: &Ui, state: &mut FileDialogState) {
    if !state.ui.type_select_enabled {
        return;
    }
    let now = Instant::now();
    let timeout = Duration::from_millis(state.ui.type_select_timeout_ms);
    if let Some(last) = state.ui.type_select_last_input {
        if now.duration_since(last) > timeout {
            state.ui.type_select_buffer.clear();
        }
    }

    let Some(ch) = collect_type_select_char(ui) else {
        return;
    };
    if ch.is_whitespace() {
        return;
    }
    state.ui.type_select_buffer.push(ch.to_ascii_lowercase());
    state.ui.type_select_last_input = Some(now);
    let _ = state.core.handle_event(CoreEvent::SelectByPrefix(
        state.ui.type_select_buffer.clone(),
    ));
}

fn collect_type_select_char(ui: &Ui) -> Option<char> {
    let alpha = [
        (Key::A, 'a'),
        (Key::B, 'b'),
        (Key::C, 'c'),
        (Key::D, 'd'),
        (Key::E, 'e'),
        (Key::F, 'f'),
        (Key::G, 'g'),
        (Key::H, 'h'),
        (Key::I, 'i'),
        (Key::J, 'j'),
        (Key::K, 'k'),
        (Key::L, 'l'),
        (Key::M, 'm'),
        (Key::N, 'n'),
        (Key::O, 'o'),
        (Key::P, 'p'),
        (Key::Q, 'q'),
        (Key::R, 'r'),
        (Key::S, 's'),
        (Key::T, 't'),
        (Key::U, 'u'),
        (Key::V, 'v'),
        (Key::W, 'w'),
        (Key::X, 'x'),
        (Key::Y, 'y'),
        (Key::Z, 'z'),
    ];
    for (k, c) in alpha {
        if ui.is_key_pressed(k) {
            return Some(c);
        }
    }

    let digits = [
        (Key::Key0, '0'),
        (Key::Key1, '1'),
        (Key::Key2, '2'),
        (Key::Key3, '3'),
        (Key::Key4, '4'),
        (Key::Key5, '5'),
        (Key::Key6, '6'),
        (Key::Key7, '7'),
        (Key::Key8, '8'),
        (Key::Key9, '9'),
    ];
    for (k, c) in digits {
        if ui.is_key_pressed(k) {
            return Some(c);
        }
    }

    let punct = [(Key::Minus, '-'), (Key::Period, '.'), (Key::Slash, '/')];
    for (k, c) in punct {
        if ui.is_key_pressed(k) {
            return Some(c);
        }
    }

    None
}
