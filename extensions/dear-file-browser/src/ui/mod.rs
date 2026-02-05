use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;
use dear_imgui_rs::input::{Key, MouseButton};

use crate::browser_core::{
    BrowserEntry, apply_event_with_fs, filter_entries_in_place, read_entries_with_fs,
    sort_entries_in_place,
};
use crate::browser_events::BrowserEvent;
pub use crate::browser_state::FileBrowserState;
use crate::core::{ClickAction, DialogMode, FileDialogError, LayoutStyle, Selection, SortBy};
use crate::fs::{FileSystem, StdFileSystem};

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
        state: &mut FileBrowserState,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.draw_contents_with_fs(state, &StdFileSystem)
    }

    /// Draw only the contents of the file browser (no window/modal host) using a custom filesystem.
    pub fn draw_contents_with_fs(
        &self,
        state: &mut FileBrowserState,
        fs: &dyn FileSystem,
    ) -> Option<Result<Selection, FileDialogError>> {
        draw_contents_with_fs(self.ui, state, fs)
    }

    /// Draw the file browser in a standard ImGui window with default host config.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show(&self, state: &mut FileBrowserState) -> Option<Result<Selection, FileDialogError>> {
        let cfg = WindowHostConfig::for_mode(state.mode);
        self.show_windowed(state, &cfg)
    }

    /// Draw the file browser in a standard ImGui window using the given host configuration.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show_windowed(
        &self,
        state: &mut FileBrowserState,
        cfg: &WindowHostConfig,
    ) -> Option<Result<Selection, FileDialogError>> {
        self.show_windowed_with_fs(state, cfg, &StdFileSystem)
    }

    /// Draw the file browser in a standard ImGui window using a custom filesystem.
    pub fn show_windowed_with_fs(
        &self,
        state: &mut FileBrowserState,
        cfg: &WindowHostConfig,
        fs: &dyn FileSystem,
    ) -> Option<Result<Selection, FileDialogError>> {
        if !state.visible {
            return None;
        }

        let mut out: Option<Result<Selection, FileDialogError>> = None;
        self.ui
            .window(&cfg.title)
            .size(cfg.initial_size, cfg.size_condition)
            .build(|| {
                out = draw_contents_with_fs(self.ui, state, fs);
            });
        out
    }
}

fn draw_contents(
    ui: &Ui,
    state: &mut FileBrowserState,
) -> Option<Result<Selection, FileDialogError>> {
    draw_contents_with_fs(ui, state, &StdFileSystem)
}

