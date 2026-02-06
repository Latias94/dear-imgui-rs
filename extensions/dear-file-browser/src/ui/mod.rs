use std::path::{Path, PathBuf};

use dear_imgui_rs::TreeNodeFlags;
use dear_imgui_rs::Ui;
use dear_imgui_rs::input::{Key, MouseButton};

use crate::core::{ClickAction, DialogMode, FileDialogError, LayoutStyle, Selection, SortBy};
use crate::custom_pane::{CustomPane, CustomPaneCtx};
use crate::dialog_core::{ConfirmGate, DirEntry, Modifiers};
use crate::dialog_state::FileDialogState;
use crate::fs::{FileSystem, StdFileSystem};
use crate::places::Places;

/// Configuration for hosting the file browser in an ImGui window.
#[derive(Clone, Debug)]
pub struct WindowHostConfig {
    /// Window title
    pub title: String,
    /// Initial window size (used with `size_condition`)
    pub initial_size: [f32; 2],
    /// Condition used when setting the window size
    pub size_condition: dear_imgui_rs::Condition,
}

impl WindowHostConfig {
    /// Default window host configuration for the given dialog mode.
    pub fn for_mode(mode: DialogMode) -> Self {
        let title = match mode {
            DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
            DialogMode::PickFolder => "Select Folder",
            DialogMode::SaveFile => "Save",
        };
        Self {
            title: title.to_string(),
            initial_size: [760.0, 520.0],
            size_condition: dear_imgui_rs::Condition::FirstUseEver,
        }
    }
}

/// UI handle for file browser
pub struct FileBrowser<'ui> {
    pub ui: &'ui Ui,
}

/// Extend Ui with a file browser entry point
pub trait FileDialogExt {
    /// Entry point for showing the file browser widget
    fn file_browser(&self) -> FileBrowser<'_>;
}

impl FileDialogExt for Ui {
    fn file_browser(&self) -> FileBrowser<'_> {
        FileBrowser { ui: self }
    }
}

impl<'ui> FileBrowser<'ui> {
    /// Draw only the contents of the file browser (no window/modal host).
    ///
    /// This is useful for embedding the browser into an existing window, popup,
    /// tab, or child region managed by the caller.
    ///
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn draw_contents(
        &self,
        state: &mut FileDialogState,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.draw_contents_with_fs(state, &StdFileSystem)
    }

    /// Draw only the contents of the file browser (no window/modal host) using a custom filesystem.
    pub fn draw_contents_with_fs(
        &self,
        state: &mut FileDialogState,
        fs: &dyn FileSystem,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs_and_custom_pane(self.ui, state, fs, None)
    }

    /// Draw only the contents of the file browser (no window/modal host) with a custom pane.
    ///
    /// The pane can draw additional UI below the file list and can block confirmation.
    pub fn draw_contents_with_custom_pane(
        &self,
        state: &mut FileDialogState,
        custom_pane: &mut dyn CustomPane,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.draw_contents_with_fs_and_custom_pane(state, &StdFileSystem, custom_pane)
    }

    /// Draw only the contents of the file browser (no window/modal host) using a custom filesystem
    /// and a custom pane.
    pub fn draw_contents_with_fs_and_custom_pane(
        &self,
        state: &mut FileDialogState,
        fs: &dyn FileSystem,
        custom_pane: &mut dyn CustomPane,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs_and_custom_pane(self.ui, state, fs, Some(custom_pane))
    }

