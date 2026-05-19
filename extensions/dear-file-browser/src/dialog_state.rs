use std::path::PathBuf;
use std::time::Duration;

use dear_imgui_rs::FontId;

use crate::core::{ClickAction, DialogMode, LayoutStyle};
use crate::dialog_core::{EntryId, FileDialogCore, ScanPolicy, ScanStatus};
use crate::file_style::FileStyleRegistry;
use crate::thumbnails::{ThumbnailCache, ThumbnailCacheConfig};

/// View mode for the file list region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileListViewMode {
    /// Table-style list view (columns: name/size/modified, optional thumbnail preview column).
    List,
    /// Table-style list view with thumbnails enabled and preview column shown.
    ///
    /// This is intended to match IGFD’s “thumbnails list” mode (small thumbs on the same row).
    ThumbnailsList,
    /// Thumbnail grid view.
    Grid,
}

/// Data column identifier for list view (excluding optional preview thumbnail column).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileListDataColumn {
    /// File name (main selectable cell).
    Name,
    /// File extension (derived from name).
    Extension,
    /// File size.
    Size,
    /// Last-modified timestamp.
    Modified,
}

impl FileListDataColumn {
    fn compact_token(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Extension => "ext",
            Self::Size => "size",
            Self::Modified => "modified",
        }
    }

    fn from_compact_token(token: &str) -> Option<Self> {
        match token {
            "name" => Some(Self::Name),
            "ext" => Some(Self::Extension),
            "size" => Some(Self::Size),
            "modified" => Some(Self::Modified),
            _ => None,
        }
    }
}

/// Optional per-column width overrides for list view.
///
/// Values are interpreted as ImGui table stretch weights and must be finite and positive.
/// When an override is `None`, the built-in heuristic weight is used.
#[derive(Clone, Debug, PartialEq)]
pub struct FileListColumnWeightOverrides {
    /// Preview (thumbnail) column weight.
    pub preview: Option<f32>,
    /// Name column weight.
    pub name: Option<f32>,
    /// Extension column weight.
    pub extension: Option<f32>,
    /// Size column weight.
    pub size: Option<f32>,
    /// Modified column weight.
    pub modified: Option<f32>,
}

impl Default for FileListColumnWeightOverrides {
    fn default() -> Self {
        Self {
            preview: None,
            name: None,
            extension: None,
            size: None,
            modified: None,
        }
    }
}

/// Column visibility configuration for list view.
#[derive(Clone, Debug, PartialEq)]
pub struct FileListColumnsConfig {
    /// Show preview column in list view when thumbnails are enabled.
    pub show_preview: bool,
    /// Show extension column in list view.
    pub show_extension: bool,
    /// Show size column in list view.
    pub show_size: bool,
    /// Show modified time column in list view.
    pub show_modified: bool,
    /// Render order for data columns (Name/Extension/Size/Modified).
    ///
    /// Name/Extension are always shown, while Size/Modified still obey visibility flags.
    pub order: [FileListDataColumn; 4],
    /// Optional per-column stretch-weight overrides.
    pub weight_overrides: FileListColumnWeightOverrides,
}

/// Error returned by [`FileListColumnsConfig::deserialize_compact`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileListColumnsDeserializeError {
    msg: String,
}

impl FileListColumnsDeserializeError {
    fn new(msg: impl Into<String>) -> Self {
        Self { msg: msg.into() }
    }
}

impl std::fmt::Display for FileListColumnsDeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file list columns deserialize error: {}", self.msg)
    }
}

impl std::error::Error for FileListColumnsDeserializeError {}

impl FileListColumnsConfig {
    /// Serializes list-column preferences to a compact string.
    ///
    /// This is dependency-free (no serde) and intended for app-level persistence.
    pub fn serialize_compact(&self) -> String {
        let order = self
            .normalized_order()
            .iter()
            .map(|c| c.compact_token())
            .collect::<Vec<_>>()
            .join(",");
        let weights = [
            self.weight_overrides.preview,
            self.weight_overrides.name,
            self.weight_overrides.extension,
            self.weight_overrides.size,
            self.weight_overrides.modified,
        ]
        .into_iter()
        .map(|v| {
            v.map(|w| format!("{w:.4}"))
                .unwrap_or_else(|| "auto".to_string())
        })
        .collect::<Vec<_>>()
        .join(",");

        format!(
            "v1;preview={};ext={};size={};modified={};order={};weights={}",
            u8::from(self.show_preview),
            u8::from(self.show_extension),
            u8::from(self.show_size),
            u8::from(self.show_modified),
            order,
            weights,
        )
    }

