use std::path::{Path, PathBuf};
use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    hash::Hasher,
};

use crate::core::{
    ClickAction, DialogMode, ExtensionPolicy, FileDialogError, FileFilter, SavePolicy, Selection,
    SortBy,
};
use crate::fs::FileSystem;
use crate::places::Places;
use regex::RegexBuilder;

/// Keyboard/mouse modifier keys used by selection/navigation logic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Ctrl key held.
    pub ctrl: bool,
    /// Shift key held.
    pub shift: bool,
}

/// Input event for driving the dialog core without direct UI coupling.
#[derive(Clone, Debug)]
pub enum CoreEvent {
    /// Ctrl+A style select all visible entries.
    SelectAll,
    /// Move focus by row/column delta.
    MoveFocus {
        /// Signed movement in current view order.
        delta: i32,
        /// Modifier keys used by selection semantics.
        modifiers: Modifiers,
    },
    /// Click an entry row/cell.
    ClickEntry {
        /// Entry base name.
        name: String,
        /// Whether entry is directory.
        is_dir: bool,
        /// Modifier keys used by selection semantics.
        modifiers: Modifiers,
    },
    /// Double-click an entry row/cell.
    DoubleClickEntry {
        /// Entry base name.
        name: String,
        /// Whether entry is directory.
        is_dir: bool,
    },
    /// Type-to-select prefix.
    SelectByPrefix(String),
    /// Activate focused entry (Enter).
    ActivateFocused,
    /// Navigate to parent directory.
    NavigateUp,
    /// Navigate to a target directory.
    NavigateTo(PathBuf),
    /// Focus and select one entry by base name.
    FocusAndSelectByName(String),
    /// Replace current selection by names.
    ReplaceSelectionByNames(Vec<String>),
    /// Clear current selection/focus/anchor.
    ClearSelection,
}

/// Side effect emitted after applying a [`CoreEvent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreEventOutcome {
    /// No extra action required by host/UI.
    None,
    /// Confirmation should be attempted by host/UI.
    RequestConfirm,
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

/// Stable identifier for a directory entry within dialog snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryId(u64);

impl EntryId {
    fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Rich metadata attached to a filesystem entry.
#[derive(Clone, Debug)]
pub struct FileMeta {
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// File size in bytes (files only).
    pub size: Option<u64>,
    /// Last modified timestamp.
    pub modified: Option<std::time::SystemTime>,
}

/// Snapshot of one directory listing refresh.
#[derive(Clone, Debug)]
pub struct DirSnapshot {
    /// Directory path used to build this snapshot.
    pub cwd: PathBuf,
    /// Number of captured entries in this snapshot.
    pub entry_count: usize,

    pub(crate) entries: Vec<DirEntry>,
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

    fn stable_id(&self) -> EntryId {
        let mut hasher = DefaultHasher::new();
        hasher.write(self.path.as_os_str().to_string_lossy().as_bytes());
        hasher.write_u8(0);
        hasher.write(self.name.as_bytes());
        hasher.write_u8(0);
        hasher.write_u8(u8::from(self.is_dir));
        EntryId::new(hasher.finish())
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
    selected_ids: Vec<EntryId>,
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
    /// Optional cap for maximum number of selected files (OpenFiles mode).
    ///
    /// - `None` => no limit
    /// - `Some(1)` => single selection
    pub max_selection: Option<usize>,
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
    focused_id: Option<EntryId>,
    selection_anchor_id: Option<EntryId>,
    view_names: Vec<String>,
    view_ids: Vec<EntryId>,
    entries: Vec<DirEntry>,

    dir_snapshot: DirSnapshot,
    dir_snapshot_dirty: bool,
    last_view_key: Option<ViewKey>,

    pending_selected_names: Vec<String>,
}

impl FileDialogCore {
    /// Creates a new dialog core for a mode.
    pub fn new(mode: DialogMode) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            mode,
            cwd,
            selected: Vec::new(),
            selected_ids: Vec::new(),
            save_name: String::new(),
            filters: Vec::new(),
            active_filter: None,
            click_action: ClickAction::Select,
            search: String::new(),
            sort_by: SortBy::Name,
            sort_ascending: true,
            dirs_first: true,
            allow_multi: matches!(mode, DialogMode::OpenFiles),
            max_selection: None,
            show_hidden: false,
            double_click: true,
            places: Places::default(),
            save_policy: SavePolicy::default(),
            result: None,
            pending_overwrite: None,
            focused_name: None,
            selection_anchor_name: None,
            focused_id: None,
            selection_anchor_id: None,
            view_names: Vec::new(),
            view_ids: Vec::new(),
            entries: Vec::new(),
            dir_snapshot: DirSnapshot {
                cwd: PathBuf::new(),
                entry_count: 0,
                entries: Vec::new(),
            },
            dir_snapshot_dirty: true,
            last_view_key: None,
            pending_selected_names: Vec::new(),
        }
    }