    /// Draw the file browser in a standard ImGui window with default host config.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show(&self, state: &mut FileDialogState) -> Option<Result<Selection, FileDialogError>> {
        let cfg = WindowHostConfig::for_mode(state.core.mode);
        self.show_windowed(state, &cfg)
    }

    /// Draw the file browser in a standard ImGui window using the given host configuration.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show_windowed(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs(state, cfg, &StdFileSystem)
    }

    /// Draw the file browser in a standard ImGui window using a custom filesystem.
    pub fn show_windowed_with_fs(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
        fs: &dyn FileSystem,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs_and_custom_pane(state, cfg, fs, None)
    }

    /// Draw the file browser in a standard ImGui window using a custom filesystem and custom pane.
    pub fn show_windowed_with_fs_and_custom_pane(
        &self,
        state: &mut FileDialogState,
        cfg: &WindowHostConfig,
        fs: &dyn FileSystem,
        mut custom_pane: Option<&mut dyn CustomPane>,
    ) -> Option<Result<Selection, FileDialogError>> {
        if !state.ui.visible {
            return None;
        }

        let mut out: Option<Result<Selection, FileDialogError>> = None;
        self.ui
            .window(&cfg.title)
            .size(cfg.initial_size, cfg.size_condition)
            .build(|| {
                out = draw_contents_with_fs_and_custom_pane(self.ui, state, fs, custom_pane.take());
            });
        out
    }
}

fn draw_contents(
    ui: &Ui,
    state: &mut FileDialogState,
) -> Option<Result<Selection, FileDialogError>> {
    draw_contents_with_fs_and_custom_pane(ui, state, &StdFileSystem, None)
}

