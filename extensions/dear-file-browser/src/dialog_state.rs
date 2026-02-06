use std::path::PathBuf;

use dear_imgui_rs::FontId;

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

/// Column visibility configuration for list view.
#[derive(Clone, Debug)]
pub struct FileListColumnsConfig {
    /// Show preview column in list view when thumbnails are enabled.
    pub show_preview: bool,
    /// Show size column in list view.
    pub show_size: bool,
    /// Show modified time column in list view.
    pub show_modified: bool,
}

impl Default for FileListColumnsConfig {
    fn default() -> Self {
        Self {
            show_preview: true,
            show_size: true,
            show_modified: true,
        }
    }
}

impl Default for FileListViewMode {
    fn default() -> Self {
        Self::List
    }
}

/// Alignment of the validation button row (Ok/Cancel).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValidationButtonsAlign {
    /// Align buttons to the left side of the row.
    #[default]
    Left,
    /// Align buttons to the right side of the row.
    Right,
}

/// Button ordering for the validation row (Ok/Cancel).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValidationButtonsOrder {
    /// Confirm button, then Cancel.
    #[default]
    ConfirmCancel,
    /// Cancel button, then Confirm.
    CancelConfirm,
}

/// Configuration for the validation button row (Ok/Cancel).
#[derive(Clone, Debug)]
pub struct ValidationButtonsConfig {
    /// Row alignment (left/right).
    pub align: ValidationButtonsAlign,
    /// Button ordering.
    pub order: ValidationButtonsOrder,
    /// Optional confirm label override (defaults to "Open"/"Save"/"Select").
    pub confirm_label: Option<String>,
    /// Optional cancel label override (defaults to "Cancel").
    pub cancel_label: Option<String>,
    /// Optional width applied to both buttons (in pixels).
    pub button_width: Option<f32>,
    /// Optional confirm button width override (in pixels).
    pub confirm_width: Option<f32>,
    /// Optional cancel button width override (in pixels).
    pub cancel_width: Option<f32>,
}

impl Default for ValidationButtonsConfig {
    fn default() -> Self {
        Self {
            align: ValidationButtonsAlign::Left,
            order: ValidationButtonsOrder::ConfirmCancel,
            confirm_label: None,
            cancel_label: None,
            button_width: None,
            confirm_width: None,
            cancel_width: None,
        }
    }
}

/// Clipboard operation kind used by the in-UI file browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardOp {
    /// Copy sources into the destination directory on paste.
    Copy,
    /// Move sources into the destination directory on paste.
    Cut,
}

/// In-dialog clipboard for file operations (copy/cut/paste).
#[derive(Clone, Debug)]
pub struct FileClipboard {
    /// Operation kind.
    pub op: ClipboardOp,
    /// Absolute source paths captured when the clipboard was populated.
    pub sources: Vec<PathBuf>,
}

/// Conflict action used when a paste target already exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PasteConflictAction {
    /// Replace the existing destination entry.
    Overwrite,
    /// Skip this source entry.
    Skip,
    /// Keep both entries by allocating a unique destination name.
    KeepBoth,
}

/// Pending conflict information shown in the paste conflict modal.
#[derive(Clone, Debug)]
pub(crate) struct PasteConflictPrompt {
    /// Source path currently being pasted.
    pub source: PathBuf,
    /// Destination path that already exists.
    pub dest: PathBuf,
    /// Whether to reuse the chosen action for all remaining conflicts.
    pub apply_to_all: bool,
}

/// In-progress paste job state (supports modal conflict resolution).
#[derive(Clone, Debug)]
pub(crate) struct PendingPasteJob {
    /// Clipboard snapshot captured when paste was triggered.
    pub clipboard: FileClipboard,
    /// Destination directory where entries are pasted.
    pub dest_dir: PathBuf,
    /// Next source index to process.
    pub next_index: usize,
    /// Destination entry names created by this job.
    pub created: Vec<String>,
    /// Optional action reused for all remaining conflicts.
    pub apply_all_conflicts: Option<PasteConflictAction>,
    /// One-shot action for the next pending conflict only.
    pub pending_conflict_action: Option<PasteConflictAction>,
    /// Current conflict waiting for user decision.
    pub conflict: Option<PasteConflictPrompt>,
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

/// Places edit modal mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PlacesEditMode {
    /// Create a new user group.
    #[default]
    AddGroup,
    /// Rename an existing group.
    RenameGroup,
    /// Add a new user place into a group.
    AddPlace,
    /// Edit an existing user place (label/path).
    EditPlace,
    /// Confirm removing a group.
    RemoveGroupConfirm,
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
    /// Validation button row configuration (Ok/Cancel).
    pub validation_buttons: ValidationButtonsConfig,
    /// File list view mode (list vs grid).
    pub file_list_view: FileListViewMode,
    /// List-view column visibility configuration.
    pub file_list_columns: FileListColumnsConfig,
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
    /// Whether directory deletion should be recursive (`remove_dir_all`) instead of requiring empty directories.
    pub delete_recursive: bool,
    /// Error string shown inside the delete modal.
    pub delete_error: Option<String>,
    /// Clipboard state for copy/cut/paste operations.
    pub clipboard: Option<FileClipboard>,
    /// Optional font mapping used by file style `font_token`.
    pub file_style_fonts: std::collections::HashMap<String, FontId>,
    /// In-progress paste job state.
    pub(crate) paste_job: Option<PendingPasteJob>,
    /// Open the paste conflict modal on next frame.
    pub(crate) paste_conflict_open_next: bool,
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

    /// Places edit modal mode.
    pub(crate) places_edit_mode: PlacesEditMode,
    /// Open the places edit modal on next frame.
    pub(crate) places_edit_open_next: bool,
    /// Focus the first input in the places edit modal on next frame.
    pub(crate) places_edit_focus_next: bool,
    /// Error string shown inside the places edit modal.
    pub(crate) places_edit_error: Option<String>,
    /// Target group label (add/edit place, rename/remove group).
    pub(crate) places_edit_group: String,
    /// Source group label (rename/remove group).
    pub(crate) places_edit_group_from: Option<String>,
    /// Source place path for editing (stable identity).
    pub(crate) places_edit_place_from_path: Option<PathBuf>,
    /// Place label buffer (add/edit place).
    pub(crate) places_edit_place_label: String,
    /// Place path buffer (add/edit place).
    pub(crate) places_edit_place_path: String,

    pub(crate) type_select_buffer: String,
    pub(crate) type_select_last_input: Option<std::time::Instant>,
}

impl Default for FileDialogUiState {
    fn default() -> Self {
        Self {
            visible: true,
            layout: LayoutStyle::Standard,
            validation_buttons: ValidationButtonsConfig::default(),
            file_list_view: FileListViewMode::default(),
            file_list_columns: FileListColumnsConfig::default(),
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
            delete_recursive: false,
            delete_error: None,
            clipboard: None,
            file_style_fonts: std::collections::HashMap::new(),
            paste_job: None,
            paste_conflict_open_next: false,
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
            places_edit_mode: PlacesEditMode::default(),
            places_edit_open_next: false,
            places_edit_focus_next: false,
            places_edit_error: None,
            places_edit_group: String::new(),
            places_edit_group_from: None,
            places_edit_place_from_path: None,
            places_edit_place_label: String::new(),
            places_edit_place_path: String::new(),
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
