use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;

use crate::core::{DialogMode, FileFilter};
use crate::dialog_core::{ConfirmGate, EntryId};

/// Context passed to a [`CustomPane`] while rendering.
///
/// This is a read-only view of the dialog state relevant for building
/// IGFD-style extra widgets (per-filter options, validation, etc).
#[derive(Clone, Copy, Debug)]
pub struct CustomPaneCtx<'a> {
    /// Dialog mode.
    pub mode: DialogMode,
    /// Current working directory.
    pub cwd: &'a Path,
    /// Selected entry IDs in deterministic selection order.
    pub selected_entry_ids: &'a [EntryId],
    /// Selected entry paths in deterministic selection order.
    pub selected_paths: &'a [PathBuf],
    /// Number of selected files in the current snapshot.
    pub selected_files_count: usize,
    /// Number of selected directories in the current snapshot.
    pub selected_dirs_count: usize,
    /// Current save filename buffer (SaveFile mode).
    pub save_name: &'a str,
    /// Active filter (None = "All files").
    pub active_filter: Option<&'a FileFilter>,
}

/// IGFD-style custom pane that can draw extra UI and optionally block confirmation.
///
/// The pane is rendered inside the file dialog (typically below the file list).
/// It returns a [`ConfirmGate`] each frame which is used to enable/disable the
/// confirm action (button, Enter key, double-click confirm, etc).
pub trait CustomPane {
    /// Draw the pane contents and return the current confirm gate.
    fn draw(&mut self, ui: &Ui, ctx: CustomPaneCtx<'_>) -> ConfirmGate;
}
