use std::borrow::Cow;

#[cfg(test)]
use crate::core::DialogMode;
use crate::core::{LayoutStyle, SortBy};
use crate::dialog_core::{
    CoreEvent, CoreEventOutcome, DirEntry, EntryId, Modifiers, igfd_type_dots_to_extract,
};
use crate::dialog_state::{
    ClipboardOp, FileDialogState, FileDialogUiState, FileListDataColumn, FileListViewMode,
    HeaderStyle,
};
use dear_imgui_rs::input::{Key, MouseButton};
use dear_imgui_rs::{ListClipper, TableRowFlags, Ui};
use web_time::Instant;

use super::ops::{
    clipboard_set_from_selection, open_delete_modal_from_selection,
    open_rename_modal_from_selection, start_paste_into_cwd,
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
    resolved_data_column_weight, resolved_preview_column_weight,
    sync_runtime_column_order_from_table, sync_runtime_column_weights_from_table,
    type_extension_by_dot_count,
};
use format::{ModifiedTimeFormat, format_size};
use style::{TextColorToken, style_visual_for_entry};

use super::apply_file_list_view_from_ui;
#[cfg(test)]
pub(in crate::ui) use columns::{ListColumnLayout, list_column_layout, merged_order_with_current};

fn ellipsize_text<'a>(ui: &Ui, text: &'a str, max_w: f32) -> Cow<'a, str> {
    if max_w <= 1.0 || text.is_empty() {
        return Cow::Borrowed(text);
    }

    let w = ui.calc_text_size(text)[0];
    if w <= max_w {
        return Cow::Borrowed(text);
    }

    let ell = "…";
    let ell_w = ui.calc_text_size(ell)[0];
    if ell_w >= max_w {
        return Cow::Borrowed(ell);
    }

    let target = (max_w - ell_w).max(0.0);
    let mut indices: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    indices.push(text.len());

    let mut lo = 0usize;
    let mut hi = indices.len().saturating_sub(1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let s = &text[..indices[mid]];
        let sw = ui.calc_text_size(s)[0];
        if sw <= target {
            lo = mid;
        } else {
            hi = mid.saturating_sub(1);
        }
    }

    let prefix = &text[..indices[lo]];
    Cow::Owned(format!("{prefix}{ell}"))
}

fn single_line(text: &str) -> Cow<'_, str> {
    if text.contains(['\n', '\r']) {
        Cow::Owned(text.replace(['\n', '\r'], "⏎"))
    } else {
        Cow::Borrowed(text)
    }
}

fn handle_entry_click(ui: &Ui, state: &mut FileDialogState, id: EntryId) {
    let modifiers = Modifiers {
        ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
        shift: ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift),
    };
    let _ = state
        .core
        .handle_event(CoreEvent::ClickEntry { id, modifiers });
    state.core.use_entry_as_save_name(id);
}

pub(super) fn draw_file_table(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    request_confirm: &mut bool,
    has_thumbnail_backend: bool,
) {
    state.tick_scan();
    match state.ui.config.file_list_view {
        FileListViewMode::List => draw_file_table_view(
            ui,
            state,
            size,
            request_confirm,
            has_thumbnail_backend,
            false,
        ),
        FileListViewMode::ThumbnailsList => {
            state.ui.config.thumbnails_enabled = true;
            draw_file_table_view(
                ui,
                state,
                size,
                request_confirm,
                has_thumbnail_backend,
                true,
            )
        }
        FileListViewMode::Grid => {
            draw_file_grid_view(ui, state, size, request_confirm, has_thumbnail_backend)
        }
    }
}

