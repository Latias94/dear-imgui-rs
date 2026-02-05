use std::collections::HashMap;

use dear_imgui_rs::Ui;

use crate::browser_state::FileBrowserState;
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
#[derive(Debug, Default)]
pub struct DialogManager {
    next_id: u64,
    browsers: HashMap<DialogId, FileBrowserState>,
}

impl DialogManager {
    /// Create a new manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a new in-UI file browser dialog with a default state.
    pub fn open_browser(&mut self, mode: crate::DialogMode) -> DialogId {
        self.open_browser_with_state(FileBrowserState::new(mode))
    }

    /// Open a new in-UI file browser dialog with a fully configured state.
    pub fn open_browser_with_state(&mut self, state: FileBrowserState) -> DialogId {
        self.next_id = self.next_id.wrapping_add(1);
        let id = DialogId(self.next_id);
        self.browsers.insert(id, state);
        id
    }

    /// Close an open dialog and return its state (if any).
    pub fn close(&mut self, id: DialogId) -> Option<FileBrowserState> {
        self.browsers.remove(&id)
    }

    /// Returns `true` if the dialog exists in the manager.
    pub fn contains(&self, id: DialogId) -> bool {
        self.browsers.contains_key(&id)
    }

    /// Get immutable access to a dialog state.
    pub fn browser_state(&self, id: DialogId) -> Option<&FileBrowserState> {
        self.browsers.get(&id)
    }

    /// Get mutable access to a dialog state (to tweak filters/layout/etc).
    pub fn browser_state_mut(&mut self, id: DialogId) -> Option<&mut FileBrowserState> {
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
        let res = ui.file_browser().show(state);
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
        let res = ui.file_browser().show_windowed(state, cfg);
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
        let res = ui.file_browser().draw_contents(state);
        if res.is_some() {
            self.browsers.remove(&id);
        }
        res
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

        let s1 = mgr.close(id1).unwrap();
        assert_eq!(s1.mode, DialogMode::OpenFile);
        assert!(!mgr.contains(id1));
        assert!(mgr.contains(id2));
    }
}
