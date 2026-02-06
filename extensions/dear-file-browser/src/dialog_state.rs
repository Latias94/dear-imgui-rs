use crate::core::{DialogMode, LayoutStyle};
use crate::dialog_core::FileDialogCore;
use crate::file_style::FileStyleRegistry;
use crate::thumbnails::{ThumbnailCache, ThumbnailCacheConfig};

/// View mode for the file list region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileListViewMode {
    /// Table-style list view (columns: name/size/modified, optional thumbnail preview column).
    List,
    /// Thumbnail grid view.
    Grid,
}

impl Default for FileListViewMode {
    fn default() -> Self {
        Self::List
    }
}

/// Places import/export modal mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PlacesIoMode {
    /// Export the current places into a text buffer.
    #[default]
    Export,
    /// Import places from a text buffer.
    Import,
}

/// UI-only state for hosting a [`FileDialogCore`] in Dear ImGui.
///
/// This struct contains transient UI state (visibility, focus requests, text
/// buffers) and does not affect the core selection/navigation semantics.
#[derive(Debug)]
pub struct FileDialogUiState {
    /// Whether to draw the dialog (show/hide).
    pub visible: bool,
    /// Layout style for the dialog UI.
    pub layout: LayoutStyle,
    /// File list view mode (list vs grid).
    pub file_list_view: FileListViewMode,
    /// Max breadcrumb segments to display (compress with ellipsis when exceeded).
    pub breadcrumbs_max_segments: usize,
    /// Show a hint row when no entries match filters/search.
    pub empty_hint_enabled: bool,
    /// RGBA color of the empty hint text.
    pub empty_hint_color: [f32; 4],
    /// Custom static hint message when entries list is empty; if None, a default message is built.
    pub empty_hint_static_message: Option<String>,
    /// Path edit mode (Ctrl+L).
    pub path_edit: bool,
    /// Path edit buffer.
    pub path_edit_buffer: String,
    /// Focus path edit on next frame.
    pub focus_path_edit_next: bool,
    /// Focus search on next frame (Ctrl+F).
    pub focus_search_next: bool,
    /// Error string to display in UI (non-fatal).
    pub ui_error: Option<String>,
    /// Open the "New Folder" modal on next frame.
    pub new_folder_open_next: bool,
    /// New folder name buffer (used by the "New Folder" modal).
    pub new_folder_name: String,
    /// Focus the new folder input on next frame.
    pub new_folder_focus_next: bool,
    /// Error string shown inside the "New Folder" modal.
    pub new_folder_error: Option<String>,
    /// Open the "Rename" modal on next frame.
    pub rename_open_next: bool,
    /// Focus the rename input on next frame.
    pub rename_focus_next: bool,
    /// Rename target entry name (relative to cwd).
    pub rename_target: Option<String>,
    /// Rename "to" buffer.
    pub rename_to: String,
    /// Error string shown inside the rename modal.
    pub rename_error: Option<String>,
    /// Open the "Delete" confirmation modal on next frame.
    pub delete_open_next: bool,
    /// Pending delete targets (relative to cwd).
    pub delete_targets: Vec<String>,
    /// Error string shown inside the delete modal.
    pub delete_error: Option<String>,
    /// Reveal (scroll to) a specific entry name on the next draw, then clear.
    pub(crate) reveal_name_next: Option<String>,
    /// Style registry used to decorate the file list (icons/colors/tooltips).
    pub file_styles: FileStyleRegistry,
    /// Enable thumbnails in the file list (adds a Preview column).
    pub thumbnails_enabled: bool,
    /// Thumbnail preview size in pixels.
    pub thumbnail_size: [f32; 2],
    /// Thumbnail cache (requests + LRU).
    pub thumbnails: ThumbnailCache,
    /// Enable "type-to-select" behavior in the file list (IGFD-style).
    pub type_select_enabled: bool,
    /// Timeout (milliseconds) after which the type-to-select buffer resets.
    pub type_select_timeout_ms: u64,
    /// Whether to render a custom pane region (when a pane is provided by the caller).
    pub custom_pane_enabled: bool,
    /// Height of the custom pane region (in pixels).
    pub custom_pane_height: f32,

    /// Places modal mode (export/import).
    pub(crate) places_io_mode: PlacesIoMode,
    /// Places modal text buffer.
    pub(crate) places_io_buffer: String,
    /// Open the places modal on next frame.
    pub(crate) places_io_open_next: bool,
    /// Whether export should include code-defined places.
    pub(crate) places_io_include_code: bool,
    /// Error string shown inside the places modal.
    pub(crate) places_io_error: Option<String>,

    pub(crate) type_select_buffer: String,
    pub(crate) type_select_last_input: Option<std::time::Instant>,
}

impl Default for FileDialogUiState {
    fn default() -> Self {
        Self {
            visible: true,
            layout: LayoutStyle::Standard,
            file_list_view: FileListViewMode::default(),
            breadcrumbs_max_segments: 6,
            empty_hint_enabled: true,
            empty_hint_color: [0.7, 0.7, 0.7, 1.0],
            empty_hint_static_message: None,
            path_edit: false,
            path_edit_buffer: String::new(),
            focus_path_edit_next: false,
            focus_search_next: false,
            ui_error: None,
            new_folder_open_next: false,
            new_folder_name: String::new(),
            new_folder_focus_next: false,
            new_folder_error: None,
            rename_open_next: false,
            rename_focus_next: false,
            rename_target: None,
            rename_to: String::new(),
            rename_error: None,
            delete_open_next: false,
            delete_targets: Vec::new(),
            delete_error: None,
            reveal_name_next: None,
            file_styles: FileStyleRegistry::default(),
            thumbnails_enabled: false,
            thumbnail_size: [32.0, 32.0],
            thumbnails: ThumbnailCache::new(ThumbnailCacheConfig::default()),
            type_select_enabled: true,
            type_select_timeout_ms: 750,
            custom_pane_enabled: true,
            custom_pane_height: 120.0,
            places_io_mode: PlacesIoMode::Export,
            places_io_buffer: String::new(),
            places_io_open_next: false,
            places_io_include_code: false,
            places_io_error: None,
            type_select_buffer: String::new(),
            type_select_last_input: None,
        }
    }
}

/// Combined state for the in-UI file dialog.
#[derive(Debug)]
pub struct FileDialogState {
    /// Core state machine.
    pub core: FileDialogCore,
    /// UI-only state.
    pub ui: FileDialogUiState,
}

impl FileDialogState {
    /// Creates a new dialog state for a mode.
    pub fn new(mode: DialogMode) -> Self {
        Self {
            core: FileDialogCore::new(mode),
            ui: FileDialogUiState::default(),
        }
    }
}
