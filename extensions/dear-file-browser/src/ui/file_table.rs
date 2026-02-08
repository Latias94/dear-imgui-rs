use std::time::{Duration, Instant};

use crate::core::{DialogMode, FileFilter, LayoutStyle, SortBy};
use crate::dialog_core::{CoreEvent, CoreEventOutcome, DirEntry, Modifiers};
use crate::dialog_state::{
    ClipboardOp, FileDialogState, FileListColumnsConfig, FileListDataColumn, FileListViewMode,
    HeaderStyle,
};
use crate::file_style::EntryKind;
use crate::fs::FileSystem;
use crate::thumbnails::ThumbnailBackend;
use dear_imgui_rs::Ui;
use dear_imgui_rs::input::{Key, MouseButton};
use dear_imgui_rs::sys;

use super::ops::{
    clipboard_set_from_selection, open_delete_modal_from_selection,
    open_rename_modal_from_selection, run_paste_job_until_wait_or_done, start_paste_into_cwd,
};

struct TextColorToken {
    pushed: bool,
}

struct StyleVisual {
    text_color: Option<[f32; 4]>,
    icon: Option<String>,
    tooltip: Option<String>,
    font_id: Option<dear_imgui_rs::FontId>,
}

fn style_visual_for_entry(state: &mut FileDialogState, e: &DirEntry) -> StyleVisual {
    let kind = if e.is_symlink {
        EntryKind::Link
    } else if e.is_dir {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let style = state.ui.file_styles.style_for_owned(&e.name, kind);
    let font_id = style
        .as_ref()
        .and_then(|s| s.font_token.as_deref())
        .and_then(|token| state.ui.file_style_fonts.get(token).copied());

    StyleVisual {
        text_color: style.as_ref().and_then(|s| s.text_color),
        icon: style.as_ref().and_then(|s| s.icon.clone()),
        tooltip: style.as_ref().and_then(|s| s.tooltip.clone()),
        font_id,
    }
}

impl TextColorToken {
    fn push(color: [f32; 4]) -> Self {
        unsafe {
            sys::igPushStyleColor_Vec4(
                sys::ImGuiCol_Text as i32,
                sys::ImVec4 {
                    x: color[0],
                    y: color[1],
                    z: color[2],
                    w: color[3],
                },
            );
        }
        Self { pushed: true }
    }

    fn none() -> Self {
        Self { pushed: false }
    }
}

impl Drop for TextColorToken {
    fn drop(&mut self) {
        if self.pushed {
            unsafe { sys::igPopStyleColor(1) };
        }
    }
}

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

fn data_column_label(column: FileListDataColumn) -> &'static str {
    match column {
        FileListDataColumn::Name => "Name",
        FileListDataColumn::Extension => "Ext",
        FileListDataColumn::Size => "Size",
        FileListDataColumn::Modified => "Modified",
    }
}

pub(super) fn extension_ui_label(state: &FileDialogState) -> &'static str {
    if matches!(state.ui.header_style, HeaderStyle::IgfdClassic) {
        "Type"
    } else {
        "Ext"
    }
}

fn igfd_type_dots_to_extract(active_filter: Option<&FileFilter>) -> usize {
    let Some(filter) = active_filter else {
        return 1;
    };
    let mut max_dots = 1usize;
    for token in &filter.extensions {
        let t = token.trim();
        if t.is_empty() {
            continue;
        }
        if is_regex_token(t) {
            continue;
        }
        let dot_count = t.as_bytes().iter().filter(|&&b| b == b'.').count();
        let token_dots = if t.contains('*') || t.contains('?') {
            dot_count
        } else if t.starts_with('.') {
            dot_count
        } else {
            dot_count.saturating_add(1)
        };
        max_dots = max_dots.max(token_dots);
    }
    max_dots.max(1)
}

fn is_regex_token(token: &str) -> bool {
    let t = token.trim();
    t.starts_with("((") && t.ends_with("))") && t.len() >= 4
}