    /// Deserializes list-column preferences from [`Self::serialize_compact`] format.
    pub fn deserialize_compact(input: &str) -> Result<Self, FileListColumnsDeserializeError> {
        let mut version_ok = false;
        let mut preview = None;
        let mut ext = None;
        let mut size = None;
        let mut modified = None;
        let mut order = None;
        let mut weights = None;

        for token in input.split(';').filter(|s| !s.trim().is_empty()) {
            if token == "v1" {
                version_ok = true;
                continue;
            }
            if token.starts_with('v') {
                return Err(FileListColumnsDeserializeError::new(format!(
                    "unsupported version token `{token}`"
                )));
            }
            let (key, value) = token.split_once('=').ok_or_else(|| {
                FileListColumnsDeserializeError::new(format!("invalid token `{token}`"))
            })?;
            match key {
                "preview" => preview = Some(parse_compact_bool(value)?),
                "ext" => ext = Some(parse_compact_bool(value)?),
                "size" => size = Some(parse_compact_bool(value)?),
                "modified" => modified = Some(parse_compact_bool(value)?),
                "order" => order = Some(parse_compact_order(value)?),
                "weights" => weights = Some(parse_compact_weights(value)?),
                _ => {
                    return Err(FileListColumnsDeserializeError::new(format!(
                        "unknown key `{key}`"
                    )));
                }
            }
        }

        if !version_ok {
            return Err(FileListColumnsDeserializeError::new(
                "missing or unsupported version token",
            ));
        }

        Ok(Self {
            show_preview: preview
                .ok_or_else(|| FileListColumnsDeserializeError::new("missing key `preview`"))?,
            show_extension: ext
                .ok_or_else(|| FileListColumnsDeserializeError::new("missing key `ext`"))?,
            show_size: size
                .ok_or_else(|| FileListColumnsDeserializeError::new("missing key `size`"))?,
            show_modified: modified
                .ok_or_else(|| FileListColumnsDeserializeError::new("missing key `modified`"))?,
            order: order
                .ok_or_else(|| FileListColumnsDeserializeError::new("missing key `order`"))?,
            weight_overrides: weights
                .ok_or_else(|| FileListColumnsDeserializeError::new("missing key `weights`"))?,
        })
    }

    /// Returns a deterministic valid order (dedup + append missing columns).
    pub fn normalized_order(&self) -> [FileListDataColumn; 4] {
        normalized_order(self.order)
    }
}

impl Default for FileListColumnsConfig {
    fn default() -> Self {
        Self {
            show_preview: true,
            show_extension: true,
            show_size: true,
            show_modified: true,
            order: [
                FileListDataColumn::Name,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
            ],
            weight_overrides: FileListColumnWeightOverrides::default(),
        }
    }
}

fn normalized_order(order: [FileListDataColumn; 4]) -> [FileListDataColumn; 4] {
    let mut out = Vec::with_capacity(4);
    for c in order {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    for c in [
        FileListDataColumn::Name,
        FileListDataColumn::Extension,
        FileListDataColumn::Size,
        FileListDataColumn::Modified,
    ] {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    [out[0], out[1], out[2], out[3]]
}

fn parse_compact_bool(value: &str) -> Result<bool, FileListColumnsDeserializeError> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(FileListColumnsDeserializeError::new(format!(
            "invalid bool value `{value}`"
        ))),
    }
}

fn parse_compact_order(
    value: &str,
) -> Result<[FileListDataColumn; 4], FileListColumnsDeserializeError> {
    let cols = value
        .split(',')
        .map(FileListDataColumn::from_compact_token)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| FileListColumnsDeserializeError::new("invalid column token in `order`"))?;
    if cols.len() != 4 {
        return Err(FileListColumnsDeserializeError::new(
            "`order` must contain exactly 4 columns",
        ));
    }
    let order = [cols[0], cols[1], cols[2], cols[3]];
    let normalized = normalized_order(order);
    if normalized != order {
        return Err(FileListColumnsDeserializeError::new(
            "`order` must contain each column exactly once",
        ));
    }
    Ok(order)
}

fn parse_compact_optional_weight(
    value: &str,
) -> Result<Option<f32>, FileListColumnsDeserializeError> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let parsed = value.parse::<f32>().map_err(|_| {
        FileListColumnsDeserializeError::new(format!("invalid weight value `{value}`"))
    })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(FileListColumnsDeserializeError::new(format!(
            "weight must be finite and > 0, got `{value}`"
        )));
    }
    Ok(Some(parsed))
}

fn parse_compact_weights(
    value: &str,
) -> Result<FileListColumnWeightOverrides, FileListColumnsDeserializeError> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 5 {
        return Err(FileListColumnsDeserializeError::new(
            "`weights` must contain exactly 5 values",
        ));
    }
    Ok(FileListColumnWeightOverrides {
        preview: parse_compact_optional_weight(parts[0])?,
        name: parse_compact_optional_weight(parts[1])?,
        extension: parse_compact_optional_weight(parts[2])?,
        size: parse_compact_optional_weight(parts[3])?,
        modified: parse_compact_optional_weight(parts[4])?,
    })
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