fn draw_entry_context_menu(
    ui: &Ui,
    state: &mut FileDialogState,
    request_confirm: &mut bool,
    entry_id: EntryId,
    selected: bool,
) {
    if !selected {
        let _ = state
            .core
            .handle_event(CoreEvent::FocusAndSelectById(entry_id));
    }

    let has_selection = state.core.has_selection();
    let can_paste = state
        .ui
        .operations
        .paste
        .clipboard
        .as_ref()
        .map(|c| !c.sources.is_empty())
        .unwrap_or(false);

    if ui.menu_item_enabled_selected("Open", Some("Enter"), false, true) {
        state.ui.runtime.error = None;
        *request_confirm |= matches!(
            state
                .core
                .handle_event(CoreEvent::DoubleClickEntry { id: entry_id }),
            CoreEventOutcome::RequestConfirm
        );
        ui.close_current_popup();
        return;
    }

    ui.separator();

    let can_rename = state.core.selected_len() == 1;
    if ui.menu_item_enabled_selected("Rename", Some("F2"), false, can_rename) {
        open_rename_modal_from_selection(state);
        ui.close_current_popup();
        return;
    }
    if ui.menu_item_enabled_selected("Delete", Some("Del"), false, true) {
        open_delete_modal_from_selection(state);
        ui.close_current_popup();
        return;
    }

    ui.separator();

    // Note: Copy/Cut/Paste here operate on the dialog's internal file clipboard
    // (not the OS clipboard). Keep wording aligned with IGFD-style actions.
    if ui.menu_item_enabled_selected("Copy", Some("Ctrl+C"), false, has_selection) {
        clipboard_set_from_selection(state, ClipboardOp::Copy);
        ui.close_current_popup();
        return;
    }
    if ui.menu_item_enabled_selected("Cut", Some("Ctrl+X"), false, has_selection) {
        clipboard_set_from_selection(state, ClipboardOp::Cut);
        ui.close_current_popup();
        return;
    }
    if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste) {
        state.ui.runtime.error = start_paste_into_cwd(state).err();
        ui.close_current_popup();
    }
}

fn draw_file_list_window_context_menu(
    ui: &Ui,
    state: &mut FileDialogState,
    has_thumbnail_backend: bool,
) {
    let can_paste = state
        .ui
        .operations
        .paste
        .clipboard
        .as_ref()
        .map(|c| !c.sources.is_empty())
        .unwrap_or(false);

    if ui.menu_item_enabled_selected("Refresh", Some("F5"), false, true) {
        let _ = state.core.handle_event(CoreEvent::Refresh);
        ui.close_current_popup();
        return;
    }

    if state.ui.config.new_folder_enabled
        && ui.menu_item_enabled_selected("New Folder", None::<&str>, false, true)
    {
        match state.ui.config.layout {
            LayoutStyle::Standard => {
                state.ui.operations.new_folder.inline_active = true;
            }
            LayoutStyle::Minimal => {
                state.ui.operations.new_folder.open_next = true;
            }
        }
        state.ui.operations.new_folder.name.clear();
        state.ui.operations.new_folder.error = None;
        state.ui.operations.new_folder.focus_next = true;
        ui.close_current_popup();
        return;
    }

    ui.separator();

    let list_active = matches!(state.ui.config.file_list_view, FileListViewMode::List);
    let thumbs_active = matches!(
        state.ui.config.file_list_view,
        FileListViewMode::ThumbnailsList
    );
    let grid_active = matches!(state.ui.config.file_list_view, FileListViewMode::Grid);

    if let Some(_menu) = ui.begin_menu("View") {
        if ui.menu_item_enabled_selected("File List", None::<&str>, list_active, true) {
            apply_file_list_view_from_ui(state, FileListViewMode::List, has_thumbnail_backend);
        }
        if ui.menu_item_enabled_selected(
            "Thumbnails List",
            None::<&str>,
            thumbs_active,
            has_thumbnail_backend,
        ) {
            apply_file_list_view_from_ui(
                state,
                FileListViewMode::ThumbnailsList,
                has_thumbnail_backend,
            );
        }
        if ui.menu_item_enabled_selected(
            "Thumbnails Grid",
            None::<&str>,
            grid_active,
            has_thumbnail_backend,
        ) {
            apply_file_list_view_from_ui(state, FileListViewMode::Grid, has_thumbnail_backend);
        }

        if matches!(
            state.ui.config.file_list_view,
            FileListViewMode::List | FileListViewMode::ThumbnailsList
        ) {
            ui.separator();
            if ui.menu_item_enabled_selected("Columns...", None::<&str>, false, true) {
                ui.open_popup("##fb_columns_popup");
                ui.close_current_popup();
                return;
            }
        }
        _menu.end();
    }

    ui.separator();

    if ui.menu_item_enabled_selected("Options...", None::<&str>, false, true) {
        ui.open_popup("##fb_options");
        ui.close_current_popup();
        return;
    }

    let show_hidden = state.core.show_hidden;
    if ui.menu_item_enabled_selected("Show hidden files", None::<&str>, show_hidden, true) {
        state.core.show_hidden = !show_hidden;
    }

    ui.separator();

    if ui.menu_item_enabled_selected("Paste", Some("Ctrl+V"), false, can_paste) {
        state.ui.runtime.error = start_paste_into_cwd(state).err();
        ui.close_current_popup();
    }
}