fn type_extension_by_dot_count<'a>(name: &'a str, dots_to_extract: usize) -> &'a str {
    if dots_to_extract == 0 {
        return name.find('.').map(|i| &name[i..]).unwrap_or("");
    }
    let bytes = name.as_bytes();
    let total_dots = bytes.iter().filter(|&&b| b == b'.').count();
    if total_dots == 0 {
        return "";
    }
    let dots = dots_to_extract.min(total_dots);
    let mut seen = 0usize;
    for i in (0..bytes.len()).rev() {
        if bytes[i] == b'.' {
            seen += 1;
            if seen == dots {
                return &name[i..];
            }
        }
    }
    ""
}

pub(super) fn data_column_label_for_state(
    state: &FileDialogState,
    column: FileListDataColumn,
) -> &'static str {
    match column {
        FileListDataColumn::Extension => extension_ui_label(state),
        _ => data_column_label(column),
    }
}

pub(super) fn is_data_column_visible(
    config: &FileListColumnsConfig,
    column: FileListDataColumn,
) -> bool {
    match column {
        FileListDataColumn::Name => true,
        FileListDataColumn::Extension => config.show_extension,
        FileListDataColumn::Size => config.show_size,
        FileListDataColumn::Modified => config.show_modified,
    }
}

pub(super) fn apply_compact_column_layout(config: &mut FileListColumnsConfig) {
    config.show_preview = false;
    config.show_extension = true;
    config.show_size = true;
    config.show_modified = false;
    config.order = [
        FileListDataColumn::Name,
        FileListDataColumn::Extension,
        FileListDataColumn::Size,
        FileListDataColumn::Modified,
    ];
}

pub(super) fn apply_compact_column_layout_keep_preview(config: &mut FileListColumnsConfig) {
    let preview = config.show_preview;
    apply_compact_column_layout(config);
    config.show_preview = preview;
}

pub(super) fn apply_balanced_column_layout(config: &mut FileListColumnsConfig) {
    config.show_preview = true;
    config.show_extension = true;
    config.show_size = true;
    config.show_modified = true;
    config.order = [
        FileListDataColumn::Name,
        FileListDataColumn::Extension,
        FileListDataColumn::Size,
        FileListDataColumn::Modified,
    ];
}

pub(super) fn apply_balanced_column_layout_keep_preview(config: &mut FileListColumnsConfig) {
    let preview = config.show_preview;
    apply_balanced_column_layout(config);
    config.show_preview = preview;
}

pub(super) fn move_column_order_up(order: &mut [FileListDataColumn; 4], index: usize) -> bool {
    if index == 0 || index >= order.len() {
        return false;
    }
    order.swap(index, index - 1);
    true
}

