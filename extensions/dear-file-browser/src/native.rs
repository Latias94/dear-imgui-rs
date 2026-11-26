//! Native (rfd) backend.
//!
//! This module implements the OS-native file dialogs via the `rfd` crate.
//! This path-based adapter supports desktop platforms. Browser `rfd` handles
//! have no host paths, so the adapter is unavailable on `wasm32`. Both blocking
//! and async desktop flows are exposed via the
//! `FileDialog` builder:
//!
//! - `open_blocking()` opens a modal, OS-native dialog and returns on close.
//! - `open_async()` awaits the desktop selection.
//!
//! Notes
//! - Filters map to `rfd::FileDialog::add_filter` and accept lowercase
//!   extensions without dots (e.g. "png").
//! - When `start_dir` is provided it is forwarded to `rfd`.
//! - On the Web (wasm32), the ImGui in-UI browser cannot enumerate the local
//!   filesystem. Use `rfd::AsyncFileDialog` directly and retain its file handle,
//!   or supply an application virtual-filesystem adapter to the in-UI browser.
use crate::core::{DialogMode, FileDialog, FileDialogError, Selection};

#[cfg(feature = "tracing")]
use tracing::trace;

impl FileDialog {
    fn to_rfd(&self) -> rfd::FileDialog {
        let mut d = rfd::FileDialog::new();
        if let Some(dir) = &self.start_dir {
            d = d.set_directory(dir);
        }
        if let Some(name) = &self.default_name {
            d = d.set_file_name(name);
        }
        for f in &self.filters {
            let extensions: Vec<String> = f
                .extensions
                .iter()
                .filter_map(|s| plain_extension_for_native(s))
                .collect();
            if !extensions.is_empty() {
                d = d.add_filter(&f.name, &extensions);
            }
        }
        d
    }

    fn to_rfd_async(&self) -> rfd::AsyncFileDialog {
        let mut a = rfd::AsyncFileDialog::new();
        if let Some(dir) = self.start_dir.as_deref() {
            a = a.set_directory(dir);
        }
        if let Some(name) = self.default_name.as_deref() {
            a = a.set_file_name(name);
        }
        for f in &self.filters {
            let exts_owned: Vec<String> = f
                .extensions
                .iter()
                .filter_map(|s| plain_extension_for_native(s))
                .collect();
            if !exts_owned.is_empty() {
                a = a.add_filter(&f.name, &exts_owned);
            }
        }
        a
    }

    /// Open a dialog synchronously (blocking).
    pub fn open_blocking(self) -> Result<Selection, FileDialogError> {
        self.require_native_backend()?;
        for filter in &self.filters {
            filter.validate()?;
        }
        #[cfg(feature = "tracing")]
        trace!(?self.mode, "rfd blocking open");
        let mut sel = Selection::default();
        match self.mode {
            DialogMode::OpenFile => {
                if let Some(p) = self.to_rfd().pick_file() {
                    sel.paths.push(p);
                }
            }
            DialogMode::OpenFiles => {
                if !self.allow_multi {
                    if let Some(p) = self.to_rfd().pick_file() {
                        sel.paths.push(p);
                    }
                } else if let Some(v) = self.to_rfd().pick_files() {
                    sel.paths.extend(v);
                }
            }
            DialogMode::PickFolder => {
                if let Some(p) = self.to_rfd().pick_folder() {
                    sel.paths.push(p);
                }
            }
            DialogMode::SaveFile => {
                if let Some(p) = self.to_rfd().save_file() {
                    sel.paths.push(p);
                }
            }
        }
        if let Some(max) = self.max_selection.filter(|&m| m > 0) {
            sel.paths.truncate(max);
        }
        if sel.paths.is_empty() {
            Err(FileDialogError::Cancelled)
        } else {
            Ok(sel)
        }
    }

    /// Open a native dialog asynchronously. An explicit ImGui backend returns
    /// [`FileDialogError::ImGuiBackendRequiresUi`], just as [`Self::open_blocking`] does.
    pub async fn open_async(self) -> Result<Selection, FileDialogError> {
        self.require_native_backend()?;
        for filter in &self.filters {
            filter.validate()?;
        }
        #[cfg(feature = "tracing")]
        trace!(?self.mode, "rfd async open");
        let mut sel = Selection::default();
        match self.mode {
            DialogMode::OpenFile => {
                let a = self.to_rfd_async();
                let f = a.pick_file().await;
                if let Some(h) = f {
                    sel.paths.push(h.path().to_path_buf());
                }
            }
            DialogMode::OpenFiles => {
                let a = self.to_rfd_async();
                if !self.allow_multi {
                    let f = a.pick_file().await;
                    if let Some(h) = f {
                        sel.paths.push(h.path().to_path_buf());
                    }
                } else {
                    let v = a.pick_files().await;
                    if let Some(v) = v {
                        sel.paths
                            .extend(v.into_iter().map(|h| h.path().to_path_buf()));
                    }
                }
            }
            DialogMode::PickFolder => {
                let f = self.to_rfd_async().pick_folder().await;
                if let Some(h) = f {
                    sel.paths.push(h.path().to_path_buf());
                }
            }
            DialogMode::SaveFile => {
                let a = self.to_rfd_async();
                let f = a.save_file().await;
                if let Some(h) = f {
                    sel.paths.push(h.path().to_path_buf());
                }
            }
        }
        if let Some(max) = self.max_selection.filter(|&m| m > 0) {
            sel.paths.truncate(max);
        }
        if sel.paths.is_empty() {
            Err(FileDialogError::Cancelled)
        } else {
            Ok(sel)
        }
    }
}

fn is_plain_extension_token(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with("((") && t.ends_with("))") {
        return false;
    }
    !(t.contains('*') || t.contains('?'))
}

fn plain_extension_for_native(token: &str) -> Option<String> {
    if !is_plain_extension_token(token) {
        return None;
    }
    let t = token.trim().trim_start_matches('.');
    if t.is_empty() {
        return None;
    }
    Some(t.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_programmatic_regex_rejects_native_open_before_showing_a_dialog() {
        let dialog = FileDialog::new(DialogMode::OpenFile).filter(("Invalid", &["(([))"][..]));
        assert!(matches!(
            dialog.clone().open_blocking(),
            Err(FileDialogError::InvalidFilter(_))
        ));
        assert!(matches!(
            pollster::block_on(dialog.open_async()),
            Err(FileDialogError::InvalidFilter(_))
        ));
    }
}
