#![deny(missing_docs)]
//! File dialogs and in-UI file browser for `dear-imgui-rs`.
//!
//! Note on UTF-8/CJK: Dear ImGui's default font does not include CJK glyphs.
//! If you need Chinese/Japanese/Korean or emoji, load a font that contains
//! those glyphs (e.g., Noto Sans SC) into the font atlas during initialization.
//! For best quality, enable the `freetype` feature and follow the pattern in
//! `examples/style_and_fonts.rs` to add fonts and ranges (e.g., Simplified
//! Chinese common glyphs).
//!
//! Two backends:
//! - Native (via `rfd`) for OS dialogs (desktop) and Web File Picker (wasm)
//! - ImGui (pure-UI) browser that works everywhere and is fully themeable

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
#[cfg(feature = "native-rfd")]
mod native;
#[cfg(feature = "imgui")]
mod places;
#[cfg(feature = "imgui")]
mod thumbnails;
#[cfg(feature = "thumbnails-image")]
mod thumbnails_image;
#[cfg(feature = "imgui")]
mod ui;

pub use core::{
    Backend, ClickAction, DialogMode, ExtensionPolicy, FileDialog, FileDialogError, FileFilter,
    LayoutStyle, SavePolicy, Selection, SortBy,
};
#[cfg(feature = "imgui")]
pub use custom_pane::{CustomPane, CustomPaneCtx};
#[cfg(feature = "imgui")]
pub use dialog_core::{ConfirmGate, FileDialogCore, Modifiers};
#[cfg(feature = "imgui")]
pub use dialog_manager::{DialogId, DialogManager};
#[cfg(feature = "imgui")]
pub use dialog_state::FileListViewMode;
#[cfg(feature = "imgui")]
pub use dialog_state::{FileDialogState, FileDialogUiState};
#[cfg(feature = "imgui")]
pub use file_style::{EntryKind, FileStyle, FileStyleRegistry, StyleMatcher, StyleRule};
#[cfg(feature = "imgui")]
pub use fs::{FileSystem, FsEntry, FsMetadata, StdFileSystem};
#[cfg(feature = "imgui")]
pub use places::{
    Place, PlaceGroup, PlaceOrigin, Places, PlacesDeserializeError, PlacesSerializeOptions,
};
#[cfg(feature = "imgui")]
pub use thumbnails::{
    DecodedRgbaImage, ThumbnailBackend, ThumbnailCache, ThumbnailCacheConfig, ThumbnailProvider,
    ThumbnailRenderer, ThumbnailRequest,
};
#[cfg(feature = "thumbnails-image")]
pub use thumbnails_image::ImageThumbnailProvider;
#[cfg(feature = "imgui")]
pub use ui::{FileDialogExt, WindowHostConfig};