    /// Returns a snapshot of the current entries list.
    pub(crate) fn entries(&self) -> &[DirEntry] {
        &self.entries
    }

    /// Apply one core event and return host-facing outcome.
    pub fn handle_event(&mut self, event: CoreEvent) -> CoreEventOutcome {
        match event {
            CoreEvent::SelectAll => {
                self.select_all();
                CoreEventOutcome::None
            }
            CoreEvent::MoveFocus { delta, modifiers } => {
                self.move_focus(delta, modifiers);
                CoreEventOutcome::None
            }
            CoreEvent::ClickEntry {
                name,
                is_dir,
                modifiers,
            } => {
                self.click_entry(name, is_dir, modifiers);
                CoreEventOutcome::None
            }
            CoreEvent::DoubleClickEntry { name, is_dir } => {
                if self.double_click_entry(name, is_dir) {
                    CoreEventOutcome::RequestConfirm
                } else {
                    CoreEventOutcome::None
                }
            }
            CoreEvent::SelectByPrefix(prefix) => {
                self.select_by_prefix(&prefix);
                CoreEventOutcome::None
            }
            CoreEvent::ActivateFocused => {
                if self.activate_focused() {
                    CoreEventOutcome::RequestConfirm
                } else {
                    CoreEventOutcome::None
                }
            }
            CoreEvent::NavigateUp => {
                self.navigate_up();
                CoreEventOutcome::None
            }
            CoreEvent::NavigateTo(path) => {
                self.navigate_to(path);
                CoreEventOutcome::None
            }
            CoreEvent::FocusAndSelectByName(name) => {
                self.focus_and_select_by_name(name);
                CoreEventOutcome::None
            }
            CoreEvent::ReplaceSelectionByNames(names) => {
                self.replace_selection_by_names(names);
                CoreEventOutcome::None
            }
            CoreEvent::ClearSelection => {
                self.clear_selection();
                CoreEventOutcome::None
            }
        }
    }

    /// Mark the current directory snapshot as dirty so it will be refreshed on next draw.
    pub fn invalidate_dir_cache(&mut self) {
        self.dir_snapshot_dirty = true;
        self.last_view_key = None;
    }

    /// Returns the final result once the user confirms/cancels, and clears it.
    pub(crate) fn take_result(&mut self) -> Option<Result<Selection, FileDialogError>> {
        self.result.take()
    }

