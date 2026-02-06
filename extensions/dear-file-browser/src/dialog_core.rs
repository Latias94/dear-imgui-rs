use std::path::{Path, PathBuf};

use crate::core::{
    ClickAction, DialogMode, ExtensionPolicy, FileDialogError, FileFilter, SavePolicy, Selection,
    SortBy,
};
use crate::fs::FileSystem;
use crate::places::Places;

/// Keyboard/mouse modifier keys used by selection/navigation logic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Ctrl key held.
    pub ctrl: bool,
    /// Shift key held.
    pub shift: bool,
}

/// Per-frame gate for whether the dialog is allowed to confirm.
///
/// This is primarily used by IGFD-style custom panes to disable confirmation
/// and provide user feedback when extra validation fails.
#[derive(Clone, Debug)]
pub struct ConfirmGate {
    /// Whether confirmation is allowed.
    pub can_confirm: bool,
    /// Optional user-facing message shown when confirmation is blocked.
    pub message: Option<String>,
}

impl Default for ConfirmGate {
    fn default() -> Self {
        Self {
            can_confirm: true,
            message: None,
        }
    }
}

/// A single directory entry in the current directory view.
#[derive(Clone, Debug)]
pub(crate) struct DirEntry {
    /// Base name (no parent path).
    pub(crate) name: String,
    /// Full path.
    pub(crate) path: PathBuf,
    /// Whether this entry is a directory.
    pub(crate) is_dir: bool,
    /// File size in bytes (files only).
    pub(crate) size: Option<u64>,
    /// Last modified timestamp.
    pub(crate) modified: Option<std::time::SystemTime>,
}

