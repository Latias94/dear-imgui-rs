use std::path::{Path, PathBuf};
use std::{
    collections::{VecDeque, hash_map::DefaultHasher},
    hash::Hasher,
};

use crate::core::{
    ClickAction, DialogMode, ExtensionPolicy, FileDialogError, FileFilter, SavePolicy, Selection,
    SortBy,
};
use crate::fs::{FileSystem, FsEntry};
use crate::places::Places;
use indexmap::IndexSet;
use regex::RegexBuilder;

#[cfg(feature = "tracing")]
use tracing::trace;

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
        /// Entry identity in current view.
        id: EntryId,
        /// Modifier keys used by selection semantics.
        modifiers: Modifiers,
    },
    /// Double-click an entry row/cell.
    DoubleClickEntry {
        /// Entry identity in current view.
        id: EntryId,
    },
    /// Type-to-select prefix.
    SelectByPrefix(String),
    /// Activate focused entry (Enter).
    ActivateFocused,
    /// Navigate to parent directory.
    NavigateUp,
    /// Navigate to a target directory.
    NavigateTo(PathBuf),
    /// Focus and select one entry by id.
    FocusAndSelectById(EntryId),
    /// Replace current selection by entry ids.
    ReplaceSelectionByIds(Vec<EntryId>),
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

    /// Build a stable entry id from an absolute or relative path.
    pub fn from_path(path: &Path) -> Self {
        let mut hasher = DefaultHasher::new();
        hasher.write(path.to_string_lossy().as_bytes());
        Self::new(hasher.finish())
    }
}

/// Decision returned by a scan hook for one filesystem entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanHookAction {
    /// Keep the entry in the directory snapshot.
    Keep,
    /// Drop the entry before filter/sort/view processing.
    Drop,
}

/// Directory scan strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanPolicy {
    /// Run scan synchronously on the caller thread.
    Sync,
    /// Consume scan output incrementally in bounded entry batches.
    Incremental {
        /// Max number of entries per batch.
        batch_entries: usize,
    },
    /// Run scan on background worker and consume batches in UI ticks.
    Background {
        /// Max number of entries per batch.
        batch_entries: usize,
        /// Debounce interval for rapid rescan requests.
        debounce_ms: u64,
    },
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self::Sync
    }
}

impl ScanPolicy {
    fn normalized(self) -> Self {
        match self {
            Self::Sync => Self::Sync,
            Self::Incremental { batch_entries } => Self::Incremental {
                batch_entries: batch_entries.max(1),
            },
            Self::Background {
                batch_entries,
                debounce_ms,
            } => Self::Background {
                batch_entries: batch_entries.max(1),
                debounce_ms,
            },
        }
    }
}

/// Immutable scan request descriptor.
#[derive(Clone, Debug)]
pub struct ScanRequest {
    /// Monotonic scan generation.
    pub generation: u64,
    /// Directory being scanned.
    pub cwd: PathBuf,
    /// Scan policy at submission time.
    pub scan_policy: ScanPolicy,
    /// Submission timestamp.
    pub submitted_at: std::time::Instant,
}

/// One batch emitted by the scan pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanBatch {
    /// Batch generation.
    pub generation: u64,
    /// Batch payload kind.
    pub kind: ScanBatchKind,
    /// Whether this batch completes the generation.
    pub is_final: bool,
}

impl ScanBatch {
    fn begin(generation: u64) -> Self {
        Self {
            generation,
            kind: ScanBatchKind::Begin,
            is_final: false,
        }
    }

    fn entries(generation: u64, loaded: usize, is_final: bool) -> Self {
        Self {
            generation,
            kind: ScanBatchKind::Entries { loaded },
            is_final,
        }
    }

    fn complete(generation: u64, loaded: usize) -> Self {
        Self {
            generation,
            kind: ScanBatchKind::Complete { loaded },
            is_final: true,
        }
    }

    fn error(generation: u64, message: String) -> Self {
        Self {
            generation,
            kind: ScanBatchKind::Error { message },
            is_final: true,
        }
    }
}

/// Payload kind for [`ScanBatch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanBatchKind {
    /// Scan generation started.
    Begin,
    /// A payload with currently loaded entry count.
    Entries {
        /// Number of entries currently loaded.
        loaded: usize,
    },
    /// Scan generation completed.
    Complete {
        /// Final loaded entry count.
        loaded: usize,
    },
    /// Scan generation failed.
    Error {
        /// Human-readable error message.
        message: String,
    },
}

/// Current scan status for the dialog core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanStatus {
    /// No scan is currently running.
    Idle,
    /// A scan generation is running.
    Scanning {
        /// Active generation id.
        generation: u64,
    },
    /// A scan generation has partial data.
    Partial {
        /// Active generation id.
        generation: u64,
        /// Number of currently loaded entries.
        loaded: usize,
    },
    /// A scan generation finished successfully.
    Complete {
        /// Completed generation id.
        generation: u64,
        /// Final number of loaded entries.
        loaded: usize,
    },
    /// A scan generation failed.
    Failed {
        /// Failed generation id.
        generation: u64,
        /// Error message captured from filesystem backend.
        message: String,
    },
}

impl Default for ScanStatus {
    fn default() -> Self {
        Self::Idle
    }
}

type ScanHookFn = dyn FnMut(&mut FsEntry) -> ScanHookAction + 'static;

struct ScanHook {
    inner: Box<ScanHookFn>,
}

impl std::fmt::Debug for ScanHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanHook").finish_non_exhaustive()
    }
}

impl ScanHook {
    fn new<F>(hook: F) -> Self
    where
        F: FnMut(&mut FsEntry) -> ScanHookAction + 'static,
    {
        Self {
            inner: Box::new(hook),
        }
    }

    fn apply(&mut self, entry: &mut FsEntry) -> ScanHookAction {
        (self.inner)(entry)
    }
}

/// Rich metadata attached to a filesystem entry.
#[derive(Clone, Debug)]
pub struct FileMeta {
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Whether this entry is a symbolic link.
    pub is_symlink: bool,
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
    /// Stable entry id.
    pub(crate) id: EntryId,
    /// Base name (no parent path).
    pub(crate) name: String,
    /// Full path.
    pub(crate) path: PathBuf,
    /// Whether this entry is a directory.
    pub(crate) is_dir: bool,
    /// Whether this entry itself is a symbolic link.
    pub(crate) is_symlink: bool,
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
        self.id
    }
}

#[derive(Debug)]
enum ScanRuntime {
    Sync(SyncScanRuntime),
    Worker(WorkerScanRuntime),
}

impl Default for ScanRuntime {
    fn default() -> Self {
        Self::Sync(SyncScanRuntime::default())
    }
}

impl ScanRuntime {
    fn from_policy(policy: ScanPolicy) -> Self {
        match policy {
            ScanPolicy::Sync => Self::Sync(SyncScanRuntime::default()),
            ScanPolicy::Incremental { batch_entries } => {
                Self::Worker(WorkerScanRuntime::new(batch_entries))
            }
            ScanPolicy::Background { batch_entries, .. } => {
                Self::Worker(WorkerScanRuntime::new(batch_entries))
            }
        }
    }

    fn set_policy(&mut self, policy: ScanPolicy) {
        match policy {
            ScanPolicy::Sync => {
                if !matches!(self, Self::Sync(_)) {
                    *self = Self::Sync(SyncScanRuntime::default());
                }
            }
            ScanPolicy::Incremental { batch_entries }
            | ScanPolicy::Background { batch_entries, .. } => {
                if let Self::Worker(runtime) = self {
                    runtime.set_batch_entries(batch_entries);
                } else {
                    *self = Self::Worker(WorkerScanRuntime::new(batch_entries));
                }
            }
        }
    }