    /// Sets the current directory and clears selection/focus.
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
        self.clear_selection();
    }

    /// Selects and focuses a single entry by base name.
    ///
    /// This is useful for UI-driven actions that create a new entry and want to
    /// immediately reveal it (e.g. "New Folder").
    pub fn focus_and_select_by_name(&mut self, name: impl Into<String>) {
        self.select_single_by_name(name.into());
    }

    /// Replace selection by entry names (used by multi-create/paste flows).
    pub fn replace_selection_by_names(&mut self, names: Vec<String>) {
        self.selected.clear();
        self.selected.extend(names.iter().cloned());
        self.pending_selected_names = names;
        self.selected_ids.clear();
        self.focused_name = None;
        self.selection_anchor_name = None;
        self.focused_id = None;
        self.selection_anchor_id = None;
    }

    /// Clear current selection, focus and anchor.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.selected_ids.clear();
        self.focused_name = None;
        self.selection_anchor_name = None;
        self.focused_id = None;
        self.selection_anchor_id = None;
        self.pending_selected_names.clear();
    }

    pub(crate) fn selected_len(&self) -> usize {
        self.selected.len()
    }

    pub(crate) fn has_selection(&self) -> bool {
        !self.selected.is_empty()
    }

    pub(crate) fn first_selected_name(&self) -> Option<&str> {
        self.selected.first().map(String::as_str)
    }

    pub(crate) fn selected_names(&self) -> &[String] {
        &self.selected
    }

    pub(crate) fn is_selected_name(&self, name: &str) -> bool {
        self.selected.iter().any(|n| n == name)
    }

    /// Refreshes the directory snapshot and view cache if needed.
    pub(crate) fn rescan_if_needed(&mut self, fs: &dyn FileSystem) {
        self.refresh_dir_snapshot_if_needed(fs);

        let key = ViewKey::new(self);
        if self.last_view_key.as_ref() == Some(&key) {
            return;
        }

        let mut entries = self.dir_snapshot.entries.clone();
        filter_entries_in_place(
            &mut entries,
            self.mode,
            self.show_hidden,
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
        self.view_ids = entries.iter().map(DirEntry::stable_id).collect();
        self.entries = entries;
        self.resolve_pending_selected_names();
        self.retain_selected_visible();
        self.sync_id_state_from_names();
        self.last_view_key = Some(key);
    }

    fn refresh_dir_snapshot_if_needed(&mut self, fs: &dyn FileSystem) {
        let cwd_changed = self.dir_snapshot.cwd != self.cwd;
        let should_refresh = self.dir_snapshot_dirty || cwd_changed;
        if !should_refresh {
            return;
        }

        self.dir_snapshot = read_entries_snapshot_with_fs(fs, &self.cwd);
        self.dir_snapshot_dirty = false;
        // Always rebuild the view after a filesystem refresh even if the view inputs didn't change.
        self.last_view_key = None;
    }

    fn resolve_pending_selected_names(&mut self) {
        if self.pending_selected_names.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_selected_names);
        self.selected.clear();
        for name in pending {
            if self.view_names.iter().any(|visible| visible == &name) {
                self.selected.push(name);
            }
        }
        self.enforce_selection_cap();
        if let Some(last) = self.selected.last().cloned() {
            self.focused_name = Some(last.clone());
            self.selection_anchor_name = Some(last);
        }
    }

    fn id_for_name(&self, name: &str) -> Option<EntryId> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(DirEntry::stable_id)
    }

    fn sync_id_state_from_names(&mut self) {
        let mut seen = HashSet::new();
        let mut ids = Vec::with_capacity(self.selected.len());
        for name in &self.selected {
            if let Some(id) = self.id_for_name(name) {
                if seen.insert(id) {
                    ids.push(id);
                }
            }
        }
        self.selected_ids = ids;
        self.focused_id = self
            .focused_name
            .as_deref()
            .and_then(|name| self.id_for_name(name));
        self.selection_anchor_id = self
            .selection_anchor_name
            .as_deref()
            .and_then(|name| self.id_for_name(name));
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
        let cap = self.selection_cap();
        if cap <= 1 {
            return;
        }
        let take = self.view_names.len().min(cap);
        self.selected = self.view_names.iter().take(take).cloned().collect();
        self.sync_id_state_from_names();
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

            if let Some(range) = select_range_by_name_capped(
                &self.view_names,
                &anchor,
                &target,
                self.selection_cap(),
            ) {
                self.selected = range;
                self.focused_name = Some(target);
                self.sync_id_state_from_names();
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
                self.sync_id_state_from_names();
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
                    self.clear_selection();
                }
            }
            return;
        }

        if modifiers.shift {
            if let Some(anchor) = self.selection_anchor_name.clone() {
                if let Some(range) = select_range_by_name_capped(
                    &self.view_names,
                    &anchor,
                    &name,
                    self.selection_cap(),
                ) {
                    self.selected = range;
                    self.focused_name = Some(name);
                    self.sync_id_state_from_names();
                    return;
                }
            }
            // Fallback if range selection isn't possible.
            self.select_single_by_name(name);
            return;
        }

        if self.selection_cap() <= 1 || !modifiers.ctrl {
            self.select_single_by_name(name);
            return;
        }

        toggle_select_name(&mut self.selected, &name);
        self.focused_name = Some(name.clone());
        self.selection_anchor_name = Some(name);
        self.enforce_selection_cap();
        self.sync_id_state_from_names();
    }

    /// Handles a double-click on an entry row.
    pub(crate) fn double_click_entry(&mut self, name: String, is_dir: bool) -> bool {
        if !self.double_click {
            return false;
        }
        if is_dir {
            self.cwd.push(&name);
            self.clear_selection();
            return false;
        }

        if matches!(self.mode, DialogMode::OpenFile | DialogMode::OpenFiles) {
            self.selected.clear();
            self.selected.push(name);
            self.sync_id_state_from_names();
            return true;
        }
        false
    }

    /// Navigates one directory up.
    pub(crate) fn navigate_up(&mut self) {
        let _ = self.cwd.pop();
        self.clear_selection();
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
                self.clear_selection();
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
        self.sync_id_state_from_names();
    }

    fn selection_cap(&self) -> usize {
        if !self.allow_multi {
            return 1;
        }
        self.max_selection.unwrap_or(usize::MAX).max(1)
    }

    fn enforce_selection_cap(&mut self) {
        let cap = self.selection_cap();
        if cap == usize::MAX || self.selected.len() <= cap {
            return;
        }
        while self.selected.len() > cap {
            self.selected.remove(0);
        }
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
        self.enforce_selection_cap();
        self.sync_id_state_from_names();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ViewKey {
    cwd: PathBuf,
    mode: DialogMode,
    show_hidden: bool,
    search: String,
    sort_by: SortBy,
    sort_ascending: bool,
    dirs_first: bool,
    active_filter_hash: u64,
}

impl ViewKey {
    fn new(core: &FileDialogCore) -> Self {
        Self {
            cwd: core.cwd.clone(),
            mode: core.mode,
            show_hidden: core.show_hidden,
            search: core.search.clone(),
            sort_by: core.sort_by,
            sort_ascending: core.sort_ascending,
            dirs_first: core.dirs_first,
            active_filter_hash: active_filter_hash(&core.filters, core.active_filter),
        }
    }
}

fn active_filter_hash(filters: &[FileFilter], active_filter: Option<usize>) -> u64 {
    let Some(i) = active_filter else {
        return 0;
    };
    let Some(f) = filters.get(i) else {
        return 0;
    };
    let mut hasher = DefaultHasher::new();
    // Hash both name and tokens so changes trigger a view rebuild.
    hasher.write(f.name.as_bytes());
    for t in &f.extensions {
        hasher.write(t.as_bytes());
        hasher.write_u8(0);
    }
    hasher.finish()
}

fn effective_filters(filters: &[FileFilter], active_filter: Option<usize>) -> Vec<FileFilter> {
    match active_filter {
        Some(i) => filters.get(i).cloned().into_iter().collect(),
        None => Vec::new(),
    }
}

#[derive(Debug)]
enum FilterMatcher {
    Any,
    Extension(String),
    ExtensionGlob(String),
    NameRegex(regex::Regex),
}

fn compile_filter_matchers(filters: &[FileFilter]) -> Vec<FilterMatcher> {
    let mut out = Vec::new();
    for f in filters {
        for token in &f.extensions {
            let t = token.trim();
            if t.is_empty() {
                continue;
            }

            if let Some(re) = parse_regex_token(t) {
                let built = RegexBuilder::new(re)
                    .case_insensitive(true)
                    .build()
                    .map(FilterMatcher::NameRegex);
                if let Ok(m) = built {
                    out.push(m);
                }
                continue;
            }

            if t == "*" {
                out.push(FilterMatcher::Any);
                continue;
            }

            if t.contains('*') || t.contains('?') {
                let p = normalize_extension_glob(t);
                out.push(FilterMatcher::ExtensionGlob(p));
                continue;
            }

            if let Some(ext) = plain_extension_token(t) {
                out.push(FilterMatcher::Extension(ext.to_string()));
            }
        }
    }
    out
}

#[cfg(test)]
fn matches_filters(name: &str, filters: &[FileFilter]) -> bool {
    let matchers = compile_filter_matchers(filters);
    matches_filter_matchers(name, &matchers)
}

fn matches_filter_matchers(name: &str, matchers: &[FilterMatcher]) -> bool {
    if matchers.is_empty() {
        return true;
    }
    let name_lower = name.to_lowercase();
    let ext_full = full_extension_lower(&name_lower);

    matchers.iter().any(|m| match m {
        FilterMatcher::Any => true,
        FilterMatcher::Extension(ext) => has_extension_suffix(&name_lower, ext),
        FilterMatcher::ExtensionGlob(pat) => wildcard_match(pat.as_str(), ext_full),
        FilterMatcher::NameRegex(re) => re.is_match(name),
    })
}

fn parse_regex_token(token: &str) -> Option<&str> {
    let t = token.trim();
    if t.starts_with("((") && t.ends_with("))") && t.len() >= 4 {
        Some(&t[2..t.len() - 2])
    } else {
        None
    }
}

fn plain_extension_token(token: &str) -> Option<&str> {
    let t = token.trim().trim_start_matches('.');
    if t.is_empty() {
        return None;
    }
    if parse_regex_token(t).is_some() {
        return None;
    }
    if t.contains('*') || t.contains('?') {
        return None;
    }
    Some(t)
}

fn normalize_extension_glob(token: &str) -> String {
    let t = token.trim().to_lowercase();
    if t.starts_with('.') || t.starts_with('*') || t.starts_with('?') {
        t
    } else {
        format!(".{t}")
    }
}

fn full_extension_lower(name_lower: &str) -> &str {
    name_lower.find('.').map(|i| &name_lower[i..]).unwrap_or("")
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    // Basic glob matcher supporting `*` and `?`.
    //
    // - `*` matches any sequence (including empty)
    // - `?` matches any single byte
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star_pi: Option<usize> = None;
    let mut star_ti: usize = 0;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
            continue;
        }
        if pi < p.len() && p[pi] == b'*' {
            star_pi = Some(pi);
            pi += 1;
            star_ti = ti;
            continue;
        }
        if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
            continue;
        }
        return false;
    }

    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
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

