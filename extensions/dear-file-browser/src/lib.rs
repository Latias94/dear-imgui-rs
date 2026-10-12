#![deny(missing_docs)]
//! File dialogs and in-UI file browser for `dear-imgui-rs`.
//!
//! Note on UTF-8/CJK: Dear ImGui's default font does not include CJK glyphs.
//! If you need Chinese/Japanese/Korean or emoji, load a font that contains
//! those glyphs (e.g., Noto Sans SC) into the font atlas during initialization.
//! For best quality, enable the `freetype` feature and follow the pattern in
//! `examples/03-features/style_and_fonts.rs` to add fonts and ranges (e.g., Simplified
//! Chinese common glyphs).
//!
//! Two backends:
//! - Native (via `rfd`) for desktop OS dialogs; the path-based adapter is desktop-only.
//! - ImGui (pure-UI) browser, using an application-provided virtual filesystem on WASM.
//!
//! On WASM, disable default features and enable `imgui`; do not enable `native-rfd`.
//! Browser pickers need `rfd::AsyncFileDialog` directly because their file handles
//! do not expose host paths that can be stored in this crate's [`Selection`].

mod core;
#[cfg(feature = "imgui")]
mod custom_pane;
#[cfg(feature = "imgui")]
mod dialog_core;
#[cfg(feature = "imgui")]
mod dialog_manager;
#[cfg(feature = "imgui")]
mod dialog_state;
#[cfg(feature = "imgui")]
mod file_style;
#[cfg(feature = "imgui")]
mod fs;
#[cfg(feature = "imgui")]
mod fs_ops;
#[cfg(feature = "native-rfd")]
mod native;
#[cfg(feature = "imgui")]
mod places;
#[cfg(feature = "imgui")]
mod scan;
#[cfg(feature = "imgui")]
mod thumbnails;
#[cfg(feature = "thumbnails-image")]
mod thumbnails_image;
#[cfg(feature = "imgui")]
mod ui;

pub use core::{
    Backend, ClickAction, DialogMode, ExtensionPolicy, FileDialog, FileDialogError, FileFilter,
    LayoutStyle, SavePolicy, Selection, SortBy, SortMode,
};
#[cfg(feature = "imgui")]
pub use custom_pane::{CustomPane, CustomPaneCtx};
#[cfg(feature = "imgui")]
pub use dialog_core::{
    ConfirmGate, CoreEvent, CoreEventOutcome, DirSnapshot, EntryId, FileDialogCore, FileMeta,
    Modifiers, ScanGeneration, ScanHookAction, ScanPolicy, ScanStatus,
};
#[cfg(feature = "imgui")]
pub use dialog_manager::{DialogId, DialogManager};
#[cfg(feature = "imgui")]
pub use dialog_state::FileListViewMode;
#[cfg(feature = "imgui")]
pub use dialog_state::{
    ClipboardOp, CustomPaneDock, FileClipboard, FileDialogState, FileDialogUiConfig,
    FileDialogUiState, FileListColumnWeightOverrides, FileListColumnsConfig,
    FileListColumnsDeserializeError, FileListDataColumn, HeaderStyle, PathBarStyle, ToolbarConfig,
    ToolbarDensity, ToolbarIconMode, ToolbarIcons, ValidationButtonsAlign, ValidationButtonsConfig,
    ValidationButtonsOrder,
};
#[cfg(feature = "imgui")]
pub use file_style::FileStyleCallback;
#[cfg(feature = "imgui")]
pub use file_style::{EntryKind, FileStyle, FileStyleRegistry, StyleMatcher, StyleRule};
#[cfg(feature = "imgui")]
pub use fs::{FileSystem, FsEntry, FsMetadata, ScanVisit, StdFileSystem};
#[cfg(feature = "imgui")]
pub use places::{
    Place, PlaceGroup, PlaceOrigin, Places, PlacesDeserializeError, PlacesMergeOptions,
    PlacesSerializeOptions,
};
#[cfg(feature = "imgui")]
pub use thumbnails::{
    DecodedRgbaImage, ThumbnailBackend, ThumbnailCache, ThumbnailCacheConfig, ThumbnailFrameIndex,
    ThumbnailProvider, ThumbnailRenderer, ThumbnailRequest,
};
#[cfg(feature = "thumbnails-image")]
pub use thumbnails_image::ImageThumbnailProvider;
#[cfg(feature = "imgui")]
pub use ui::{FileDialogExt, ModalHostConfig, WindowHostConfig};