    fn submit(&mut self, request: ScanRequest, result: std::io::Result<DirSnapshot>) {
        match self {
            Self::Sync(runtime) => runtime.submit(request, result),
            Self::Worker(runtime) => runtime.submit(request, result),
        }
    }

    fn poll_batch(&mut self) -> Option<RuntimeBatch> {
        match self {
            Self::Sync(runtime) => runtime.poll_batch(),
            Self::Worker(runtime) => runtime.poll_batch(),
        }
    }

    fn cancel_generation(&mut self, generation: u64) {
        match self {
            Self::Sync(runtime) => runtime.cancel_generation(generation),
            Self::Worker(runtime) => runtime.cancel_generation(generation),
        }
    }
}

#[derive(Debug, Default)]
struct SyncScanRuntime {
    batches: VecDeque<RuntimeBatch>,
}

impl SyncScanRuntime {
    fn submit(&mut self, request: ScanRequest, result: std::io::Result<DirSnapshot>) {
        self.batches.clear();
        self.batches.push_back(RuntimeBatch {
            generation: request.generation,
            kind: RuntimeBatchKind::Begin {
                cwd: request.cwd.clone(),
            },
        });

        match result {
            Ok(snapshot) => {
                let loaded = snapshot.entry_count;
                self.batches.push_back(RuntimeBatch {
                    generation: request.generation,
                    kind: RuntimeBatchKind::ReplaceSnapshot { snapshot },
                });
                self.batches.push_back(RuntimeBatch {
                    generation: request.generation,
                    kind: RuntimeBatchKind::Complete { loaded },
                });
            }
            Err(err) => {
                self.batches.push_back(RuntimeBatch {
                    generation: request.generation,
                    kind: RuntimeBatchKind::Error {
                        cwd: request.cwd,
                        message: err.to_string(),
                    },
                });
            }
        }
    }

    fn poll_batch(&mut self) -> Option<RuntimeBatch> {
        self.batches.pop_front()
    }

    fn cancel_generation(&mut self, generation: u64) {
        self.batches.retain(|batch| batch.generation != generation);
    }
}

#[derive(Debug)]
struct WorkerScanRuntime {
    batches: VecDeque<RuntimeBatch>,
    batch_entries: usize,
}

impl WorkerScanRuntime {
    fn new(batch_entries: usize) -> Self {
        Self {
            batches: VecDeque::new(),
            batch_entries: batch_entries.max(1),
        }
    }

    fn set_batch_entries(&mut self, batch_entries: usize) {
        self.batch_entries = batch_entries.max(1);
    }

    fn submit(&mut self, request: ScanRequest, result: std::io::Result<DirSnapshot>) {
        self.batches.clear();
        self.batches.push_back(RuntimeBatch {
            generation: request.generation,
            kind: RuntimeBatchKind::Begin {
                cwd: request.cwd.clone(),
            },
        });

        match result {
            Ok(snapshot) => {
                let DirSnapshot { cwd, entries, .. } = snapshot;
                let total = entries.len();
                let mut loaded = 0usize;
                let mut chunk = Vec::with_capacity(self.batch_entries);
                for entry in entries {
                    chunk.push(entry);
                    if chunk.len() >= self.batch_entries {
                        loaded += chunk.len();
                        self.batches.push_back(RuntimeBatch {
                            generation: request.generation,
                            kind: RuntimeBatchKind::AppendEntries {
                                cwd: cwd.clone(),
                                entries: std::mem::take(&mut chunk),
                                loaded,
                            },
                        });
                    }
                }

                if !chunk.is_empty() {
                    loaded += chunk.len();
                    self.batches.push_back(RuntimeBatch {
                        generation: request.generation,
                        kind: RuntimeBatchKind::AppendEntries {
                            cwd: cwd.clone(),
                            entries: chunk,
                            loaded,
                        },
                    });
                }

                self.batches.push_back(RuntimeBatch {
                    generation: request.generation,
                    kind: RuntimeBatchKind::Complete { loaded: total },
                });
            }
            Err(err) => {
                self.batches.push_back(RuntimeBatch {
                    generation: request.generation,
                    kind: RuntimeBatchKind::Error {
                        cwd: request.cwd,
                        message: err.to_string(),
                    },
                });
            }
        }
    }

    fn poll_batch(&mut self) -> Option<RuntimeBatch> {
        self.batches.pop_front()
    }

    fn cancel_generation(&mut self, generation: u64) {
        self.batches.retain(|batch| batch.generation != generation);
    }
}

#[derive(Debug)]
struct RuntimeBatch {
    generation: u64,
    kind: RuntimeBatchKind,
}

#[derive(Debug)]
enum RuntimeBatchKind {
    Begin {
        cwd: PathBuf,
    },
    ReplaceSnapshot {
        snapshot: DirSnapshot,
    },
    AppendEntries {
        cwd: PathBuf,
        entries: Vec<DirEntry>,
        loaded: usize,
    },
    Complete {
        loaded: usize,
    },
    Error {
        cwd: PathBuf,
        message: String,
    },
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
    selected_ids: IndexSet<EntryId>,
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
    focused_id: Option<EntryId>,
    selection_anchor_id: Option<EntryId>,
    view_names: Vec<String>,
    view_ids: Vec<EntryId>,
    entries: Vec<DirEntry>,

    scan_hook: Option<ScanHook>,
    scan_policy: ScanPolicy,
    scan_status: ScanStatus,
    scan_generation: u64,
    scan_started_at: Option<std::time::Instant>,
    scan_runtime: ScanRuntime,
    dir_snapshot: DirSnapshot,
    dir_snapshot_dirty: bool,
    last_view_key: Option<ViewKey>,
}