fn draw_contents_with_fs_and_custom_pane(
    ui: &Ui,
    state: &mut FileDialogState,
    fs: &dyn FileSystem,
    mut custom_pane: Option<&mut dyn CustomPane>,
) -> Option<Result<Selection, FileDialogError>> {
    if !state.ui.visible {
        return None;
    }

    let mut request_confirm = false;
    let mut confirm_gate = ConfirmGate::default();

    // Top toolbar: Up, Refresh, Hidden toggle, Breadcrumbs, Filter, Search
    if ui.button("Up") {
        state.core.navigate_up();
    }
    ui.same_line();
    if ui.button("Refresh") { /* rescan happens each frame */ }
    ui.same_line();
    let mut show_hidden = state.core.show_hidden;
    if ui.checkbox("Hidden", &mut show_hidden) {
        state.core.show_hidden = show_hidden;
    }
    ui.same_line();
    // Breadcrumbs or Path Edit
    if state.ui.path_edit {
        if state.ui.focus_path_edit_next {
            ui.set_keyboard_focus_here();
            state.ui.focus_path_edit_next = false;
        }
        ui.input_text("##path_edit", &mut state.ui.path_edit_buffer)
            .build();
        ui.same_line();
        if ui.button("Go") {
            submit_path_edit(state, fs);
        }
        ui.same_line();
        if ui.button("Cancel") {
            state.ui.path_edit = false;
        }
    } else {
        if let Some(p) = draw_breadcrumbs(ui, &state.core.cwd, state.ui.breadcrumbs_max_segments) {
            state.core.navigate_to(p);
        }
    }
    // Search box (aligned to the right)
    ui.same_line();
    if state.ui.focus_search_next {
        ui.set_keyboard_focus_here();
        state.ui.focus_search_next = false;
    }
    let search_changed = ui.input_text("Search", &mut state.core.search).build();
    if search_changed {
        // `rescan()` will apply search filtering.
    }

    ui.separator();

    // Content region
    let avail = ui.content_region_avail();
    match state.ui.layout {
        LayoutStyle::Standard => {
            let left_w = 180.0f32;
            let mut new_cwd: Option<PathBuf> = None;
            ui.child_window("quick_locations")
                .size([left_w, avail[1] - 80.0])
                .build(ui, || {
                    new_cwd = draw_quick_locations(ui, state);
                });
            if let Some(p) = new_cwd {
                state.core.navigate_to(p);
            }
            ui.same_line();
            ui.child_window("file_list")
                .size([avail[0] - left_w - 8.0, avail[1] - 80.0])
                .build(ui, || {
                    let inner = ui.content_region_avail();
                    let mut table_h = inner[1];
                    let show_pane =
                        state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                    let pane_h = if show_pane {
                        state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0))
                    } else {
                        0.0
                    };
                    if pane_h > 0.0 {
                        table_h = (table_h - pane_h - 8.0).max(0.0);
                    }

                    draw_file_table(ui, state, [inner[0], table_h], fs, &mut request_confirm);

                    if let Some(pane) = custom_pane.as_deref_mut() {
                        if state.ui.custom_pane_enabled && pane_h > 0.0 {
                            ui.separator();
                            ui.child_window("custom_pane")
                                .size([inner[0], pane_h])
                                .border(true)
                                .build(ui, || {
                                    let ctx = CustomPaneCtx {
                                        mode: state.core.mode,
                                        cwd: &state.core.cwd,
                                        selected_names: &state.core.selected,
                                        save_name: &state.core.save_name,
                                        active_filter: state
                                            .core
                                            .active_filter
                                            .and_then(|i| state.core.filters.get(i)),
                                    };
                                    confirm_gate = pane.draw(ui, ctx);
                                });
                        }
                    }
                });
        }
        LayoutStyle::Minimal => {
            ui.child_window("file_list_min")
                .size([avail[0], avail[1] - 80.0])
                .build(ui, || {
                    let inner = ui.content_region_avail();
                    let mut table_h = inner[1];
                    let show_pane =
                        state.ui.custom_pane_enabled && custom_pane.as_deref_mut().is_some();
                    let pane_h = if show_pane {
                        state.ui.custom_pane_height.clamp(0.0, inner[1].max(0.0))
                    } else {
                        0.0
                    };
                    if pane_h > 0.0 {
                        table_h = (table_h - pane_h - 8.0).max(0.0);
                    }

                    draw_file_table(ui, state, [inner[0], table_h], fs, &mut request_confirm);

                    if let Some(pane) = custom_pane.as_deref_mut() {
                        if state.ui.custom_pane_enabled && pane_h > 0.0 {
                            ui.separator();
                            ui.child_window("custom_pane")
                                .size([inner[0], pane_h])
                                .border(true)
                                .build(ui, || {
                                    let ctx = CustomPaneCtx {
                                        mode: state.core.mode,
                                        cwd: &state.core.cwd,
                                        selected_names: &state.core.selected,
                                        save_name: &state.core.save_name,
                                        active_filter: state
                                            .core
                                            .active_filter
                                            .and_then(|i| state.core.filters.get(i)),
                                    };
                                    confirm_gate = pane.draw(ui, ctx);
                                });
                        }
                    }
                });
        }
    }

    draw_places_io_modal(ui, state);

    ui.separator();
    // Footer: file name (Save) + buttons
    if matches!(state.core.mode, DialogMode::SaveFile) {
        ui.text("File name:");
        ui.same_line();
        ui.input_text("##save_name", &mut state.core.save_name)
            .build();
        ui.same_line();
    }
    // Filter selector (moved to footer like ImGuiFileDialog)
    if !state.core.filters.is_empty() && !matches!(state.core.mode, DialogMode::PickFolder) {
        ui.same_line();
        let preview = state
            .core
            .active_filter
            .and_then(|i| state.core.filters.get(i))
            .map(|f| f.name.as_str())
            .unwrap_or("All files");
        let mut next_active_filter = state.core.active_filter;
        if let Some(_c) = ui.begin_combo("Filter", preview) {
            if ui
                .selectable_config("All files")
                .selected(state.core.active_filter.is_none())
                .build()
            {
                next_active_filter = None;
            }
            for (i, f) in state.core.filters.iter().enumerate() {
                if ui
                    .selectable_config(&f.name)
                    .selected(state.core.active_filter == Some(i))
                    .build()
                {
                    next_active_filter = Some(i);
                }
            }
        }
        if next_active_filter != state.core.active_filter {
            state.core.active_filter = next_active_filter;
        }
    }

    let confirm_label = match state.core.mode {
        DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
        DialogMode::PickFolder => "Select",
        DialogMode::SaveFile => "Save",
    };
    let _disabled = ui.begin_disabled_with_cond(!confirm_gate.can_confirm);
    let confirm = ui.button(confirm_label);
    drop(_disabled);
    if !confirm_gate.can_confirm {
        if let Some(msg) = confirm_gate.message.as_deref() {
            ui.same_line();
            ui.text_disabled(msg);
        }
    }
    ui.same_line();
    let cancel = ui.button("Cancel");
    ui.same_line();
    // Click behavior toggle
    let mut nav_on_click = matches!(state.core.click_action, ClickAction::Navigate);
    if ui.checkbox("Navigate on click", &mut nav_on_click) {
        state.core.click_action = if nav_on_click {
            ClickAction::Navigate
        } else {
            ClickAction::Select
        };
    }
    ui.same_line();
    let mut dbl = state.core.double_click;
    if ui.checkbox("DblClick confirm", &mut dbl) {
        state.core.double_click = dbl;
    }

    // Keyboard shortcuts (only when the host window is focused)
    if state.ui.visible && ui.is_window_focused() {
        let ctrl = ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl);
        if ctrl && ui.is_key_pressed(Key::L) {
            state.ui.path_edit = true;
            state.ui.path_edit_buffer = state.core.cwd.display().to_string();
            state.ui.focus_path_edit_next = true;
        }
        if ctrl && ui.is_key_pressed(Key::F) {
            state.ui.focus_search_next = true;
        }
        if !ui.io().want_capture_keyboard() && ui.is_key_pressed(Key::Backspace) {
            state.core.navigate_up();
        }
        if !state.ui.path_edit && !ui.io().want_text_input() && ui.is_key_pressed(Key::Enter) {
            request_confirm |= state.core.activate_focused();
        }
    }

    request_confirm |= confirm;
    if cancel {
        state.core.cancel();
    } else if request_confirm {
        state.ui.ui_error = None;
        if let Err(e) = state.core.confirm(fs, &confirm_gate) {
            state.ui.ui_error = Some(e.to_string());
        }
    }

    draw_confirm_overwrite_modal(ui, state);

    if let Some(err) = &state.ui.ui_error {
        ui.separator();
        ui.text_colored([1.0, 0.3, 0.3, 1.0], format!("Error: {err}"));
    }

    let out = state.core.take_result();
    if out.is_some() {
        state.ui.visible = false;
    }
    out
}