fn select_range_by_name_capped(
    view_names: &[String],
    anchor: &str,
    target: &str,
    cap: usize,
) -> Option<Vec<String>> {
    let ia = view_names.iter().position(|s| s == anchor)?;
    let it = view_names.iter().position(|s| s == target)?;
    let (lo, hi) = if ia <= it { (ia, it) } else { (it, ia) };
    let mut range = view_names[lo..=hi].to_vec();
    if cap != usize::MAX && range.len() > cap {
        if it >= ia {
            let start = range.len() - cap;
            range = range[start..].to_vec();
        } else {
            range.truncate(cap);
        }
    }
    Some(range)
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
    let matchers = compile_filter_matchers(&eff_filters);
    match mode {
        DialogMode::PickFolder => {
            sel.paths.push(cwd.to_path_buf());
        }
        DialogMode::OpenFile | DialogMode::OpenFiles => {
            if selected_names.is_empty() {
                return Err(FileDialogError::InvalidPath("no selection".into()));
            }
            for n in selected_names {
                if !matches_filter_matchers(&n, &matchers) {
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
        .and_then(|f| f.extensions.iter().find_map(|s| plain_extension_token(s)))
        .map(|s| s.trim_start_matches('.'));
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
    show_hidden: bool,
    filters: &[FileFilter],
    active_filter: Option<usize>,
    search: &str,
) {
    let display_filters = effective_filters(filters, active_filter);
    let matchers = compile_filter_matchers(&display_filters);
    let search_lower = if search.is_empty() {
        None
    } else {
        Some(search.to_lowercase())
    };
    entries.retain(|e| {
        if !show_hidden && e.name.starts_with('.') {
            return false;
        }
        let pass_kind = if matches!(mode, DialogMode::PickFolder) {
            e.is_dir
        } else {
            e.is_dir || matches_filter_matchers(&e.name, &matchers)
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
            SortBy::Name => {
                let al = a.name.to_lowercase();
                let bl = b.name.to_lowercase();
                natural_cmp_lower(&al, &bl)
            }
            SortBy::Extension => {
                use std::cmp::Ordering;
                let al = a.name.to_lowercase();
                let bl = b.name.to_lowercase();
                let ae = full_extension_lower(&al);
                let be = full_extension_lower(&bl);
                let ord = natural_cmp_lower(ae, be);
                if ord == Ordering::Equal {
                    natural_cmp_lower(&al, &bl)
                } else {
                    ord
                }
            }
            SortBy::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortBy::Modified => a.modified.cmp(&b.modified),
        };
        if sort_ascending { ord } else { ord.reverse() }
    });
}

fn natural_cmp_lower(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let (mut i, mut j) = (0usize, 0usize);

    while i < ab.len() && j < bb.len() {
        let ca = ab[i];
        let cb = bb[j];

        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let (a_end, a_trim, a_trim_end) = scan_number(ab, i);
            let (b_end, b_trim, b_trim_end) = scan_number(bb, j);

            let a_len = a_trim_end.saturating_sub(a_trim);
            let b_len = b_trim_end.saturating_sub(b_trim);

            let ord = match a_len.cmp(&b_len) {
                Ordering::Equal => ab[a_trim..a_trim_end].cmp(&bb[b_trim..b_trim_end]),
                o => o,
            };

            if ord != Ordering::Equal {
                return ord;
            }

            // Same numeric value: shorter (fewer leading zeros) sorts first.
            let ord = (a_end - i).cmp(&(b_end - j));
            if ord != Ordering::Equal {
                return ord;
            }

            i = a_end;
            j = b_end;
            continue;
        }

        if ca != cb {
            return ca.cmp(&cb);
        }
        i += 1;
        j += 1;
    }

    a.len().cmp(&b.len())
}

fn scan_number(bytes: &[u8], start: usize) -> (usize, usize, usize) {
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    let mut trim = start;
    while trim < end && bytes[trim] == b'0' {
        trim += 1;
    }
    let trim_end = if trim == end { end } else { end };
    (end, trim, trim_end)
}

fn read_entries_snapshot_with_fs(fs: &dyn FileSystem, dir: &Path) -> DirSnapshot {
    let mut out = Vec::new();
    let Ok(rd) = fs.read_dir(dir) else {
        return DirSnapshot {
            cwd: dir.to_path_buf(),
            entry_count: out.len(),
            entries: out,
        };
    };
    for e in rd {
        let meta = FileMeta {
            is_dir: e.is_dir,
            size: e.size,
            modified: e.modified,
        };
        out.push(DirEntry {
            name: e.name,
            path: e.path,
            is_dir: meta.is_dir,
            size: meta.size,
            modified: meta.modified,
        });
    }
    DirSnapshot {
        cwd: dir.to_path_buf(),
        entry_count: out.len(),
        entries: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::StdFileSystem;
    use std::cell::Cell;

    fn mods(ctrl: bool, shift: bool) -> Modifiers {
        Modifiers { ctrl, shift }
    }

    #[derive(Default)]
    struct TestFs {
        meta: std::collections::HashMap<PathBuf, crate::fs::FsMetadata>,
        entries: Vec<crate::fs::FsEntry>,
        read_dir_calls: Cell<usize>,
    }

    impl crate::fs::FileSystem for TestFs {
        fn read_dir(&self, _dir: &Path) -> std::io::Result<Vec<crate::fs::FsEntry>> {
            self.read_dir_calls.set(self.read_dir_calls.get() + 1);
            Ok(self.entries.clone())
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

        fn create_dir(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "create_dir not supported in TestFs",
            ))
        }

        fn rename(&self, _from: &Path, _to: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "rename not supported in TestFs",
            ))
        }

        fn remove_file(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "remove_file not supported in TestFs",
            ))
        }

        fn remove_dir(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "remove_dir not supported in TestFs",
            ))
        }

        fn remove_dir_all(&self, _path: &Path) -> std::io::Result<()> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "remove_dir_all not supported in TestFs",
            ))
        }

        fn copy_file(&self, _from: &Path, _to: &Path) -> std::io::Result<u64> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "copy_file not supported in TestFs",
            ))
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
    fn focus_and_select_by_name_sets_focus_and_anchor() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.focus_and_select_by_name("new_folder");
        assert_eq!(core.selected, vec!["new_folder"]);
        assert_eq!(core.focused_name.as_deref(), Some("new_folder"));
        assert_eq!(core.selection_anchor_name.as_deref(), Some("new_folder"));
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
    fn ctrl_a_respects_max_selection_cap() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.max_selection = Some(2);
        core.view_names = vec!["a".into(), "b".into(), "c".into()];
        core.select_all();
        assert_eq!(core.selected, vec!["a", "b"]);
    }

    #[test]
    fn shift_click_respects_max_selection_cap_and_keeps_target() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.max_selection = Some(2);
        core.view_names = vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()];
        core.click_entry("b".into(), false, mods(false, false));
        core.click_entry("e".into(), false, mods(false, true));
        assert_eq!(core.selected, vec!["d", "e"]);

        core.click_entry("d".into(), false, mods(false, false));
        core.click_entry("b".into(), false, mods(false, true));
        assert_eq!(core.selected, vec!["b", "c"]);
    }

    #[test]
    fn ctrl_click_caps_by_dropping_oldest_selected() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.max_selection = Some(2);
        core.view_names = vec!["a".into(), "b".into(), "c".into()];
        core.click_entry("a".into(), false, mods(false, false));
        core.click_entry("b".into(), false, mods(true, false));
        assert_eq!(core.selected, vec!["a", "b"]);
        core.click_entry("c".into(), false, mods(true, false));
        assert_eq!(core.selected, vec!["b", "c"]);
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
    fn handle_event_activate_focused_requests_confirm() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.view_names = vec!["a.txt".into()];
        core.focused_name = Some("a.txt".into());

        let outcome = core.handle_event(CoreEvent::ActivateFocused);
        assert_eq!(outcome, CoreEventOutcome::RequestConfirm);
        assert_eq!(core.selected, vec!["a.txt"]);
    }

    #[test]
    fn handle_event_double_click_file_requests_confirm() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);

        let outcome = core.handle_event(CoreEvent::DoubleClickEntry {
            name: "a.txt".into(),
            is_dir: false,
        });

        assert_eq!(outcome, CoreEventOutcome::RequestConfirm);
        assert_eq!(core.selected, vec!["a.txt"]);
    }

    #[test]
    fn handle_event_navigate_up_updates_cwd() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp/child");

        let outcome = core.handle_event(CoreEvent::NavigateUp);

        assert_eq!(outcome, CoreEventOutcome::None);
        assert_eq!(core.cwd, PathBuf::from("/tmp"));
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
    fn matches_filters_supports_extension_globs() {
        let filters = vec![FileFilter::new(
            "VS-ish",
            vec![".vcx*".to_string(), ".*.filters".to_string()],
        )];
        assert!(matches_filters("proj.vcxproj.filters", &filters));
        assert!(matches_filters("proj.vcxproj", &filters));
        assert!(!matches_filters("README", &filters));
    }

    #[test]
    fn matches_filters_supports_regex_tokens() {
        let filters = vec![FileFilter::new(
            "Re",
            vec![r"((^imgui_.*\.rs$))".to_string()],
        )];
        assert!(matches_filters("imgui_demo.rs", &filters));
        assert!(matches_filters("ImGui_DEMO.RS", &filters));
        assert!(!matches_filters("demo_imgui.rs", &filters));
    }

    #[test]
    fn natural_sort_orders_digit_runs() {
        let mut entries = vec![
            DirEntry {
                name: "file10.txt".into(),
                path: PathBuf::from("/tmp/file10.txt"),
                is_dir: false,
                size: None,
                modified: None,
            },
            DirEntry {
                name: "file2.txt".into(),
                path: PathBuf::from("/tmp/file2.txt"),
                is_dir: false,
                size: None,
                modified: None,
            },
            DirEntry {
                name: "file1.txt".into(),
                path: PathBuf::from("/tmp/file1.txt"),
                is_dir: false,
                size: None,
                modified: None,
            },
        ];
        sort_entries_in_place(&mut entries, SortBy::Name, true, false);
        let names: Vec<_> = entries.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["file1.txt", "file2.txt", "file10.txt"]);
    }

    #[test]
    fn sort_by_extension_orders_by_full_extension_then_name() {
        let mut entries = vec![
            DirEntry {
                name: "alpha.tar.gz".into(),
                path: PathBuf::from("/tmp/alpha.tar.gz"),
                is_dir: false,
                size: None,
                modified: None,
            },
            DirEntry {
                name: "beta.rs".into(),
                path: PathBuf::from("/tmp/beta.rs"),
                is_dir: false,
                size: None,
                modified: None,
            },
            DirEntry {
                name: "gamma.tar.gz".into(),
                path: PathBuf::from("/tmp/gamma.tar.gz"),
                is_dir: false,
                size: None,
                modified: None,
            },
            DirEntry {
                name: "noext".into(),
                path: PathBuf::from("/tmp/noext"),
                is_dir: false,
                size: None,
                modified: None,
            },
        ];

        sort_entries_in_place(&mut entries, SortBy::Extension, true, false);
        let names: Vec<_> = entries.into_iter().map(|e| e.name).collect();
        assert_eq!(
            names,
            vec!["noext", "beta.rs", "alpha.tar.gz", "gamma.tar.gz"]
        );
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

    #[test]
    fn rescan_if_needed_caches_directory_listing() {
        let fs = TestFs {
            entries: vec![
                crate::fs::FsEntry {
                    name: "a.txt".into(),
                    path: PathBuf::from("/tmp/a.txt"),
                    is_dir: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "b.txt".into(),
                    path: PathBuf::from("/tmp/b.txt"),
                    is_dir: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: ".hidden".into(),
                    path: PathBuf::from("/tmp/.hidden"),
                    is_dir: false,
                    size: None,
                    modified: None,
                },
            ],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");

        core.rescan_if_needed(&fs);
        assert_eq!(fs.read_dir_calls.get(), 1);
        assert!(core.entries().iter().all(|e| e.name != ".hidden"));

        // Same key => no rescan, no fs hit.
        core.rescan_if_needed(&fs);
        assert_eq!(fs.read_dir_calls.get(), 1);

        // View-only changes should rebuild without hitting fs again.
        core.search = "b".into();
        core.rescan_if_needed(&fs);
        assert_eq!(fs.read_dir_calls.get(), 1);
        assert_eq!(core.entries().len(), 1);
        assert_eq!(core.entries()[0].name, "b.txt");

        core.search.clear();
        core.show_hidden = true;
        core.rescan_if_needed(&fs);
        assert_eq!(fs.read_dir_calls.get(), 1);
        assert!(core.entries().iter().any(|e| e.name == ".hidden"));

        // Explicit refresh should hit fs again even if the view inputs didn't change.
        core.invalidate_dir_cache();
        core.rescan_if_needed(&fs);
        assert_eq!(fs.read_dir_calls.get(), 2);

        // Changing cwd should refresh snapshot.
        core.set_cwd(PathBuf::from("/other"));
        core.rescan_if_needed(&fs);
        assert_eq!(fs.read_dir_calls.get(), 3);
    }
}