/// Density preset for the dialog top toolbar ("chrome").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolbarDensity {
    /// Use the host's default Dear ImGui style values.
    #[default]
    Normal,
    /// Reduce padding and spacing to fit more controls (IGFD-like).
    Compact,
    /// Increase padding and spacing for touch-friendly UIs.
    Spacious,
}

/// How to render toolbar buttons when optional icons are provided.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolbarIconMode {
    /// Render text-only labels.
    #[default]
    Text,
    /// Render icon-only labels (falls back to text if an icon is not provided).
    IconOnly,
    /// Render icon + text (falls back to text if an icon is not provided).
    IconAndText,
}

/// Optional toolbar icons (host-provided glyphs, typically from an icon font).
#[derive(Clone, Debug, Default)]
pub struct ToolbarIcons {
    /// Icon rendering mode.
    pub mode: ToolbarIconMode,
    /// Icon for "Places".
    pub places: Option<String>,
    /// Icon for "Refresh".
    pub refresh: Option<String>,
    /// Icon for "New Folder".
    pub new_folder: Option<String>,
    /// Icon for "Columns".
    pub columns: Option<String>,
    /// Icon for "Options".
    pub options: Option<String>,
}

/// Configuration for the dialog top toolbar ("chrome").
#[derive(Clone, Debug)]
pub struct ToolbarConfig {
    /// Density preset affecting padding/spacing.
    pub density: ToolbarDensity,
    /// Optional icon glyphs for toolbar buttons.
    pub icons: ToolbarIcons,
    /// Whether to show tooltips for toolbar controls.
    pub show_tooltips: bool,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            density: ToolbarDensity::Normal,
            icons: ToolbarIcons::default(),
            show_tooltips: true,
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

/// Caller-facing configuration for the in-UI file dialog.
///
/// This Module Interface contains durable UI knobs that callers may configure before or during a
/// frame. Transient buffers, focus requests, modal state, and operation jobs stay in
/// [`FileDialogUiState`].
#[derive(Debug)]
pub struct FileDialogUiConfig {
    /// Header layout style.
    pub header_style: HeaderStyle,
    /// Layout style for the dialog UI.
    pub layout: LayoutStyle,
    /// Validation button row configuration (Ok/Cancel).
    pub validation_buttons: ValidationButtonsConfig,
    /// Top toolbar ("chrome") configuration.
    pub toolbar: ToolbarConfig,
    /// Whether to show the left "Places" pane in [`LayoutStyle::Standard`].
    pub places_pane_shown: bool,
    /// Width of the left "Places" pane in pixels (Standard layout only).
    pub places_pane_width: f32,
    /// File list view mode (list vs grid).
    pub file_list_view: FileListViewMode,
    /// List-view column visibility configuration.
    pub file_list_columns: FileListColumnsConfig,
    /// Path bar style (editable text input vs breadcrumb-style composer).
    pub path_bar_style: PathBarStyle,
    /// Enable quick parallel directory selection popups when clicking breadcrumb separators.
    ///
    /// This mimics IGFD's "quick path selection" feature in the path composer.
    pub breadcrumbs_quick_select: bool,
    /// Max breadcrumb segments to display (compress with ellipsis when exceeded).
    pub breadcrumbs_max_segments: usize,
    /// Show a hint row when no entries match filters/search.
    pub empty_hint_enabled: bool,
    /// RGBA color of the empty hint text.
    pub empty_hint_color: [f32; 4],
    /// Custom static hint message when entries list is empty; if None, a default message is built.
    pub empty_hint_static_message: Option<String>,
    /// Whether to show and allow the "New Folder" action.
    pub new_folder_enabled: bool,
    /// Optional font mapping used by file style `font_token`.
    pub file_style_fonts: std::collections::HashMap<String, FontId>,
    /// Style registry used to decorate the file list (icons/colors/tooltips).
    pub file_styles: FileStyleRegistry,
    /// Enable thumbnails in the file list (adds a Preview column).
    pub thumbnails_enabled: bool,
    /// Thumbnail preview size in pixels.
    pub thumbnail_size: [f32; 2],
    /// Enable "type-to-select" behavior in the file list (IGFD-style).
    pub type_select_enabled: bool,
    /// Timeout after which the type-to-select buffer resets.
    pub type_select_timeout: Duration,
    /// Whether to render a custom pane region (when a pane is provided by the caller).
    pub custom_pane_enabled: bool,
    /// Dock position for the custom pane.
    pub custom_pane_dock: CustomPaneDock,
    /// Height of the custom pane region (in pixels).
    pub custom_pane_height: f32,
    /// Width of the custom pane region when right-docked (in pixels).
    pub custom_pane_width: f32,
}

impl Default for FileDialogUiConfig {
    fn default() -> Self {
        Self {
            header_style: HeaderStyle::ToolbarAndAddress,
            layout: LayoutStyle::Standard,
            validation_buttons: ValidationButtonsConfig::default(),
            toolbar: ToolbarConfig::default(),
            places_pane_shown: true,
            places_pane_width: 150.0,
            file_list_view: FileListViewMode::default(),
            file_list_columns: FileListColumnsConfig::default(),
            path_bar_style: PathBarStyle::TextInput,
            breadcrumbs_quick_select: true,
            breadcrumbs_max_segments: 6,
            empty_hint_enabled: true,
            empty_hint_color: [0.7, 0.7, 0.7, 1.0],
            empty_hint_static_message: None,
            new_folder_enabled: true,
            file_style_fonts: std::collections::HashMap::new(),
            file_styles: FileStyleRegistry::default(),
            thumbnails_enabled: false,
            thumbnail_size: [32.0, 32.0],
            type_select_enabled: true,
            type_select_timeout: Duration::from_millis(750),
            custom_pane_enabled: true,
            custom_pane_dock: CustomPaneDock::default(),
            custom_pane_height: 120.0,
            custom_pane_width: 250.0,
        }
    }
}

impl FileDialogUiConfig {
    /// Applies an "IGFD classic" configuration preset (opt-in).
    ///
    /// This tunes durable UI knobs to feel closer to ImGuiFileDialog (IGFD) while staying
    /// Rust-first.
    pub fn apply_igfd_classic_preset(&mut self) {
        self.header_style = HeaderStyle::IgfdClassic;
        self.layout = LayoutStyle::Standard;
        self.places_pane_shown = true;
        self.places_pane_width = 150.0;
        self.file_list_view = FileListViewMode::List;
        self.thumbnails_enabled = false;
        self.toolbar.density = ToolbarDensity::Compact;
        self.path_bar_style = PathBarStyle::Breadcrumbs;
        self.breadcrumbs_quick_select = true;

        if self.file_styles.rules.is_empty() && self.file_styles.callback.is_none() {
            self.file_styles = crate::file_style::FileStyleRegistry::igfd_ascii_preset();
        }

        self.file_list_columns.show_preview = false;
        self.file_list_columns.show_extension = false;
        self.file_list_columns.show_size = true;
        self.file_list_columns.show_modified = true;
        self.file_list_columns.order = [
            FileListDataColumn::Name,
            FileListDataColumn::Extension,
            FileListDataColumn::Size,
            FileListDataColumn::Modified,
        ];

        self.custom_pane_enabled = true;
        self.custom_pane_dock = CustomPaneDock::Right;
        self.custom_pane_width = 250.0;
        self.custom_pane_height = 120.0;

        self.validation_buttons.align = ValidationButtonsAlign::Right;
        self.validation_buttons.order = ValidationButtonsOrder::CancelConfirm;
        self.validation_buttons.confirm_label = Some("OK".to_string());
        self.validation_buttons.cancel_label = Some("Cancel".to_string());
        self.validation_buttons.button_width = None;
        self.validation_buttons.confirm_width = None;
        self.validation_buttons.cancel_width = None;
    }
}

/// Transient per-frame/runtime state owned by the Dear ImGui adapter.
///
/// These fields are implementation details of the UI renderer. They are intentionally separated
/// from [`FileDialogUiConfig`] so caller-facing configuration has a narrow, durable surface.
#[derive(Debug, Default)]
pub(crate) struct FileDialogUiRuntime {
    /// Address/path editor runtime state.
    pub(crate) path: PathUiRuntime,
    /// Last cwd observed while the dialog was opened.
    pub(crate) opened_cwd: Option<PathBuf>,
    /// Focus search on next frame (Ctrl+F).
    pub(crate) focus_search_next: bool,
    /// Error string to display in UI (non-fatal).
    pub(crate) error: Option<String>,
    /// Accumulated IGFD-style type-to-select prefix.
    pub(crate) type_select_buffer: String,
    /// Last keypress timestamp used to expire the type-to-select prefix.
    pub(crate) type_select_last_input: Option<std::time::Instant>,
    /// Breadcrumb runtime state.
    pub(crate) breadcrumb: BreadcrumbUiRuntime,
    /// Footer runtime state.
    pub(crate) footer: FooterUiRuntime,
}

/// Runtime state for the address/path editor.
#[derive(Debug, Default)]
pub(crate) struct PathUiRuntime {
    /// When `true` (and `path_bar_style` is [`PathBarStyle::Breadcrumbs`]), show the editable path
    /// text input instead of the breadcrumb composer.
    ///
    /// This mimics IGFD's path composer "Edit" toggle behavior.
    pub(crate) input_mode: bool,
    /// Whether the path input is currently being edited (best-effort; updated by UI).
    pub(crate) edit: bool,
    /// Path input buffer (editable "address bar").
    pub(crate) buffer: String,
    pub(crate) last_cwd: String,
    pub(crate) history_index: Option<usize>,
    pub(crate) history_saved_buffer: Option<String>,
    pub(crate) programmatic_edit: bool,
    /// Focus path edit on next frame.
    pub(crate) focus_next: bool,
}

/// Runtime state for the breadcrumb composer and quick-select popup.
#[derive(Debug, Default)]
pub(crate) struct BreadcrumbUiRuntime {
    pub(crate) scroll_to_end_next: bool,
    /// Current parent dir for the breadcrumb quick-select popup.
    pub(crate) quick_parent: Option<PathBuf>,
}

/// Runtime state for the footer area.
#[derive(Debug, Default)]
pub(crate) struct FooterUiRuntime {
    /// Last measured footer height (in window coordinates), used to size the content region
    /// without hard-coded constants. Updated each frame after drawing the footer.
    pub(crate) height_last: f32,
    /// UI buffer for the footer "File/Folder" input.
    ///
    /// - SaveFile uses `core.save_name` instead.
    /// - OpenFile/OpenFiles can be typed to open a file by name/path (IGFD-style).
    /// - PickFolder currently uses this for display only.
    pub(crate) file_name_buffer: String,
    /// The last auto-generated display string for the footer input, used to keep the field
    /// synced to selection unless the user edits it.
    pub(crate) file_name_last_display: String,
}

/// Modal and operation state owned by the Dear ImGui adapter.
///
/// Operation state changes while the UI is driving an action. Keeping it behind this internal seam
/// prevents one-off operation buffers from becoming caller-facing configuration.
#[derive(Debug, Default)]
pub(crate) struct FileDialogOperationState {
    /// State for the "New Folder" inline editor/modal.
    pub(crate) new_folder: NewFolderOperationState,
    /// State for the rename modal.
    pub(crate) rename: RenameOperationState,
    /// State for the delete confirmation modal.
    pub(crate) delete: DeleteOperationState,
    /// State for copy/cut/paste operations.
    pub(crate) paste: PasteOperationState,
    /// State for places import/export/edit UI.
    pub(crate) places: PlacesOperationState,
    /// Reveal (scroll to) a specific entry id on the next draw, then clear.
    pub(crate) reveal_id_next: Option<EntryId>,
}

/// Runtime state for the "New Folder" operation.
#[derive(Debug, Default)]
pub(crate) struct NewFolderOperationState {
    /// Whether the inline editor is active (toolbar-local, IGFD-like).
    pub(crate) inline_active: bool,
    /// Open the modal on next frame.
    pub(crate) open_next: bool,
    /// Folder name input buffer.
    pub(crate) name: String,
    /// Focus the input on next frame.
    pub(crate) focus_next: bool,
    /// Error string shown inside the inline editor/modal.
    pub(crate) error: Option<String>,
}

/// Runtime state for the rename operation.
#[derive(Debug, Default)]
pub(crate) struct RenameOperationState {
    /// Open the modal on next frame.
    pub(crate) open_next: bool,
    /// Focus the input on next frame.
    pub(crate) focus_next: bool,
    /// Target entry id.
    pub(crate) target_id: Option<EntryId>,
    /// Rename target buffer.
    pub(crate) to: String,
    /// Error string shown inside the rename modal.
    pub(crate) error: Option<String>,
}

/// Runtime state for the delete operation.
#[derive(Debug, Default)]
pub(crate) struct DeleteOperationState {
    /// Open the confirmation modal on next frame.
    pub(crate) open_next: bool,
    /// Pending delete target ids.
    pub(crate) target_ids: Vec<EntryId>,
    /// Whether directory deletion should be recursive (`remove_dir_all`) instead of requiring empty directories.
    pub(crate) recursive: bool,
    /// Error string shown inside the delete modal.
    pub(crate) error: Option<String>,
}

/// Runtime state for the in-dialog clipboard and paste operation.
#[derive(Debug, Default)]
pub(crate) struct PasteOperationState {
    /// Clipboard state for copy/cut/paste operations.
    pub(crate) clipboard: Option<FileClipboard>,
    /// In-progress paste job state.
    pub(crate) job: Option<PendingPasteJob>,
    /// Open the paste conflict modal on next frame.
    pub(crate) conflict_open_next: bool,
}

/// Runtime state for places import/export, edit modals, and inline edit.
#[derive(Debug, Default)]
pub(crate) struct PlacesOperationState {
    pub(crate) io: PlacesIoOperationState,
    pub(crate) edit: PlacesEditOperationState,
    /// UI-only selection inside the places pane: (group_label, place_path).
    pub(crate) selected: Option<(String, PathBuf)>,
    pub(crate) inline_edit: PlacesInlineEditState,
}

/// Runtime state for the places import/export modal.
#[derive(Debug, Default)]
pub(crate) struct PlacesIoOperationState {
    pub(crate) mode: PlacesIoMode,
    pub(crate) buffer: String,
    pub(crate) open_next: bool,
    pub(crate) include_code: bool,
    pub(crate) error: Option<String>,
}

/// Runtime state for the places edit modal.
#[derive(Debug, Default)]
pub(crate) struct PlacesEditOperationState {
    pub(crate) mode: PlacesEditMode,
    pub(crate) open_next: bool,
    pub(crate) focus_next: bool,
    pub(crate) error: Option<String>,
    /// Target group label (add/edit place, rename/remove group).
    pub(crate) group: String,
    /// Source group label (rename/remove group).
    pub(crate) group_from: Option<String>,
    /// Source place path for editing (stable identity).
    pub(crate) place_from_path: Option<PathBuf>,
    /// Place label buffer (add/edit place).
    pub(crate) place_label: String,
    /// Place path buffer (add/edit place).
    pub(crate) place_path: String,
}

/// Runtime state for inline place-label editing.
#[derive(Debug, Default)]
pub(crate) struct PlacesInlineEditState {
    /// Inline edit (IGFD-like) target for place labels: (group_label, place_path).
    pub(crate) target: Option<(String, PathBuf)>,
    /// Inline edit buffer for the selected place label.
    pub(crate) buffer: String,
    /// Focus the inline edit input on next frame.
    pub(crate) focus_next: bool,
}

/// UI-only state for hosting a [`FileDialogCore`] in Dear ImGui.
///
/// This struct contains transient UI state (visibility, focus requests, text buffers) and owns the
/// caller-facing [`FileDialogUiConfig`]. It does not affect the core selection/navigation
/// semantics.
#[derive(Debug)]
pub struct FileDialogUiState {
    /// Whether to draw the dialog (show/hide). Prefer [`FileDialogState::open`]/[`FileDialogState::close`].
    pub visible: bool,
    /// Caller-facing UI configuration.
    pub config: FileDialogUiConfig,
    /// Transient runtime state owned by the UI renderer.
    pub(crate) runtime: FileDialogUiRuntime,
    /// Modal/operation state owned by the UI renderer.
    pub(crate) operations: FileDialogOperationState,
    /// Thumbnail cache (requests + LRU).
    pub thumbnails: ThumbnailCache,
}

impl Default for FileDialogUiState {
    fn default() -> Self {
        Self {
            visible: true,
            config: FileDialogUiConfig::default(),
            runtime: FileDialogUiRuntime::default(),
            operations: FileDialogOperationState::default(),
            thumbnails: ThumbnailCache::new(ThumbnailCacheConfig::default()),
        }
    }
}

impl FileDialogUiState {
    /// Applies an "IGFD classic" UI preset (opt-in).
    ///
    /// This tunes UI defaults to feel closer to ImGuiFileDialog (IGFD) while staying Rust-first:
    /// - standard layout with places pane,
    /// - IGFD-like single-row header layout,
    /// - list view as the default,
    /// - right-docked custom pane (when provided) with a splitter-resizable width,
    /// - dialog-style button row aligned to the right.
    pub fn apply_igfd_classic_preset(&mut self) {
        self.config.apply_igfd_classic_preset();
        self.runtime.path.input_mode = false;
        self.runtime.breadcrumb.scroll_to_end_next = true;
    }
}

/// Header layout style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HeaderStyle {
    /// Two-row layout: a top toolbar row plus a separate address/search row.
    #[default]
    ToolbarAndAddress,
    /// A single-row header that mimics ImGuiFileDialog's classic header layout.
    IgfdClassic,
}