fn draw_confirm_overwrite_modal(ui: &Ui, state: &mut FileDialogState) {
    const POPUP_ID: &str = "Confirm overwrite";

    let Some(path_text) = state
        .core
        .pending_overwrite()
        .and_then(|s| s.paths.get(0))
        .map(|p| p.display().to_string())
    else {
        return;
    };

    if !ui.is_popup_open(POPUP_ID) {
        ui.open_popup(POPUP_ID);
    }

    ui.modal_popup(POPUP_ID, || {
        ui.text("The file already exists:");
        ui.separator();
        ui.text(&path_text);
        ui.separator();
        if ui.button("Overwrite") {
            state.core.accept_overwrite();
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Cancel") {
            state.core.cancel_overwrite();
            ui.close_current_popup();
        }
    });
}

fn submit_path_edit(state: &mut FileDialogState, fs: &dyn FileSystem) {
    let input = state.ui.path_edit_buffer.trim();
    let raw_p = std::path::PathBuf::from(input);
    let p = fs.canonicalize(&raw_p).unwrap_or(raw_p.clone());
    match fs.metadata(&p) {
        Ok(md) => {
            if md.is_dir {
                state.core.set_cwd(p);
                state.ui.path_edit = false;
                state.ui.ui_error = None;
            } else {
                state.ui.ui_error = Some("Path exists but is not a directory".into());
            }
        }
        Err(e) => {
            use std::io::ErrorKind::*;
            let msg = match e.kind() {
                NotFound => format!("No such directory: {}", input),
                PermissionDenied => format!("Permission denied: {}", input),
                _ => format!("Invalid directory '{}': {}", input, e),
            };
            state.ui.ui_error = Some(msg);
        }
    }
}

