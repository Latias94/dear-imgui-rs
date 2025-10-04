use std::fs;
use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;
use dear_imgui_rs::input::{Key, MouseButton};

use crate::core::{
    ClickAction, DialogMode, FileDialogError, FileFilter, LayoutStyle, Selection, SortBy,
};

/// State for in-UI file browser
#[derive(Debug)]
pub struct FileBrowserState {
    /// Whether to draw the browser (show/hide)
    pub visible: bool,
    /// Mode
    pub mode: DialogMode,
    /// Current working directory
    pub cwd: PathBuf,
    /// Selected entry names (relative to cwd)
    pub selected: Vec<String>,
    /// Optional filename input for SaveFile
    pub save_name: String,
    /// Filters (lower-case extensions)
    pub filters: Vec<FileFilter>,
    /// Active filter index (None = All)
    pub active_filter: Option<usize>,
    /// Click behavior for directories: select or navigate
    pub click_action: ClickAction,
    /// Search query to filter entries by substring (case-insensitive)
    pub search: String,
    /// Current sort column
    pub sort_by: SortBy,
    /// Sort order flag (true = ascending)
    pub sort_ascending: bool,
    /// Layout style for the browser UI
    pub layout: LayoutStyle,
    /// Allow selecting multiple files
    pub allow_multi: bool,
    /// Show dotfiles (simple heuristic)
    pub show_hidden: bool,
    /// Double-click navigates/confirm (directories/files)
    pub double_click: bool,
    /// Path edit mode (Ctrl+L)
    pub path_edit: bool,
    /// Path edit buffer
    pub path_edit_buffer: String,
    /// Focus path edit on next frame
    pub focus_path_edit_next: bool,
    /// Focus search on next frame (Ctrl+F)
    pub focus_search_next: bool,
    /// Result emitted when the user confirms or cancels
    pub result: Option<Result<Selection, FileDialogError>>,
    /// Error string to display in UI (non-fatal)
    pub ui_error: Option<String>,
    /// Max breadcrumb segments to display (compress with ellipsis when exceeded)
    pub breadcrumbs_max_segments: usize,
    /// Put directories before files when sorting
    pub dirs_first: bool,
    /// Show a hint row when no entries match filters/search
    pub empty_hint_enabled: bool,
    /// RGBA color of the empty hint text
    pub empty_hint_color: [f32; 4],
    /// Custom static hint message when entries list is empty; if None, a default message is built
    pub empty_hint_static_message: Option<String>,
}