/// Path bar style (text input vs breadcrumb composer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PathBarStyle {
    /// Always show an editable path text input ("address bar").
    #[default]
    TextInput,
    /// Show a breadcrumb-style path composer; edit mode can still be entered via Ctrl+L or context menu.
    Breadcrumbs,
}

/// Dock position for the custom pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CustomPaneDock {
    /// Dock the custom pane below the file list (default).
    #[default]
    Bottom,
    /// Dock the custom pane on the right side, similar to IGFD `sidePane`.
    Right,
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
        let mut core = FileDialogCore::new(mode);
        core.set_scan_policy(ScanPolicy::tuned_incremental());
        Self {
            core,
            ui: FileDialogUiState::default(),
        }
    }

    /// Opens (or reopens) the dialog.
    ///
    /// This mirrors IGFD's `OpenDialog` step before `Display`.
    pub fn open(&mut self) {
        self.ui.visible = true;
        self.ui.runtime.opened_cwd = Some(self.core.cwd.clone());
    }

    /// Reopens the dialog.
    ///
    /// Alias of [`FileDialogState::open`].
    pub fn reopen(&mut self) {
        self.open();
    }

    /// Closes the dialog.
    ///
    /// This mirrors IGFD's `Close` call.
    pub fn close(&mut self) {
        self.ui.visible = false;
    }

    /// Returns whether the dialog is currently open.
    pub fn is_open(&self) -> bool {
        self.ui.visible
    }

    /// Returns the active scan policy.
    pub fn scan_policy(&self) -> ScanPolicy {
        self.core.scan_policy()
    }

    /// Sets scan policy for future directory refreshes.
    pub fn set_scan_policy(&mut self, policy: ScanPolicy) {
        self.core.set_scan_policy(policy);
    }

    /// Returns the latest scan status from core.
    pub fn scan_status(&self) -> &ScanStatus {
        self.core.scan_status()
    }

    /// Requests a rescan on the next draw tick.
    pub fn request_rescan(&mut self) {
        self.core.request_rescan();
    }

    /// Installs a scan hook on the core listing pipeline.
    ///
    /// The hook runs during directory scan and can mutate or drop entries.
    pub fn set_scan_hook<F>(&mut self, hook: F)
    where
        F: FnMut(&mut crate::FsEntry) -> crate::ScanHookAction + 'static,
    {
        self.core.set_scan_hook(hook);
    }

    /// Clears the scan hook and restores raw filesystem listing.
    pub fn clear_scan_hook(&mut self) {
        self.core.clear_scan_hook();
    }

    /// Applies an "IGFD classic" preset for both UI and core.
    ///
    /// This is a convenience wrapper over [`FileDialogUiState::apply_igfd_classic_preset`] that
    /// also tunes core defaults to match typical IGFD behavior.
    pub fn apply_igfd_classic_preset(&mut self) {
        self.ui.apply_igfd_classic_preset();
        self.core.click_action = ClickAction::Navigate;
        self.core.sort_mode = crate::core::SortMode::Natural;
        self.core.sort_by = crate::core::SortBy::Name;
        self.core.sort_ascending = true;
        self.core.dirs_first = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn igfd_classic_preset_updates_ui_and_core() {
        let mut state = FileDialogState::new(DialogMode::OpenFile);
        state.apply_igfd_classic_preset();

        assert_eq!(state.ui.config.layout, LayoutStyle::Standard);
        assert_eq!(state.ui.config.file_list_view, FileListViewMode::List);
        assert_eq!(state.ui.config.custom_pane_dock, CustomPaneDock::Right);
        assert!(!state.ui.config.file_list_columns.show_extension);
        assert_eq!(
            state.ui.config.validation_buttons.align,
            ValidationButtonsAlign::Right
        );
        assert_eq!(
            state.ui.config.validation_buttons.order,
            ValidationButtonsOrder::CancelConfirm
        );
        assert_eq!(state.core.click_action, ClickAction::Navigate);
        assert_eq!(state.core.sort_mode, crate::core::SortMode::Natural);
    }

    #[test]
    fn open_close_roundtrip() {
        let mut state = FileDialogState::new(DialogMode::OpenFile);

        assert!(state.is_open());
        state.close();
        assert!(!state.is_open());

        state.open();
        assert!(state.is_open());

        state.close();
        assert!(!state.is_open());

        state.reopen();
        assert!(state.is_open());
    }

    #[test]
    fn default_scan_policy_is_tuned_incremental() {
        let state = FileDialogState::new(DialogMode::OpenFile);
        assert_eq!(state.scan_policy(), ScanPolicy::tuned_incremental());
    }

    #[test]
    fn ui_config_defaults_own_caller_facing_ui_knobs() {
        let state = FileDialogUiState::default();

        assert_eq!(state.config.header_style, HeaderStyle::ToolbarAndAddress);
        assert_eq!(state.config.layout, LayoutStyle::Standard);
        assert_eq!(state.config.file_list_view, FileListViewMode::default());
        assert_eq!(state.config.path_bar_style, PathBarStyle::TextInput);
        assert!(state.config.breadcrumbs_quick_select);
        assert_eq!(state.config.type_select_timeout, Duration::from_millis(750));
        assert!(!state.config.thumbnails_enabled);
        assert_eq!(state.config.thumbnail_size, [32.0, 32.0]);
    }

    #[test]
    fn ui_config_igfd_classic_preset_updates_config_without_runtime_buffers() {
        let mut state = FileDialogUiState::default();
        state.runtime.path.buffer = "keep-runtime-buffer".to_string();
        state.runtime.path.input_mode = true;

        state.apply_igfd_classic_preset();

        assert_eq!(state.config.header_style, HeaderStyle::IgfdClassic);
        assert_eq!(state.config.layout, LayoutStyle::Standard);
        assert_eq!(state.config.file_list_view, FileListViewMode::List);
        assert_eq!(state.config.toolbar.density, ToolbarDensity::Compact);
        assert_eq!(state.config.path_bar_style, PathBarStyle::Breadcrumbs);
        assert_eq!(state.config.custom_pane_dock, CustomPaneDock::Right);
        assert!(!state.config.file_list_columns.show_extension);
        assert_eq!(
            state.config.validation_buttons.align,
            ValidationButtonsAlign::Right
        );
        assert_eq!(state.runtime.path.buffer, "keep-runtime-buffer");
        assert!(!state.runtime.path.input_mode);
        assert!(state.runtime.breadcrumb.scroll_to_end_next);
    }

    #[test]
    fn ui_runtime_and_operation_state_are_internal_to_ui_state() {
        let state = FileDialogUiState::default();

        assert!(state.config.new_folder_enabled);
        assert!(state.config.type_select_enabled);
        assert!(state.runtime.type_select_buffer.is_empty());
        assert!(state.runtime.type_select_last_input.is_none());
        assert!(!state.operations.new_folder.inline_active);
        assert!(!state.operations.new_folder.open_next);
        assert!(state.operations.new_folder.name.is_empty());
        assert!(!state.operations.new_folder.focus_next);
        assert!(state.operations.new_folder.error.is_none());
        assert!(!state.runtime.path.input_mode);
        assert!(!state.runtime.path.edit);
        assert!(state.runtime.path.buffer.is_empty());
        assert!(state.runtime.path.history_index.is_none());
        assert!(state.runtime.path.history_saved_buffer.is_none());
        assert!(!state.runtime.focus_search_next);
        assert!(state.runtime.error.is_none());
        assert!(state.runtime.breadcrumb.quick_parent.is_none());
        assert_eq!(state.runtime.footer.height_last, 0.0);
        assert!(state.runtime.footer.file_name_buffer.is_empty());
        assert!(state.operations.rename.target_id.is_none());
        assert!(!state.operations.rename.open_next);
        assert!(state.operations.rename.to.is_empty());
        assert!(state.operations.delete.target_ids.is_empty());
        assert!(!state.operations.delete.open_next);
        assert!(state.operations.paste.clipboard.is_none());
        assert!(state.operations.paste.job.is_none());
        assert!(!state.operations.paste.conflict_open_next);
        assert!(state.operations.reveal_id_next.is_none());
        assert!(state.operations.places.io.buffer.is_empty());
        assert!(state.operations.places.selected.is_none());
        assert!(state.operations.places.inline_edit.target.is_none());
    }

    #[test]
    fn file_list_columns_compact_roundtrip() {
        let cfg = FileListColumnsConfig {
            show_preview: false,
            show_extension: true,
            show_size: true,
            show_modified: false,
            order: [
                FileListDataColumn::Name,
                FileListDataColumn::Size,
                FileListDataColumn::Modified,
                FileListDataColumn::Extension,
            ],
            weight_overrides: FileListColumnWeightOverrides {
                preview: Some(0.15),
                name: Some(0.61),
                extension: Some(0.1),
                size: Some(0.17),
                modified: None,
            },
        };

        let encoded = cfg.serialize_compact();
        let decoded = FileListColumnsConfig::deserialize_compact(&encoded).unwrap();
        assert_eq!(decoded, cfg);
    }

    #[test]
    fn file_list_columns_deserialize_rejects_duplicate_order_entries() {
        let err = FileListColumnsConfig::deserialize_compact(
            "v1;preview=1;ext=1;size=1;modified=1;order=name,name,size,modified;weights=auto,auto,auto,auto,auto",
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("order` must contain each column exactly once")
        );
    }

    #[test]
    fn file_list_columns_deserialize_rejects_non_positive_weight() {
        let err = FileListColumnsConfig::deserialize_compact(
            "v1;preview=1;ext=1;size=1;modified=1;order=name,ext,size,modified;weights=auto,0,auto,auto,auto",
        )
        .unwrap_err();
        assert!(err.to_string().contains("weight must be finite and > 0"));
    }

    #[test]
    fn file_list_columns_normalized_order_dedupes_and_fills_missing() {
        let normalized = normalized_order([
            FileListDataColumn::Name,
            FileListDataColumn::Name,
            FileListDataColumn::Modified,
            FileListDataColumn::Modified,
        ]);
        assert_eq!(
            normalized,
            [
                FileListDataColumn::Name,
                FileListDataColumn::Modified,
                FileListDataColumn::Extension,
                FileListDataColumn::Size,
            ]
        );
    }
}