fn draw_file_table_view(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    request_confirm: &mut bool,
    has_thumbnail_backend: bool,
    force_preview: bool,
) {
    // Table
    use dear_imgui_rs::{SortDirection, TableColumnFlags, TableFlags, TableSizingPolicy};
    let flags = TableFlags::RESIZABLE
        | TableFlags::REORDERABLE
        | TableFlags::ROW_BG
        | TableFlags::BORDERS_V
        | TableFlags::BORDERS_OUTER
        | TableFlags::SCROLL_Y
        | TableFlags::SORTABLE; // enable built-in header sorting
    let columns_config = &state.ui.config.file_list_columns;
    let show_preview =
        state.ui.config.thumbnails_enabled && (columns_config.show_preview || force_preview);
    let show_size = columns_config.show_size;
    let show_modified = columns_config.show_modified;
    let layout = list_column_layout_impl(show_preview, columns_config);
    let type_dots_to_extract = if matches!(state.ui.config.header_style, HeaderStyle::IgfdClassic) {
        igfd_type_dots_to_extract(state.core.active_filter())
    } else {
        1
    };

    let mut table = ui
        .table("file_table")
        .flags(flags)
        .sizing_policy(TableSizingPolicy::StretchProp)
        .outer_size(size);
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
                .user_data(1_u32)
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
                .user_data(2_u32)
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
                .user_data(3_u32)
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
        if let Some(mut specs) = ui.table_get_sort_specs()
            && specs.is_dirty()
        {
            let extension_sort_by =
                if matches!(state.ui.config.header_style, HeaderStyle::IgfdClassic) {
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
                    (i, SortDirection::Ascending) if layout.size == Some(i) => (SortBy::Size, true),
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
                state.tick_scan();
            }
            specs.clear_dirty(ui);
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
                state.ui.runtime.error = start_paste_into_cwd(state).err();
            }
            if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                move_focus_and_reveal(state, -1, modifiers);
            }
            if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                move_focus_and_reveal(state, 1, modifiers);
            }
            if state.ui.config.type_select_enabled && !modifiers.ctrl && !modifiers.shift {
                handle_type_select(ui, state);
            }
        }

        let row_count = state.core.entries().len().min(i32::MAX as usize - 1);
        if row_count == 0 {
            let _ = resolve_reveal_index(state, 0);
            if state.ui.config.empty_hint_enabled {
                ui.table_next_row();
                if show_preview {
                    ui.table_next_column();
                    ui.text("");
                }
                ui.table_next_column();
                let msg = if let Some(custom) = &state.ui.config.empty_hint_static_message {
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
                ui.text_colored(state.ui.config.empty_hint_color, msg);
            }
            return;
        }

        let modified_times = ModifiedTimeFormat::default();
        let mut content_height = ui.text_line_height();
        for font_id in state.ui.config.file_style_fonts.values() {
            let _font = ui.push_font(*font_id);
            content_height = content_height.max(ui.text_line_height());
        }
        if show_preview {
            content_height = content_height.max(state.ui.config.thumbnail_size[1]);
        }
        let row_height = content_height + 2.0 * ui.clone_style().cell_padding()[1];
        let mut clipper = ListClipper::new(row_count)
            .items_height(row_height)
            .begin(ui);
        if let Some(index) = resolve_reveal_index(state, row_count) {
            clipper.include_item_by_index(index);
        }
        let mut row_ids = Vec::new();
        while clipper.step() {
            row_ids.clear();
            state
                .core
                .append_row_ids(clipper.display_start()..clipper.display_end(), &mut row_ids);
            for &id in &row_ids {
                let Some(entry) = state.core.entry_by_id(id) else {
                    continue;
                };
                ui.table_next_row_with_flags(TableRowFlags::empty(), row_height);
                let _id = ui.push_id(id.widget_id());
                if show_preview {
                    ui.table_next_column();
                    draw_thumbnail_cell(ui, &mut state.ui, entry);
                }

                let selected = state.core.is_selected_id(id);
                let visual = style_visual_for_entry(&mut state.ui.config, entry);

                for column in &layout.data_columns {
                    ui.table_next_column();
                    let Some(e) = state.core.entry_by_id(id) else {
                        continue;
                    };
                    match column {
                        FileListDataColumn::Name => {
                            let label = e.display_name();
                            let label = if let Some(icon) = visual.icon.as_deref() {
                                Cow::Owned(format!("{icon} {label}"))
                            } else {
                                label
                            };
                            let label = single_line(&label);
                            let _font = visual.font_id.map(|id| ui.push_font(id));
                            let _color = visual
                                .text_color
                                .map(|color| TextColorToken::push(ui, color))
                                .unwrap_or_else(TextColorToken::none);
                            {
                                if ui
                                    .selectable_config(label.as_ref())
                                    .selected(selected)
                                    .span_all_columns(true)
                                    .build()
                                {
                                    handle_entry_click(ui, state, id);
                                }
                            }

                            if ui.is_item_hovered()
                                && let Some(t) = visual.tooltip.as_deref()
                            {
                                ui.tooltip_text(t);
                            }

                            if let Some(_popup) = ui.begin_popup_context_item() {
                                draw_entry_context_menu(ui, state, request_confirm, id, selected);
                            }

                            if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left)
                            {
                                state.ui.runtime.error = None;
                                *request_confirm |= matches!(
                                    state.core.handle_event(CoreEvent::DoubleClickEntry { id }),
                                    CoreEventOutcome::RequestConfirm
                                );
                            }
                        }
                        FileListDataColumn::Extension => {
                            let extension = if e.is_dir {
                                ""
                            } else if matches!(
                                state.ui.config.header_style,
                                HeaderStyle::IgfdClassic
                            ) {
                                type_extension_by_dot_count(&e.name, type_dots_to_extract)
                            } else if let Some(i) = e.name.find('.') {
                                &e.name[i..]
                            } else {
                                ""
                            };
                            ui.text(single_line(extension).as_ref());
                        }
                        FileListDataColumn::Size => {
                            ui.text(match e.size {
                                Some(s) => format_size(s),
                                None => String::new(),
                            });
                        }
                        FileListDataColumn::Modified => {
                            ui.text(modified_times.label(e.modified));
                            if ui.is_item_hovered()
                                && let Some(text) = modified_times.tooltip(e.modified)
                            {
                                ui.tooltip_text(text);
                            }
                        }
                    }
                }

                if state.ui.operations.reveal_id_next == Some(id) {
                    ui.set_scroll_here_y(0.5);
                    state.ui.operations.reveal_id_next = None;
                }
            }
        }

        if let Some(_popup) = ui.begin_popup_context_window() {
            draw_file_list_window_context_menu(ui, state, has_thumbnail_backend);
        }

        sync_runtime_column_order_from_table(&layout, &mut state.ui.config.file_list_columns);

        sync_runtime_column_weights_from_table(
            show_preview,
            &layout,
            &mut state.ui.config.file_list_columns,
        );
    });
}