impl FileBrowserState {
    /// Create a new state for a mode.
    ///
    /// Examples
    /// ```no_run
    /// use dear_file_browser::{DialogMode, FileBrowserState, FileDialogExt, FileFilter};
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut state = FileBrowserState::new(DialogMode::OpenFiles);
    /// // Optional configuration
    /// state.dirs_first = true;
    /// state.double_click = true;            // dbl-click file = confirm; dbl-click dir = enter
    /// state.click_action = dear_file_browser::ClickAction::Select; // or Navigate
    /// state.breadcrumbs_max_segments = 6;   // compress deep paths
    /// // Filters are case-insensitive and extension names shouldn't include dots
    /// state.set_filters(vec![FileFilter::from(("Images", &["png", "jpg", "jpeg"]))]);
    ///
    /// ui.window("File Browser").build(|| {
    ///     if let Some(res) = ui.file_browser().show(&mut state) {
    ///         match res {
    ///             Ok(sel) => {
    ///                 for p in sel.paths { eprintln!("{:?}", p); }
    ///             }
    ///             Err(e) => eprintln!("dialog cancelled or error: {e}"),
    ///         }
    ///     }
    /// });
    /// ```
    pub fn new(mode: DialogMode) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            visible: true,
            mode,
            cwd,
            selected: Vec::new(),
            save_name: String::new(),
            filters: Vec::new(),
            active_filter: None,
            click_action: ClickAction::Select,
            search: String::new(),
            sort_by: SortBy::Name,
            sort_ascending: true,
            layout: LayoutStyle::Standard,
            allow_multi: matches!(mode, DialogMode::OpenFiles),
            show_hidden: false,
            double_click: true,
            path_edit: false,
            path_edit_buffer: String::new(),
            focus_path_edit_next: false,
            focus_search_next: false,
            result: None,
            ui_error: None,
            breadcrumbs_max_segments: 6,
            dirs_first: true,
            empty_hint_enabled: true,
            empty_hint_color: [0.7, 0.7, 0.7, 1.0],
            empty_hint_static_message: None,
        }
    }

    /// Configure filters
    pub fn set_filters<I, F>(&mut self, filters: I)
    where
        I: IntoIterator<Item = F>,
        F: Into<FileFilter>,
    {
        self.filters = filters.into_iter().map(Into::into).collect();
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
    /// Draw the file browser and update the state.
    /// Returns Some(result) once the user confirms/cancels; None otherwise.
    pub fn show(&self, state: &mut FileBrowserState) -> Option<Result<Selection, FileDialogError>> {
        if !state.visible {
            return None;
        }
        let title = match state.mode {
            DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
            DialogMode::PickFolder => "Select Folder",
            DialogMode::SaveFile => "Save",
        };
        self.ui
            .window(title)
            .size([760.0, 520.0], dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                // Top toolbar: Up, Refresh, Hidden toggle, Breadcrumbs, Filter, Search
                if self.ui.button("Up") {
                    let _ = up_dir(&mut state.cwd);
                    state.selected.clear();
                }
                self.ui.same_line();
                if self.ui.button("Refresh") { /* rescan happens each frame */ }
                self.ui.same_line();
                let mut show_hidden = state.show_hidden;
                if self.ui.checkbox("Hidden", &mut show_hidden) {
                    state.show_hidden = show_hidden;
                }
                self.ui.same_line();
                // Breadcrumbs or Path Edit
                if state.path_edit {
                    if state.focus_path_edit_next {
                        self.ui.set_keyboard_focus_here();
                        state.focus_path_edit_next = false;
                    }
                    self.ui
                        .input_text("##path_edit", &mut state.path_edit_buffer)
                        .build();
                    self.ui.same_line();
                    if self.ui.button("Go") {
                        let input = state.path_edit_buffer.trim();
                        let raw_p = PathBuf::from(input);
                        // Try to canonicalize for nicer navigation; fall back to raw path on error
                        let p = std::fs::canonicalize(&raw_p).unwrap_or(raw_p.clone());
                        match std::fs::metadata(&p) {
                            Ok(md) => {
                                if md.is_dir() {
                                    state.cwd = p;
                                    state.selected.clear();
                                    state.path_edit = false;
                                    state.ui_error = None;
                                } else {
                                    state.ui_error =
                                        Some("Path exists but is not a directory".into());
                                }
                            }
                            Err(e) => {
                                use std::io::ErrorKind::*;
                                let msg = match e.kind() {
                                    NotFound => format!("No such directory: {}", input),
                                    PermissionDenied => format!("Permission denied: {}", input),
                                    _ => format!("Invalid directory '{}': {}", input, e),
                                };
                                state.ui_error = Some(msg);
                            }
                        }
                    }
                    self.ui.same_line();
                    if self.ui.button("Cancel") {
                        state.path_edit = false;
                    }
                } else {
                    draw_breadcrumbs(self.ui, &mut state.cwd, state.breadcrumbs_max_segments);
                }
                // Search box (aligned to the right)
                self.ui.same_line();
                if state.focus_search_next {
                    self.ui.set_keyboard_focus_here();
                    state.focus_search_next = false;
                }
                self.ui.input_text("Search", &mut state.search).build();

                self.ui.separator();

                // Content region
                let avail = self.ui.content_region_avail();
                match state.layout {
                    LayoutStyle::Standard => {
                        let left_w = 180.0f32;
                        self.ui
                            .child_window("quick_locations")
                            .size([left_w, avail[1] - 80.0])
                            .build(self.ui, || {
                                draw_quick_locations(self.ui, &mut state.cwd);
                            });
                        self.ui.same_line();
                        self.ui
                            .child_window("file_list")
                            .size([avail[0] - left_w - 8.0, avail[1] - 80.0])
                            .build(self.ui, || {
                                draw_file_table(
                                    self.ui,
                                    state,
                                    [avail[0] - left_w - 8.0, avail[1] - 110.0],
                                );
                            });
                    }
                    LayoutStyle::Minimal => {
                        self.ui
                            .child_window("file_list_min")
                            .size([avail[0], avail[1] - 80.0])
                            .build(self.ui, || {
                                draw_file_table(self.ui, state, [avail[0], avail[1] - 110.0]);
                            });
                    }
                }

                self.ui.separator();
                // Footer: file name (Save) + buttons
                if matches!(state.mode, DialogMode::SaveFile) {
                    self.ui.text("File name:");
                    self.ui.same_line();
                    self.ui
                        .input_text("##save_name", &mut state.save_name)
                        .build();
                    self.ui.same_line();
                }
                // Filter selector (moved to footer like ImGuiFileDialog)
                if !state.filters.is_empty() && !matches!(state.mode, DialogMode::PickFolder) {
                    self.ui.same_line();
                    let preview = state
                        .active_filter
                        .and_then(|i| state.filters.get(i))
                        .map(|f| f.name.as_str())
                        .unwrap_or("All files");
                    if let Some(_c) = self.ui.begin_combo("Filter", preview) {
                        if self
                            .ui
                            .selectable_config("All files")
                            .selected(state.active_filter.is_none())
                            .build()
                        {
                            state.active_filter = None;
                        }
                        for (i, f) in state.filters.iter().enumerate() {
                            if self
                                .ui
                                .selectable_config(&f.name)
                                .selected(state.active_filter == Some(i))
                                .build()
                            {
                                state.active_filter = Some(i);
                            }
                        }
                    }
                }

                let confirm_label = match state.mode {
                    DialogMode::OpenFile | DialogMode::OpenFiles => "Open",
                    DialogMode::PickFolder => "Select",
                    DialogMode::SaveFile => "Save",
                };
                let confirm = self.ui.button(confirm_label);
                self.ui.same_line();
                let cancel = self.ui.button("Cancel");
                self.ui.same_line();
                // Click behavior toggle
                let mut nav_on_click = matches!(state.click_action, ClickAction::Navigate);
                if self.ui.checkbox("Navigate on click", &mut nav_on_click) {
                    state.click_action = if nav_on_click {
                        ClickAction::Navigate
                    } else {
                        ClickAction::Select
                    };
                }
                self.ui.same_line();
                let mut dbl = state.double_click;
                if self.ui.checkbox("DblClick confirm", &mut dbl) {
                    state.double_click = dbl;
                }

                if cancel {
                    state.result = Some(Err(FileDialogError::Cancelled));
                    state.visible = false;
                } else if confirm {
                    // Special-case: if a single directory selected in file-open modes, navigate into it instead of confirming
                    if matches!(state.mode, DialogMode::OpenFile | DialogMode::OpenFiles)
                        && state.selected.len() == 1
                    {
                        let sel = state.selected[0].clone();
                        let is_dir = state.cwd.join(&sel).is_dir();
                        if is_dir {
                            state.cwd.push(sel);
                            state.selected.clear();
                        } else {
                            match finalize_selection(state) {
                                Ok(sel) => {
                                    state.result = Some(Ok(sel));
                                    state.visible = false;
                                }
                                Err(e) => state.ui_error = Some(e.to_string()),
                            }
                        }
                    } else {
                        match finalize_selection(state) {
                            Ok(sel) => {
                                state.result = Some(Ok(sel));
                                state.visible = false;
                            }
                            Err(e) => state.ui_error = Some(e.to_string()),
                        }
                    }
                }

                if let Some(err) = &state.ui_error {
                    self.ui.separator();
                    self.ui
                        .text_colored([1.0, 0.3, 0.3, 1.0], format!("Error: {err}"));
                }
            });

        // Keyboard shortcuts
        let ctrl = self.ui.is_key_down(Key::LeftCtrl) || self.ui.is_key_down(Key::RightCtrl);
        if ctrl && self.ui.is_key_pressed(Key::L) {
            state.path_edit = true;
            state.path_edit_buffer = state.cwd.display().to_string();
            state.focus_path_edit_next = true;
        }
        if ctrl && self.ui.is_key_pressed(Key::F) {
            state.focus_search_next = true;
        }
        if !self.ui.io().want_capture_keyboard() && self.ui.is_key_pressed(Key::Backspace) {
            let _ = up_dir(&mut state.cwd);
            state.selected.clear();
        }
        if !state.path_edit && self.ui.is_key_pressed(Key::Enter) {
            if matches!(state.mode, DialogMode::OpenFile | DialogMode::OpenFiles)
                && state.selected.len() == 1
            {
                let sel = state.selected[0].clone();
                let is_dir = state.cwd.join(&sel).is_dir();
                if is_dir {
                    state.cwd.push(sel);
                    state.selected.clear();
                } else {
                    match finalize_selection(state) {
                        Ok(sel) => {
                            state.result = Some(Ok(sel));
                            state.visible = false;
                        }
                        Err(e) => state.ui_error = Some(e.to_string()),
                    }
                }
            } else {
                match finalize_selection(state) {
                    Ok(sel) => {
                        state.result = Some(Ok(sel));
                        state.visible = false;
                    }
                    Err(e) => state.ui_error = Some(e.to_string()),
                }
            }
        }
        state.result.take()
    }
}

