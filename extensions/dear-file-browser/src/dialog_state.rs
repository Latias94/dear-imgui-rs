use crate::core::{DialogMode, LayoutStyle};
use crate::dialog_core::FileDialogCore;
use crate::file_style::FileStyleRegistry;

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
    /// Style registry used to decorate the file list (icons/colors/tooltips).
    pub file_styles: FileStyleRegistry,
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
}

impl Default for FileDialogUiState {
    fn default() -> Self {
        Self {
            visible: true,
            layout: LayoutStyle::Standard,
            breadcrumbs_max_segments: 6,
            empty_hint_enabled: true,
            empty_hint_color: [0.7, 0.7, 0.7, 1.0],
            empty_hint_static_message: None,
            path_edit: false,
            path_edit_buffer: String::new(),
            focus_path_edit_next: false,
            focus_search_next: false,
            ui_error: None,
            file_styles: FileStyleRegistry::default(),
            custom_pane_enabled: true,
            custom_pane_height: 120.0,
            places_io_mode: PlacesIoMode::Export,
            places_io_buffer: String::new(),
            places_io_open_next: false,
            places_io_include_code: false,
            places_io_error: None,
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