impl DirEntry {
    /// A display label used by the default UI (dirs are bracketed).
    pub(crate) fn display_name(&self) -> String {
        if self.is_dir {
            format!("[{}]", self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Core state machine for the ImGui-embedded file dialog.
///
/// This type contains only domain state and logic (selection, navigation,
/// filtering, sorting). It does not depend on Dear ImGui types and can be unit
/// tested by driving its methods.
#[derive(Debug)]
pub struct FileDialogCore {
    /// Mode.
    pub mode: DialogMode,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Selected entry names (relative to cwd).
    pub selected: Vec<String>,
    /// Optional filename input for SaveFile.
    pub save_name: String,
    /// Filters (lower-case extensions).
    pub filters: Vec<FileFilter>,
    /// Active filter index (None = All).
    pub active_filter: Option<usize>,
    /// Click behavior for directories: select or navigate.
    pub click_action: ClickAction,
    /// Search query to filter entries by substring (case-insensitive).
    pub search: String,
    /// Current sort column.
    pub sort_by: SortBy,
    /// Sort order flag (true = ascending).
    pub sort_ascending: bool,
    /// Put directories before files when sorting.
    pub dirs_first: bool,
    /// Allow selecting multiple files.
    pub allow_multi: bool,
    /// Show dotfiles (simple heuristic).
    pub show_hidden: bool,
    /// Double-click navigates/confirm (directories/files).
    pub double_click: bool,
    /// Places shown in the left pane (System + Bookmarks + custom groups).
    pub places: Places,
    /// Save behavior knobs (SaveFile mode only).
    pub save_policy: SavePolicy,

    result: Option<Result<Selection, FileDialogError>>,
    pending_overwrite: Option<Selection>,
    focused_name: Option<String>,
    selection_anchor_name: Option<String>,
    view_names: Vec<String>,
    entries: Vec<DirEntry>,
}

impl FileDialogCore {
    /// Creates a new dialog core for a mode.
    pub fn new(mode: DialogMode) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            mode,
            cwd,
            selected: Vec::new(),
            save_name: String::new(),
            filters: Vec::new(),
            active_filter: None,
            click_action: ClickAction::Select,
            search: String::new(),
            sort_by: SortBy::Name,
            sort_ascending: true,
            dirs_first: true,
            allow_multi: matches!(mode, DialogMode::OpenFiles),
            show_hidden: false,
            double_click: true,
            places: Places::default(),
            save_policy: SavePolicy::default(),
            result: None,
            pending_overwrite: None,
            focused_name: None,
            selection_anchor_name: None,
            view_names: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Returns a snapshot of the current entries list.
    pub(crate) fn entries(&self) -> &[DirEntry] {
        &self.entries
    }

    /// Returns the final result once the user confirms/cancels, and clears it.
    pub(crate) fn take_result(&mut self) -> Option<Result<Selection, FileDialogError>> {
        self.result.take()
    }

    /// Sets the current directory and clears selection/focus.
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
        self.selected.clear();
        self.focused_name = None;
        self.selection_anchor_name = None;
    }

    /// Rescans the current directory into an entries cache.
    pub(crate) fn rescan(&mut self, fs: &dyn FileSystem) {
        let mut entries = read_entries_with_fs(fs, &self.cwd, self.show_hidden);
        filter_entries_in_place(
            &mut entries,
            self.mode,
            &self.filters,
            self.active_filter,
            &self.search,
        );
        sort_entries_in_place(
            &mut entries,
            self.sort_by,
            self.sort_ascending,
            self.dirs_first,
        );
        self.view_names = entries.iter().map(|e| e.name.clone()).collect();
        self.entries = entries;
        self.retain_selected_visible();
    }

    /// Select the next entry whose base name starts with the given prefix (case-insensitive).
    ///
    /// This is used by "type-to-select" navigation (IGFD-style).
    pub(crate) fn select_by_prefix(&mut self, prefix: &str) {
        let prefix = prefix.trim();
        if prefix.is_empty() || self.view_names.is_empty() {
            return;
        }
        let prefix_lower = prefix.to_lowercase();

        let len = self.view_names.len();
        let start_idx = self
            .focused_name
            .as_deref()
            .and_then(|f| self.view_names.iter().position(|n| n == f))
            .map(|i| (i + 1) % len)
            .unwrap_or(0);

        for off in 0..len {
            let i = (start_idx + off) % len;
            let name = &self.view_names[i];
            if name.to_lowercase().starts_with(&prefix_lower) {
                self.select_single_by_name(name.clone());
                break;
            }
        }
    }

    /// Applies Ctrl+A style selection to all currently visible entries.
    pub(crate) fn select_all(&mut self) {
        if self.allow_multi {
            self.selected = self.view_names.clone();
        }
    }

    /// Moves keyboard focus up/down within the current view.
    pub(crate) fn move_focus(&mut self, delta: i32, modifiers: Modifiers) {
        if self.view_names.is_empty() {
            return;
        }

        let len = self.view_names.len();
        let current_idx = self
            .focused_name
            .as_ref()
            .and_then(|n| self.view_names.iter().position(|s| s == n));
        let next_idx = match current_idx {
            Some(i) => {
                let next = i as i32 + delta;
                next.clamp(0, (len - 1) as i32) as usize
            }
            None => {
                if delta >= 0 {
                    0
                } else {
                    len - 1
                }
            }
        };

        let target = self.view_names[next_idx].clone();
        if modifiers.shift {
            let anchor = self
                .selection_anchor_name
                .clone()
                .or_else(|| self.focused_name.clone())
                .unwrap_or_else(|| target.clone());
            if self.selection_anchor_name.is_none() {
                self.selection_anchor_name = Some(anchor.clone());
            }

            if let Some(range) = select_range_by_name(&self.view_names, &anchor, &target) {
                self.selected = range;
                self.focused_name = Some(target);
            } else {
                self.select_single_by_name(target);
            }
        } else {
            self.select_single_by_name(target);
        }
    }

    /// Activates the focused entry (Enter).
    ///
    /// If no selection exists, the focused item becomes selected, then confirm is attempted.
    pub(crate) fn activate_focused(&mut self) -> bool {
        if self.selected.is_empty() {
            if let Some(name) = self.focused_name.clone() {
                self.selected.push(name);
            }
        }
        !self.selected.is_empty()
    }

    /// Handles a click on an entry row.
    pub(crate) fn click_entry(&mut self, name: String, is_dir: bool, modifiers: Modifiers) {
        if is_dir {
            match self.click_action {
                ClickAction::Select => {
                    self.select_single_by_name(name);
                }
                ClickAction::Navigate => {
                    self.cwd.push(&name);
                    self.selected.clear();
                    self.focused_name = None;
                    self.selection_anchor_name = None;
                }
            }
            return;
        }

        if modifiers.shift {
            if let Some(anchor) = self.selection_anchor_name.clone() {
                if let Some(range) = select_range_by_name(&self.view_names, &anchor, &name) {
                    self.selected = range;
                    self.focused_name = Some(name);
                    return;
                }
            }
            // Fallback if range selection isn't possible.
            self.select_single_by_name(name);
            return;
        }

        if !self.allow_multi || !modifiers.ctrl {
            self.select_single_by_name(name);
            return;
        }

        toggle_select_name(&mut self.selected, &name);
        self.focused_name = Some(name.clone());
        self.selection_anchor_name = Some(name);
    }

    /// Handles a double-click on an entry row.
    pub(crate) fn double_click_entry(&mut self, name: String, is_dir: bool) -> bool {
        if !self.double_click {
            return false;
        }
        if is_dir {
            self.cwd.push(&name);
            self.selected.clear();
            self.focused_name = None;
            self.selection_anchor_name = None;
            return false;
        }

        if matches!(self.mode, DialogMode::OpenFile | DialogMode::OpenFiles) {
            self.selected.clear();
            self.selected.push(name);
            return true;
        }
        false
    }

    /// Navigates one directory up.
    pub(crate) fn navigate_up(&mut self) {
        let _ = self.cwd.pop();
        self.selected.clear();
        self.focused_name = None;
        self.selection_anchor_name = None;
    }

    /// Navigates to a directory.
    pub(crate) fn navigate_to(&mut self, p: PathBuf) {
        self.set_cwd(p);
    }

    /// Confirms the dialog. On success, stores a result and signals the UI to close.
    pub(crate) fn confirm(
        &mut self,
        fs: &dyn FileSystem,
        gate: &ConfirmGate,
    ) -> Result<(), FileDialogError> {
        self.result = None;
        self.pending_overwrite = None;

        // Special-case: if a single directory selected in file-open modes, navigate into it
        // instead of confirming.
        if matches!(self.mode, DialogMode::OpenFile | DialogMode::OpenFiles)
            && self.selected.len() == 1
        {
            let sel = self.selected[0].clone();
            let p = self.cwd.join(&sel);
            let is_dir = fs.metadata(&p).map(|m| m.is_dir).unwrap_or(false);
            if is_dir {
                self.cwd.push(sel);
                self.selected.clear();
                self.focused_name = None;
                self.selection_anchor_name = None;
                return Ok(());
            }
        }

        if !gate.can_confirm {
            let msg = gate
                .message
                .clone()
                .unwrap_or_else(|| "validation blocked".to_string());
            return Err(FileDialogError::ValidationBlocked(msg));
        }

        let sel = finalize_selection(
            self.mode,
            &self.cwd,
            self.selected.clone(),
            &self.save_name,
            &self.filters,
            self.active_filter,
            &self.save_policy,
        )?;

        if matches!(self.mode, DialogMode::SaveFile) {
            let target = sel
                .paths
                .get(0)
                .cloned()
                .unwrap_or_else(|| self.cwd.clone());
            match fs.metadata(&target) {
                Ok(md) => {
                    if md.is_dir {
                        return Err(FileDialogError::InvalidPath(
                            "file name points to a directory".into(),
                        ));
                    }
                    if self.save_policy.confirm_overwrite {
                        self.pending_overwrite = Some(sel);
                        return Ok(());
                    }
                }
                Err(_) => {}
            }
        }

        self.result = Some(Ok(sel));
        Ok(())
    }

    /// Cancels the dialog.
    pub(crate) fn cancel(&mut self) {
        self.result = Some(Err(FileDialogError::Cancelled));
    }

    /// Returns the pending overwrite selection (SaveFile mode) if confirmation is required.
    pub(crate) fn pending_overwrite(&self) -> Option<&Selection> {
        self.pending_overwrite.as_ref()
    }

    /// Accept an overwrite prompt and produce the stored selection.
    pub(crate) fn accept_overwrite(&mut self) {
        if let Some(sel) = self.pending_overwrite.take() {
            self.result = Some(Ok(sel));
        }
    }

    /// Cancel an overwrite prompt and return to the dialog.
    pub(crate) fn cancel_overwrite(&mut self) {
        self.pending_overwrite = None;
    }

    fn select_single_by_name(&mut self, name: String) {
        self.selected.clear();
        self.selected.push(name.clone());
        self.focused_name = Some(name.clone());
        self.selection_anchor_name = Some(name);
    }

    fn retain_selected_visible(&mut self) {
        if self.selected.is_empty() || self.view_names.is_empty() {
            return;
        }
        let mut keep = Vec::with_capacity(self.selected.len());
        for n in self.selected.drain(..) {
            if self.view_names.iter().any(|v| v == &n) {
                keep.push(n);
            }
        }
        self.selected = keep;
    }
}

fn effective_filters(filters: &[FileFilter], active_filter: Option<usize>) -> Vec<FileFilter> {
    match active_filter {
        Some(i) => filters.get(i).cloned().into_iter().collect(),
        None => Vec::new(),
    }
}

fn matches_filters(name: &str, filters: &[FileFilter]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let name_lower = name.to_lowercase();
    filters.iter().any(|f| {
        f.extensions
            .iter()
            .any(|ext| has_extension_suffix(&name_lower, ext))
    })
}

fn has_extension_suffix(name_lower: &str, ext: &str) -> bool {
    let ext = ext.trim_start_matches('.');
    if ext.is_empty() {
        return false;
    }
    if !name_lower.ends_with(ext) {
        return false;
    }
    let prefix_len = name_lower.len() - ext.len();
    if prefix_len == 0 {
        return false;
    }
    name_lower.as_bytes()[prefix_len - 1] == b'.'
}

fn toggle_select_name(list: &mut Vec<String>, name: &str) {
    if let Some(i) = list.iter().position(|s| s == name) {
        list.remove(i);
    } else {
        list.push(name.to_string());
    }
}

fn select_range_by_name(view_names: &[String], anchor: &str, target: &str) -> Option<Vec<String>> {
    let ia = view_names.iter().position(|s| s == anchor)?;
    let it = view_names.iter().position(|s| s == target)?;
    let (lo, hi) = if ia <= it { (ia, it) } else { (it, ia) };
    Some(view_names[lo..=hi].to_vec())
}

fn finalize_selection(
    mode: DialogMode,
    cwd: &Path,
    selected_names: Vec<String>,
    save_name: &str,
    filters: &[FileFilter],
    active_filter: Option<usize>,
    save_policy: &SavePolicy,
) -> Result<Selection, FileDialogError> {
    let mut sel = Selection { paths: Vec::new() };
    let eff_filters = effective_filters(filters, active_filter);
    match mode {
        DialogMode::PickFolder => {
            sel.paths.push(cwd.to_path_buf());
        }
        DialogMode::OpenFile | DialogMode::OpenFiles => {
            if selected_names.is_empty() {
                return Err(FileDialogError::InvalidPath("no selection".into()));
            }
            for n in selected_names {
                if !matches_filters(&n, &eff_filters) {
                    continue;
                }
                sel.paths.push(cwd.join(n));
            }
            if sel.paths.is_empty() {
                return Err(FileDialogError::InvalidPath(
                    "no file matched filters".into(),
                ));
            }
        }
        DialogMode::SaveFile => {
            let name = normalize_save_name(save_name, &eff_filters, save_policy.extension_policy);
            if name.is_empty() {
                return Err(FileDialogError::InvalidPath("empty file name".into()));
            }
            sel.paths.push(cwd.join(name));
        }
    }
    Ok(sel)
}

fn normalize_save_name(save_name: &str, filters: &[FileFilter], policy: ExtensionPolicy) -> String {
    let name = save_name.trim().to_string();
    if name.is_empty() {
        return name;
    }

    let default_ext = filters
        .first()
        .and_then(|f| f.extensions.first())
        .map(|s| s.as_str());
    let Some(default_ext) = default_ext else {
        return name;
    };

    let p = Path::new(&name);
    let has_ext = p.extension().and_then(|s| s.to_str()).is_some();

    match policy {
        ExtensionPolicy::KeepUser => name,
        ExtensionPolicy::AddIfMissing => {
            if has_ext {
                name
            } else {
                format!("{name}.{default_ext}")
            }
        }
        ExtensionPolicy::ReplaceByFilter => {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(&name);
            format!("{stem}.{default_ext}")
        }
    }
}

fn filter_entries_in_place(
    entries: &mut Vec<DirEntry>,
    mode: DialogMode,
    filters: &[FileFilter],
    active_filter: Option<usize>,
    search: &str,
) {
    let display_filters = effective_filters(filters, active_filter);
    let search_lower = if search.is_empty() {
        None
    } else {
        Some(search.to_lowercase())
    };
    entries.retain(|e| {
        let pass_kind = if matches!(mode, DialogMode::PickFolder) {
            e.is_dir
        } else {
            e.is_dir || matches_filters(&e.name, &display_filters)
        };
        let pass_search = match &search_lower {
            None => true,
            Some(q) => e.name.to_lowercase().contains(q),
        };
        pass_kind && pass_search
    });
}

fn sort_entries_in_place(
    entries: &mut Vec<DirEntry>,
    sort_by: SortBy,
    sort_ascending: bool,
    dirs_first: bool,
) {
    entries.sort_by(|a, b| {
        if dirs_first && a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        let ord = match sort_by {
            SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortBy::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortBy::Modified => a.modified.cmp(&b.modified),
        };
        if sort_ascending { ord } else { ord.reverse() }
    });
}

fn read_entries_with_fs(fs: &dyn FileSystem, dir: &Path, show_hidden: bool) -> Vec<DirEntry> {
    let mut out = Vec::new();
    let Ok(rd) = fs.read_dir(dir) else {
        return out;
    };
    for e in rd {
        if !show_hidden && e.name.starts_with('.') {
            continue;
        }
        out.push(DirEntry {
            name: e.name,
            path: e.path,
            is_dir: e.is_dir,
            size: e.size,
            modified: e.modified,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::StdFileSystem;

    fn mods(ctrl: bool, shift: bool) -> Modifiers {
        Modifiers { ctrl, shift }
    }

    #[derive(Default)]
    struct TestFs {
        meta: std::collections::HashMap<PathBuf, crate::fs::FsMetadata>,
    }

    impl crate::fs::FileSystem for TestFs {
        fn read_dir(&self, _dir: &Path) -> std::io::Result<Vec<crate::fs::FsEntry>> {
            Ok(Vec::new())
        }

        fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
            Ok(path.to_path_buf())
        }

        fn metadata(&self, path: &Path) -> std::io::Result<crate::fs::FsMetadata> {
            self.meta
                .get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
        }
    }

    #[test]
    fn cancel_sets_result() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cancel();
        assert!(matches!(
            core.take_result(),
            Some(Err(crate::FileDialogError::Cancelled))
        ));
    }

    #[test]
    fn click_file_toggles_in_multi_select() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.view_names = vec!["a.txt".into()];
        core.click_entry("a.txt".into(), false, mods(true, false));
        assert_eq!(core.selected, vec!["a.txt"]);
        core.click_entry("a.txt".into(), false, mods(true, false));
        assert!(core.selected.is_empty());
    }

    #[test]
    fn shift_click_selects_a_range_in_view_order() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.view_names = vec![
            "a.txt".into(),
            "b.txt".into(),
            "c.txt".into(),
            "d.txt".into(),
            "e.txt".into(),
        ];
        core.click_entry("b.txt".into(), false, mods(false, false));
        core.click_entry("e.txt".into(), false, mods(false, true));
        assert_eq!(core.selected, vec!["b.txt", "c.txt", "d.txt", "e.txt"]);
    }

    #[test]
    fn ctrl_a_selects_all_when_multi_select_enabled() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.view_names = vec!["a".into(), "b".into(), "c".into()];
        core.select_all();
        assert_eq!(core.selected, vec!["a", "b", "c"]);
    }

    #[test]
    fn move_focus_with_shift_extends_range() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.view_names = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        core.click_entry("b".into(), false, mods(false, false));
        core.move_focus(2, mods(false, true));
        assert_eq!(core.selected, vec!["b", "c", "d"]);
        assert_eq!(core.focused_name.as_deref(), Some("d"));
    }

