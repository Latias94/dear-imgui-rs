use dear_imgui_rs::input::Key;
use dear_imgui_rs::{Direction, StyleColor, StyleVar, Ui};

use crate::core::{LayoutStyle, SortBy};
use crate::dialog_core::CoreEvent;
use crate::dialog_state::{
    FileDialogState, FileListViewMode, HeaderStyle, PathBarStyle, ToolbarDensity, ToolbarIconMode,
};
use crate::fs::FileSystem;
use crate::places::Places;

use super::path_bar;

fn toolbar_label(id: &str, text: &str, icon: Option<&str>, mode: ToolbarIconMode) -> String {
    let display = match mode {
        ToolbarIconMode::Text => text.to_string(),
        ToolbarIconMode::IconOnly => icon.unwrap_or(text).to_string(),
        ToolbarIconMode::IconAndText => {
            icon.map_or_else(|| text.to_string(), |icon| format!("{icon} {text}"))
        }
    };
    format!("{display}###{id}")
}

fn toolbar_button(
    ui: &Ui,
    id: &str,
    text: &str,
    icon: Option<&str>,
    mode: ToolbarIconMode,
    show_tooltips: bool,
    tooltip: &str,
) -> bool {
    let clicked = ui.button(toolbar_label(id, text, icon, mode));
    if show_tooltips && !tooltip.is_empty() && ui.is_item_hovered() {
        ui.tooltip_text(tooltip);
    }
    clicked
}

fn toolbar_toggle_button(
    ui: &Ui,
    id: &str,
    text: &str,
    icon: Option<&str>,
    mode: ToolbarIconMode,
    show_tooltips: bool,
    tooltip: &str,
    active: bool,
) -> bool {
    if !active {
        return toolbar_button(ui, id, text, icon, mode, show_tooltips, tooltip);
    }

    let style = ui.clone_style();
    let _c0 = ui.push_style_color(StyleColor::Button, style.color(StyleColor::Header));
    let _c1 = ui.push_style_color(
        StyleColor::ButtonHovered,
        style.color(StyleColor::HeaderHovered),
    );
    let _c2 = ui.push_style_color(
        StyleColor::ButtonActive,
        style.color(StyleColor::HeaderActive),
    );
    toolbar_button(ui, id, text, icon, mode, show_tooltips, tooltip)
}

