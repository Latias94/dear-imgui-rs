use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors returned when parsing IGFD-style filter strings.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("invalid IGFD filter spec: {message}")]
pub struct IgfdFilterParseError {
    message: String,
}

impl IgfdFilterParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

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
/// leading dot. Multi-layer extensions are supported by including dots in the
/// extension string (e.g. `"vcxproj.filters"`).
///
/// Advanced patterns (ImGui backend only):
/// - Wildcards: tokens containing `*` or `?` are treated like IGFD's asterisk filters and matched
///   against the full extension string (e.g. `".vcx.*"`, `".*.filters"`, `"*.*"`).
/// - Regex: tokens wrapped in `((` ... `))` are treated as regular expressions and matched
///   against the full base name (case-insensitive).
///
/// The variants created from tuples will be normalized to lowercase
/// automatically.
#[derive(Clone, Debug, Default)]
pub struct FileFilter {
    /// Filter display name
    pub name: String,
    /// Lower-case extensions without dot (e.g., "png", "vcxproj.filters")
    pub extensions: Vec<String>,
}

impl FileFilter {
    /// Create a filter from a name and extensions.
    ///
    /// Extensions should be provided without leading dots (e.g. `"png"`,
    /// `"vcxproj.filters"`). Matching is case-insensitive.
    pub fn new(name: impl Into<String>, exts: impl Into<Vec<String>>) -> Self {
        let mut extensions: Vec<String> = exts.into();
        // Normalize to lowercase so matching is case-insensitive even if callers
        // provide mixed-case extensions.
        //
        // Note: keep regex patterns verbatim; lowercasing can change regex meaning
        // (e.g. Unicode categories like `\\p{Lu}`).
        for token in &mut extensions {
            if is_regex_token(token) {
                continue;
            }
            *token = token.to_lowercase();
        }
        Self {
            name: name.into(),
            extensions,
        }
    }

    /// Parse an ImGuiFileDialog (IGFD) style filter spec into one or more [`FileFilter`]s.
    ///
    /// Supported forms:
    ///
    /// - Simple list: `".cpp,.h,.hpp"`
    /// - Collections: `"C/C++{.c,.cpp,.h},Rust{.rs}"`
    ///
    /// Notes:
    /// - Commas inside parentheses `(...)` do not split (IGFD rule 2).
    /// - Regex tokens `((...))` are preserved verbatim.
    /// - Whitespace around commas/tokens is ignored.
    pub fn parse_igfd(spec: &str) -> Result<Vec<FileFilter>, IgfdFilterParseError> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Ok(Vec::new());
        }

        let parts = split_igfd_commas(spec);
        let mut out: Vec<FileFilter> = Vec::new();
        let mut loose_tokens: Vec<String> = Vec::new();

        for part in parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            if let Some((label, inner)) = parse_igfd_collection(part)? {
                if !loose_tokens.is_empty() {
                    out.push(FileFilter::new(
                        if out.is_empty() {
                            spec.to_string()
                        } else {
                            "Custom".to_string()
                        },
                        std::mem::take(&mut loose_tokens),
                    ));
                }
                out.push(FileFilter::new(label, inner));
            } else {
                loose_tokens.push(part.to_string());
            }
        }

        if !loose_tokens.is_empty() {
            out.push(FileFilter::new(
                if out.is_empty() {
                    spec.to_string()
                } else {
                    "Custom".to_string()
                },
                loose_tokens,
            ));
        }

        Ok(out)
    }
}

impl From<(&str, &[&str])> for FileFilter {
    fn from(value: (&str, &[&str])) -> Self {
        Self {
            name: value.0.to_owned(),
            extensions: value
                .1
                .iter()
                .map(|s| {
                    if is_regex_token(s) {
                        (*s).to_string()
                    } else {
                        s.to_lowercase()
                    }
                })
                .collect(),
        }
    }
}

fn is_regex_token(token: &str) -> bool {
    let t = token.trim();
    t.starts_with("((") && t.ends_with("))") && t.len() >= 4
}

fn split_igfd_commas(input: &str) -> Vec<&str> {
    let bytes = input.as_bytes();
    let mut out: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut brace_depth: i32 = 0;
    let mut paren_depth: i32 = 0;

    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => brace_depth += 1,
            b'}' => brace_depth = (brace_depth - 1).max(0),
            b'(' => paren_depth += 1,
            b')' => paren_depth = (paren_depth - 1).max(0),
            b',' if brace_depth == 0 && paren_depth == 0 => {
                out.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&input[start..]);
    out
}