fn draw_file_grid_view(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    request_confirm: &mut bool,
    has_thumbnail_backend: bool,
) {
    use dear_imgui_rs::{
        StyleColor, TableColumnFlags, TableColumnSetup, TableFlags, TableSizingPolicy,
    };

    let entry_count = state.core.entries().len();
    if entry_count == 0 {
        if state.ui.config.empty_hint_enabled {
            let msg = state
                .ui
                .config
                .empty_hint_static_message
                .clone()
                .unwrap_or_else(|| "No matching entries.".to_string());
            ui.text_colored(state.ui.config.empty_hint_color, msg);
        }
        return;
    }

    let thumb = state.ui.config.thumbnail_size;
    let style = ui.clone_style();
    let frame_pad = style.frame_padding();
    let pad_x = frame_pad[0].max(4.0);
    let pad_y = frame_pad[1].max(4.0);
    let spacing_y = style.item_spacing()[1].max(2.0);
    let text_h = ui.text_line_height_with_spacing();
    let cell_w = (thumb[0] + pad_x * 2.0).max(64.0);
    let cell_h = (thumb[1] + spacing_y + text_h + pad_y * 2.0).max(64.0);
    let cols = ((size[0].max(1.0)) / cell_w).floor() as usize;
    let cols = cols.clamp(1, 16);

    let flags = TableFlags::SCROLL_Y | TableFlags::NO_PAD_OUTER_X | TableFlags::NO_PAD_INNER_X;
    let mut col_setups = Vec::with_capacity(cols);
    for i in 0..cols {
        col_setups.push(
            TableColumnSetup::new(format!("##grid_col_{i}"))
                .flags(TableColumnFlags::NO_SORT | TableColumnFlags::NO_RESIZE)
                .fixed_width(cell_w),
        );
    }

    ui.table("file_grid")
        .flags(flags)
        .sizing_policy(TableSizingPolicy::FixedFit)
        .outer_size(size)
        .columns(col_setups)
        .headers(false)
        .build(|ui| {
            ui.with_window_draw_list(|dl| {
                let font_size = ui.current_font_size();

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
                        state.ui.runtime.error = start_paste_into_cwd(state).err();
                    }
                    if ui.is_key_pressed_with_repeat(Key::LeftArrow, true) {
                        move_focus_and_reveal(state, -1, modifiers);
                    }
                    if ui.is_key_pressed_with_repeat(Key::RightArrow, true) {
                        move_focus_and_reveal(state, 1, modifiers);
                    }
                    if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                        move_focus_and_reveal(state, -(cols as i32), modifiers);
                    }
                    if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                        move_focus_and_reveal(state, cols as i32, modifiers);
                    }
                    if state.ui.config.type_select_enabled && !modifiers.ctrl && !modifiers.shift {
                        handle_type_select(ui, state);
                    }
                }

                let row_count = entry_count.div_ceil(cols).min(i32::MAX as usize - 1);
                let row_height = cell_h + 2.0 * style.cell_padding()[1];
                let mut clipper = ListClipper::new(row_count)
                    .items_height(row_height)
                    .begin(ui);
                if let Some(row) = resolve_reveal_index(state, entry_count)
                    .map(|index| index / cols)
                    .filter(|row| *row < row_count)
                {
                    clipper.include_item_by_index(row);
                }
                let mut row_ids = Vec::new();
                while clipper.step() {
                    row_ids.clear();
                    let start = clipper.display_start() * cols;
                    let end = (clipper.display_end() * cols).min(entry_count);
                    state.core.append_row_ids(start..end, &mut row_ids);
                    for row in row_ids.chunks(cols) {
                        ui.table_next_row_with_flags(TableRowFlags::empty(), row_height);
                        for &id in row {
                            ui.table_next_column();
                            let Some(e) = state.core.entry_by_id(id) else {
                                continue;
                            };

                            let selected = state.core.is_selected_id(id);
                            let focused = state.core.focused_entry_id() == Some(id);
                            let visual = style_visual_for_entry(&mut state.ui.config, e);

                            let mut label = e.display_name();
                            if let Some(icon) = visual.icon.as_deref() {
                                label = Cow::Owned(format!("{icon} {label}"));
                            }

                            let _id = ui.push_id(id.widget_id());
                            let clicked = ui.invisible_button("##grid_item", [cell_w, cell_h]);
                            let hovered = ui.is_item_hovered();
                            let active = ui.is_item_active();

                            let item_min = ui.item_rect_min();
                            let item_max = ui.item_rect_max();
                            let img_min = [item_min[0] + pad_x, item_min[1] + pad_y];
                            let img_max = [img_min[0] + thumb[0], img_min[1] + thumb[1]];

                            if state.ui.operations.reveal_id_next == Some(id) {
                                ui.set_scroll_here_y(0.5);
                                state.ui.operations.reveal_id_next = None;
                            }

                            if state.ui.config.thumbnails_enabled && !e.is_dir {
                                let max_size_u32 =
                                    [thumb[0].max(1.0) as u32, thumb[1].max(1.0) as u32];
                                if ui.is_item_visible() {
                                    state.ui.thumbnails.request_visible(&e.path, max_size_u32);
                                }
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

                            if hovered || selected || active {
                                let overlay_style_color = if selected || active {
                                    StyleColor::HeaderActive
                                } else {
                                    StyleColor::HeaderHovered
                                };
                                let mut overlay = style.color(overlay_style_color);
                                overlay[3] *= if selected || active { 0.55 } else { 0.35 };
                                dl.add_rect(item_min, item_max, overlay)
                                    .filled(true)
                                    .rounding(style.frame_rounding())
                                    .build();
                            }
                            if focused {
                                let mut border = style.color(StyleColor::NavCursor);
                                border[3] *= 0.9;
                                dl.add_rect(item_min, item_max, border)
                                    .rounding(style.frame_rounding())
                                    .thickness(1.0)
                                    .build();
                            }

                            let label_min = [item_min[0] + pad_x, img_max[1] + spacing_y];
                            let mut label_max = [item_max[0] - pad_x, item_max[1] - pad_y];
                            if label_max[0] < label_min[0] {
                                label_max[0] = label_min[0];
                            }
                            if label_max[1] < label_min[1] {
                                label_max[1] = label_min[1];
                            }
                            let label_h = (label_max[1] - label_min[1]).max(0.0);
                            let text_y = label_min[1] + ((label_h - font_size).max(0.0) * 0.5);
                            let text_pos = [label_min[0], text_y];
                            let col = visual
                                .text_color
                                .map(dear_imgui_rs::Color::from_array)
                                .unwrap_or_else(|| dear_imgui_rs::Color::rgb(1.0, 1.0, 1.0));
                            let _font = visual.font_id.map(|id| ui.push_font(id));
                            {
                                let max_label_w =
                                    (item_max[0] - item_min[0] - pad_x * 2.0).max(0.0);
                                let display_label = ellipsize_text(ui, &label, max_label_w);
                                dl.with_clip_rect(label_min, label_max, || {
                                    dl.add_text(text_pos, col, display_label.as_ref());
                                });
                            }

                            if clicked {
                                handle_entry_click(ui, state, id);
                            }

                            if ui.is_item_hovered()
                                && let Some(t) = visual.tooltip.as_deref()
                            {
                                ui.tooltip_text(t);
                            }

                            if let Some(_popup) = ui.begin_popup_context_item() {
                                draw_entry_context_menu(ui, state, request_confirm, id, selected);
                            }

                            if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left)
                            {
                                state.ui.runtime.error = None;
                                *request_confirm |= matches!(
                                    state.core.handle_event(CoreEvent::DoubleClickEntry { id }),
                                    CoreEventOutcome::RequestConfirm
                                );
                            }
                        }
                    }
                }
                if let Some(_popup) = ui.begin_popup_context_window() {
                    draw_file_list_window_context_menu(ui, state, has_thumbnail_backend);
                }
            })
        });
}

