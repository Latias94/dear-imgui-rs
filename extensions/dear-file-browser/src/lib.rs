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
#[cfg(feature = "native-rfd")]
mod native;
#[cfg(feature = "imgui")]
mod ui;

pub use core::{
    Backend, ClickAction, DialogMode, FileDialog, FileDialogError, FileFilter, LayoutStyle,
    Selection, SortBy,
};
#[cfg(feature = "imgui")]
pub use ui::{FileBrowserState, FileDialogExt};