fn parse_igfd_collection(
    part: &str,
) -> Result<Option<(String, Vec<String>)>, IgfdFilterParseError> {
    let bytes = part.as_bytes();
    let mut brace_depth: i32 = 0;
    let mut paren_depth: i32 = 0;
    let mut open_idx: Option<usize> = None;
    let mut close_idx: Option<usize> = None;

    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'{' if brace_depth == 0 && paren_depth == 0 => {
                open_idx = Some(i);
                brace_depth = 1;
            }
            b'{' => brace_depth += 1,
            b'}' => {
                brace_depth = (brace_depth - 1).max(0);
                if brace_depth == 0 && open_idx.is_some() {
                    close_idx = Some(i);
                    break;
                }
            }
            b'(' => paren_depth += 1,
            b')' => paren_depth = (paren_depth - 1).max(0),
            _ => {}
        }
        i += 1;
    }

    let Some(open) = open_idx else {
        return Ok(None);
    };
    let Some(close) = close_idx else {
        return Err(IgfdFilterParseError::new(
            "unterminated '{' in filter collection",
        ));
    };

    let label = part[..open].trim();
    if label.is_empty() {
        return Err(IgfdFilterParseError::new(
            "collection label is empty (expected 'Name{...}')",
        ));
    }
    let tail = part[close + 1..].trim();
    if !tail.is_empty() {
        return Err(IgfdFilterParseError::new(
            "unexpected trailing characters after '}'",
        ));
    }

    let inner = part[open + 1..close].trim();
    if inner.is_empty() {
        return Err(IgfdFilterParseError::new(
            "collection has no filters (empty '{...}')",
        ));
    }

    let mut tokens: Vec<String> = Vec::new();
    for t in split_igfd_commas(inner) {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        tokens.push(t.to_string());
    }
    if tokens.is_empty() {
        return Err(IgfdFilterParseError::new("collection has no filters"));
    }

    Ok(Some((label.to_string(), tokens)))
}

/// Selection result containing one or more paths
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// Selected filesystem paths
    pub paths: Vec<PathBuf>,
}

impl Selection {
    /// Returns true when no path was selected.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Returns the number of selected paths.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Returns selected paths as a slice.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Consumes the selection and returns owned paths.
    pub fn into_paths(self) -> Vec<PathBuf> {
        self.paths
    }

    /// IGFD-like convenience: get the first selected full path.
    ///
    /// This corresponds to `GetFilePathName()` semantics for single selection.
    /// For multi-selection, this returns the first selected path in stable order.
    pub fn file_path_name(&self) -> Option<&Path> {
        self.paths.first().map(PathBuf::as_path)
    }

    /// IGFD-like convenience: get the first selected base file name.
    ///
    /// This corresponds to `GetFileName()` semantics for single selection.
    /// For multi-selection, this returns the first selected file name.
    pub fn file_name(&self) -> Option<&str> {
        self.file_path_name()
            .and_then(Path::file_name)
            .and_then(|v| v.to_str())
    }

    /// IGFD-like convenience: get all selected `(file_name, full_path)` pairs.
    ///
    /// This is a Rust-friendly equivalent of `GetSelection()`.
    pub fn selection_named_paths(&self) -> Vec<(String, PathBuf)> {
        self.paths
            .iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| path.display().to_string());
                (name, path.clone())
            })
            .collect()
    }
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
    /// Confirmation blocked by custom validation logic (e.g. custom pane).
    #[error("validation blocked: {0}")]
    ValidationBlocked(String),
}

/// Extension handling policy for SaveFile mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionPolicy {
    /// Keep user-provided extension as-is.
    KeepUser,
    /// If the user did not provide an extension, append the active filter's first extension.
    AddIfMissing,
    /// Always enforce the active filter's first extension (replace or add).
    ReplaceByFilter,
}

/// SaveFile mode policy knobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavePolicy {
    /// Whether to prompt before overwriting an existing file.
    pub confirm_overwrite: bool,
    /// How to apply the active filter extension to the save name.
    pub extension_policy: ExtensionPolicy,
}

