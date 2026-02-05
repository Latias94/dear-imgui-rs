use std::path::PathBuf;

use crate::core::{
    ClickAction, DialogMode, FileDialogError, FileFilter, LayoutStyle, Selection, SortBy,
};

/// State for the in-UI file browser.
///
/// This is intentionally independent of ImGui types to keep the core testable.
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
    /// state.set_filters(vec![FileFilter::from(("Images", &["png", "jpg", "jpeg"][..]))]);
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

    /// Configure filters.
    pub fn set_filters<I, F>(&mut self, filters: I)
    where
        I: IntoIterator<Item = F>,
        F: Into<FileFilter>,
    {
        self.filters = filters.into_iter().map(Into::into).collect();
    }
}
