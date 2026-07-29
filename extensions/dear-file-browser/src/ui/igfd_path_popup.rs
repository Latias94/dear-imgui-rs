use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;
use dear_imgui_rs::sys;

use crate::core::SortMode;
use crate::dialog_state::FileDialogState;
use crate::file_style::EntryKind;

pub(super) fn draw_igfd_path_popup(
    ui: &Ui,
    state: &mut FileDialogState,
    ref_size: [f32; 2],
) -> Option<PathBuf> {
    let mut out: Option<PathBuf> = None;

    let mut ref_size = ref_size;
    if ref_size[0] <= 0.0 || ref_size[1] <= 0.0 {
        ref_size = ui.window_size();
    }
    let min_w = ui.frame_height() * 18.0;
    let min_h = ui.frame_height() * 12.0;
    let w = (ref_size[0] * 0.5).clamp(min_w, ref_size[0].max(min_w));
    let h = (ref_size[1] * 0.5).clamp(min_h, ref_size[1].max(min_h));
    unsafe {
        sys::igSetNextWindowSize(
            sys::ImVec2 { x: w, y: h },
            dear_imgui_rs::Condition::Appearing as i32,
        );
    }

    if let Some(_popup) = ui.begin_popup("##igfd_path_popup") {
        let Some(parent) = state.ui.runtime.breadcrumb.quick_parent.clone() else {
            ui.text_disabled("No path");
            return None;
        };
        draw_igfd_path_table_popup(ui, state, parent.as_path(), &mut out);
    }
    out
}

fn draw_igfd_path_table_popup(
    ui: &Ui,
    state: &mut FileDialogState,
    parent: &Path,
    out: &mut Option<PathBuf>,
) {
    // IGFD uses the global search tag for path popup filtering.
    let needle = state.core.search.trim().to_lowercase();
    let mut dirs: Vec<(String, PathBuf)> = state
        .core
        .recent_paths()
        .filter_map(|path| immediate_child(parent, path))
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().to_string();
            if needle.is_empty() {
                return Some((name, path));
            }
            name.to_lowercase()
                .contains(&needle)
                .then_some((name, path))
        })
        .collect();
    dirs.sort_by(
        |a: &(String, PathBuf), b: &(String, PathBuf)| match state.core.sort_mode {
            SortMode::Natural => {
                crate::dialog_core::natural_cmp_lower(&a.0.to_lowercase(), &b.0.to_lowercase())
            }
            SortMode::Lexicographic => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        },
    );
    dirs.dedup_by(|a, b| a.1 == b.1);

    if dirs.is_empty() {
        ui.text_disabled("No known recent path");
        return;
    }

    let flags = dear_imgui_rs::TableFlags::HIDEABLE
        | dear_imgui_rs::TableFlags::ROW_BG
        | dear_imgui_rs::TableFlags::SCROLL_Y
        | dear_imgui_rs::TableFlags::NO_HOST_EXTEND_Y;
    let options = dear_imgui_rs::TableOptions::from(flags)
        .sizing_policy(dear_imgui_rs::TableSizingPolicy::FixedFit);
    let table_size = ui.content_region_avail();
    if let Some(_t) =
        ui.begin_table_with_sizing("##FileDialog_pathTable", 1, options, table_size, 0.0)
    {
        ui.table_setup_scroll_freeze(0, 1);
        ui.table_setup_column_stretch_weight(
            "File name",
            dear_imgui_rs::TableColumnFlags::NONE,
            1.0,
        );
        ui.table_headers_row();

        let clipper = dear_imgui_rs::ListClipper::new(dirs.len())
            .items_height(ui.text_line_height_with_spacing())
            .begin(ui);
        for idx in clipper.iter() {
            let (name, path) = &dirs[idx];
            let style = state
                .ui
                .config
                .file_styles
                .style_for_owned(name, EntryKind::Dir);
            let mut label = name.to_string();
            if let Some(icon) = style.as_ref().and_then(|s| s.icon.as_deref()) {
                label = format!("{icon} {label}");
            }
            ui.table_next_row();
            ui.table_next_column();
            if ui
                .selectable_config(label.as_str())
                .flags(dear_imgui_rs::SelectableFlags::SPAN_ALL_COLUMNS)
                .build()
            {
                *out = Some(path.clone());
                ui.close_current_popup();
                break;
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(path.display().to_string());
            }
        }
    }
}

fn immediate_child(parent: &Path, descendant: &Path) -> Option<PathBuf> {
    let relative = descendant.strip_prefix(parent).ok()?;
    let component = relative.components().next()?;
    let mut child = parent.to_path_buf();
    child.push(component.as_os_str());
    (child != parent).then_some(child)
}