    #[test]
    fn activate_focused_confirms_selection() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.view_names = vec!["a.txt".into()];
        core.focused_name = Some("a.txt".into());
        let gate = ConfirmGate::default();
        assert!(core.activate_focused());
        core.confirm(&StdFileSystem, &gate).unwrap();
        let sel = core.take_result().unwrap().unwrap();
        assert_eq!(sel.paths.len(), 1);
        assert_eq!(
            sel.paths[0].file_name().and_then(|s| s.to_str()),
            Some("a.txt")
        );
    }

    #[test]
    fn save_adds_extension_from_active_filter_when_missing() {
        let mut core = FileDialogCore::new(DialogMode::SaveFile);
        core.cwd = PathBuf::from("/tmp");
        core.save_name = "asset".into();
        core.filters = vec![FileFilter::new("Images", vec!["png".to_string()])];
        core.active_filter = Some(0);
        core.save_policy.extension_policy = ExtensionPolicy::AddIfMissing;
        core.save_policy.confirm_overwrite = false;

        let gate = ConfirmGate::default();
        let fs = TestFs::default();
        core.confirm(&fs, &gate).unwrap();
        let sel = core.take_result().unwrap().unwrap();
        assert_eq!(sel.paths[0], PathBuf::from("/tmp/asset.png"));
    }

    #[test]
    fn save_keep_user_extension_does_not_modify_name() {
        let mut core = FileDialogCore::new(DialogMode::SaveFile);
        core.cwd = PathBuf::from("/tmp");
        core.save_name = "asset.jpg".into();
        core.filters = vec![FileFilter::new("Images", vec!["png".to_string()])];
        core.active_filter = Some(0);
        core.save_policy.extension_policy = ExtensionPolicy::KeepUser;
        core.save_policy.confirm_overwrite = false;

        let gate = ConfirmGate::default();
        let fs = TestFs::default();
        core.confirm(&fs, &gate).unwrap();
        let sel = core.take_result().unwrap().unwrap();
        assert_eq!(sel.paths[0], PathBuf::from("/tmp/asset.jpg"));
    }

    #[test]
    fn save_replace_by_filter_replaces_existing_extension() {
        let mut core = FileDialogCore::new(DialogMode::SaveFile);
        core.cwd = PathBuf::from("/tmp");
        core.save_name = "asset.jpg".into();
        core.filters = vec![FileFilter::new("Images", vec!["png".to_string()])];
        core.active_filter = Some(0);
        core.save_policy.extension_policy = ExtensionPolicy::ReplaceByFilter;
        core.save_policy.confirm_overwrite = false;

        let gate = ConfirmGate::default();
        let fs = TestFs::default();
        core.confirm(&fs, &gate).unwrap();
        let sel = core.take_result().unwrap().unwrap();
        assert_eq!(sel.paths[0], PathBuf::from("/tmp/asset.png"));
    }

    #[test]
    fn matches_filters_supports_multi_layer_extensions() {
        let filters = vec![FileFilter::new("VS", vec!["vcxproj.filters".to_string()])];
        assert!(matches_filters("proj.vcxproj.filters", &filters));
        assert!(!matches_filters("proj.vcxproj", &filters));
        assert!(!matches_filters("vcxproj.filters", &filters));
    }

    #[test]
    fn select_by_prefix_cycles_from_current_focus() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.view_names = vec!["alpha".into(), "beta".into(), "alpine".into()];
        core.focused_name = Some("alpha".into());
        core.select_by_prefix("al");
        assert_eq!(core.selected, vec!["alpine"]);
        assert_eq!(core.focused_name.as_deref(), Some("alpine"));

        core.select_by_prefix("al");
        assert_eq!(core.selected, vec!["alpha"]);
        assert_eq!(core.focused_name.as_deref(), Some("alpha"));
    }

    #[test]
    fn save_prompts_overwrite_when_target_exists_and_policy_enabled() {
        let mut core = FileDialogCore::new(DialogMode::SaveFile);
        core.cwd = PathBuf::from("/tmp");
        core.save_name = "asset.png".into();
        core.save_policy.confirm_overwrite = true;

        let mut fs = TestFs::default();
        fs.meta.insert(
            PathBuf::from("/tmp/asset.png"),
            crate::fs::FsMetadata { is_dir: false },
        );

        let gate = ConfirmGate::default();
        core.confirm(&fs, &gate).unwrap();
        assert!(core.take_result().is_none());
        assert!(core.pending_overwrite().is_some());

        core.accept_overwrite();
        assert!(core.pending_overwrite().is_none());
        let sel = core.take_result().unwrap().unwrap();
        assert_eq!(sel.paths[0], PathBuf::from("/tmp/asset.png"));
    }
}