fn sort_label(name: &str, active: bool, asc: bool) -> String {
    if active {
        format!("{} {}", name, if asc { "▲" } else { "▼" })
    } else {
        name.to_string()
    }
}

fn toggle_sort(sort_by: &mut SortBy, asc: &mut bool, new_key: SortBy) {
    if *sort_by == new_key {
        *asc = !*asc;
    } else {
        *sort_by = new_key;
        *asc = true;
    }
}

fn draw_breadcrumbs(ui: &Ui, cwd: &mut PathBuf, max_segments: usize) {
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
    if let Some(p) = new_cwd {
        *cwd = p;
    }
}

fn draw_quick_locations(ui: &Ui, cwd: &mut PathBuf) {
    // Home
    if ui.button("Home") {
        if let Some(home) = home_dir() {
            *cwd = home;
        }
    }
    // Root
    if ui.button("Root") {
        *cwd = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
    }
    // Drives (Windows)
    #[cfg(target_os = "windows")]
    {
        ui.separator();
        ui.text("Drives");
        for d in windows_drives() {
            if ui.button(&d) {
                *cwd = PathBuf::from(d);
            }
        }
    }
}

fn read_entries(dir: &Path, show_hidden: bool) -> Vec<DirEntry> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            if let Ok(ft) = e.file_type() {
                let name = e.file_name().to_string_lossy().to_string();
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                let meta = e.metadata().ok();
                let modified = meta.as_ref().and_then(|m| m.modified().ok());
                let size = if ft.is_file() {
                    meta.as_ref().map(|m| m.len())
                } else {
                    None
                };
                out.push(DirEntry {
                    name,
                    is_dir: ft.is_dir(),
                    size,
                    modified,
                });
            }
        }
    }
    out
}