fn draw_thumbnail_cell(ui: &Ui, view: &mut FileDialogUiState, e: &DirEntry) {
    if e.is_dir {
        ui.text("");
        return;
    }

    let max_size_u32 = [
        view.config.thumbnail_size[0].max(1.0) as u32,
        view.config.thumbnail_size[1].max(1.0) as u32,
    ];
    let size = view.config.thumbnail_size;

    if let Some(tex) = view.thumbnails.texture_id(&e.path) {
        ui.image(tex, size);
    } else {
        let p0 = ui.cursor_screen_pos();
        let p1 = [p0[0] + size[0], p0[1] + size[1]];
        ui.with_window_draw_list(|dl| {
            dl.add_rect(p0, p1, dear_imgui_rs::Color::new(0.2, 0.2, 0.2, 1.0))
                .filled(true)
                .build();
        });
        ui.dummy(size);
    }
    if ui.is_item_visible() {
        view.thumbnails.request_visible(&e.path, max_size_u32);
    }
}

fn resolve_reveal_index(state: &mut FileDialogState, max_count: usize) -> Option<usize> {
    let id = state.ui.operations.reveal_id_next?;
    if let Some(index) = state.core.row_index(id).filter(|index| *index < max_count) {
        return Some(index);
    }
    match state.core.scan_status() {
        crate::ScanStatus::Scanning { .. } | crate::ScanStatus::Partial { .. } => {}
        crate::ScanStatus::Idle
        | crate::ScanStatus::Complete { .. }
        | crate::ScanStatus::Failed { .. } => {
            state.ui.operations.reveal_id_next = None;
        }
    }
    None
}