impl FileDialogCore {
    /// Creates a new dialog core for a mode.
    pub fn new(mode: DialogMode) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            mode,
            cwd,
            selected_ids: IndexSet::new(),
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
            focused_id: None,
            selection_anchor_id: None,
            view_names: Vec::new(),
            view_ids: Vec::new(),
            entries: Vec::new(),
            scan_hook: None,
            scan_policy: ScanPolicy::default(),
            scan_status: ScanStatus::default(),
            scan_generation: 0,
            scan_started_at: None,
            scan_runtime: ScanRuntime::from_policy(ScanPolicy::default()),
            dir_snapshot: DirSnapshot {
                cwd: PathBuf::new(),
                entry_count: 0,
                entries: Vec::new(),
            },
            dir_snapshot_dirty: true,
            last_view_key: None,
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
            CoreEvent::ClickEntry { id, modifiers } => {
                self.click_entry(id, modifiers);
                CoreEventOutcome::None
            }
            CoreEvent::DoubleClickEntry { id } => {
                if self.double_click_entry(id) {
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
            CoreEvent::FocusAndSelectById(id) => {
                self.focus_and_select_by_id(id);
                CoreEventOutcome::None
            }
            CoreEvent::ReplaceSelectionByIds(ids) => {
                self.replace_selection_by_ids(ids);
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

    /// Returns the currently configured scan policy.
    pub fn scan_policy(&self) -> ScanPolicy {
        self.scan_policy
    }

    /// Sets scan policy for future directory refreshes.
    ///
    /// Values are normalized to avoid invalid batch sizes.
    /// Calling this invalidates the directory cache when policy changes.
    pub fn set_scan_policy(&mut self, policy: ScanPolicy) {
        let normalized = policy.normalized();
        if self.scan_policy == normalized {
            return;
        }
        self.scan_policy = normalized;
        self.scan_runtime.set_policy(normalized);
        self.invalidate_dir_cache();
    }

    /// Returns the latest issued scan generation.
    pub fn scan_generation(&self) -> u64 {
        self.scan_generation
    }

    /// Returns the current scan status.
    pub fn scan_status(&self) -> &ScanStatus {
        &self.scan_status
    }

    /// Requests a rescan on the next refresh tick.
    pub fn request_rescan(&mut self) {
        self.invalidate_dir_cache();
    }

    /// Installs a scan hook that can mutate or drop filesystem entries.
    ///
    /// The hook runs before filtering/sorting and before snapshot ids are built.
    /// Calling this invalidates the directory cache.
    pub fn set_scan_hook<F>(&mut self, hook: F)
    where
        F: FnMut(&mut FsEntry) -> ScanHookAction + 'static,
    {
        self.scan_hook = Some(ScanHook::new(hook));
        self.invalidate_dir_cache();
    }

    /// Clears the scan hook and reverts to raw filesystem entries.
    ///
    /// Calling this invalidates the directory cache.
    pub fn clear_scan_hook(&mut self) {
        if self.scan_hook.is_none() {
            return;
        }
        self.scan_hook = None;
        self.invalidate_dir_cache();
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

    /// Selects and focuses a single entry by id.
    pub fn focus_and_select_by_id(&mut self, id: EntryId) {
        self.select_single_by_id(id);
    }

    /// Replace selection by entry ids.
    pub fn replace_selection_by_ids<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = EntryId>,
    {
        self.selected_ids.clear();
        for id in ids {
            self.selected_ids.insert(id);
        }
        self.enforce_selection_cap();
        let last = self.selected_ids.iter().next_back().copied();
        self.focused_id = last;
        self.selection_anchor_id = last;
    }

    /// Clear current selection, focus and anchor.
    pub fn clear_selection(&mut self) {
        self.selected_ids.clear();
        self.focused_id = None;
        self.selection_anchor_id = None;
    }

    /// Returns the number of currently selected entries.
    pub fn selected_len(&self) -> usize {
        self.selected_ids.len()
    }

    /// Returns whether there is at least one selected entry.
    pub fn has_selection(&self) -> bool {
        !self.selected_ids.is_empty()
    }

    /// Returns selected entry ids in deterministic selection order.
    pub fn selected_entry_ids(&self) -> Vec<EntryId> {
        self.selected_ids.iter().copied().collect()
    }

    /// Resolves selected entry paths from ids in the current snapshot.
    ///
    /// Any ids that are not currently resolvable are skipped.
    pub fn selected_entry_paths(&self) -> Vec<PathBuf> {
        self.selected_ids
            .iter()
            .filter_map(|id| self.entry_path_by_id(*id).map(Path::to_path_buf))
            .collect()
    }

    /// Counts selected files and directories in the current snapshot.
    ///
    /// Returns `(files, dirs)`. Any ids that are not currently resolvable are skipped.
    pub fn selected_entry_counts(&self) -> (usize, usize) {
        self.selected_ids
            .iter()
            .filter_map(|id| self.entry_by_id(*id))
            .fold((0usize, 0usize), |(files, dirs), entry| {
                if entry.is_dir {
                    (files, dirs + 1)
                } else {
                    (files + 1, dirs)
                }
            })
    }
    /// Resolves an entry path from an entry id in the current snapshot.
    ///
    /// Returns `None` when the id is not currently resolvable (for example,
    /// before the next rescan after create/rename/paste).
    pub fn entry_path_by_id(&self, id: EntryId) -> Option<&Path> {
        self.entry_by_id(id).map(|entry| entry.path.as_path())
    }

    /// Returns the currently focused entry id, if any.
    pub fn focused_entry_id(&self) -> Option<EntryId> {
        self.focused_id
    }

    pub(crate) fn is_selected_id(&self, id: EntryId) -> bool {
        self.selected_ids.contains(&id)
    }

    /// Refreshes the directory snapshot and view cache if needed.
    pub(crate) fn rescan_if_needed(&mut self, fs: &dyn FileSystem) {
        self.refresh_dir_snapshot_if_needed(fs);

        let key = ViewKey::new(self);
        if self.last_view_key.as_ref() == Some(&key) {
            return;
        }

        let rebuild_reason = if self.last_view_key.is_none() {
            "snapshot_or_forced"
        } else {
            "view_inputs_changed"
        };
        let rebuild_started_at = std::time::Instant::now();

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
        self.retain_selected_visible();
        self.last_view_key = Some(key);

        trace_projector_rebuild(
            rebuild_reason,
            self.entries.len(),
            rebuild_started_at.elapsed().as_micros(),
        );
    }

    fn refresh_dir_snapshot_if_needed(&mut self, fs: &dyn FileSystem) {
        let cwd_changed = self.dir_snapshot.cwd != self.cwd;
        let should_refresh = self.dir_snapshot_dirty || cwd_changed;

        if should_refresh {
            let request = self.begin_scan_request();
            self.scan_runtime
                .cancel_generation(request.generation.saturating_sub(1));
            let scan_result =
                read_entries_snapshot_with_fs(fs, &request.cwd, self.scan_hook.as_mut());
            self.scan_runtime.submit(request, scan_result);
            self.dir_snapshot_dirty = false;
        }

        let mut budget = self.scan_runtime_batch_budget();
        while budget > 0 {
            let Some(runtime_batch) = self.scan_runtime.poll_batch() else {
                break;
            };
            self.apply_runtime_batch(runtime_batch);
            budget = budget.saturating_sub(1);
        }
    }

    fn scan_runtime_batch_budget(&self) -> usize {
        match self.scan_policy {
            ScanPolicy::Sync => usize::MAX,
            ScanPolicy::Incremental { .. } | ScanPolicy::Background { .. } => 1,
        }
    }

    fn begin_scan_request(&mut self) -> ScanRequest {
        let generation = self.scan_generation.saturating_add(1);
        self.scan_generation = generation;
        let request = ScanRequest {
            generation,
            cwd: self.cwd.clone(),
            scan_policy: self.scan_policy,
            submitted_at: std::time::Instant::now(),
        };
        trace_scan_requested(&request);
        request
    }

    fn apply_runtime_batch(&mut self, runtime_batch: RuntimeBatch) {
        if runtime_batch.generation != self.scan_generation {
            trace_scan_dropped_stale_batch(
                runtime_batch.generation,
                self.scan_generation,
                "runtime_batch",
            );
            return;
        }

        match runtime_batch.kind {
            RuntimeBatchKind::Begin { cwd } => {
                self.dir_snapshot = empty_snapshot_for_cwd(&cwd);
                self.last_view_key = None;
                self.apply_scan_batch(ScanBatch::begin(runtime_batch.generation));
                trace_scan_batch_applied(runtime_batch.generation, 0, "begin");
            }
            RuntimeBatchKind::ReplaceSnapshot { snapshot } => {
                let loaded = snapshot.entry_count;
                self.dir_snapshot = snapshot;
                self.last_view_key = None;
                self.apply_scan_batch(ScanBatch::entries(runtime_batch.generation, loaded, false));
                trace_scan_batch_applied(runtime_batch.generation, loaded, "replace_snapshot");
            }
            RuntimeBatchKind::AppendEntries {
                cwd,
                entries,
                loaded,
            } => {
                if self.dir_snapshot.cwd != cwd {
                    self.dir_snapshot = empty_snapshot_for_cwd(&cwd);
                }
                let batch_entries = entries.len();
                self.dir_snapshot.entries.extend(entries);
                self.dir_snapshot.entry_count = self.dir_snapshot.entries.len();
                self.last_view_key = None;
                self.apply_scan_batch(ScanBatch::entries(runtime_batch.generation, loaded, false));
                trace_scan_batch_applied(runtime_batch.generation, batch_entries, "append_entries");
            }
            RuntimeBatchKind::Complete { loaded } => {
                self.last_view_key = None;
                self.apply_scan_batch(ScanBatch::complete(runtime_batch.generation, loaded));
                trace_scan_batch_applied(runtime_batch.generation, 0, "complete");
            }
            RuntimeBatchKind::Error { cwd, message } => {
                self.dir_snapshot = empty_snapshot_for_cwd(&cwd);
                self.last_view_key = None;
                self.apply_scan_batch(ScanBatch::error(runtime_batch.generation, message));
                trace_scan_batch_applied(runtime_batch.generation, 0, "error");
            }
        }
    }

    fn apply_scan_batch(&mut self, batch: ScanBatch) {
        if batch.generation != self.scan_generation {
            trace_scan_dropped_stale_batch(batch.generation, self.scan_generation, "scan_batch");
            return;
        }

        self.scan_status = match batch.kind {
            ScanBatchKind::Begin => {
                self.scan_started_at = Some(std::time::Instant::now());
                ScanStatus::Scanning {
                    generation: batch.generation,
                }
            }
            ScanBatchKind::Entries { loaded } => {
                if batch.is_final {
                    let duration_ms = self
                        .scan_started_at
                        .take()
                        .map(|started| started.elapsed().as_millis())
                        .unwrap_or(0);
                    trace_scan_completed(batch.generation, loaded, duration_ms);
                    ScanStatus::Complete {
                        generation: batch.generation,
                        loaded,
                    }
                } else {
                    ScanStatus::Partial {
                        generation: batch.generation,
                        loaded,
                    }
                }
            }
            ScanBatchKind::Complete { loaded } => {
                let duration_ms = self
                    .scan_started_at
                    .take()
                    .map(|started| started.elapsed().as_millis())
                    .unwrap_or(0);
                trace_scan_completed(batch.generation, loaded, duration_ms);
                ScanStatus::Complete {
                    generation: batch.generation,
                    loaded,
                }
            }
            ScanBatchKind::Error { message } => {
                self.scan_started_at = None;
                ScanStatus::Failed {
                    generation: batch.generation,
                    message,
                }
            }
        };
    }

    fn entry_by_id(&self, id: EntryId) -> Option<&DirEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn name_for_id(&self, id: EntryId) -> Option<&str> {
        self.entry_by_id(id)
            .map(|entry| entry.name.as_str())
            .or_else(|| {
                self.view_ids
                    .iter()
                    .position(|candidate| *candidate == id)
                    .and_then(|index| self.view_names.get(index).map(String::as_str))
            })
    }

    /// Select the next entry whose base name starts with the given prefix (case-insensitive).
    ///
    /// This is used by "type-to-select" navigation (IGFD-style).
    pub(crate) fn select_by_prefix(&mut self, prefix: &str) {
        let prefix = prefix.trim();
        if prefix.is_empty() || self.view_ids.is_empty() {
            return;
        }
        let prefix_lower = prefix.to_lowercase();

        let len = self.view_ids.len();
        let start_idx = self
            .focused_id
            .and_then(|focused| self.view_ids.iter().position(|id| *id == focused))
            .map(|i| (i + 1) % len)
            .unwrap_or(0);

        for offset in 0..len {
            let index = (start_idx + offset) % len;
            let id = self.view_ids[index];
            if self
                .name_for_id(id)
                .map(|name| name.to_lowercase().starts_with(&prefix_lower))
                .unwrap_or(false)
            {
                self.select_single_by_id(id);
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
        self.selected_ids.clear();
        for id in self.view_ids.iter().take(cap).copied() {
            self.selected_ids.insert(id);
        }
        let last = self.selected_ids.iter().next_back().copied();
        self.focused_id = last;
        self.selection_anchor_id = last;
    }

    /// Moves keyboard focus up/down within the current view.
    pub(crate) fn move_focus(&mut self, delta: i32, modifiers: Modifiers) {
        if self.view_ids.is_empty() {
            return;
        }

        let len = self.view_ids.len();
        let current_idx = self
            .focused_id
            .and_then(|id| self.view_ids.iter().position(|candidate| *candidate == id));
        let next_idx = match current_idx {
            Some(index) => {
                let next = index as i32 + delta;
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

        let target_id = self.view_ids[next_idx];
        if modifiers.shift {
            let anchor_id = self
                .selection_anchor_id
                .or(self.focused_id)
                .unwrap_or(target_id);
            if self.selection_anchor_id.is_none() {
                self.selection_anchor_id = Some(anchor_id);
            }
            if let Some(range) = select_range_by_id_capped(
                &self.view_ids,
                anchor_id,
                target_id,
                self.selection_cap(),
            ) {
                self.selected_ids = range.into_iter().collect();
                self.focused_id = Some(target_id);
            } else {
                self.select_single_by_id(target_id);
            }
        } else {
            self.select_single_by_id(target_id);
        }
    }

    /// Activates the focused entry (Enter).
    ///
    /// If no selection exists, the focused item becomes selected, then confirm is attempted.
    pub(crate) fn activate_focused(&mut self) -> bool {
        if self.selected_ids.is_empty() {
            if let Some(id) = self.focused_id {
                self.selected_ids.insert(id);
                self.selection_anchor_id = Some(id);
            }
        }
        !self.selected_ids.is_empty()
    }

    /// Handles a click on an entry row.
    pub(crate) fn click_entry(&mut self, id: EntryId, modifiers: Modifiers) {
        let Some(entry) = self.entry_by_id(id).cloned() else {
            return;
        };

        if entry.is_dir {
            match self.click_action {
                ClickAction::Select => {
                    self.select_single_by_id(id);
                }
                ClickAction::Navigate => {
                    self.cwd.push(&entry.name);
                    self.clear_selection();
                }
            }
            return;
        }

        if modifiers.shift {
            if let Some(anchor_id) = self.selection_anchor_id {
                if let Some(range) =
                    select_range_by_id_capped(&self.view_ids, anchor_id, id, self.selection_cap())
                {
                    self.selected_ids = range.into_iter().collect();
                    self.focused_id = Some(id);
                    return;
                }
            }
            self.select_single_by_id(id);
            return;
        }

        if self.selection_cap() <= 1 || !modifiers.ctrl {
            self.select_single_by_id(id);
            return;
        }

        toggle_select_id(&mut self.selected_ids, id);
        self.focused_id = Some(id);
        self.selection_anchor_id = Some(id);
        self.enforce_selection_cap();
    }

    /// Handles a double-click on an entry row.
    pub(crate) fn double_click_entry(&mut self, id: EntryId) -> bool {
        if !self.double_click {
            return false;
        }

        let Some(entry) = self.entry_by_id(id).cloned() else {
            return false;
        };

        if entry.is_dir {
            self.cwd.push(&entry.name);
            self.clear_selection();
            return false;
        }

        if matches!(self.mode, DialogMode::OpenFile | DialogMode::OpenFiles) {
            self.select_single_by_id(id);
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
        let selected_entries = self
            .selected_ids
            .iter()
            .filter_map(|id| self.entry_by_id(*id).cloned())
            .collect::<Vec<_>>();

        // Special-case: if a single directory selected in file-open modes, navigate into it
        // instead of confirming.
        if matches!(self.mode, DialogMode::OpenFile | DialogMode::OpenFiles)
            && selected_entries.len() == 1
            && selected_entries[0].is_dir
        {
            self.cwd.push(&selected_entries[0].name);
            self.clear_selection();
            return Ok(());
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
            selected_entries,
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

    fn select_single_by_id(&mut self, id: EntryId) {
        self.selected_ids.clear();
        self.selected_ids.insert(id);
        self.focused_id = Some(id);
        self.selection_anchor_id = Some(id);
    }

    fn selection_cap(&self) -> usize {
        if !self.allow_multi {
            return 1;
        }
        self.max_selection.unwrap_or(usize::MAX).max(1)
    }

    fn enforce_selection_cap(&mut self) {
        let cap = self.selection_cap();
        if cap == usize::MAX || self.selected_ids.len() <= cap {
            return;
        }
        while self.selected_ids.len() > cap {
            let Some(first) = self.selected_ids.iter().next().copied() else {
                break;
            };
            self.selected_ids.shift_remove(&first);
        }
    }

    fn retain_selected_visible(&mut self) {
        if self.selected_ids.is_empty() {
            return;
        }

        let allow_unresolved = self.allow_unresolved_selection();
        if !allow_unresolved {
            self.selected_ids
                .retain(|id| self.view_ids.iter().any(|visible| visible == id));
            self.enforce_selection_cap();
        }

        if self.view_ids.is_empty() {
            if !allow_unresolved {
                self.focused_id = None;
                self.selection_anchor_id = None;
            }
            return;
        }

        if !allow_unresolved
            && self
                .focused_id
                .map(|id| self.view_ids.iter().all(|visible| *visible != id))
                .unwrap_or(false)
        {
            self.focused_id = self.selected_ids.iter().next_back().copied();
        }
        if !allow_unresolved
            && self
                .selection_anchor_id
                .map(|id| self.view_ids.iter().all(|visible| *visible != id))
                .unwrap_or(false)
        {
            self.selection_anchor_id = self.focused_id;
        }
    }

    fn allow_unresolved_selection(&self) -> bool {
        matches!(
            self.scan_status,
            ScanStatus::Scanning { .. } | ScanStatus::Partial { .. }
        )
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

fn toggle_select_id(list: &mut IndexSet<EntryId>, id: EntryId) {
    if !list.shift_remove(&id) {
        list.insert(id);
    }
}

fn select_range_by_id_capped(
    view_ids: &[EntryId],
    anchor: EntryId,
    target: EntryId,
    cap: usize,
) -> Option<Vec<EntryId>> {
    let ia = view_ids.iter().position(|id| *id == anchor)?;
    let it = view_ids.iter().position(|id| *id == target)?;
    let (lo, hi) = if ia <= it { (ia, it) } else { (it, ia) };
    let mut range = view_ids[lo..=hi].to_vec();
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
    selected_entries: Vec<DirEntry>,
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
            if selected_entries.is_empty() {
                return Err(FileDialogError::InvalidPath("no selection".into()));
            }
            for entry in selected_entries {
                if !matches_filter_matchers(&entry.name, &matchers) {
                    continue;
                }
                sel.paths.push(entry.path);
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

fn entry_id_from_path(path: &Path, is_dir: bool, is_symlink: bool) -> EntryId {
    let mut hasher = DefaultHasher::new();
    hasher.write(path.to_string_lossy().as_bytes());
    hasher.write_u8(if is_dir { 1 } else { 0 });
    hasher.write_u8(if is_symlink { 1 } else { 0 });
    EntryId::new(hasher.finish())
}

fn sanitize_scanned_entry(mut entry: FsEntry, dir: &Path) -> Option<FsEntry> {
    if entry.path.as_os_str().is_empty() {
        if entry.name.trim().is_empty() {
            return None;
        }
        entry.path = dir.join(&entry.name);
    }

    if entry.name.trim().is_empty() {
        let inferred_name = entry
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.is_empty())?;
        entry.name = inferred_name;
    }

    if entry.is_dir {
        entry.size = None;
    }

    Some(entry)
}

fn read_entries_snapshot_with_fs(
    fs: &dyn FileSystem,
    dir: &Path,
    mut scan_hook: Option<&mut ScanHook>,
) -> std::io::Result<DirSnapshot> {
    let mut out = Vec::new();
    let rd = fs.read_dir(dir)?;
    for mut entry in rd {
        if let Some(hook) = scan_hook.as_deref_mut() {
            if matches!(hook.apply(&mut entry), ScanHookAction::Drop) {
                continue;
            }
        }

        let Some(entry) = sanitize_scanned_entry(entry, dir) else {
            continue;
        };

        let meta = FileMeta {
            is_dir: entry.is_dir,
            is_symlink: entry.is_symlink,
            size: entry.size,
            modified: entry.modified,
        };
        out.push(DirEntry {
            id: entry_id_from_path(&entry.path, meta.is_dir, meta.is_symlink),
            name: entry.name,
            path: entry.path,
            is_dir: meta.is_dir,
            is_symlink: meta.is_symlink,
            size: meta.size,
            modified: meta.modified,
        });
    }
    Ok(DirSnapshot {
        cwd: dir.to_path_buf(),
        entry_count: out.len(),
        entries: out,
    })
}

fn empty_snapshot_for_cwd(cwd: &Path) -> DirSnapshot {
    DirSnapshot {
        cwd: cwd.to_path_buf(),
        entry_count: 0,
        entries: Vec::new(),
    }
}

#[cfg(feature = "tracing")]
fn trace_scan_requested(request: &ScanRequest) {
    trace!(
        event = "scan.requested",
        generation = request.generation,
        cwd = %request.cwd.display(),
        ?request.scan_policy,
        "scan requested"
    );
}

#[cfg(not(feature = "tracing"))]
fn trace_scan_requested(_request: &ScanRequest) {}

#[cfg(feature = "tracing")]
fn trace_scan_batch_applied(generation: u64, entries: usize, kind: &'static str) {
    trace!(
        event = "scan.batch_applied",
        generation, entries, kind, "scan batch applied"
    );
}

#[cfg(not(feature = "tracing"))]
fn trace_scan_batch_applied(_generation: u64, _entries: usize, _kind: &'static str) {}

#[cfg(feature = "tracing")]
fn trace_scan_completed(generation: u64, total_entries: usize, duration_ms: u128) {
    trace!(
        event = "scan.completed",
        generation, total_entries, duration_ms, "scan completed"
    );
}

#[cfg(not(feature = "tracing"))]
fn trace_scan_completed(_generation: u64, _total_entries: usize, _duration_ms: u128) {}

#[cfg(feature = "tracing")]
fn trace_scan_dropped_stale_batch(generation: u64, current_generation: u64, source: &'static str) {
    trace!(
        event = "scan.dropped_stale_batch",
        generation, current_generation, source, "scan dropped stale batch"
    );
}

#[cfg(not(feature = "tracing"))]
fn trace_scan_dropped_stale_batch(
    _generation: u64,
    _current_generation: u64,
    _source: &'static str,
) {
}

#[cfg(feature = "tracing")]
fn trace_projector_rebuild(reason: &'static str, visible_entries: usize, duration_us: u128) {
    trace!(
        event = "projector.rebuild",
        reason, visible_entries, duration_us, "projector rebuilt"
    );
}

#[cfg(not(feature = "tracing"))]
fn trace_projector_rebuild(_reason: &'static str, _visible_entries: usize, _duration_us: u128) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::StdFileSystem;
    use std::cell::Cell;
    use std::time::Duration;

    fn mods(ctrl: bool, shift: bool) -> Modifiers {
        Modifiers { ctrl, shift }
    }

    fn make_file_entry(name: &str) -> DirEntry {
        let path = PathBuf::from("/tmp").join(name);
        DirEntry {
            id: entry_id_from_path(&path, false, false),
            name: name.to_string(),
            path,
            is_dir: false,
            is_symlink: false,
            size: None,
            modified: None,
        }
    }

    fn make_dir_entry(name: &str) -> DirEntry {
        let path = PathBuf::from("/tmp").join(name);
        DirEntry {
            id: entry_id_from_path(&path, true, false),
            name: name.to_string(),
            path,
            is_dir: true,
            is_symlink: false,
            size: None,
            modified: None,
        }
    }

    fn set_view_files(core: &mut FileDialogCore, names: &[&str]) {
        core.entries = names.iter().map(|name| make_file_entry(name)).collect();
        core.view_names = core
            .entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        core.view_ids = core.entries.iter().map(|entry| entry.id).collect();
    }

    fn entry_id(core: &FileDialogCore, name: &str) -> EntryId {
        core.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.id)
            .unwrap_or_else(|| panic!("missing entry id for {name}"))
    }

    fn selected_entry_names(core: &FileDialogCore) -> Vec<String> {
        let entries = core.entries();
        core.selected_entry_ids()
            .into_iter()
            .filter_map(|id| {
                entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .map(|entry| entry.name.clone())
            })
            .collect()
    }

    #[derive(Default)]
    struct TestFs {
        meta: std::collections::HashMap<PathBuf, crate::fs::FsMetadata>,
        entries: Vec<crate::fs::FsEntry>,
        read_dir_calls: Cell<usize>,
        read_dir_error: Option<std::io::ErrorKind>,
    }

    impl crate::fs::FileSystem for TestFs {
        fn read_dir(&self, _dir: &Path) -> std::io::Result<Vec<crate::fs::FsEntry>> {
            self.read_dir_calls.set(self.read_dir_calls.get() + 1);
            if let Some(kind) = self.read_dir_error {
                return Err(std::io::Error::new(kind, "read_dir failure"));
            }
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
    fn entry_path_by_id_resolves_visible_entry_path() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        set_view_files(&mut core, &["a.txt", "b.txt"]);

        let b = entry_id(&core, "b.txt");
        assert_eq!(core.entry_path_by_id(b), Some(Path::new("/tmp/b.txt")));
    }

    #[test]
    fn entry_path_by_id_returns_none_for_unresolved_id() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        set_view_files(&mut core, &["a.txt"]);

        let missing = entry_id_from_path(Path::new("/tmp/missing.txt"), false, false);
        assert!(core.entry_path_by_id(missing).is_none());
    }
    #[test]
    fn selected_entry_paths_skips_unresolved_ids() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        set_view_files(&mut core, &["a.txt", "b.txt"]);

        let a = entry_id(&core, "a.txt");
        let missing = entry_id_from_path(Path::new("/tmp/missing.txt"), false, false);
        core.replace_selection_by_ids([a, missing]);

        assert_eq!(
            core.selected_entry_paths(),
            vec![PathBuf::from("/tmp/a.txt")]
        );
    }

    #[test]
    fn selected_entry_counts_tracks_files_and_dirs() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.entries = vec![make_file_entry("a.txt"), make_dir_entry("folder")];
        core.view_names = core
            .entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        core.view_ids = core.entries.iter().map(|entry| entry.id).collect();

        let a = entry_id(&core, "a.txt");
        let folder = entry_id(&core, "folder");
        let missing = entry_id_from_path(Path::new("/tmp/missing.txt"), false, false);
        core.replace_selection_by_ids([a, folder, missing]);

        assert_eq!(core.selected_entry_counts(), (1, 1));
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
        set_view_files(&mut core, &["a.txt"]);

        let a = entry_id(&core, "a.txt");
        core.click_entry(a, mods(true, false));
        assert_eq!(selected_entry_names(&core), vec!["a.txt"]);
        core.click_entry(a, mods(true, false));
        assert!(selected_entry_names(&core).is_empty());
    }

    #[test]
    fn focus_and_select_by_id_accepts_unresolved_entry_until_rescan() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;

        let pending = EntryId::from_path(&core.cwd.join("new_folder"));
        core.focus_and_select_by_id(pending);

        assert_eq!(core.selected_entry_ids(), vec![pending]);
        assert_eq!(core.focused_entry_id(), Some(pending));
        assert!(selected_entry_names(&core).is_empty());
    }

    #[test]
    fn focus_and_select_by_id_sets_focus_and_anchor() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        set_view_files(&mut core, &["a.txt", "b.txt"]);

        let id = entry_id(&core, "b.txt");
        core.focus_and_select_by_id(id);

        assert_eq!(core.selected_entry_ids(), vec![id]);
        assert_eq!(core.focused_entry_id(), Some(id));
        assert_eq!(selected_entry_names(&core), vec!["b.txt"]);
    }

    #[test]
    fn shift_click_selects_a_range_in_view_order() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        set_view_files(&mut core, &["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"]);

        core.click_entry(entry_id(&core, "b.txt"), mods(false, false));
        core.click_entry(entry_id(&core, "e.txt"), mods(false, true));
        assert_eq!(
            selected_entry_names(&core),
            vec!["b.txt", "c.txt", "d.txt", "e.txt"]
        );
    }

    #[test]
    fn ctrl_a_selects_all_when_multi_select_enabled() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        set_view_files(&mut core, &["a", "b", "c"]);

        core.select_all();
        assert_eq!(selected_entry_names(&core), vec!["a", "b", "c"]);
    }

    #[test]
    fn ctrl_a_respects_max_selection_cap() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.max_selection = Some(2);
        set_view_files(&mut core, &["a", "b", "c"]);

        core.select_all();
        assert_eq!(selected_entry_names(&core), vec!["a", "b"]);
    }

    #[test]
    fn shift_click_respects_max_selection_cap_and_keeps_target() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.max_selection = Some(2);
        set_view_files(&mut core, &["a", "b", "c", "d", "e"]);

        core.click_entry(entry_id(&core, "b"), mods(false, false));
        core.click_entry(entry_id(&core, "e"), mods(false, true));
        assert_eq!(selected_entry_names(&core), vec!["d", "e"]);

        core.click_entry(entry_id(&core, "d"), mods(false, false));
        core.click_entry(entry_id(&core, "b"), mods(false, true));
        assert_eq!(selected_entry_names(&core), vec!["b", "c"]);
    }

    #[test]
    fn ctrl_click_caps_by_dropping_oldest_selected() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        core.max_selection = Some(2);
        set_view_files(&mut core, &["a", "b", "c"]);

        core.click_entry(entry_id(&core, "a"), mods(false, false));
        core.click_entry(entry_id(&core, "b"), mods(true, false));
        assert_eq!(selected_entry_names(&core), vec!["a", "b"]);

        core.click_entry(entry_id(&core, "c"), mods(true, false));
        assert_eq!(selected_entry_names(&core), vec!["b", "c"]);
    }

    #[test]
    fn move_focus_with_shift_extends_range() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        set_view_files(&mut core, &["a", "b", "c", "d"]);

        core.click_entry(entry_id(&core, "b"), mods(false, false));
        core.move_focus(2, mods(false, true));
        assert_eq!(selected_entry_names(&core), vec!["b", "c", "d"]);
        assert_eq!(core.focused_entry_id(), Some(entry_id(&core, "d")));
    }

    #[test]
    fn handle_event_activate_focused_requests_confirm() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        set_view_files(&mut core, &["a.txt"]);
        core.focused_id = Some(entry_id(&core, "a.txt"));

        let outcome = core.handle_event(CoreEvent::ActivateFocused);
        assert_eq!(outcome, CoreEventOutcome::RequestConfirm);
        assert_eq!(selected_entry_names(&core), vec!["a.txt"]);
    }

    #[test]
    fn handle_event_double_click_file_requests_confirm() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        set_view_files(&mut core, &["a.txt"]);

        let outcome = core.handle_event(CoreEvent::DoubleClickEntry {
            id: entry_id(&core, "a.txt"),
        });

        assert_eq!(outcome, CoreEventOutcome::RequestConfirm);
        assert_eq!(selected_entry_names(&core), vec!["a.txt"]);
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
    fn handle_event_replace_selection_by_ids_uses_id_order() {
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.allow_multi = true;
        set_view_files(&mut core, &["a.txt", "b.txt", "c.txt"]);

        let c = entry_id(&core, "c.txt");
        let a = entry_id(&core, "a.txt");
        let outcome = core.handle_event(CoreEvent::ReplaceSelectionByIds(vec![c, a]));

        assert_eq!(outcome, CoreEventOutcome::None);
        assert_eq!(core.selected_entry_ids(), vec![c, a]);
        assert_eq!(selected_entry_names(&core), vec!["c.txt", "a.txt"]);
    }

    #[test]
    fn activate_focused_confirms_selection() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        set_view_files(&mut core, &["a.txt"]);
        core.focused_id = Some(entry_id(&core, "a.txt"));

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
            make_file_entry("file10.txt"),
            make_file_entry("file2.txt"),
            make_file_entry("file1.txt"),
        ];
        sort_entries_in_place(&mut entries, SortBy::Name, true, false);
        let names: Vec<_> = entries.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["file1.txt", "file2.txt", "file10.txt"]);
    }