fn draw_breadcrumbs(ui: &Ui, cwd: &Path, max_segments: usize) -> Option<PathBuf> {
    // Build crumbs first to avoid borrowing cwd while mutating it
    let mut crumbs: Vec<(String, PathBuf)> = Vec::new();
    let mut acc = PathBuf::new();
    for comp in cwd.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(p) => {
                acc.push(p.as_os_str());
                crumbs.push((p.as_os_str().to_string_lossy().to_string(), acc.clone()));
            }
            Component::RootDir => {
                acc.push(std::path::MAIN_SEPARATOR.to_string());
                crumbs.push((String::from(std::path::MAIN_SEPARATOR), acc.clone()));
            }
            Component::Normal(seg) => {
                acc.push(seg);
                crumbs.push((seg.to_string_lossy().to_string(), acc.clone()));
            }
            _ => {}
        }
    }
    let mut new_cwd: Option<PathBuf> = None;
    let n = crumbs.len();
    let compress = max_segments > 0 && n > max_segments && max_segments >= 3;
    if !compress {
        for (i, (label, path)) in crumbs.iter().enumerate() {
            if ui.button(label) {
                new_cwd = Some(path.clone());
            }
            ui.same_line();
            if i + 1 < n {
                ui.text(">");
                ui.same_line();
            }
        }
    } else {
        // First segment
        if let Some((label, path)) = crumbs.first() {
            if ui.button(label) {
                new_cwd = Some(path.clone());
            }
            ui.same_line();
            ui.text(">");
            ui.same_line();
        }
        // Ellipsis
        ui.text("...");
        ui.same_line();
        ui.text(">");
        ui.same_line();
        // Tail segments
        let tail = max_segments - 2;
        let start_tail = n.saturating_sub(tail);
        for (i, (label, path)) in crumbs.iter().enumerate().skip(start_tail) {
            if ui.button(label) {
                new_cwd = Some(path.clone());
            }
            ui.same_line();
            if i + 1 < n {
                ui.text(">");
                ui.same_line();
            }
        }
    }
    ui.new_line();
    new_cwd
}

fn draw_quick_locations(ui: &Ui, state: &mut FileDialogState) -> Option<PathBuf> {
    let mut out: Option<PathBuf> = None;

    if ui.button("+ Bookmark") {
        state.core.places.add_bookmark_path(state.core.cwd.clone());
    }
    ui.same_line();
    if ui.button("Refresh") {
        state.core.places.refresh_system_places();
    }
    ui.same_line();
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
        state.ui.places_io_open_next = true;
    }
    ui.same_line();
    if ui.button("Import") {
        state.ui.places_io_mode = crate::dialog_state::PlacesIoMode::Import;
        state.ui.places_io_buffer.clear();
        state.ui.places_io_error = None;
        state.ui.places_io_open_next = true;
    }

    ui.separator();

    let mut remove: Option<(String, PathBuf)> = None;
    for (gi, g) in state.core.places.groups.iter().enumerate() {
        let open = ui.collapsing_header(&g.label, TreeNodeFlags::DEFAULT_OPEN);
        if !open {
            continue;
        }

        if g.places.is_empty() {
            ui.text_disabled("Empty");
            continue;
        }

        for (pi, p) in g.places.iter().enumerate() {
            let _id = ui.push_id((gi * 10_000 + pi) as i32);
            if ui.selectable_config(&p.label).build() {
                out = Some(p.path.clone());
            }
            if let Some(_popup) = ui.begin_popup_context_item() {
                ui.text_disabled(&p.path.display().to_string());
                ui.separator();
                if ui.menu_item("Remove") {
                    remove = Some((g.label.clone(), p.path.clone()));
                }
            }
        }
    }
    if let Some((g, p)) = remove {
        state.core.places.remove_place_path(&g, &p);
    }
    out
}

