//! Native (rfd) backend.
//!
//! This module implements the OS-native file dialogs via the `rfd` crate.
//! On desktop platforms it opens the system dialog; on `wasm32` targets it
//! uses the Web File Picker. Both blocking and async flows are exposed via the
//! `FileDialog` builder:
//!
//! - `open_blocking()` opens a modal, OS-native dialog and returns on close.
//! - `open_async()` awaits the selection (desktop and wasm32 supported).
//!
//! Notes
//! - Filters map to `rfd::FileDialog::add_filter` and accept lowercase
//!   extensions without dots (e.g. "png").
//! - When `start_dir` is provided it is forwarded to `rfd`.
//! - On the Web (wasm32), the ImGui in-UI browser cannot enumerate the local
//!   filesystem – prefer the native backend to access user files.
use crate::core::{Backend, DialogMode, FileDialog, FileDialogError, Selection};

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
            let exts_owned: Vec<String> = f
                .extensions
                .iter()
                .filter_map(|s| plain_extension_for_native(s))
                .collect();
            let exts: Vec<&str> = exts_owned.iter().map(|s| s.as_str()).collect();
            if !exts.is_empty() {
                d = d.add_filter(&f.name, &exts);
            }
        }
        d
    }

    /// Open a dialog synchronously (blocking).
    pub fn open_blocking(self) -> Result<Selection, FileDialogError> {
        match self.effective_backend() {
            Backend::Native => self.open_blocking_native(),
            Backend::ImGui => Err(FileDialogError::Unsupported),
            Backend::Auto => unreachable!("resolved in effective_backend"),
        }
    }

    fn open_blocking_native(self) -> Result<Selection, FileDialogError> {
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
                if let Some(v) = self.to_rfd().pick_files() {
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
        if sel.paths.is_empty() {
            Err(FileDialogError::Cancelled)
        } else {
            Ok(sel)
        }
    }

    /// Open a dialog asynchronously via `rfd::AsyncFileDialog`.
    pub async fn open_async(self) -> Result<Selection, FileDialogError> {
        use rfd::AsyncFileDialog as A;
        #[cfg(feature = "tracing")]
        trace!(?self.mode, "rfd async open");
        let mut sel = Selection::default();
        match self.mode {
            DialogMode::OpenFile => {
                let mut a = A::new();
                if let Some(dir) = self.start_dir.as_deref() {
                    a = a.set_directory(dir);
                }
                if let Some(name) = self.default_name.as_deref() {
                    a = a.set_file_name(name);
                }
                let f = a.pick_file().await;
                if let Some(h) = f {
                    sel.paths.push(h.path().to_path_buf());
                }
            }
            DialogMode::OpenFiles => {
                let mut a = A::new();
                if let Some(dir) = self.start_dir.as_deref() {
                    a = a.set_directory(dir);
                }
                if let Some(name) = self.default_name.as_deref() {
                    a = a.set_file_name(name);
                }
                let v = a.pick_files().await;
                if let Some(v) = v {
                    sel.paths
                        .extend(v.into_iter().map(|h| h.path().to_path_buf()));
                }
            }
            DialogMode::PickFolder => {
                let mut a = A::new();
                if let Some(dir) = self.start_dir.as_deref() {
                    a = a.set_directory(dir);
                }
                let f = a.pick_folder().await;
                if let Some(h) = f {
                    sel.paths.push(h.path().to_path_buf());
                }
            }
            DialogMode::SaveFile => {
                let mut a = A::new();
                if let Some(dir) = self.start_dir.as_deref() {
                    a = a.set_directory(dir);
                }
                if let Some(name) = self.default_name.as_deref() {
                    a = a.set_file_name(name);
                }
                let f = a.save_file().await;
                if let Some(h) = f {
                    sel.paths.push(h.path().to_path_buf());
                }
            }
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