impl Default for SavePolicy {
    fn default() -> Self {
        Self {
            confirm_overwrite: true,
            extension_policy: ExtensionPolicy::AddIfMissing,
        }
    }
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
    /// Sort by full extension (multi-layer aware, e.g. `.tar.gz`)
    Extension,
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
    pub(crate) max_selection: Option<usize>,
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
            max_selection: None,
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

    /// Limit the maximum number of selected files (IGFD `countSelectionMax`-like).
    ///
    /// - `0` means "infinite" (no limit).
    /// - `1` behaves like a single-selection dialog.
    ///
    /// Note: native dialogs may not be able to enforce the limit interactively;
    /// results are clamped best-effort.
    pub fn max_selection(mut self, max: usize) -> Self {
        self.max_selection = if max == 0 { None } else { Some(max) };
        if max == 1 {
            self.allow_multi = false;
        }
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
    ///     .filter(("Images", &["png", "jpg"][..]))
    ///     .filter(("Rust", &["rs"][..]))
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
    ///     FileFilter::from(("Images", &["png", "jpg", "jpeg"][..]))
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

    /// Parse and add one or more IGFD-style filters.
    ///
    /// This is a convenience wrapper over [`FileFilter::parse_igfd`].
    pub fn filters_igfd(mut self, spec: impl AsRef<str>) -> Result<Self, IgfdFilterParseError> {
        let parsed = FileFilter::parse_igfd(spec.as_ref())?;
        self.filters.extend(parsed);
        Ok(self)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_filter_new_normalizes_extensions_to_lowercase_but_preserves_regex() {
        let f = FileFilter::new(
            "Images",
            vec![
                "PNG".to_string(),
                "Jpg".to_string(),
                "gif".to_string(),
                "((\\p{Lu}+))".to_string(),
            ],
        );
        assert_eq!(f.extensions, vec!["png", "jpg", "gif", "((\\p{Lu}+))"]);
    }

    #[test]
    fn parse_igfd_simple_list_becomes_single_filter() {
        let v = FileFilter::parse_igfd(".cpp,.h,.hpp").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, ".cpp,.h,.hpp");
        assert_eq!(v[0].extensions, vec![".cpp", ".h", ".hpp"]);
    }

    #[test]
    fn parse_igfd_collections_build_multiple_filters() {
        let v = FileFilter::parse_igfd("C/C++{.c,.cpp,.h},Rust{.rs}").unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "C/C++");
        assert_eq!(v[0].extensions, vec![".c", ".cpp", ".h"]);
        assert_eq!(v[1].name, "Rust");
        assert_eq!(v[1].extensions, vec![".rs"]);
    }

    #[test]
    fn parse_igfd_does_not_split_commas_inside_parentheses() {
        let v = FileFilter::parse_igfd("C files(png, jpg){.png,.jpg}").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "C files(png, jpg)");
    }

    #[test]
    fn parse_igfd_regex_token_can_contain_commas() {
        let v = FileFilter::parse_igfd("Rx{((a,b)),.txt}").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].extensions, vec!["((a,b))", ".txt"]);
    }

    #[test]
    fn selection_convenience_accessors_for_single_path() {
        let sel = Selection {
            paths: vec![PathBuf::from("/tmp/demo.txt")],
        };
        assert!(!sel.is_empty());
        assert_eq!(sel.len(), 1);
        assert_eq!(sel.file_name(), Some("demo.txt"));
        assert_eq!(sel.file_path_name(), Some(Path::new("/tmp/demo.txt")));
        assert_eq!(sel.paths(), &[PathBuf::from("/tmp/demo.txt")]);
    }

    #[test]
    fn selection_named_paths_for_multi_selection() {
        let sel = Selection {
            paths: vec![PathBuf::from("/a/one.txt"), PathBuf::from("/b/two.bin")],
        };
        let pairs = sel.selection_named_paths();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "one.txt");
        assert_eq!(pairs[0].1, PathBuf::from("/a/one.txt"));
        assert_eq!(pairs[1].0, "two.bin");
        assert_eq!(pairs[1].1, PathBuf::from("/b/two.bin"));
    }

    #[test]
    fn selection_into_paths_moves_owned_paths() {
        let sel = Selection {
            paths: vec![PathBuf::from("a"), PathBuf::from("b")],
        };
        let out = sel.into_paths();
        assert_eq!(out, vec![PathBuf::from("a"), PathBuf::from("b")]);
    }
}