    #[test]
    fn sort_by_extension_orders_by_full_extension_then_name() {
        let mut entries = vec![
            make_file_entry("alpha.tar.gz"),
            make_file_entry("beta.rs"),
            make_file_entry("gamma.tar.gz"),
            make_file_entry("noext"),
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
        set_view_files(&mut core, &["alpha", "beta", "alpine"]);
        core.focused_id = Some(entry_id(&core, "alpha"));

        core.select_by_prefix("al");
        assert_eq!(selected_entry_names(&core), vec!["alpine"]);
        assert_eq!(core.focused_entry_id(), Some(entry_id(&core, "alpine")));

        core.select_by_prefix("al");
        assert_eq!(selected_entry_names(&core), vec!["alpha"]);
        assert_eq!(core.focused_entry_id(), Some(entry_id(&core, "alpha")));
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
            crate::fs::FsMetadata {
                is_dir: false,
                is_symlink: false,
            },
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
    fn scan_hook_can_drop_entries_before_snapshot() {
        let fs = TestFs {
            entries: vec![
                crate::fs::FsEntry {
                    name: "keep.txt".into(),
                    path: PathBuf::from("/tmp/keep.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: Some(1),
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "drop.txt".into(),
                    path: PathBuf::from("/tmp/drop.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: Some(2),
                    modified: None,
                },
            ],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_hook(|entry| {
            if entry.name == "drop.txt" {
                ScanHookAction::Drop
            } else {
                ScanHookAction::Keep
            }
        });

        core.rescan_if_needed(&fs);

        let names: Vec<&str> = core
            .entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, vec!["keep.txt"]);
        assert_eq!(core.dir_snapshot.entry_count, 1);
    }

    #[test]
    fn scan_hook_can_mutate_entry_metadata() {
        let fs = TestFs {
            entries: vec![crate::fs::FsEntry {
                name: "a.txt".into(),
                path: PathBuf::from("/tmp/a.txt"),
                is_dir: false,
                is_symlink: false,
                size: Some(12),
                modified: Some(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(7)),
            }],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_hook(|entry| {
            entry.name = "renamed.log".to_string();
            entry.path = PathBuf::from("/tmp/renamed.log");
            entry.size = Some(99);
            entry.modified = None;
            ScanHookAction::Keep
        });

        core.rescan_if_needed(&fs);

        let entry = core
            .entries()
            .iter()
            .find(|entry| entry.name == "renamed.log")
            .expect("mutated entry should exist");
        assert_eq!(entry.path, PathBuf::from("/tmp/renamed.log"));
        assert_eq!(entry.size, Some(99));
        assert_eq!(entry.modified, None);
    }

    #[test]
    fn scan_hook_invalid_mutation_is_skipped_safely() {
        let fs = TestFs {
            entries: vec![crate::fs::FsEntry {
                name: "a.txt".into(),
                path: PathBuf::from("/tmp/a.txt"),
                is_dir: false,
                is_symlink: false,
                size: Some(12),
                modified: None,
            }],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_hook(|entry| {
            entry.name.clear();
            entry.path = PathBuf::new();
            ScanHookAction::Keep
        });

        core.rescan_if_needed(&fs);

        assert!(core.entries().is_empty());
        assert_eq!(core.dir_snapshot.entry_count, 0);
    }

    #[test]
    fn clear_scan_hook_restores_raw_listing() {
        let fs = TestFs {
            entries: vec![crate::fs::FsEntry {
                name: "a.txt".into(),
                path: PathBuf::from("/tmp/a.txt"),
                is_dir: false,
                is_symlink: false,
                size: Some(1),
                modified: None,
            }],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_hook(|_| ScanHookAction::Drop);
        core.rescan_if_needed(&fs);
        assert!(core.entries().is_empty());

        core.clear_scan_hook();
        core.rescan_if_needed(&fs);
        assert_eq!(core.entries().len(), 1);
        assert_eq!(core.entries()[0].name, "a.txt");
    }

    #[test]
    fn scan_policy_normalizes_and_invalidates_cache() {
        let fs = TestFs::default();
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");

        core.rescan_if_needed(&fs);
        assert_eq!(core.scan_generation(), 1);
        assert!(!core.dir_snapshot_dirty);

        core.set_scan_policy(ScanPolicy::Incremental { batch_entries: 0 });
        assert_eq!(
            core.scan_policy(),
            ScanPolicy::Incremental { batch_entries: 1 }
        );
        assert!(core.dir_snapshot_dirty);

        core.rescan_if_needed(&fs);
        assert_eq!(core.scan_generation(), 2);
    }

    #[test]
    fn rescan_updates_generation_and_status() {
        let fs = TestFs {
            entries: vec![
                crate::fs::FsEntry {
                    name: "a.txt".into(),
                    path: PathBuf::from("/tmp/a.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "b.txt".into(),
                    path: PathBuf::from("/tmp/b.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
            ],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");
        core.rescan_if_needed(&fs);

        assert_eq!(core.scan_generation(), 1);
        assert_eq!(
            core.scan_status(),
            &ScanStatus::Complete {
                generation: 1,
                loaded: 2,
            }
        );
    }

    #[test]
    fn incremental_scan_policy_emits_partial_batches_across_ticks() {
        let fs = TestFs {
            entries: vec![
                crate::fs::FsEntry {
                    name: "a.txt".into(),
                    path: PathBuf::from("/tmp/a.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "b.txt".into(),
                    path: PathBuf::from("/tmp/b.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "c.txt".into(),
                    path: PathBuf::from("/tmp/c.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
            ],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_policy(ScanPolicy::Incremental { batch_entries: 2 });

        core.rescan_if_needed(&fs);
        assert_eq!(core.scan_generation(), 1);
        assert_eq!(core.scan_status(), &ScanStatus::Scanning { generation: 1 });
        assert!(core.entries().is_empty());

        core.rescan_if_needed(&fs);
        assert_eq!(
            core.scan_status(),
            &ScanStatus::Partial {
                generation: 1,
                loaded: 2,
            }
        );
        assert_eq!(core.entries().len(), 2);

        core.rescan_if_needed(&fs);
        assert_eq!(
            core.scan_status(),
            &ScanStatus::Partial {
                generation: 1,
                loaded: 3,
            }
        );
        assert_eq!(core.entries().len(), 3);

        core.rescan_if_needed(&fs);
        assert_eq!(
            core.scan_status(),
            &ScanStatus::Complete {
                generation: 1,
                loaded: 3,
            }
        );
        assert_eq!(core.entries().len(), 3);
        assert_eq!(fs.read_dir_calls.get(), 1);
    }

    #[test]
    fn incremental_scan_supersedes_pending_generation_batches() {
        let fs_old = TestFs {
            entries: vec![
                crate::fs::FsEntry {
                    name: "old-a.txt".into(),
                    path: PathBuf::from("/tmp/old-a.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "old-b.txt".into(),
                    path: PathBuf::from("/tmp/old-b.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
            ],
            ..Default::default()
        };
        let fs_new = TestFs {
            entries: vec![crate::fs::FsEntry {
                name: "new.txt".into(),
                path: PathBuf::from("/tmp/new.txt"),
                is_dir: false,
                is_symlink: false,
                size: None,
                modified: None,
            }],
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_policy(ScanPolicy::Incremental { batch_entries: 1 });

        core.rescan_if_needed(&fs_old);
        core.rescan_if_needed(&fs_old);
        assert_eq!(core.scan_generation(), 1);
        assert_eq!(
            core.scan_status(),
            &ScanStatus::Partial {
                generation: 1,
                loaded: 1,
            }
        );

        core.request_rescan();
        core.rescan_if_needed(&fs_new);
        assert_eq!(core.scan_generation(), 2);
        assert_eq!(core.scan_status(), &ScanStatus::Scanning { generation: 2 });

        core.rescan_if_needed(&fs_new);
        core.rescan_if_needed(&fs_new);

        assert_eq!(
            core.scan_status(),
            &ScanStatus::Complete {
                generation: 2,
                loaded: 1,
            }
        );
        let names: Vec<&str> = core
            .entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, vec!["new.txt"]);
        assert_eq!(fs_old.read_dir_calls.get(), 1);
        assert_eq!(fs_new.read_dir_calls.get(), 1);
    }

    #[test]
    fn incremental_scan_keeps_unresolved_selection_until_entry_arrives() {
        let fs = TestFs {
            entries: vec![
                crate::fs::FsEntry {
                    name: "a.txt".into(),
                    path: PathBuf::from("/tmp/a.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "b.txt".into(),
                    path: PathBuf::from("/tmp/b.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "c.txt".into(),
                    path: PathBuf::from("/tmp/c.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
            ],
            ..Default::default()
        };

        let delayed = entry_id_from_path(Path::new("/tmp/c.txt"), false, false);
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_policy(ScanPolicy::Incremental { batch_entries: 1 });
        core.focus_and_select_by_id(delayed);

        core.rescan_if_needed(&fs);
        core.rescan_if_needed(&fs);
        assert_eq!(core.selected_entry_ids(), vec![delayed]);
        assert!(core.selected_entry_paths().is_empty());

        core.rescan_if_needed(&fs);
        assert_eq!(core.selected_entry_ids(), vec![delayed]);
        assert!(core.selected_entry_paths().is_empty());

        core.rescan_if_needed(&fs);
        assert_eq!(core.selected_entry_ids(), vec![delayed]);
        assert_eq!(
            core.selected_entry_paths(),
            vec![PathBuf::from("/tmp/c.txt")]
        );

        core.rescan_if_needed(&fs);
        assert_eq!(
            core.scan_status(),
            &ScanStatus::Complete {
                generation: 1,
                loaded: 3,
            }
        );
        assert_eq!(core.selected_entry_ids(), vec![delayed]);
    }

    #[test]
    fn complete_scan_drops_missing_selection_ids() {
        let fs = TestFs {
            entries: vec![crate::fs::FsEntry {
                name: "a.txt".into(),
                path: PathBuf::from("/tmp/a.txt"),
                is_dir: false,
                is_symlink: false,
                size: None,
                modified: None,
            }],
            ..Default::default()
        };

        let missing = entry_id_from_path(Path::new("/tmp/missing.txt"), false, false);
        let mut core = FileDialogCore::new(DialogMode::OpenFiles);
        core.cwd = PathBuf::from("/tmp");
        core.set_scan_policy(ScanPolicy::Incremental { batch_entries: 1 });
        core.focus_and_select_by_id(missing);

        core.rescan_if_needed(&fs);
        core.rescan_if_needed(&fs);
        assert_eq!(core.selected_entry_ids(), vec![missing]);

        core.rescan_if_needed(&fs);
        assert_eq!(
            core.scan_status(),
            &ScanStatus::Complete {
                generation: 1,
                loaded: 1,
            }
        );
        assert!(core.selected_entry_ids().is_empty());
        assert_eq!(core.focused_entry_id(), None);
    }

    #[test]
    fn stale_scan_batch_is_ignored() {
        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.scan_generation = 3;
        core.scan_status = ScanStatus::Scanning { generation: 3 };

        core.apply_scan_batch(ScanBatch::error(2, "stale".to_string()));

        assert_eq!(core.scan_status, ScanStatus::Scanning { generation: 3 });
    }

    #[test]
    fn read_dir_failure_sets_failed_scan_status() {
        let fs = TestFs {
            read_dir_error: Some(std::io::ErrorKind::PermissionDenied),
            ..Default::default()
        };

        let mut core = FileDialogCore::new(DialogMode::OpenFile);
        core.cwd = PathBuf::from("/tmp");
        core.rescan_if_needed(&fs);

        assert_eq!(core.scan_generation(), 1);
        match core.scan_status() {
            ScanStatus::Failed {
                generation,
                message,
            } => {
                assert_eq!(*generation, 1);
                assert!(message.contains("read_dir failure"));
            }
            other => panic!("unexpected scan status: {other:?}"),
        }
        assert!(core.entries().is_empty());
        assert_eq!(core.dir_snapshot.cwd, PathBuf::from("/tmp"));
        assert_eq!(core.dir_snapshot.entry_count, 0);
    }

    #[test]
    fn rescan_if_needed_caches_directory_listing() {
        let fs = TestFs {
            entries: vec![
                crate::fs::FsEntry {
                    name: "a.txt".into(),
                    path: PathBuf::from("/tmp/a.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: "b.txt".into(),
                    path: PathBuf::from("/tmp/b.txt"),
                    is_dir: false,
                    is_symlink: false,
                    size: None,
                    modified: None,
                },
                crate::fs::FsEntry {
                    name: ".hidden".into(),
                    path: PathBuf::from("/tmp/.hidden"),
                    is_dir: false,
                    is_symlink: false,
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