fn draw_places_io_modal(ui: &Ui, state: &mut FileDialogState) {
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
                        for g in p.groups {
                            for place in g.places {
                                state.core.places.add_place(g.label.clone(), place);
                            }
                        }
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

fn draw_file_table(
    ui: &Ui,
    state: &mut FileDialogState,
    size: [f32; 2],
    fs: &dyn FileSystem,
    request_confirm: &mut bool,
) {
    state.core.rescan(fs);

    // Table
    use dear_imgui_rs::{SortDirection, TableColumnFlags, TableFlags};
    let flags = TableFlags::RESIZABLE
        | TableFlags::ROW_BG
        | TableFlags::BORDERS_V
        | TableFlags::BORDERS_OUTER
        | TableFlags::SCROLL_Y
        | TableFlags::SIZING_STRETCH_PROP
        | TableFlags::SORTABLE; // enable built-in header sorting
    ui.table("file_table")
        .flags(flags)
        .outer_size(size)
        .column("Name")
        .flags(TableColumnFlags::PREFER_SORT_ASCENDING)
        .user_id(0)
        .weight(0.6)
        .done()
        .column("Size")
        .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
        .user_id(1)
        .weight(0.2)
        .done()
        .column("Modified")
        .flags(TableColumnFlags::PREFER_SORT_DESCENDING)
        .user_id(2)
        .weight(0.2)
        .done()
        .headers(true)
        .build(|ui| {
            // Apply ImGui sort specs (single primary sort)
            if let Some(mut specs) = ui.table_get_sort_specs() {
                if specs.is_dirty() {
                    if let Some(s) = specs.iter().next() {
                        let (by, asc) = match (s.column_index, s.sort_direction) {
                            (0, SortDirection::Ascending) => (SortBy::Name, true),
                            (0, SortDirection::Descending) => (SortBy::Name, false),
                            (1, SortDirection::Ascending) => (SortBy::Size, true),
                            (1, SortDirection::Descending) => (SortBy::Size, false),
                            (2, SortDirection::Ascending) => (SortBy::Modified, true),
                            (2, SortDirection::Descending) => (SortBy::Modified, false),
                            _ => (state.core.sort_by, state.core.sort_ascending),
                        };
                        state.core.sort_by = by;
                        state.core.sort_ascending = asc;
                        state.core.rescan(fs);
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
                    state.core.select_all();
                }
                if ui.is_key_pressed_with_repeat(Key::UpArrow, true) {
                    state.core.move_focus(-1, modifiers);
                }
                if ui.is_key_pressed_with_repeat(Key::DownArrow, true) {
                    state.core.move_focus(1, modifiers);
                }
            }

            // Clone the entry list so we can mutate `state.core` while iterating (selection, navigation).
            let entries: Vec<DirEntry> = state.core.entries().to_vec();
            if entries.is_empty() {
                if state.ui.empty_hint_enabled {
                    ui.table_next_row();
                    ui.table_next_column();
                    let msg = if let Some(custom) = &state.ui.empty_hint_static_message {
                        custom.clone()
                    } else {
                        let filter_label = state
                            .core
                            .active_filter
                            .and_then(|i| state.core.filters.get(i))
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
                ui.table_next_column();

                let selected = state.core.selected.iter().any(|s| s == &e.name);
                let label = e.display_name();
                if ui
                    .selectable_config(label)
                    .selected(selected)
                    .span_all_columns(false)
                    .build()
                {
                    let modifiers = Modifiers {
                        ctrl: ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl),
                        shift: ui.is_key_down(Key::LeftShift) || ui.is_key_down(Key::RightShift),
                    };
                    state.core.click_entry(e.name.clone(), e.is_dir, modifiers);
                }

                if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                    state.ui.ui_error = None;
                    if state.core.double_click_entry(e.name.clone(), e.is_dir) {
                        *request_confirm = true;
                    }
                }

                if matches!(state.core.mode, DialogMode::SaveFile) && !e.is_dir {
                    state.core.save_name = e.name.clone();
                }

                ui.table_next_column();
                ui.text(match e.size {
                    Some(s) => format_size(s),
                    None => String::new(),
                });

                ui.table_next_column();
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
        });
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

// Places helpers live in `places.rs`.