fn move_focus_and_reveal(state: &mut FileDialogState, delta: i32, modifiers: Modifiers) {
    if let Some(id) = state.core.move_focus(delta, modifiers) {
        state.ui.operations.reveal_id_next = Some(id);
    }
}

fn handle_type_select(ui: &Ui, state: &mut FileDialogState) {
    if !state.ui.config.type_select_enabled {
        return;
    }
    update_type_select(state, collect_type_select_char(ui), Instant::now());
}

fn update_type_select(state: &mut FileDialogState, input: Option<char>, now: Instant) {
    let timeout = state.ui.config.type_select_timeout;
    if let Some(last) = state.ui.runtime.type_select_last_input
        && now.duration_since(last) > timeout
    {
        state.ui.runtime.type_select_buffer.clear();
    }

    let Some(ch) = input else {
        return;
    };
    if ch.is_whitespace() {
        return;
    }
    state
        .ui
        .runtime
        .type_select_buffer
        .push(ch.to_ascii_lowercase());
    state.ui.runtime.type_select_last_input = Some(now);
    if let Some(id) = state
        .core
        .select_by_prefix(&state.ui.runtime.type_select_buffer)
    {
        state.ui.operations.reveal_id_next = Some(id);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn type_select_expires_after_the_last_keypress() {
        let mut state = FileDialogState::new(DialogMode::OpenFile);
        let timeout = Duration::from_millis(750);
        state.ui.config.type_select_timeout = timeout;
        let first = Instant::now();
        update_type_select(&mut state, Some('a'), first);
        let second = first + timeout;
        update_type_select(&mut state, Some('b'), second);
        assert_eq!(state.ui.runtime.type_select_buffer, "ab");
        update_type_select(&mut state, None, second + timeout);
        update_type_select(
            &mut state,
            Some('c'),
            second + timeout + Duration::from_millis(1),
        );
        assert_eq!(state.ui.runtime.type_select_buffer, "c");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn keyboard_navigation_reveals_the_target_even_if_already_focused() {
        use crate::StdFileSystem;
        use crate::fs_ops::tests::TempDir;

        let temp = TempDir::new();
        let target_path = temp.path().join("zebra.txt");
        std::fs::write(temp.path().join("apple.txt"), []).unwrap();
        std::fs::write(&target_path, []).unwrap();
        let mut state = FileDialogState::with_blocking_filesystem(
            DialogMode::OpenFile,
            Box::new(StdFileSystem),
        );
        state.core.set_cwd(temp.path().to_owned());
        state.tick_scan();

        move_focus_and_reveal(&mut state, -1, Modifiers::default());
        let target = state.ui.operations.reveal_id_next.take().unwrap();
        assert_eq!(
            state.core.entry_path_by_id(target),
            Some(target_path.as_path())
        );
        assert_eq!(state.core.focused_entry_id(), Some(target));

        let now = Instant::now();
        update_type_select(&mut state, Some('z'), now);
        assert_eq!(state.ui.operations.reveal_id_next.take(), Some(target));
        update_type_select(&mut state, Some('q'), now);
        assert_eq!(state.ui.operations.reveal_id_next, None);

        // Once scanning is idle, a target hidden by search/filters is cleared so it cannot
        // trigger an unexpected scroll jump later when filters change.
        state.ui.operations.reveal_id_next = Some(target);
        state.core.search = "apple".to_string();
        state.tick_scan();
        let entry_count = state.core.entries().len();
        assert_eq!(resolve_reveal_index(&mut state, entry_count), None);
        assert_eq!(state.ui.operations.reveal_id_next, None);
    }
}
