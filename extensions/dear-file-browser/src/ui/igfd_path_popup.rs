use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;
use dear_imgui_rs::sys;

use crate::core::SortMode;
use crate::dialog_state::FileDialogState;
use crate::file_style::EntryKind;
use crate::fs::FileSystem;

pub(super) fn draw_igfd_path_popup(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
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
        let Some(parent) = state.ui.breadcrumb_quick_parent.clone() else {
            ui.text_disabled("No path");
            return None;
        };
        draw_igfd_path_table_popup(ui, state, fs, parent.as_path(), &mut out);
    }
    out
}

fn draw_igfd_path_table_popup(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
    parent: &Path,
    out: &mut Option<PathBuf>,
) {
    let Ok(rd) = fs.read_dir(parent) else {
        ui.text_disabled("Failed to read directory");
        return;
    };

    // IGFD uses the global search tag for path popup filtering.
    let needle = state.core.search.trim().to_lowercase();
    let mut dirs: Vec<_> = rd
        .into_iter()
        .filter(|e| {
            if !e.is_dir {
                return false;
            }
            if needle.is_empty() {
                return true;
            }
            e.name.to_lowercase().contains(&needle)
        })
        .collect();
    dirs.sort_by(|a, b| match state.core.sort_mode {
        SortMode::Natural => {
            crate::dialog_core::natural_cmp_lower(&a.name.to_lowercase(), &b.name.to_lowercase())
        }
        SortMode::Lexicographic => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    let flags = dear_imgui_rs::TableFlags::SIZING_FIXED_FIT
        | dear_imgui_rs::TableFlags::HIDEABLE
        | dear_imgui_rs::TableFlags::ROW_BG
        | dear_imgui_rs::TableFlags::SCROLL_Y
        | dear_imgui_rs::TableFlags::NO_HOST_EXTEND_Y;
    let table_size = ui.content_region_avail();
    if let Some(_t) =
        ui.begin_table_with_sizing("##FileDialog_pathTable", 1, flags, table_size, 0.0)
    {
        ui.table_setup_scroll_freeze(0, 1);
        ui.table_setup_column(
            "File name",
            dear_imgui_rs::TableColumnFlags::WIDTH_STRETCH,
            -1.0,
            0,
        );
        ui.table_headers_row();

        let items_count = i32::try_from(dirs.len()).unwrap_or(i32::MAX);
        let clipper = dear_imgui_rs::ListClipper::new(items_count)
            .items_height(ui.text_line_height_with_spacing())
            .begin(ui);
        for i in clipper.iter() {
            let idx = i as usize;
            if idx >= dirs.len() {
                continue;
            }
            let e = &dirs[idx];
            let style = state
                .ui
                .file_styles
                .style_for_owned(&e.name, EntryKind::Dir);
            let mut label = e.name.as_str().to_string();
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
                *out = Some(e.path.clone());
                ui.close_current_popup();
                break;
            }
            if ui.is_item_hovered() {
                ui.tooltip_text(e.path.display().to_string());
            }
        }
    }
}