fn draw_contents_with_fs(
    ui: &Ui,
    state: &mut FileBrowserState,
    fs: &dyn FileSystem,
) -> Option<Result<Selection, FileDialogError>> {
    if !state.visible {
        return None;
    }

    // Top toolbar: Up, Refresh, Hidden toggle, Breadcrumbs, Filter, Search
    if ui.button("Up") {
        apply_event_with_fs(state, BrowserEvent::NavigateUp, fs);
    }
    ui.same_line();
    if ui.button("Refresh") { /* rescan happens each frame */ }
    ui.same_line();
    let mut show_hidden = state.show_hidden;
    if ui.checkbox("Hidden", &mut show_hidden) {
        apply_event_with_fs(state, BrowserEvent::SetShowHidden(show_hidden), fs);
    }
    ui.same_line();
    // Breadcrumbs or Path Edit
    if state.path_edit {
        if state.focus_path_edit_next {
            ui.set_keyboard_focus_here();
            state.focus_path_edit_next = false;
        }
        ui.input_text("##path_edit", &mut state.path_edit_buffer)
            .build();
        ui.same_line();
        if ui.button("Go") {
            apply_event_with_fs(state, BrowserEvent::SubmitPathEdit, fs);
        }
        ui.same_line();
        if ui.button("Cancel") {
            apply_event_with_fs(state, BrowserEvent::CancelPathEdit, fs);
        }
    } else {
        if let Some(p) = draw_breadcrumbs(ui, &state.cwd, state.breadcrumbs_max_segments) {
            apply_event_with_fs(state, BrowserEvent::NavigateTo(p), fs);
        }
    }
    // Search box (aligned to the right)
    ui.same_line();
    if state.focus_search_next {
        ui.set_keyboard_focus_here();
        state.focus_search_next = false;
    }
    let search_changed = ui.input_text("Search", &mut state.search).build();
    if search_changed {
        apply_event_with_fs(state, BrowserEvent::SetSearch(state.search.clone()), fs);
    }

    ui.separator();

    // Content region
    let avail = ui.content_region_avail();
    match state.layout {
        LayoutStyle::Standard => {
            let left_w = 180.0f32;
            let mut new_cwd: Option<PathBuf> = None;
            ui.child_window("quick_locations")
                .size([left_w, avail[1] - 80.0])
                .build(ui, || {
                    new_cwd = draw_quick_locations(ui);
                });
            if let Some(p) = new_cwd {
                apply_event_with_fs(state, BrowserEvent::NavigateTo(p), fs);
            }
            ui.same_line();
            ui.child_window("file_list")
                .size([avail[0] - left_w - 8.0, avail[1] - 80.0])
                .build(ui, || {
                    draw_file_table(ui, state, [avail[0] - left_w - 8.0, avail[1] - 110.0], fs);
                });
        }
        LayoutStyle::Minimal => {
            ui.child_window("file_list_min")
                .size([avail[0], avail[1] - 80.0])
                .build(ui, || {
                    draw_file_table(ui, state, [avail[0], avail[1] - 110.0], fs);
                });
        }
    }

    ui.separator();
    // Footer: file name (Save) + buttons
    if matches!(state.mode, DialogMode::SaveFile) {
        ui.text("File name:");
        ui.same_line();
        ui.input_text("##save_name", &mut state.save_name).build();
        ui.same_line();
    }
    // Filter selector (moved to footer like ImGuiFileDialog)
    if !state.filters.is_empty() && !matches!(state.mode, DialogMode::PickFolder) {
        ui.same_line();
        let preview = state
            .active_filter
            .and_then(|i| state.filters.get(i))
            .map(|f| f.name.as_str())
            .unwrap_or("All files");
        let mut next_active_filter = state.active_filter;
        if let Some(_c) = ui.begin_combo("Filter", preview) {
            if ui
                .selectable_config("All files")
                .selected(state.active_filter.is_none())
                .build()
            {
                next_active_filter = None;
            }
            for (i, f) in state.filters.iter().enumerate() {
                if ui
                    .selectable_config(&f.name)
                    .selected(state.active_filter == Some(i))
                    .build()
                {
                    next_active_filter = Some(i);
                }
            }
        }
        if next_active_filter != state.active_filter {
            apply_event_with_fs(state, BrowserEvent::SetActiveFilter(next_active_filter), fs);
        }
    }

    let confirm_label = match state.mode {
        DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
        DialogMode::PickFolder => "Select",
        DialogMode::SaveFile => "Save",
    };
    let confirm = ui.button(confirm_label);
    ui.same_line();
    let cancel = ui.button("Cancel");
    ui.same_line();
    // Click behavior toggle
    let mut nav_on_click = matches!(state.click_action, ClickAction::Navigate);
    if ui.checkbox("Navigate on click", &mut nav_on_click) {
        let next = if nav_on_click {
            ClickAction::Navigate
        } else {
            ClickAction::Select
        };
        apply_event_with_fs(state, BrowserEvent::SetClickAction(next), fs);
    }
    ui.same_line();
    let mut dbl = state.double_click;
    if ui.checkbox("DblClick confirm", &mut dbl) {
        apply_event_with_fs(state, BrowserEvent::SetDoubleClick(dbl), fs);
    }

    if cancel {
        apply_event_with_fs(state, BrowserEvent::Cancel, fs);
    } else if confirm {
        apply_event_with_fs(state, BrowserEvent::Confirm, fs);
    }

    if let Some(err) = &state.ui_error {
        ui.separator();
        ui.text_colored([1.0, 0.3, 0.3, 1.0], format!("Error: {err}"));
    }

    // Keyboard shortcuts (only when the host window is focused)
    if state.visible && ui.is_window_focused() {
        let ctrl = ui.is_key_down(Key::LeftCtrl) || ui.is_key_down(Key::RightCtrl);
        if ctrl && ui.is_key_pressed(Key::L) {
            apply_event_with_fs(state, BrowserEvent::StartPathEdit, fs);
        }
        if ctrl && ui.is_key_pressed(Key::F) {
            apply_event_with_fs(state, BrowserEvent::RequestSearchFocus, fs);
        }
        if !ui.io().want_capture_keyboard() && ui.is_key_pressed(Key::Backspace) {
            apply_event_with_fs(state, BrowserEvent::NavigateUp, fs);
        }
        if !state.path_edit && ui.is_key_pressed(Key::Enter) {
            apply_event_with_fs(state, BrowserEvent::Confirm, fs);
        }
    }

    state.result.take()
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

fn draw_quick_locations(ui: &Ui) -> Option<PathBuf> {
    let mut out: Option<PathBuf> = None;
    // Home
    if ui.button("Home") {
        if let Some(home) = home_dir() {
            out = Some(home);
        }
    }
    // Root
    if ui.button("Root") {
        out = Some(PathBuf::from(std::path::MAIN_SEPARATOR.to_string()));
    }
    // Drives (Windows)
    #[cfg(target_os = "windows")]
    {
        ui.separator();
        ui.text("Drives");
        for d in windows_drives() {
            if ui.button(&d) {
                out = Some(PathBuf::from(d));
            }
        }
    }
    out
}

fn draw_file_table(ui: &Ui, state: &mut FileBrowserState, size: [f32; 2], fs: &dyn FileSystem) {
    // Gather entries
    let mut entries: Vec<BrowserEntry> = read_entries_with_fs(fs, &state.cwd, state.show_hidden);
    filter_entries_in_place(
        &mut entries,
        state.mode,
        &state.filters,
        state.active_filter,
        &state.search,
    );

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
                            _ => (state.sort_by, state.sort_ascending),
                        };
                        apply_event_with_fs(
                            state,
                            BrowserEvent::SetSort { by, ascending: asc },
                            fs,
                        );
                    }
                    specs.clear_dirty();
                }
            }

            sort_entries_in_place(
                &mut entries,
                state.sort_by,
                state.sort_ascending,
                state.dirs_first,
            );

            // Rows
            if entries.is_empty() {
                if state.empty_hint_enabled {
                    ui.table_next_row();
                    ui.table_next_column();
                    let msg = if let Some(custom) = &state.empty_hint_static_message {
                        custom.clone()
                    } else {
                        let filter_label = state
                            .active_filter
                            .and_then(|i| state.filters.get(i))
                            .map(|f| f.name.as_str())
                            .unwrap_or("All files");
                        let hidden_label = if state.show_hidden { "on" } else { "off" };
                        if state.search.is_empty() {
                            format!(
                                "No matching entries. Filter: {}, Hidden: {}",
                                filter_label, hidden_label
                            )
                        } else {
                            format!(
                                "No matching entries. Filter: {}, Search: '{}', Hidden: {}",
                                filter_label, state.search, hidden_label
                            )
                        }
                    };
                    ui.text_colored(state.empty_hint_color, msg);
                }
            } else {
                for e in &entries {
                    ui.table_next_row();
                    // Name
                    ui.table_next_column();
                    let selected = state.selected.iter().any(|s| s == &e.name);
                    let label = e.display_name();
                    if ui
                        .selectable_config(label)
                        .selected(selected)
                        .span_all_columns(false)
                        .build()
                    {
                        apply_event_with_fs(
                            state,
                            BrowserEvent::ClickEntry {
                                name: e.name.clone(),
                                is_dir: e.is_dir,
                            },
                            fs,
                        );
                    }
                    // Optional: Double-click behavior (navigate into dir or confirm selection)
                    if state.double_click
                        && ui.is_item_hovered()
                        && ui.is_mouse_double_clicked(MouseButton::Left)
                    {
                        apply_event_with_fs(
                            state,
                            BrowserEvent::DoubleClickEntry {
                                name: e.name.clone(),
                                is_dir: e.is_dir,
                            },
                            fs,
                        );
                    }
                    // Size
                    ui.table_next_column();
                    ui.text(match e.size {
                        Some(s) => format_size(s),
                        None => String::new(),
                    });
                    // Modified
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

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(target_os = "windows")]
fn windows_drives() -> Vec<String> {
    let mut v = Vec::new();
    for c in b'A'..=b'Z' {
        let s = format!("{}:\\", c as char);
        if Path::new(&s).exists() {
            v.push(s);
        }
    }
    v
}