fn up_dir(path: &mut PathBuf) -> bool {
    path.pop()
}

fn toggle_select(list: &mut Vec<String>, name: &str) {
    if let Some(i) = list.iter().position(|s| s == name) {
        list.remove(i);
    } else {
        list.push(name.to_string());
    }
}

fn matches_filters(name: &str, filters: &[FileFilter]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());
    match ext {
        Some(e) => filters.iter().any(|f| f.extensions.iter().any(|x| x == &e)),
        None => false,
    }
}

fn finalize_selection(state: &mut FileBrowserState) -> Result<Selection, FileDialogError> {
    let mut sel = Selection { paths: Vec::new() };
    let eff_filters = effective_filters(state);
    match state.mode {
        DialogMode::PickFolder => {
            sel.paths.push(state.cwd.clone());
        }
        DialogMode::OpenFile | DialogMode::OpenFiles => {
            let names = std::mem::take(&mut state.selected);
            if names.is_empty() {
                return Err(FileDialogError::InvalidPath("no selection".into()));
            }
            for n in names {
                if !matches_filters(&n, &eff_filters) {
                    continue;
                }
                sel.paths.push(state.cwd.join(n));
            }
            if sel.paths.is_empty() {
                return Err(FileDialogError::InvalidPath(
                    "no file matched filters".into(),
                ));
            }
        }
        DialogMode::SaveFile => {
            let name = if state.save_name.trim().is_empty() {
                return Err(FileDialogError::InvalidPath("empty file name".into()));
            } else {
                state.save_name.trim().to_string()
            };
            sel.paths.push(state.cwd.join(name));
        }
    }
    Ok(sel)
}