pub(super) fn move_column_order_down(order: &mut [FileListDataColumn; 4], index: usize) -> bool {
    if index + 1 >= order.len() {
        return false;
    }
    order.swap(index, index + 1);
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ListColumnLayout {
    pub(super) data_columns: Vec<FileListDataColumn>,
    pub(super) name: i16,
    pub(super) extension: Option<i16>,
    pub(super) size: Option<i16>,
    pub(super) modified: Option<i16>,
}

pub(super) fn list_column_layout(
    show_preview: bool,
    config: &FileListColumnsConfig,
) -> ListColumnLayout {
    let mut data_columns = Vec::with_capacity(4);
    for column in config.normalized_order() {
        match column {
            FileListDataColumn::Name => data_columns.push(column),
            FileListDataColumn::Extension if config.show_extension => data_columns.push(column),
            FileListDataColumn::Size if config.show_size => data_columns.push(column),
            FileListDataColumn::Modified if config.show_modified => data_columns.push(column),
            _ => {}
        }
    }

    let mut index: i16 = if show_preview { 1 } else { 0 };
    let mut name = None;
    let mut extension = None;
    let mut size = None;
    let mut modified = None;

    for column in &data_columns {
        match column {
            FileListDataColumn::Name => name = Some(index),
            FileListDataColumn::Extension => extension = Some(index),
            FileListDataColumn::Size => size = Some(index),
            FileListDataColumn::Modified => modified = Some(index),
        }
        index += 1;
    }

    ListColumnLayout {
        data_columns,
        name: name.expect("name column should always be present"),
        extension,
        size,
        modified,
    }
}

fn validated_column_weight(override_weight: Option<f32>, default_weight: f32) -> f32 {
    match override_weight {
        Some(weight) if weight.is_finite() && weight > 0.0 => weight,
        _ => default_weight,
    }
}

fn default_preview_column_weight() -> f32 {
    0.12
}

fn default_data_column_weight(
    column: FileListDataColumn,
    show_preview: bool,
    show_size: bool,
    show_modified: bool,
) -> f32 {
    match column {
        FileListDataColumn::Name => {
            if show_size || show_modified {
                if show_preview { 0.52 } else { 0.56 }
            } else if show_preview {
                0.88
            } else {
                0.92
            }
        }
        FileListDataColumn::Extension => {
            if show_size || show_modified {
                0.12
            } else {
                0.08
            }
        }
        FileListDataColumn::Size => {
            if show_modified {
                0.16
            } else {
                0.2
            }
        }
        FileListDataColumn::Modified => {
            if show_size {
                0.2
            } else {
                0.24
            }
        }
    }
}

fn column_weight_override(
    config: &FileListColumnsConfig,
    column: FileListDataColumn,
) -> Option<f32> {
    match column {
        FileListDataColumn::Name => config.weight_overrides.name,
        FileListDataColumn::Extension => config.weight_overrides.extension,
        FileListDataColumn::Size => config.weight_overrides.size,
        FileListDataColumn::Modified => config.weight_overrides.modified,
    }
}

fn resolved_preview_column_weight(config: &FileListColumnsConfig) -> f32 {
    validated_column_weight(
        config.weight_overrides.preview,
        default_preview_column_weight(),
    )
}

fn resolved_data_column_weight(
    config: &FileListColumnsConfig,
    column: FileListDataColumn,
    show_preview: bool,
    show_size: bool,
    show_modified: bool,
) -> f32 {
    validated_column_weight(
        column_weight_override(config, column),
        default_data_column_weight(column, show_preview, show_size, show_modified),
    )
}

fn table_column_stretch_weight(table: *const sys::ImGuiTable, column_index: i16) -> Option<f32> {
    if table.is_null() || column_index < 0 {
        return None;
    }
    let columns_count = unsafe { (*table).ColumnsCount.max(0) as usize };
    let index = column_index as usize;
    if index >= columns_count {
        return None;
    }

    let columns_ptr = unsafe { (*table).Columns.Data };
    if columns_ptr.is_null() {
        return None;
    }

    let weight = unsafe { (*columns_ptr.add(index)).StretchWeight };
    if weight.is_finite() && weight > 0.0 {
        Some(weight)
    } else {
        None
    }
}

fn table_data_column_for_index(
    layout: &ListColumnLayout,
    column_index: i16,
) -> Option<FileListDataColumn> {
    if column_index == layout.name {
        return Some(FileListDataColumn::Name);
    }
    if layout.extension == Some(column_index) {
        return Some(FileListDataColumn::Extension);
    }
    if layout.size == Some(column_index) {
        return Some(FileListDataColumn::Size);
    }
    if layout.modified == Some(column_index) {
        return Some(FileListDataColumn::Modified);
    }
    None
}

pub(super) fn merged_order_with_current(
    visible_order: &[FileListDataColumn],
    current_order: [FileListDataColumn; 4],
) -> [FileListDataColumn; 4] {
    let mut merged = Vec::with_capacity(4);
    for &column in visible_order {
        if !merged.contains(&column) {
            merged.push(column);
        }
    }
    for column in current_order {
        if !merged.contains(&column) {
            merged.push(column);
        }
    }
    for column in [
        FileListDataColumn::Name,
        FileListDataColumn::Extension,
        FileListDataColumn::Size,
        FileListDataColumn::Modified,
    ] {
        if !merged.contains(&column) {
            merged.push(column);
        }
    }
    [merged[0], merged[1], merged[2], merged[3]]
}

fn table_data_columns_by_display_order(
    table: *const sys::ImGuiTable,
    layout: &ListColumnLayout,
) -> Vec<FileListDataColumn> {
    if table.is_null() {
        return Vec::new();
    }
    let columns_count = unsafe { (*table).ColumnsCount.max(0) as usize };
    let columns_ptr = unsafe { (*table).Columns.Data };
    if columns_ptr.is_null() {
        return Vec::new();
    }

    let mut ordered = Vec::with_capacity(layout.data_columns.len());
    for index in 0..columns_count {
        let index_i16 = index as i16;
        let Some(column) = table_data_column_for_index(layout, index_i16) else {
            continue;
        };
        let display_order = unsafe { (*columns_ptr.add(index)).DisplayOrder };
        ordered.push((display_order, column));
    }
    ordered.sort_by_key(|(display_order, _)| *display_order);
    ordered.into_iter().map(|(_, column)| column).collect()
}

fn sync_runtime_column_order_from_table(
    layout: &ListColumnLayout,
    config: &mut FileListColumnsConfig,
) {
    let table = unsafe { sys::igGetCurrentTable() };
    if table.is_null() {
        return;
    }
    let visible_order = table_data_columns_by_display_order(table, layout);
    if visible_order.is_empty() {
        return;
    }
    config.order = merged_order_with_current(&visible_order, config.normalized_order());
}

fn sync_runtime_column_weights_from_table(
    show_preview: bool,
    layout: &ListColumnLayout,
    config: &mut FileListColumnsConfig,
) {
    let table = unsafe { sys::igGetCurrentTable() };
    if table.is_null() {
        return;
    }

    let resized_column = unsafe { (*table).ResizedColumn };
    if resized_column < 0 {
        return;
    }

    if show_preview {
        if let Some(weight) = table_column_stretch_weight(table, 0) {
            config.weight_overrides.preview = Some(weight);
        }
    }
    if let Some(weight) = table_column_stretch_weight(table, layout.name) {
        config.weight_overrides.name = Some(weight);
    }
    if let Some(index) = layout.extension {
        if let Some(weight) = table_column_stretch_weight(table, index) {
            config.weight_overrides.extension = Some(weight);
        }
    }
    if let Some(index) = layout.size {
        if let Some(weight) = table_column_stretch_weight(table, index) {
            config.weight_overrides.size = Some(weight);
        }
    }
    if let Some(index) = layout.modified {
        if let Some(weight) = table_column_stretch_weight(table, index) {
            config.weight_overrides.modified = Some(weight);
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
    let layout = list_column_layout(show_preview, columns_config);
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

fn format_size(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let s = size as f64;
    if s >= GB {
        format!("{:.2} GB", s / GB)
    } else if s >= MB {
        format!("{:.2} MB", s / MB)
    } else if s >= KB {
        format!("{:.0} KB", s / KB)
    } else {
        format!("{} B", size)
    }
}

fn format_modified_ago(modified: Option<std::time::SystemTime>) -> String {
    use std::time::SystemTime;
    let m = match modified {
        Some(t) => t,
        None => return String::new(),
    };
    let now = SystemTime::now();
    let delta = match now.duration_since(m) {
        Ok(d) => d,
        Err(e) => e.duration(),
    };
    // For older than a week, show short absolute date inline; full datetime remains in tooltip
    const DAY: u64 = 24 * 60 * 60;
    const WEEK: u64 = 7 * DAY;
    if delta.as_secs() >= WEEK {
        use chrono::{DateTime, Local};
        let dt: DateTime<Local> = DateTime::<Local>::from(m);
        return dt.format("%Y-%m-%d").to_string();
    }
    humanize_duration(delta)
}

fn humanize_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    if secs < 10 {
        return "just now".into();
    }
    if secs < MIN {
        return format!("{}s ago", secs);
    }
    if secs < HOUR {
        return format!("{}m ago", secs / MIN);
    }
    if secs < DAY {
        return format!("{}h ago", secs / HOUR);
    }
    if secs < WEEK {
        return format!("{}d ago", secs / DAY);
    }
    let days = secs / DAY;
    format!("{}d ago", days)
}
