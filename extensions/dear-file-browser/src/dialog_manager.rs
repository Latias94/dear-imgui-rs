use std::collections::HashMap;

use dear_imgui_rs::Ui;

use crate::dialog_state::FileDialogState;
use crate::fs::{FileSystem, StdFileSystem};
use crate::ui::{FileDialogExt, WindowHostConfig};
use crate::{FileDialogError, Selection};

/// Opaque identifier for an in-UI file browser dialog instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DialogId(u64);

/// Manager for multiple in-UI file browser dialogs (IGFD-style open/display separation).
///
/// This is an incremental step toward IGFD-grade behavior:
/// - Multiple dialogs can exist concurrently (each keyed by a `DialogId`).
/// - The caller opens a dialog (`open_browser*`) and later drives rendering per-frame via
///   `show_*` / `draw_*`.
pub struct DialogManager {
    next_id: u64,
    browsers: HashMap<DialogId, FileDialogState>,
    fs: Box<dyn FileSystem>,
}

impl DialogManager {
    /// Create a new manager.
    pub fn new() -> Self {
        Self::with_fs(Box::new(StdFileSystem))
    }

    /// Create a new manager using a custom filesystem.
    pub fn with_fs(fs: Box<dyn FileSystem>) -> Self {
        Self {
            next_id: 0,
            browsers: HashMap::new(),
            fs,
        }
    }

    /// Replace the manager filesystem.
    pub fn set_fs(&mut self, fs: Box<dyn FileSystem>) {
        self.fs = fs;
    }

    /// Get a shared reference to the manager filesystem.
    pub fn fs(&self) -> &dyn FileSystem {
        self.fs.as_ref()
    }

    /// Open a new in-UI file browser dialog with a default state.
    pub fn open_browser(&mut self, mode: crate::DialogMode) -> DialogId {
        self.open_browser_with_state(FileDialogState::new(mode))
    }

    /// Open a new in-UI file browser dialog with a fully configured state.
    pub fn open_browser_with_state(&mut self, state: FileDialogState) -> DialogId {
        let mut state = state;
        // `open_browser*` mirrors IGFD's `OpenDialog` step: the returned dialog is immediately
        // visible and ready to be displayed via `show_*` / `draw_*`.
        state.open();
        self.next_id = self.next_id.wrapping_add(1);
        let id = DialogId(self.next_id);
        self.browsers.insert(id, state);
        id
    }

    /// Close an open dialog and return its state (if any).
    pub fn close(&mut self, id: DialogId) -> Option<FileDialogState> {
        self.browsers.remove(&id)
    }

    /// Returns `true` if the dialog exists in the manager.
    pub fn contains(&self, id: DialogId) -> bool {
        self.browsers.contains_key(&id)
    }

    /// Get immutable access to a dialog state.
    pub fn dialog_state(&self, id: DialogId) -> Option<&FileDialogState> {
        self.browsers.get(&id)
    }

    /// Get mutable access to a dialog state (to tweak filters/layout/etc).
    pub fn dialog_state_mut(&mut self, id: DialogId) -> Option<&mut FileDialogState> {
        self.browsers.get_mut(&id)
    }

    /// Draw a dialog hosted in its own ImGui window (default host config).
    ///
    /// If a result is produced (confirm/cancel), the dialog is removed from the manager and the
    /// result is returned.
    pub fn show_browser(
        &mut self,
        ui: &Ui,
        id: DialogId,
    ) -> Option<Result<Selection, FileDialogError>> {
        let state = self.browsers.get_mut(&id)?;
        let cfg = WindowHostConfig::for_mode(state.core.mode);
        let res = ui
            .file_browser()
            .show_windowed_with(state, &cfg, self.fs.as_ref(), None, None);
        if res.is_some() {
            self.browsers.remove(&id);
        }
        res
    }

    /// Draw a dialog hosted in an ImGui window using custom window configuration.
    ///
    /// If a result is produced (confirm/cancel), the dialog is removed from the manager and the
    /// result is returned.
    pub fn show_browser_windowed(
        &mut self,
        ui: &Ui,
        id: DialogId,
        cfg: &WindowHostConfig,
    ) -> Option<Result<Selection, FileDialogError>> {
        let state = self.browsers.get_mut(&id)?;
        let res = ui
            .file_browser()
            .show_windowed_with(state, cfg, self.fs.as_ref(), None, None);
        if res.is_some() {
            self.browsers.remove(&id);
        }
        res
    }

    /// Draw only the dialog contents (no host window) for embedding.
    ///
    /// If a result is produced (confirm/cancel), the dialog is removed from the manager and the
    /// result is returned.
    pub fn draw_browser_contents(
        &mut self,
        ui: &Ui,
        id: DialogId,
    ) -> Option<Result<Selection, FileDialogError>> {
        let state = self.browsers.get_mut(&id)?;
        let res = ui
            .file_browser()
            .draw_contents_with(state, self.fs.as_ref(), None, None);
        if res.is_some() {
            self.browsers.remove(&id);
        }
        res
    }
}

impl std::fmt::Debug for DialogManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialogManager")
            .field("next_id", &self.next_id)
            .field("browsers_len", &self.browsers.len())
            .finish()
    }
}

impl Default for DialogManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DialogMode;

    #[test]
    fn open_close_roundtrip() {
        let mut mgr = DialogManager::new();
        let id1 = mgr.open_browser(DialogMode::OpenFile);
        let id2 = mgr.open_browser(DialogMode::SaveFile);
        assert_ne!(id1, id2);

        assert!(mgr.contains(id1));
        assert!(mgr.contains(id2));

        assert!(mgr.dialog_state(id1).unwrap().is_open());
        assert!(mgr.dialog_state(id2).unwrap().is_open());

        let s1 = mgr.close(id1).unwrap();
        assert_eq!(s1.core.mode, DialogMode::OpenFile);
        assert!(!mgr.contains(id1));
        assert!(mgr.contains(id2));
    }
}
