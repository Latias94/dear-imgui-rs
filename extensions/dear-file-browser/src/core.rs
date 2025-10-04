use std::path::PathBuf;
use thiserror::Error;

/// Dialog mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogMode {
    /// Pick a single file
    OpenFile,
    /// Pick multiple files
    OpenFiles,
    /// Pick a directory
    PickFolder,
    /// Save file
    SaveFile,
}

/// Backend preference
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// Prefer native when available, fallback to ImGui
    Auto,
    /// Force native (rfd) backend
    Native,
    /// Force ImGui UI backend
    ImGui,
}

impl Default for Backend {
    fn default() -> Self {
        Backend::Auto
    }
}

/// File filter (e.g., "Images" -> ["png", "jpg"]).
///
/// Extensions are matched case-insensitively and should be provided without a
/// leading dot. The variants created from tuples will be normalized to
/// lowercase automatically.
#[derive(Clone, Debug, Default)]
pub struct FileFilter {
    /// Filter display name
    pub name: String,
    /// Lower-case extensions without dot (e.g., "png")
    pub extensions: Vec<String>,
}

impl FileFilter {
    /// Create a filter from a name and extensions.
    ///
    /// Extensions should be provided without dots (e.g. "png"). Matching is
    /// case-insensitive.
    pub fn new(name: impl Into<String>, exts: impl Into<Vec<String>>) -> Self {
        Self {
            name: name.into(),
            extensions: exts.into(),
        }
    }
}

impl From<(&str, &[&str])> for FileFilter {
    fn from(value: (&str, &[&str])) -> Self {
        Self {
            name: value.0.to_owned(),
            extensions: value.1.iter().map(|s| s.to_lowercase()).collect(),
        }
    }
}

/// Selection result containing one or more paths
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// Selected filesystem paths
    pub paths: Vec<PathBuf>,
}

/// Errors returned by file dialogs and in-UI browser
#[derive(Error, Debug)]
pub enum FileDialogError {
    /// User cancelled the dialog / browser
    #[error("cancelled")]
    Cancelled,
    /// I/O error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Requested operation unsupported by the chosen backend
    #[error("unsupported operation for backend")]
    Unsupported,
    /// Invalid or non-existing path requested
    #[error("invalid path: {0}")]
    InvalidPath(String),
    /// Platform-specific error or general failure
    #[error("internal error: {0}")]
    Internal(String),
}

/// Click behavior for directory rows
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ClickAction {
    /// Clicking a directory only selects it
    Select,
    /// Clicking a directory navigates into it
    Navigate,
}

/// Layout style for the in-UI file browser
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LayoutStyle {
    /// Standard layout with quick locations + file list
    Standard,
    /// Minimal layout with a single file list pane
    Minimal,
}

/// Sort keys for file list
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SortBy {
    /// Sort by file or directory name
    Name,
    /// Sort by file size (directories first)
    Size,
    /// Sort by last modified time
    Modified,
}

/// Builder for launching file dialogs
#[derive(Clone, Debug)]
pub struct FileDialog {
    pub(crate) backend: Backend,
    pub(crate) mode: DialogMode,
    pub(crate) start_dir: Option<PathBuf>,
    pub(crate) default_name: Option<String>,
    pub(crate) allow_multi: bool,
    pub(crate) filters: Vec<FileFilter>,
    pub(crate) show_hidden: bool,
}

impl FileDialog {
    /// Create a new builder with the given mode
    pub fn new(mode: DialogMode) -> Self {
        Self {
            backend: Backend::Auto,
            mode,
            start_dir: None,
            default_name: None,
            allow_multi: matches!(mode, DialogMode::OpenFiles),
            filters: Vec::new(),
            show_hidden: false,
        }
    }

    /// Choose a backend (Auto by default)
    pub fn backend(mut self, backend: Backend) -> Self {
        self.backend = backend;
        self
    }
    /// Set initial directory
    pub fn directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.start_dir = Some(dir.into());
        self
    }
    /// Set default file name (for SaveFile)
    pub fn default_file_name(mut self, name: impl Into<String>) -> Self {
        self.default_name = Some(name.into());
        self
    }
    /// Allow multi selection (only for OpenFiles)
    pub fn multi_select(mut self, yes: bool) -> Self {
        self.allow_multi = yes;
        self
    }
    /// Show hidden files in ImGui browser (native follows OS behavior)
    pub fn show_hidden(mut self, yes: bool) -> Self {
        self.show_hidden = yes;
        self
    }
    /// Add a filter.
    ///
    /// Examples
    /// ```
    /// use dear_file_browser::{FileDialog, DialogMode};
    /// let d = FileDialog::new(DialogMode::OpenFile)
    ///     .filter(("Images", &["png", "jpg"]))
    ///     .filter(("Rust", &["rs"]))
    ///     .show_hidden(true);
    /// ```
    pub fn filter<F: Into<FileFilter>>(mut self, filter: F) -> Self {
        self.filters.push(filter.into());
        self
    }
    /// Add multiple filters.
    ///
    /// The list will be appended to any previously-added filters. Extensions
    /// are compared case-insensitively and should be provided without dots.
    ///
    /// Examples
    /// ```
    /// use dear_file_browser::{FileDialog, DialogMode, FileFilter};
    /// let filters = vec![
    ///     FileFilter::from(("Images", &["png", "jpg", "jpeg"]))
    /// ];
    /// let d = FileDialog::new(DialogMode::OpenFiles)
    ///     .filters(filters)
    ///     .multi_select(true);
    /// ```
    pub fn filters<I, F>(mut self, filters: I) -> Self
    where
        I: IntoIterator<Item = F>,
        F: Into<FileFilter>,
    {
        self.filters.extend(filters.into_iter().map(Into::into));
        self
    }

    /// Resolve the effective backend
    pub(crate) fn effective_backend(&self) -> Backend {
        match self.backend {
            Backend::Native => Backend::Native,
            Backend::ImGui => Backend::ImGui,
            Backend::Auto => {
                #[cfg(feature = "native-rfd")]
                {
                    return Backend::Native;
                }
                #[cfg(not(feature = "native-rfd"))]
                {
                    return Backend::ImGui;
                }
            }
        }
    }
}

// Default stubs when native feature is disabled
#[cfg(not(feature = "native-rfd"))]
impl FileDialog {
    /// Open a dialog synchronously (blocking). Unsupported without `native-rfd`.
    pub fn open_blocking(self) -> Result<Selection, FileDialogError> {
        Err(FileDialogError::Unsupported)
    }
    /// Open a dialog asynchronously. Unsupported without `native-rfd`.
    pub async fn open_async(self) -> Result<Selection, FileDialogError> {
        Err(FileDialogError::Unsupported)
    }
}
