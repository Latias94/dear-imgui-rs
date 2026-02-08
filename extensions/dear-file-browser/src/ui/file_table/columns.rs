use crate::core::FileFilter;
use crate::dialog_state::{
    FileDialogState, FileListColumnsConfig, FileListDataColumn, HeaderStyle,
};
use dear_imgui_rs::sys;

fn data_column_label(column: FileListDataColumn) -> &'static str {
    match column {
        FileListDataColumn::Name => "Name",
        FileListDataColumn::Extension => "Ext",
        FileListDataColumn::Size => "Size",
        FileListDataColumn::Modified => "Modified",
    }
}

pub(in crate::ui) fn extension_ui_label(state: &FileDialogState) -> &'static str {
    if matches!(state.ui.header_style, HeaderStyle::IgfdClassic) {
        "Type"
    } else {
        "Ext"
    }
}

pub(super) fn igfd_type_dots_to_extract(active_filter: Option<&FileFilter>) -> usize {
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

pub(super) fn type_extension_by_dot_count<'a>(name: &'a str, dots_to_extract: usize) -> &'a str {
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

pub(in crate::ui) fn data_column_label_for_state(
    state: &FileDialogState,
    column: FileListDataColumn,
) -> &'static str {
    match column {
        FileListDataColumn::Extension => extension_ui_label(state),
        _ => data_column_label(column),
    }
}

pub(in crate::ui) fn is_data_column_visible(
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

pub(in crate::ui) fn apply_compact_column_layout(config: &mut FileListColumnsConfig) {
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

pub(in crate::ui) fn apply_compact_column_layout_keep_preview(config: &mut FileListColumnsConfig) {
    let preview = config.show_preview;
    apply_compact_column_layout(config);
    config.show_preview = preview;
}

pub(in crate::ui) fn apply_balanced_column_layout(config: &mut FileListColumnsConfig) {
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

pub(in crate::ui) fn apply_balanced_column_layout_keep_preview(config: &mut FileListColumnsConfig) {
    let preview = config.show_preview;
    apply_balanced_column_layout(config);
    config.show_preview = preview;
}

pub(in crate::ui) fn move_column_order_up(
    order: &mut [FileListDataColumn; 4],
    index: usize,
) -> bool {
    if index == 0 || index >= order.len() {
        return false;
    }
    order.swap(index, index - 1);
    true
}

pub(in crate::ui) fn move_column_order_down(
    order: &mut [FileListDataColumn; 4],
    index: usize,
) -> bool {
    if index + 1 >= order.len() {
        return false;
    }
    order.swap(index, index + 1);
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ui) struct ListColumnLayout {
    pub(in crate::ui) data_columns: Vec<FileListDataColumn>,
    pub(in crate::ui) name: i16,
    pub(in crate::ui) extension: Option<i16>,
    pub(in crate::ui) size: Option<i16>,
    pub(in crate::ui) modified: Option<i16>,
}

pub(in crate::ui) fn list_column_layout(
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

pub(super) fn resolved_preview_column_weight(config: &FileListColumnsConfig) -> f32 {
    validated_column_weight(
        config.weight_overrides.preview,
        default_preview_column_weight(),
    )
}

pub(super) fn resolved_data_column_weight(
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

pub(in crate::ui) fn merged_order_with_current(
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

pub(super) fn sync_runtime_column_order_from_table(
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

pub(super) fn sync_runtime_column_weights_from_table(
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
    if let Some(idx) = layout.extension {
        if let Some(weight) = table_column_stretch_weight(table, idx) {
            config.weight_overrides.extension = Some(weight);
        }
    }
    if let Some(idx) = layout.size {
        if let Some(weight) = table_column_stretch_weight(table, idx) {
            config.weight_overrides.size = Some(weight);
        }
    }
    if let Some(idx) = layout.modified {
        if let Some(weight) = table_column_stretch_weight(table, idx) {
            config.weight_overrides.modified = Some(weight);
        }
    }
}