pub(super) fn draw_chrome(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
    has_thumbnail_backend: bool,
) {
    let show_tooltips = state.ui.toolbar.show_tooltips;
    let icon_mode = state.ui.toolbar.icons.mode;
    let chrome_style = ui.clone_style();
    let (scale, min) = match state.ui.toolbar.density {
        ToolbarDensity::Normal => (1.0, 0.0),
        ToolbarDensity::Compact => (0.82, 1.0),
        ToolbarDensity::Spacious => (1.18, 0.0),
    };
    let scale_vec2 =
        |v: [f32; 2]| -> [f32; 2] { [(v[0] * scale).max(min), (v[1] * scale).max(min)] };
    let _frame_padding = ui.push_style_var(StyleVar::FramePadding(scale_vec2(
        chrome_style.frame_padding(),
    )));
    let _item_spacing = ui.push_style_var(StyleVar::ItemSpacing(scale_vec2(
        chrome_style.item_spacing(),
    )));
    let _item_inner_spacing = ui.push_style_var(StyleVar::ItemInnerSpacing(scale_vec2(
        chrome_style.item_inner_spacing(),
    )));

    let header_style = state.ui.header_style;
    if matches!(header_style, HeaderStyle::ToolbarAndAddress) {
        // Top toolbar: Back/Forward/Up/Refresh, view, sort, etc.
        let can_back = state.core.can_navigate_back();
        let can_forward = state.core.can_navigate_forward();

        let places_active =
            matches!(state.ui.layout, LayoutStyle::Standard) && state.ui.places_pane_shown;
        if toolbar_toggle_button(
            ui,
            "toolbar_places",
            "Places",
            state.ui.toolbar.icons.places.as_deref(),
            icon_mode,
            show_tooltips,
            "Places",
            places_active,
        ) {
            match state.ui.layout {
                LayoutStyle::Standard => {
                    state.ui.places_pane_shown = !state.ui.places_pane_shown;
                }
                LayoutStyle::Minimal => {
                    ui.open_popup("##fb_places_popup");
                }
            }
        }
        ui.same_line();

        {
            let _disabled = ui.begin_disabled_with_cond(!can_back);
            if ui.arrow_button("##nav_back", Direction::Left) {
                let _ = state.core.handle_event(CoreEvent::NavigateBack);
            }
        }
        if show_tooltips && ui.is_item_hovered() {
            ui.tooltip_text("Back (Alt+Left)");
        }
        ui.same_line();
        {
            let _disabled = ui.begin_disabled_with_cond(!can_forward);
            if ui.arrow_button("##nav_forward", Direction::Right) {
                let _ = state.core.handle_event(CoreEvent::NavigateForward);
            }
        }
        if show_tooltips && ui.is_item_hovered() {
            ui.tooltip_text("Forward (Alt+Right)");
        }
        ui.same_line();
        if ui.arrow_button("##nav_up", Direction::Up) {
            let _ = state.core.handle_event(CoreEvent::NavigateUp);
        }
        if show_tooltips && ui.is_item_hovered() {
            ui.tooltip_text("Up (Backspace)");
        }
        ui.same_line();
        if toolbar_button(
            ui,
            "toolbar_refresh",
            "Refresh",
            state.ui.toolbar.icons.refresh.as_deref(),
            icon_mode,
            show_tooltips,
            "Refresh (F5)",
        ) {
            let _ = state.core.handle_event(CoreEvent::Refresh);
        }
        ui.same_line();
        if state.ui.new_folder_enabled {
            if toolbar_button(
                ui,
                "toolbar_new_folder",
                "New Folder",
                state.ui.toolbar.icons.new_folder.as_deref(),
                icon_mode,
                show_tooltips,
                "New folder",
            ) {
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
            }
            ui.same_line();

            if matches!(state.ui.layout, LayoutStyle::Standard) && state.ui.new_folder_inline_active
            {
                ui.set_next_item_width(160.0);
                if state.ui.new_folder_focus_next {
                    ui.set_keyboard_focus_here();
                    state.ui.new_folder_focus_next = false;
                }
                let submitted = ui
                    .input_text("##new_folder_inline", &mut state.ui.new_folder_name)
                    .hint("New folder...")
                    .enter_returns_true(true)
                    .build();
                let input_active = ui.is_item_active();

                ui.same_line();
                let ok = ui.small_button("OK##new_folder_inline");
                ui.same_line();
                let cancel = ui.small_button("Cancel##new_folder_inline")
                    || (input_active && ui.is_key_pressed(Key::Escape));

                if cancel {
                    state.ui.new_folder_inline_active = false;
                    state.ui.new_folder_error = None;
                    state.ui.new_folder_name.clear();
                }

                if ok || submitted {
                    if super::popups::try_create_new_folder_in_cwd(state, fs) {
                        state.ui.new_folder_inline_active = false;
                    }
                }

                if let Some(err) = &state.ui.new_folder_error {
                    ui.same_line();
                    ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
                }

                ui.same_line();
            }
        }
        ui.separator_vertical();
        ui.same_line();
        if matches!(state.ui.toolbar.density, ToolbarDensity::Compact) {
            let list_active = matches!(state.ui.file_list_view, FileListViewMode::List);
            let thumbs_active = matches!(state.ui.file_list_view, FileListViewMode::ThumbnailsList);
            let grid_active = matches!(state.ui.file_list_view, FileListViewMode::Grid);

            if toolbar_toggle_button(
                ui,
                "view_list",
                "List",
                None,
                ToolbarIconMode::Text,
                show_tooltips,
                "List view",
                list_active,
            ) {
                state.ui.file_list_view = FileListViewMode::List;
            }
            ui.same_line();
            if toolbar_toggle_button(
                ui,
                "view_thumbs",
                "Thumbs",
                None,
                ToolbarIconMode::Text,
                show_tooltips,
                "Thumbnails list view",
                thumbs_active,
            ) {
                state.ui.file_list_view = FileListViewMode::ThumbnailsList;
                state.ui.thumbnails_enabled = true;
                state.ui.file_list_columns.show_preview = true;
            }
            ui.same_line();
            if toolbar_toggle_button(
                ui,
                "view_grid",
                "Grid",
                None,
                ToolbarIconMode::Text,
                show_tooltips,
                "Thumbnails grid view",
                grid_active,
            ) {
                state.ui.file_list_view = FileListViewMode::Grid;
                state.ui.thumbnails_enabled = true;
            }
        } else {
            ui.text("View:");
            ui.same_line();
            let view_preview = match state.ui.file_list_view {
                FileListViewMode::List => "List",
                FileListViewMode::ThumbnailsList => "Thumbs",
                FileListViewMode::Grid => "Grid",
            };
            if let Some(_c) = ui.begin_combo("##view_mode", view_preview) {
                if ui
                    .selectable_config("List")
                    .selected(matches!(state.ui.file_list_view, FileListViewMode::List))
                    .build()
                {
                    state.ui.file_list_view = FileListViewMode::List;
                }
                if ui
                    .selectable_config("Thumbs")
                    .selected(matches!(
                        state.ui.file_list_view,
                        FileListViewMode::ThumbnailsList
                    ))
                    .build()
                {
                    state.ui.file_list_view = FileListViewMode::ThumbnailsList;
                    state.ui.thumbnails_enabled = true;
                    state.ui.file_list_columns.show_preview = true;
                }
                if ui
                    .selectable_config("Grid")
                    .selected(matches!(state.ui.file_list_view, FileListViewMode::Grid))
                    .build()
                {
                    state.ui.file_list_view = FileListViewMode::Grid;
                    state.ui.thumbnails_enabled = true;
                }
            }
        }

        if matches!(
            state.ui.file_list_view,
            FileListViewMode::ThumbnailsList | FileListViewMode::Grid
        ) {
            state.ui.thumbnails_enabled = true;
        }

        if matches!(state.ui.file_list_view, FileListViewMode::Grid) {
            ui.same_line();
            ui.text("Sort:");
            ui.same_line();
            let type_label = "Type";
            let ext_label = "Ext";
            let preview = format!(
                "{} {}",
                match state.core.sort_by {
                    SortBy::Name => "Name",
                    SortBy::Type => type_label,
                    SortBy::Extension => ext_label,
                    SortBy::Size => "Size",
                    SortBy::Modified => "Modified",
                },
                if state.core.sort_ascending {
                    "↑"
                } else {
                    "↓"
                }
            );
            let mut next_by = state.core.sort_by;
            let mut next_asc = state.core.sort_ascending;
            if let Some(_c) = ui.begin_combo("##grid_sort", &preview) {
                let items = [
                    (SortBy::Name, "Name"),
                    (SortBy::Type, type_label),
                    (SortBy::Extension, ext_label),
                    (SortBy::Size, "Size"),
                    (SortBy::Modified, "Modified"),
                ];
                for (by, label) in items {
                    if ui.selectable_config(label).selected(next_by == by).build() {
                        next_by = by;
                    }
                }
                ui.separator();
                if ui.selectable_config("Ascending").selected(next_asc).build() {
                    next_asc = true;
                }
                if ui
                    .selectable_config("Descending")
                    .selected(!next_asc)
                    .build()
                {
                    next_asc = false;
                }
            }
            if next_by != state.core.sort_by || next_asc != state.core.sort_ascending {
                state.core.sort_by = next_by;
                state.core.sort_ascending = next_asc;
            }
        }

        if matches!(
            state.ui.file_list_view,
            FileListViewMode::List | FileListViewMode::ThumbnailsList
        ) {
            ui.same_line();
            if toolbar_button(
                ui,
                "toolbar_columns",
                "Columns",
                state.ui.toolbar.icons.columns.as_deref(),
                icon_mode,
                show_tooltips,
                "Columns",
            ) {
                ui.open_popup("##fb_columns_popup");
            }
        }
        ui.same_line();
        if toolbar_button(
            ui,
            "toolbar_options",
            "Options",
            state.ui.toolbar.icons.options.as_deref(),
            icon_mode,
            show_tooltips,
            "Options",
        ) {
            ui.open_popup("##fb_options");
        }
        ui.separator_vertical();
        ui.same_line();
    }

    if matches!(header_style, HeaderStyle::IgfdClassic) {
        let places_active =
            matches!(state.ui.layout, LayoutStyle::Standard) && state.ui.places_pane_shown;
        if toolbar_toggle_button(
            ui,
            "toolbar_places",
            "Places",
            state.ui.toolbar.icons.places.as_deref(),
            icon_mode,
            show_tooltips,
            "Places",
            places_active,
        ) {
            match state.ui.layout {
                LayoutStyle::Standard => {
                    state.ui.places_pane_shown = !state.ui.places_pane_shown;
                }
                LayoutStyle::Minimal => {
                    ui.open_popup("##fb_places_popup");
                }
            }
        }
        ui.same_line();

        if state.ui.new_folder_enabled {
            if toolbar_button(
                ui,
                "toolbar_new_folder",
                "New Folder",
                state.ui.toolbar.icons.new_folder.as_deref(),
                icon_mode,
                show_tooltips,
                "New folder",
            ) {
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
            }
            ui.same_line();

            if matches!(state.ui.layout, LayoutStyle::Standard) && state.ui.new_folder_inline_active
            {
                ui.set_next_item_width(160.0);
                if state.ui.new_folder_focus_next {
                    ui.set_keyboard_focus_here();
                    state.ui.new_folder_focus_next = false;
                }
                let submitted = ui
                    .input_text("##new_folder_inline", &mut state.ui.new_folder_name)
                    .hint("New folder...")
                    .enter_returns_true(true)
                    .build();
                let input_active = ui.is_item_active();

                ui.same_line();
                let ok = ui.small_button("OK##new_folder_inline");
                ui.same_line();
                let cancel = ui.small_button("Cancel##new_folder_inline")
                    || (input_active && ui.is_key_pressed(Key::Escape));

                if cancel {
                    state.ui.new_folder_inline_active = false;
                    state.ui.new_folder_error = None;
                    state.ui.new_folder_name.clear();
                }

                if ok || submitted {
                    if super::popups::try_create_new_folder_in_cwd(state, fs) {
                        state.ui.new_folder_inline_active = false;
                    }
                }

                if let Some(err) = &state.ui.new_folder_error {
                    ui.same_line();
                    ui.text_colored([1.0, 0.3, 0.3, 1.0], err);
                }

                ui.same_line();
            }
        }

        ui.separator_vertical();
        ui.same_line();
    }

    // Path bar (address input / breadcrumb composer) + Search.
    let cwd_s = state.core.cwd.display().to_string();
    if state.ui.path_edit_last_cwd != cwd_s && !state.ui.path_input_mode {
        state.ui.path_edit_last_cwd = cwd_s.clone();
        state.ui.path_edit_buffer = cwd_s.clone();
        if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
            state.ui.breadcrumbs_scroll_to_end_next = true;
        }
    } else if state.ui.path_edit_last_cwd.is_empty() {
        state.ui.path_edit_last_cwd = cwd_s.clone();
        if state.ui.path_edit_buffer.trim().is_empty() {
            state.ui.path_edit_buffer = cwd_s.clone();
        }
        if state.ui.path_bar_style == PathBarStyle::Breadcrumbs {
            state.ui.breadcrumbs_scroll_to_end_next = true;
        }
    }

    let breadcrumbs_mode = state.ui.path_bar_style == PathBarStyle::Breadcrumbs;
    let is_igfd_classic = matches!(header_style, HeaderStyle::IgfdClassic);
    let show_breadcrumb_composer = breadcrumbs_mode && !state.ui.path_input_mode;

    // IGFD uses compact labels for classic chrome. Keep them scoped to IgfdClassic to avoid
    // surprising other header styles.
    let reset_label = if breadcrumbs_mode && is_igfd_classic {
        "R"
    } else {
        "Reset"
    };
    let action_label = if breadcrumbs_mode {
        if is_igfd_classic { "E" } else { "Edit" }
    } else {
        "Go"
    };
    let (view_list_label, view_thumbs_label, view_grid_label) = if is_igfd_classic {
        ("FL", "TL", "TG")
    } else {
        ("List", "Thumbs", "Grid")
    };
    let style = ui.clone_style();
    let font = ui.current_font();
    let font_size = ui.current_font_size();
    let spacing_x = style.item_spacing()[0];
    let frame_pad_x = style.frame_padding()[0];
    let history_button_w = ui.frame_height();

    let min_path_w = if matches!(header_style, HeaderStyle::IgfdClassic) {
        // Keep a minimal but usable composer width in "classic" mode.
        (ui.frame_height() * 4.0).max(60.0)
    } else {
        120.0
    };
    let min_search_input_w = if matches!(header_style, HeaderStyle::IgfdClassic) {
        // Allow the search bar to shrink to keep the header on a single row.
        (ui.frame_height() * 6.0).max(90.0)
    } else {
        220.0
    };

    let action_label_w = font.calc_text_size(font_size, f32::MAX, 0.0, action_label)[0];
    let action_w = action_label_w + frame_pad_x * 2.0;

    let has_devices_button = breadcrumbs_mode
        && state
            .core
            .places
            .groups
            .iter()
            .find(|g| g.label == Places::SYSTEM_GROUP)
            .is_some_and(|g| g.places.iter().any(|p| !p.is_separator()));
    let reset_label_w = if breadcrumbs_mode {
        font.calc_text_size(font_size, f32::MAX, 0.0, reset_label)[0]
    } else {
        0.0
    };
    let reset_w = reset_label_w + frame_pad_x * 2.0;
    let devices_label_w = if has_devices_button {
        font.calc_text_size(font_size, f32::MAX, 0.0, "Devices")[0]
    } else {
        0.0
    };
    let devices_w = devices_label_w + frame_pad_x * 2.0;
    let sep_w = if breadcrumbs_mode { 1.0 } else { 0.0 };
    let path_controls_w = if breadcrumbs_mode {
        let mut w = reset_w + spacing_x;
        if has_devices_button {
            w += devices_w + spacing_x;
        }
        w += action_w + spacing_x + sep_w + spacing_x;
        w
    } else {
        action_w
    };

    let search_label_w = font.calc_text_size(font_size, f32::MAX, 0.0, "Search:")[0];
    let search_reset_w = ui.frame_height();
    let search_total_w =
        search_reset_w + spacing_x + search_label_w + spacing_x + min_search_input_w;
    let view_controls_w = if matches!(header_style, HeaderStyle::IgfdClassic) {
        let list_w =
            font.calc_text_size(font_size, f32::MAX, 0.0, view_list_label)[0] + frame_pad_x * 2.0;
        let thumbs_w =
            font.calc_text_size(font_size, f32::MAX, 0.0, view_thumbs_label)[0] + frame_pad_x * 2.0;
        let grid_w =
            font.calc_text_size(font_size, f32::MAX, 0.0, view_grid_label)[0] + frame_pad_x * 2.0;
        let buttons_w = list_w + spacing_x + thumbs_w + spacing_x + grid_w;
        let sep_w = 1.0;
        // Buttons + spacing + vertical separator + spacing.
        buttons_w + spacing_x + sep_w + spacing_x
    } else {
        0.0
    };
    let right_block_w = view_controls_w + search_total_w;

    let row_start_x = ui.cursor_pos_x();
    let row_w = ui.content_region_avail_width();
    let row_right_x = row_start_x + row_w;
    let min_total_w = history_button_w
        + spacing_x
        + min_path_w
        + spacing_x
        + path_controls_w
        + spacing_x
        + right_block_w;

    // In IGFD-classic mode we *prefer* a single-row header, but on very small widths we must
    // fall back to a stacked layout. Otherwise, `same_line_with_pos(right_block_start_x)`
    // can move the cursor backwards and cause items to overlap.
    let stacked = row_w < min_total_w;
    let right_block_start_x = row_right_x - right_block_w;

    // Path input (+ Go). If we can't fit Search on the same line, Search moves to the next line.
    let path_w = if stacked {
        (row_w - history_button_w - spacing_x - path_controls_w - spacing_x).max(40.0)
    } else {
        (right_block_start_x
            - row_start_x
            - history_button_w
            - spacing_x
            - path_controls_w
            - spacing_x * 2.0)
            .max(min_path_w)
    };

    let recent_paths = state.core.recent_paths().cloned().collect::<Vec<_>>();
    {
        let _disabled = ui.begin_disabled_with_cond(recent_paths.is_empty());
        if ui.arrow_button("##path_history_dropdown", Direction::Down) {
            ui.open_popup("##path_history_dropdown_popup");
        }
    }
    if ui.is_item_hovered() {
        ui.tooltip_text("Path history");
    }
    if let Some(_popup) = ui.begin_popup("##path_history_dropdown_popup") {
        ui.text_disabled("Recent:");
        ui.separator();
        for (i, p) in recent_paths.iter().enumerate() {
            let _id = ui.push_id(i as i32);
            let label = p.display().to_string();
            if ui.selectable(&label) {
                let _ = state.core.handle_event(CoreEvent::NavigateTo(p.clone()));
                state.ui.path_edit = false;
                state.ui.path_edit_last_cwd = state.core.cwd.display().to_string();
                state.ui.path_edit_buffer = state.ui.path_edit_last_cwd.clone();
                state.ui.path_history_index = None;
                state.ui.path_history_saved_buffer = None;
                state.ui.ui_error = None;
                ui.close_current_popup();
            }
        }
    }
    ui.same_line();

    if breadcrumbs_mode {
        let can_reset = state
            .ui
            .opened_cwd
            .as_ref()
            .is_some_and(|p| *p != state.core.cwd);
        {
            let _disabled = ui.begin_disabled_with_cond(!can_reset);
            if ui.button(reset_label) {
                if let Some(p) = state.ui.opened_cwd.clone() {
                    let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
                }
            }
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Reset to current directory");
        }
        ui.same_line();

        if has_devices_button {
            let devices = state
                .core
                .places
                .groups
                .iter()
                .find(|g| g.label == Places::SYSTEM_GROUP)
                .map(|g| {
                    g.places
                        .iter()
                        .filter(|p| !p.is_separator())
                        .map(|p| (p.label.clone(), p.path.clone()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            if ui.button("Devices") {
                ui.open_popup("##path_devices_popup");
            }
            if ui.is_item_hovered() {
                ui.tooltip_text("System devices and drives");
            }
            if let Some(_popup) = ui.begin_popup("##path_devices_popup") {
                ui.text_disabled("Devices:");
                ui.separator();
                for (i, (label, path)) in devices.iter().enumerate() {
                    let _id = ui.push_id(i as i32);
                    if ui.selectable(label) {
                        let _ = state.core.handle_event(CoreEvent::NavigateTo(path.clone()));
                        ui.close_current_popup();
                        break;
                    }
                }
            }
            ui.same_line();
        }

        if ui.button(action_label) {
            state.ui.path_input_mode = !state.ui.path_input_mode;
            if state.ui.path_input_mode {
                state.ui.path_edit_buffer = state.core.cwd.display().to_string();
                state.ui.focus_path_edit_next = true;
            } else {
                state.ui.path_edit = false;
                state.ui.focus_path_edit_next = false;
            }
        }
        if ui.is_item_hovered() {
            ui.tooltip_text("Edit path (Ctrl+L)\nYou can also right click on path buttons");
        }
        ui.same_line();
        ui.separator_vertical();
        ui.same_line();

        if show_breadcrumb_composer {
            // IGFD draws the path composer inline (no child window). We reserve a framed
            // region and draw breadcrumbs clipped to its content area.
            let frame_h = ui.frame_height();
            ui.dummy([path_w, frame_h]);
            let rect_min = ui.item_rect_min();
            let rect_max = ui.item_rect_max();
            let after = ui.cursor_pos();

            let dl = ui.get_window_draw_list();
            let rounding = style.frame_rounding();
            dl.add_rect(rect_min, rect_max, ui.get_color_u32(StyleColor::FrameBg))
                .filled(true)
                .rounding(rounding)
                .build();
            dl.add_rect(rect_min, rect_max, ui.get_color_u32(StyleColor::Border))
                .rounding(rounding)
                .build();

            // Match InputText's horizontal padding, but keep full vertical space so
            // breadcrumb buttons (which are frame-height) don't get clipped away.
            let pad = style.frame_padding();
            let content_min = [rect_min[0] + pad[0], rect_min[1]];
            let mut content_max = [rect_max[0] - pad[0], rect_max[1]];
            if content_max[0] < content_min[0] {
                content_max[0] = content_min[0];
            }
            if content_max[1] < content_min[1] {
                content_max[1] = content_min[1];
            }

            let crumbs_total_w = path_bar::estimate_breadcrumbs_total_width(
                ui,
                &state.core.cwd,
                state.ui.breadcrumbs_max_segments,
                state.ui.breadcrumbs_quick_select,
            );
            let visible_w = (content_max[0] - content_min[0]).max(0.0);
            let start_x = if crumbs_total_w > visible_w {
                // Keep the tail visible for long paths.
                content_max[0] - crumbs_total_w
            } else {
                content_min[0]
            };

            ui.with_clip_rect(content_min, content_max, true, || {
                ui.set_cursor_screen_pos([start_x, rect_min[1]]);
                if let Some(p) = path_bar::draw_breadcrumbs(
                    ui,
                    state,
                    fs,
                    state.ui.breadcrumbs_max_segments,
                    false,
                    false,
                ) {
                    let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
                }
            });

            // Restore layout cursor to after the reserved region.
            ui.set_cursor_pos(after);
        } else {
            path_bar::draw_path_input_text(ui, state, fs, &recent_paths, path_w, false);
        }
    } else {
        path_bar::draw_path_input_text(ui, state, fs, &recent_paths, path_w, true);
    }

    if stacked {
        ui.new_line();
    } else {
        // Guard against any cursor-backtracking overlap if our width estimates are off.
        if right_block_start_x < ui.cursor_pos_x() + spacing_x {
            ui.new_line();
        } else {
            ui.same_line_with_pos(right_block_start_x);
        }
    }

    if matches!(header_style, HeaderStyle::IgfdClassic) {
        let list_active = matches!(state.ui.file_list_view, FileListViewMode::List);
        let thumbs_active = matches!(state.ui.file_list_view, FileListViewMode::ThumbnailsList);
        let grid_active = matches!(state.ui.file_list_view, FileListViewMode::Grid);

        if toolbar_toggle_button(
            ui,
            "view_list",
            view_list_label,
            None,
            ToolbarIconMode::Text,
            show_tooltips,
            "File List",
            list_active,
        ) {
            state.ui.file_list_view = FileListViewMode::List;
        }
        ui.same_line();
        {
            let _disabled = ui.begin_disabled_with_cond(!has_thumbnail_backend);
            if toolbar_toggle_button(
                ui,
                "view_thumbs",
                view_thumbs_label,
                None,
                ToolbarIconMode::Text,
                show_tooltips,
                "Thumbnails List",
                thumbs_active,
            ) {
                state.ui.file_list_view = FileListViewMode::ThumbnailsList;
                state.ui.thumbnails_enabled = true;
                state.ui.file_list_columns.show_preview = true;
            }
        }
        ui.same_line();
        {
            let _disabled = ui.begin_disabled_with_cond(!has_thumbnail_backend);
            if toolbar_toggle_button(
                ui,
                "view_grid",
                view_grid_label,
                None,
                ToolbarIconMode::Text,
                show_tooltips,
                "Thumbnails Grid",
                grid_active,
            ) {
                state.ui.file_list_view = FileListViewMode::Grid;
                state.ui.thumbnails_enabled = true;
            }
        }
        if has_thumbnail_backend
            && state.ui.thumbnails_enabled
            && matches!(
                state.ui.file_list_view,
                FileListViewMode::ThumbnailsList | FileListViewMode::Grid
            )
        {
            let stats = state.ui.thumbnails.stats();
            if stats.total > 0 && stats.ready < stats.total {
                ui.same_line();
                let frac = stats.ready as f32 / stats.total as f32;
                let w = (ui.frame_height() * 4.0).max(80.0);
                let h = (ui.frame_height() * 0.55).max(6.0);
                ui.progress_bar_with_overlay(frac, format!("{}/{}", stats.ready, stats.total))
                    .size([w, h])
                    .build();
            }
        }
        ui.same_line();
        ui.separator_vertical();
        ui.same_line();
    }

    if ui.button("X##search_reset") {
        state.core.search.clear();
    }
    if ui.is_item_hovered() {
        ui.tooltip_text("Reset search");
    }
    ui.same_line();
    ui.text("Search:");
    ui.same_line();
    if state.ui.focus_search_next {
        ui.set_keyboard_focus_here();
        state.ui.focus_search_next = false;
    }
    ui.set_next_item_width(ui.content_region_avail_width().max(80.0));
    let _search_changed = ui.input_text("##search", &mut state.core.search).build();

    if !breadcrumbs_mode {
        if let Some(p) = path_bar::draw_breadcrumbs(
            ui,
            state,
            fs,
            state.ui.breadcrumbs_max_segments,
            true,
            false,
        ) {
            let _ = state.core.handle_event(CoreEvent::NavigateTo(p));
        }
    }

    ui.separator();
}