#[derive(Clone, Debug)]
struct DirEntry {
    name: String,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<std::time::SystemTime>,
}
impl DirEntry {
    fn display_name(&self) -> String {
        if self.is_dir {
            format!("[{}]", self.name)
        } else {
            self.name.clone()
        }
    }
}

fn effective_filters(state: &FileBrowserState) -> Vec<FileFilter> {
    match state.active_filter {
        Some(i) => state.filters.get(i).cloned().into_iter().collect(),
        None => Vec::new(),
    }
}

fn draw_file_table(ui: &Ui, state: &mut FileBrowserState, size: [f32; 2]) {
    // Gather entries
    let mut entries: Vec<DirEntry> = read_entries(&state.cwd, state.show_hidden);
    let display_filters: Vec<FileFilter> = effective_filters(state);
    entries.retain(|e| {
        let pass_kind = if matches!(state.mode, DialogMode::PickFolder) {
            e.is_dir
        } else {
            e.is_dir || matches_filters(&e.name, &display_filters)
        };
        let pass_search = if state.search.is_empty() {
            true
        } else {
            e.name.to_lowercase().contains(&state.search.to_lowercase())
        };
        pass_kind && pass_search
    });
    // Sort
    entries.sort_by(|a, b| {
        let ord = match state.sort_by {
            SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortBy::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortBy::Modified => a.modified.cmp(&b.modified),
        };
        if state.sort_ascending {
            ord
        } else {
            ord.reverse()
        }
    });

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
                        state.sort_by = by;
                        state.sort_ascending = asc;
                    }
                    specs.clear_dirty();
                }
            }

            // Sort entries for display
            entries.sort_by(|a, b| {
                // Directories-first precedence (independent of asc/desc)
                if state.dirs_first && a.is_dir != b.is_dir {
                    return if a.is_dir {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    };
                }
                let ord = match state.sort_by {
                    SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortBy::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
                    SortBy::Modified => a.modified.cmp(&b.modified),
                };
                if state.sort_ascending {
                    ord
                } else {
                    ord.reverse()
                }
            });

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
                        if e.is_dir {
                            match state.click_action {
                                ClickAction::Select => {
                                    state.selected.clear();
                                    state.selected.push(e.name.clone());
                                }
                                ClickAction::Navigate => {
                                    state.cwd.push(&e.name);
                                    state.selected.clear();
                                }
                            }
                        } else {
                            if !state.allow_multi {
                                state.selected.clear();
                            }
                            toggle_select(&mut state.selected, &e.name);
                        }
                    }
                    // Optional: Double-click behavior (navigate into dir or confirm selection)
                    if state.double_click
                        && ui.is_item_hovered()
                        && ui.is_mouse_double_clicked(MouseButton::Left)
                    {
                        if e.is_dir {
                            // Double-click directory: always navigate into it
                            state.cwd.push(&e.name);
                            state.selected.clear();
                        } else if matches!(state.mode, DialogMode::OpenFile | DialogMode::OpenFiles)
                        {
                            // Double-click file: confirm immediately
                            state.selected.clear();
                            state.selected.push(e.name.clone());
                            match finalize_selection(state) {
                                Ok(sel) => {
                                    state.result = Some(Ok(sel));
                                    state.visible = false;
                                }
                                Err(err) => {
                                    state.ui_error = Some(err.to_string());
                                }
                            }
                        }
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
                            use chrono::{DateTime, Local, TimeZone};
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
    use std::time::{Duration, SystemTime};
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
