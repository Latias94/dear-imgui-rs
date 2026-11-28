//! Multi-select helpers (BeginMultiSelect/EndMultiSelect)
//!
//! This module provides a small, safe wrapper around Dear ImGui's multi-select
//! API (`BeginMultiSelect` / `EndMultiSelect`), following
//! the "external storage" pattern described in the official docs:
//! <https://github.com/ocornut/imgui/wiki/Multi-Select>
//!
//! The main entry point is [`crate::Ui::multi_select_indexed`], which:
//! - wraps `BeginMultiSelect()` / `EndMultiSelect()`
//! - wires `SetNextItemSelectionUserData()` for each item (index-based)
//! - applies selection requests to your storage using a simple trait.
//!
//! Native begin/end IO is deliberately not exposed by the safe API:
//!
//! ```compile_fail
//! use dear_imgui_rs::MultiSelectEnd;
//! ```
//!
//! ```compile_fail
//! use dear_imgui_rs::MultiSelectScope;
//!
//! fn mutate_native_io(scope: &mut MultiSelectScope<'_>) {
//!     scope.begin_io_mut().RangeSrcReset = true;
//! }
//! ```

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::{ContextThreadLease, Id, ImGuiError, ImGuiResult, Ui, sys};
use std::collections::HashSet;
use std::ptr::NonNull;

impl Ui {
    /// Run an advanced multi-select block and return an owned copy of its final requests.
    ///
    /// The scope exposes only operations that are valid between `BeginMultiSelect()` and
    /// `EndMultiSelect()`. `EndMultiSelect()` runs exactly once even if `render` panics, and the
    /// returned [`MultiSelectResult`] contains no native pointers.
    #[doc(alias = "BeginMultiSelect", alias = "EndMultiSelect")]
    pub fn with_multi_select(
        &self,
        flags: impl Into<MultiSelectOptions>,
        selection_size: Option<i32>,
        items_count: usize,
        render: impl FnOnce(&mut MultiSelectScope<'_>),
    ) -> MultiSelectResult {
        let mut scope = MultiSelectScope::new(self, flags, selection_size, items_count);
        render(&mut scope);
        scope.finish()
    }

    /// Multi-select helper for index-based storage.
    ///
    /// This wraps `BeginMultiSelect()` / `EndMultiSelect()` and applies
    /// selection requests to an index-addressable selection container.
    ///
    /// Typical usage:
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut selected = vec![false; 128];
    ///
    /// ui.multi_select_indexed(&mut selected, MultiSelectOptions::new(), |ui, idx, is_selected| {
    ///     ui.text(format!(
    ///         "{} {}",
    ///         if is_selected { "[x]" } else { "[ ]" },
    ///         idx
    ///     ));
    /// });
    /// ```
    ///
    /// Notes:
    /// - `storage.len()` defines `items_count`.
    /// - This helper uses the "external storage" pattern where selection is
    ///   stored entirely on the application side.
    /// - Per-item selection toggles can be queried via
    ///   [`Ui::is_item_toggled_selection`].
    #[doc(alias = "SetNextItemSelectionUserData")]
    pub fn multi_select_indexed<S, F>(
        &self,
        storage: &mut S,
        flags: impl Into<MultiSelectOptions>,
        mut render_item: F,
    ) where
        S: MultiSelectIndexStorage,
        F: FnMut(&Ui, usize, bool),
    {
        let items_count = storage.len();
        let selection_size_i32 = storage
            .selected_count_hint()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(-1);

        let result =
            self.with_multi_select(flags, Some(selection_size_i32), items_count, |scope| {
                scope.apply_begin_requests_indexed(storage);

                for idx in 0..items_count {
                    scope.set_next_item_selection_user_data(idx as sys::ImGuiSelectionUserData);
                    let is_selected = storage.is_selected(idx);
                    render_item(self, idx, is_selected);
                }
            });
        result.apply_requests_indexed(storage);
    }

    /// Multi-select helper for index-based storage inside an active table.
    ///
    /// This is a convenience wrapper over [`Ui::multi_select_indexed`] that
    /// automatically advances table rows and starts each row at column 0.
    ///
    /// It expects to be called between `BeginTable`/`EndTable`.
    pub fn table_multi_select_indexed<S, F>(
        &self,
        storage: &mut S,
        flags: impl Into<MultiSelectOptions>,
        mut build_row: F,
    ) where
        S: MultiSelectIndexStorage,
        F: FnMut(&Ui, usize, bool),
    {
        let row_count = storage.len();
        let selection_size_i32 = storage
            .selected_count_hint()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(-1);

        let result = self.with_multi_select(flags, Some(selection_size_i32), row_count, |scope| {
            scope.apply_begin_requests_indexed(storage);

            for row in 0..row_count {
                scope.set_next_item_selection_user_data(row as sys::ImGuiSelectionUserData);
                self.table_next_row();
                self.table_next_column();

                let is_selected = storage.is_selected(row);
                build_row(self, row, is_selected);
            }
        });
        result.apply_requests_indexed(storage);
    }

    /// Multi-select helper using [`BasicSelection`] as underlying storage.
    ///
    /// This variant is suitable when items are naturally identified by `ImGuiID`
    /// (e.g. stable ids for rows or tree nodes).
    ///
    /// - `items_count`: number of items in the scope.
    /// - `id_at_index`: maps `[0, items_count)` to the corresponding item id.
    /// - `render_item`: called once per index to emit widgets for that item.
    pub fn multi_select_basic<G, F>(
        &self,
        selection: &mut BasicSelection,
        flags: impl Into<MultiSelectOptions>,
        items_count: usize,
        mut id_at_index: G,
        mut render_item: F,
    ) where
        G: FnMut(usize) -> crate::Id,
        F: FnMut(&Ui, usize, crate::Id, bool),
    {
        let selection_size_i32 = i32::try_from(selection.len()).unwrap_or(-1);

        let result =
            self.with_multi_select(flags, Some(selection_size_i32), items_count, |scope| {
                scope.apply_begin_requests_basic(selection, &mut id_at_index);

                for idx in 0..items_count {
                    scope.set_next_item_selection_user_data(idx as sys::ImGuiSelectionUserData);
                    let id = id_at_index(idx);
                    let is_selected = selection.contains(id);
                    render_item(self, idx, id, is_selected);
                }
            });
        result.apply_requests_basic(selection, id_at_index);
    }
}

bitflags::bitflags! {
    /// Independent flags controlling multi-selection behavior.
    ///
    /// The click-selection policy, box-select mode, and scope are represented by
    /// [`MultiSelectClickPolicy`], [`MultiSelectBoxSelect`], and [`MultiSelectScopeKind`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MultiSelectFlags: i32 {
        /// No flags.
        const NONE = sys::ImGuiMultiSelectFlags_None;
        /// Single-selection scope. Ctrl/Shift range selection is disabled.
        const SINGLE_SELECT = sys::ImGuiMultiSelectFlags_SingleSelect;
        /// Disable `Ctrl+A` "select all" shortcut.
        const NO_SELECT_ALL = sys::ImGuiMultiSelectFlags_NoSelectAll;
        /// Disable range selection (Shift+click / Shift+arrow).
        const NO_RANGE_SELECT = sys::ImGuiMultiSelectFlags_NoRangeSelect;
        /// Disable automatic selection of newly focused items.
        const NO_AUTO_SELECT = sys::ImGuiMultiSelectFlags_NoAutoSelect;
        /// Disable automatic clearing of selection when focus moves within the scope.
        const NO_AUTO_CLEAR = sys::ImGuiMultiSelectFlags_NoAutoClear;
        /// Disable automatic clearing when reselecting the same range.
        const NO_AUTO_CLEAR_ON_RESELECT =
            sys::ImGuiMultiSelectFlags_NoAutoClearOnReselect;
        /// Disable drag-scrolling when box-selecting near edges of the scope.
        const BOX_SELECT_NO_SCROLL = sys::ImGuiMultiSelectFlags_BoxSelectNoScroll;
        /// Clear selection when pressing Escape while the scope is focused.
        const CLEAR_ON_ESCAPE = sys::ImGuiMultiSelectFlags_ClearOnEscape;
        /// Clear selection when clicking on empty space (void) inside the scope.
        const CLEAR_ON_CLICK_VOID = sys::ImGuiMultiSelectFlags_ClearOnClickVoid;
        /// Disable default right-click behavior that selects item before opening a context menu.
        const NO_SELECT_ON_RIGHT_CLICK =
            sys::ImGuiMultiSelectFlags_NoSelectOnRightClick;
    }
}

/// Box-selection geometry for multi-select scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MultiSelectBoxSelect {
    /// Same x-position/full-row items.
    OneDimensional,
    /// Arbitrary item layout, at a higher clipping cost.
    TwoDimensional,
}

impl MultiSelectBoxSelect {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::OneDimensional => sys::ImGuiMultiSelectFlags_BoxSelect1d,
            Self::TwoDimensional => sys::ImGuiMultiSelectFlags_BoxSelect2d,
        }
    }
}

/// Click-selection policy for multi-select scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MultiSelectClickPolicy {
    /// Apply selection on mouse down for unselected items and on mouse up for
    /// selected items.
    Auto,
    /// Apply selection on mouse down for any clicked item.
    ClickAlways,
    /// Apply selection on mouse release for unselected items.
    ClickRelease,
}

impl MultiSelectClickPolicy {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Auto => sys::ImGuiMultiSelectFlags_SelectOnAuto,
            Self::ClickAlways => sys::ImGuiMultiSelectFlags_SelectOnClickAlways,
            Self::ClickRelease => sys::ImGuiMultiSelectFlags_SelectOnClickRelease,
        }
    }
}

/// Scope for box-select and clear-on-empty-click behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MultiSelectScopeKind {
    /// Scope is the whole window.
    Window,
    /// Scope is the whole window and enables Dear ImGui's temporary X-axis navigation wrap helper.
    WindowWithNavWrapX,
    /// Scope is the rectangle between `BeginMultiSelect()` and `EndMultiSelect()`.
    Rect,
}

impl MultiSelectScopeKind {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Window => sys::ImGuiMultiSelectFlags_ScopeWindow,
            Self::WindowWithNavWrapX => {
                sys::ImGuiMultiSelectFlags_ScopeWindow | sys::ImGuiMultiSelectFlags_NavWrapX
            }
            Self::Rect => sys::ImGuiMultiSelectFlags_ScopeRect,
        }
    }
}

/// Complete multi-select options assembled from independent flags and an
/// optional single-choice policies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultiSelectOptions {
    pub flags: MultiSelectFlags,
    pub click_policy: Option<MultiSelectClickPolicy>,
    pub box_select: Option<MultiSelectBoxSelect>,
    pub scope: Option<MultiSelectScopeKind>,
}

impl MultiSelectOptions {
    pub const fn new() -> Self {
        Self {
            flags: MultiSelectFlags::NONE,
            click_policy: None,
            box_select: None,
            scope: None,
        }
    }

    pub fn flags(mut self, flags: MultiSelectFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn click_policy(mut self, policy: MultiSelectClickPolicy) -> Self {
        self.click_policy = Some(policy);
        self
    }

    pub fn box_select(mut self, mode: MultiSelectBoxSelect) -> Self {
        self.box_select = Some(mode);
        self
    }

    pub fn scope(mut self, scope: MultiSelectScopeKind) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits()
            | self.click_policy.map_or(0, MultiSelectClickPolicy::raw)
            | self.box_select.map_or(0, MultiSelectBoxSelect::raw)
            | self.scope.map_or(0, MultiSelectScopeKind::raw)
    }
}

impl Default for MultiSelectOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<MultiSelectFlags> for MultiSelectOptions {
    fn from(flags: MultiSelectFlags) -> Self {
        Self::new().flags(flags)
    }
}

fn usize_to_i32(name: &str, value: usize) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| panic!("{name} exceeded ImGui's i32 range"))
}

/// Closure-scoped access to an active Dear ImGui multi-select block.
///
/// Instances are supplied only to [`crate::Ui::with_multi_select`]. The native IO pointer
/// remains private and cannot outlive the matching `EndMultiSelect()` call.
pub struct MultiSelectScope<'ui> {
    ui: &'ui crate::Ui,
    begin_result: MultiSelectResult,
    range_source_reset: bool,
    native_scope: NativeScopeToken<'ui>,
    ended: bool,
}

impl<'ui> MultiSelectScope<'ui> {
    fn new(
        ui: &'ui crate::Ui,
        flags: impl Into<MultiSelectOptions>,
        selection_size: Option<i32>,
        items_count: usize,
    ) -> Self {
        let options = flags.into();
        let selection_size = selection_size.unwrap_or(-1);
        let items_count = usize_to_i32("items_count", items_count);
        let native_io = ui.run_with_bound_context(|| unsafe {
            sys::igBeginMultiSelect(options.raw(), selection_size, items_count)
        });
        let begin_result = unsafe { copy_multi_select_result(native_io) };

        Self {
            ui,
            begin_result,
            range_source_reset: false,
            native_scope: ui.begin_native_scope(
                NativeScopePop::EndMultiSelect {
                    range_source_reset: false,
                },
                "MultiSelectScope",
            ),
            ended: false,
        }
    }

    /// Owned requests and metadata captured immediately after `BeginMultiSelect()`.
    #[must_use]
    pub fn begin_result(&self) -> &MultiSelectResult {
        &self.begin_result
    }

    /// Apply requests captured at begin time to index-addressable storage.
    pub fn apply_begin_requests_indexed<S: MultiSelectIndexStorage>(&self, storage: &mut S) {
        self.begin_result.apply_requests_indexed(storage);
    }

    /// Apply requests captured at begin time to a [`BasicSelection`].
    pub fn apply_begin_requests_basic<G>(&self, selection: &mut BasicSelection, id_at_index: G)
    where
        G: FnMut(usize) -> Id,
    {
        self.begin_result
            .apply_requests_basic(selection, id_at_index);
    }

    /// Associate the next submitted item with application-defined selection data.
    pub fn set_next_item_selection_user_data(&self, item: MultiSelectUserData) {
        self.ui.run_with_bound_context(|| unsafe {
            sys::igSetNextItemSelectionUserData(item);
        });
    }

    /// Request that Dear ImGui discard its current range-selection source at scope end.
    ///
    /// This is useful when the application deletes the range source while the scope is active.
    pub fn set_range_source_reset(&mut self, reset: bool) {
        self.range_source_reset = reset;
    }

    fn finish(mut self) -> MultiSelectResult {
        let native_io = self.end_native();
        self.ended = true;
        let mut result = unsafe { copy_multi_select_result(native_io) };
        result.record_range_source_reset(self.range_source_reset);
        result
    }

    fn end_native(&mut self) -> *mut sys::ImGuiMultiSelectIO {
        let pop = NativeScopePop::EndMultiSelect {
            range_source_reset: self.range_source_reset,
        };
        self.native_scope.replace_pop(pop);
        let mut native_io = std::ptr::null_mut();
        self.native_scope
            .finish_with(|| native_io = unsafe { pop.finish_multi_select() });
        native_io
    }
}

impl Drop for MultiSelectScope<'_> {
    fn drop(&mut self) {
        if self.ended || self.native_scope.is_finished() {
            return;
        }

        self.end_native();
        self.ended = true;
    }
}

/// Application-defined item data passed through Dear ImGui's multi-select API.
pub type MultiSelectUserData = i64;

/// Iteration order requested for a selected range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiSelectRangeDirection {
    /// Visit the first item before the last item.
    Forward,
    /// Visit the last item before the first item.
    Backward,
}

/// An owned selection change requested by Dear ImGui.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MultiSelectRequest {
    /// Select or clear every item in the multi-select scope.
    SetAll { selected: bool },
    /// Select or clear an inclusive range of application item data.
    SetRange {
        selected: bool,
        first: MultiSelectUserData,
        last: MultiSelectUserData,
        direction: MultiSelectRangeDirection,
    },
}

/// An owned copy of the IO produced by `BeginMultiSelect()` or `EndMultiSelect()`.
///
/// The native IO is temporary and may be overwritten by the next multi-select scope. This
/// value contains no native pointers, so it can be stored and applied later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiSelectResult {
    requests: Vec<MultiSelectRequest>,
    range_source_item: Option<MultiSelectUserData>,
    navigation_item: Option<MultiSelectUserData>,
    navigation_item_selected: bool,
    range_source_reset: bool,
    items_count: usize,
}

impl MultiSelectResult {
    /// Selection changes requested by Dear ImGui.
    #[must_use]
    pub fn requests(&self) -> &[MultiSelectRequest] {
        &self.requests
    }

    /// Source item used for range selection, when one is active.
    #[must_use]
    pub fn range_source_item(&self) -> Option<MultiSelectUserData> {
        self.range_source_item
    }

    /// Item associated with Dear ImGui's navigation ID, when known.
    #[must_use]
    pub fn navigation_item(&self) -> Option<MultiSelectUserData> {
        self.navigation_item
    }

    /// Whether the navigation item was selected when this result was captured.
    #[must_use]
    pub fn navigation_item_selected(&self) -> bool {
        self.navigation_item_selected
    }

    /// Whether the range source was reset before the scope ended.
    #[must_use]
    pub fn range_source_reset(&self) -> bool {
        self.range_source_reset
    }

    /// Item count supplied when the multi-select scope began.
    #[must_use]
    pub fn items_count(&self) -> usize {
        self.items_count
    }

    /// Apply these requests to index-addressable selection storage.
    pub fn apply_requests_indexed<S: MultiSelectIndexStorage>(&self, storage: &mut S) {
        let items_count = self.items_count.min(storage.len());
        self.apply_request_indices(items_count, |index, selected| {
            storage.set_selected(index, selected);
        });
    }

    /// Apply these requests to a [`BasicSelection`] using an index-to-ID mapping.
    pub fn apply_requests_basic<G>(&self, selection: &mut BasicSelection, mut id_at_index: G)
    where
        G: FnMut(usize) -> Id,
    {
        self.apply_request_indices(self.items_count, |index, selected| {
            selection.set_selected(id_at_index(index), selected);
        });
    }

    fn apply_request_indices(&self, items_count: usize, mut set_selected: impl FnMut(usize, bool)) {
        for request in &self.requests {
            match *request {
                MultiSelectRequest::SetAll { selected } => {
                    for index in 0..items_count {
                        set_selected(index, selected);
                    }
                }
                MultiSelectRequest::SetRange {
                    selected,
                    first,
                    last,
                    direction,
                } => {
                    let Some(range) = indexed_range(first, last, items_count) else {
                        continue;
                    };
                    match direction {
                        MultiSelectRangeDirection::Forward => {
                            for index in range {
                                set_selected(index, selected);
                            }
                        }
                        MultiSelectRangeDirection::Backward => {
                            for index in range.rev() {
                                set_selected(index, selected);
                            }
                        }
                    }
                }
            }
        }
    }

    fn record_range_source_reset(&mut self, reset: bool) {
        self.range_source_reset = reset;
    }
}

fn indexed_range(
    first: MultiSelectUserData,
    last: MultiSelectUserData,
    items_count: usize,
) -> Option<std::ops::RangeInclusive<usize>> {
    let first = usize::try_from(first).ok()?;
    let last = usize::try_from(last).ok()?;
    if first > last || first >= items_count {
        return None;
    }
    Some(first..=last.min(items_count.saturating_sub(1)))
}

unsafe fn copy_multi_select_result(io: *const sys::ImGuiMultiSelectIO) -> MultiSelectResult {
    unsafe {
        let io = io
            .as_ref()
            .expect("Dear ImGui returned a null multi-select IO");
        let requests = if io.Requests.Data.is_null() || io.Requests.Size <= 0 {
            Vec::new()
        } else {
            usize::try_from(io.Requests.Size)
                .ok()
                .map(|len| std::slice::from_raw_parts(io.Requests.Data, len))
                .unwrap_or_default()
                .iter()
                .filter_map(copy_request)
                .collect()
        };

        MultiSelectResult {
            requests,
            range_source_item: valid_user_data(io.RangeSrcItem),
            navigation_item: valid_user_data(io.NavIdItem),
            navigation_item_selected: io.NavIdSelected,
            range_source_reset: io.RangeSrcReset,
            items_count: usize::try_from(io.ItemsCount).unwrap_or(0),
        }
    }
}

fn copy_request(request: &sys::ImGuiSelectionRequest) -> Option<MultiSelectRequest> {
    match request.Type {
        sys::ImGuiSelectionRequestType_SetAll => Some(MultiSelectRequest::SetAll {
            selected: request.Selected,
        }),
        sys::ImGuiSelectionRequestType_SetRange => Some(MultiSelectRequest::SetRange {
            selected: request.Selected,
            first: request.RangeFirstItem,
            last: request.RangeLastItem,
            direction: if request.RangeDirection < 0 {
                MultiSelectRangeDirection::Backward
            } else {
                MultiSelectRangeDirection::Forward
            },
        }),
        _ => None,
    }
}

fn valid_user_data(value: sys::ImGuiSelectionUserData) -> Option<MultiSelectUserData> {
    (value != -1).then_some(value)
}

/// Index-based selection storage for multi-select helpers.
///
/// Implement this trait for your selection container (e.g. `Vec<bool>`,
/// `Vec<MyItem { selected: bool }>` or a custom type) to use
/// [`crate::Ui::multi_select_indexed`].
pub trait MultiSelectIndexStorage {
    /// Total number of items in the selection scope.
    fn len(&self) -> usize;

    /// Returns `true` if the selection scope is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns whether item at `index` is currently selected.
    fn is_selected(&self, index: usize) -> bool;

    /// Updates selection state for item at `index`.
    fn set_selected(&mut self, index: usize, selected: bool);

    /// Optional hint for current selection size.
    ///
    /// If provided, this is forwarded to `BeginMultiSelect()` to improve the
    /// behavior of shortcuts such as `ImGuiMultiSelectFlags_ClearOnEscape`.
    /// When `None` (default), the size is treated as "unknown".
    fn selected_count_hint(&self) -> Option<usize> {
        None
    }
}

impl MultiSelectIndexStorage for Vec<bool> {
    fn len(&self) -> usize {
        self.len()
    }

    fn is_selected(&self, index: usize) -> bool {
        self.get(index).copied().unwrap_or(false)
    }

    fn set_selected(&mut self, index: usize, selected: bool) {
        if index < self.len() {
            self[index] = selected;
        }
    }

    fn selected_count_hint(&self) -> Option<usize> {
        // For typical lists this is cheap enough; callers with large datasets
        // can implement the trait manually with a more efficient counter.
        Some(self.iter().filter(|&&b| b).count())
    }
}

impl MultiSelectIndexStorage for &mut [bool] {
    fn len(&self) -> usize {
        (**self).len()
    }

    fn is_selected(&self, index: usize) -> bool {
        self.get(index).copied().unwrap_or(false)
    }

    fn set_selected(&mut self, index: usize, selected: bool) {
        if index < self.len() {
            self[index] = selected;
        }
    }

    fn selected_count_hint(&self) -> Option<usize> {
        Some(self.iter().filter(|&&b| b).count())
    }
}

/// Index-based selection storage backed by a key slice + `HashSet` of selected keys.
///
/// This is convenient when your application stores selection as a set of
/// arbitrary keys (e.g. `HashSet<u32>` or `HashSet<MyId>`), but you still
/// want to drive a multi-select scope using contiguous indices.
pub struct KeySetSelection<'a, K>
where
    K: Eq + std::hash::Hash + Copy,
{
    keys: &'a [K],
    selected: &'a mut HashSet<K>,
}

impl<'a, K> KeySetSelection<'a, K>
where
    K: Eq + std::hash::Hash + Copy,
{
    /// Create a new index-based view over a key slice and a selection set.
    ///
    /// - `keys`: stable index->key mapping (e.g. your backing array).
    /// - `selected`: set of currently selected keys.
    pub fn new(keys: &'a [K], selected: &'a mut HashSet<K>) -> Self {
        Self { keys, selected }
    }
}

impl<'a, K> MultiSelectIndexStorage for KeySetSelection<'a, K>
where
    K: Eq + std::hash::Hash + Copy,
{
    fn len(&self) -> usize {
        self.keys.len()
    }

    fn is_selected(&self, index: usize) -> bool {
        self.keys
            .get(index)
            .map(|k| self.selected.contains(k))
            .unwrap_or(false)
    }

    fn set_selected(&mut self, index: usize, selected: bool) {
        if let Some(&key) = self.keys.get(index) {
            if selected {
                self.selected.insert(key);
            } else {
                self.selected.remove(&key);
            }
        }
    }

    fn selected_count_hint(&self) -> Option<usize> {
        Some(self.selected.len())
    }
}

/// Selection container backed by Dear ImGui's `ImGuiSelectionBasicStorage`.
///
/// This stores a set of selected `ImGuiID` values using the optimized helper
/// provided by Dear ImGui. It is suitable when items are naturally identified
/// by stable IDs (e.g. table rows, tree nodes).
#[derive(Debug)]
pub struct BasicSelection {
    raw: NonNull<sys::ImGuiSelectionBasicStorage>,
    _thread_lease: ContextThreadLease,
}

impl BasicSelection {
    /// Create an empty selection storage.
    ///
    /// # Panics
    ///
    /// Panics if storage cannot be created on this thread. See [`Self::try_new`].
    pub fn new() -> Self {
        Self::try_new().expect("failed to create Dear ImGui selection storage")
    }

    /// Tries to allocate empty selection storage on the Dear ImGui owner thread.
    pub fn try_new() -> ImGuiResult<Self> {
        let thread_lease = ContextThreadLease::acquire()?;
        let raw =
            NonNull::new(unsafe { sys::ImGuiSelectionBasicStorage_ImGuiSelectionBasicStorage() })
                .ok_or_else(|| ImGuiError::resource_allocation("Dear ImGui selection storage"))?;
        Ok(Self {
            raw,
            _thread_lease: thread_lease,
        })
    }

    /// Return the number of selected items.
    pub fn len(&self) -> usize {
        unsafe {
            let size = self.raw.as_ref().Size;
            if size <= 0 { 0 } else { size as usize }
        }
    }

    /// Returns true if the selection is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the selection set.
    pub fn clear(&mut self) {
        unsafe {
            sys::ImGuiSelectionBasicStorage_Clear(self.as_raw());
        }
    }

    /// Returns true if the given id is selected.
    pub fn contains(&self, id: Id) -> bool {
        unsafe { sys::ImGuiSelectionBasicStorage_Contains(self.as_raw(), id.raw()) }
    }

    /// Set selection state for a given id.
    pub fn set_selected(&mut self, id: Id, selected: bool) {
        unsafe {
            sys::ImGuiSelectionBasicStorage_SetItemSelected(self.as_raw(), id.raw(), selected);
        }
    }

    /// Iterate over selected ids.
    pub fn iter(&self) -> BasicSelectionIter<'_> {
        BasicSelectionIter {
            storage: self,
            it: std::ptr::null_mut(),
        }
    }

    /// Expose raw pointer for internal helpers.
    pub(crate) fn as_raw(&self) -> *mut sys::ImGuiSelectionBasicStorage {
        self.raw.as_ptr()
    }
}

impl Default for BasicSelection {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BasicSelection {
    fn drop(&mut self) {
        unsafe { sys::ImGuiSelectionBasicStorage_destroy(self.as_raw()) }
    }
}

/// Iterator over selected ids stored in [`BasicSelection`].
pub struct BasicSelectionIter<'a> {
    storage: &'a BasicSelection,
    it: *mut std::os::raw::c_void,
}

impl<'a> Iterator for BasicSelectionIter<'a> {
    type Item = Id;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut out_id: sys::ImGuiID = 0;
            let has_next = sys::ImGuiSelectionBasicStorage_GetNextSelectedItem(
                self.storage.as_raw(),
                &mut self.it,
                &mut out_id,
            );
            if has_next {
                Some(Id::from(out_id))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MultiSelectIndexStorage, MultiSelectOptions, MultiSelectRangeDirection, MultiSelectRequest,
        MultiSelectResult, copy_multi_select_result,
    };
    use crate::sys;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
        ctx
    }

    #[test]
    fn multi_select_indexed_ends_scope_after_render_panic() {
        let mut ctx = setup_context();
        let raw_ctx = ctx.as_raw();

        let ui = ctx.frame();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.window("multi_select_panic").build(|| {
                let mut selected = vec![false; 2];
                ui.multi_select_indexed(&mut selected, MultiSelectOptions::new(), |_, idx, _| {
                    if idx == 0 {
                        panic!("forced panic while multi-select is active");
                    }
                });
            });
        }));

        assert!(result.is_err());
        let _ = ui.window("multi_select_after_panic").build(|| {
            let result = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
            assert!(result.requests().is_empty());
        });
        unsafe {
            let imgui_ctx = raw_ctx as *const sys::ImGuiContext;
            assert!((*imgui_ctx).CurrentMultiSelect.is_null());
            assert_eq!((*imgui_ctx).MultiSelectTempDataStacked, 0);
        }
    }

    #[test]
    fn with_multi_select_ends_scope_exactly_once() {
        let mut ctx = setup_context();
        let raw_ctx = ctx.as_raw();

        let ui = ctx.frame();
        let _ = ui.window("multi_select_explicit_end").build(|| {
            let result = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
            assert!(result.requests().is_empty());
        });

        unsafe {
            let imgui_ctx = raw_ctx as *const sys::ImGuiContext;
            assert!((*imgui_ctx).CurrentMultiSelect.is_null());
            assert_eq!((*imgui_ctx).MultiSelectTempDataStacked, 0);
        }
    }

    #[test]
    fn with_multi_select_rejects_items_count_over_i32() {
        let mut ctx = setup_context();

        let ui = ctx.frame();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.with_multi_select(
                MultiSelectOptions::new(),
                None,
                (i32::MAX as usize) + 1,
                |_| {},
            );
        }));

        assert!(result.is_err());
    }

    #[test]
    fn result_remains_owned_after_a_later_scope_reuses_native_io() {
        let mut ctx = setup_context();

        let ui = ctx.frame();
        let _ = ui.window("multi_select_owned_result").build(|| {
            let first = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
            let first_copy = first.clone();

            let second = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |scope| {
                scope.set_range_source_reset(true);
            });

            assert_eq!(first, first_copy);
            assert!(!first.range_source_reset());
            assert!(second.range_source_reset());
        });
    }

    #[test]
    fn outer_scope_does_not_retain_io_pointer_across_nested_scope() {
        let mut ctx = setup_context();

        let ui = ctx.frame();
        let _ = ui.window("nested_multi_select").build(|| {
            let outer = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |outer| {
                let inner = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
                assert!(inner.requests().is_empty());
                outer.set_range_source_reset(true);
            });

            assert!(outer.range_source_reset());
        });
    }

    #[test]
    fn multi_select_rejects_escaped_focus_scope_before_end_ffi_and_recovers() {
        let mut ctx = setup_context();
        let ui = ctx.frame();
        let mut escaped_focus = None;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.window("multi_select_focus_scope_order").build(|| {
                ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {
                    escaped_focus = Some(ui.push_focus_scope(ui.get_id("escaped-focus")));
                });
            });
        }));
        assert!(result.is_err());

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.text("recovery is pending");
            }))
            .is_err()
        );

        drop(escaped_focus);
        let _ = ui.window("multi_select_focus_scope_recovered").build(|| {
            ui.text("scope tracker recovered");
        });
    }

    struct RecordingStorage {
        selected: Vec<bool>,
        updates: Vec<usize>,
    }

    impl MultiSelectIndexStorage for RecordingStorage {
        fn len(&self) -> usize {
            self.selected.len()
        }

        fn is_selected(&self, index: usize) -> bool {
            self.selected[index]
        }

        fn set_selected(&mut self, index: usize, selected: bool) {
            self.selected[index] = selected;
            self.updates.push(index);
        }
    }

    fn result(requests: Vec<MultiSelectRequest>, items_count: usize) -> MultiSelectResult {
        MultiSelectResult {
            requests,
            range_source_item: None,
            navigation_item: None,
            navigation_item_selected: false,
            range_source_reset: false,
            items_count,
        }
    }

    #[test]
    fn indexed_requests_preserve_backward_range_direction() {
        let requests = result(
            vec![MultiSelectRequest::SetRange {
                selected: true,
                first: 1,
                last: 3,
                direction: MultiSelectRangeDirection::Backward,
            }],
            5,
        );
        let mut storage = RecordingStorage {
            selected: vec![false; 5],
            updates: Vec::new(),
        };

        requests.apply_requests_indexed(&mut storage);

        assert_eq!(storage.updates, vec![3, 2, 1]);
        assert_eq!(storage.selected, vec![false, true, true, true, false]);
    }

    #[test]
    fn indexed_requests_clamp_to_current_storage_and_ignore_invalid_ranges() {
        let requests = result(
            vec![
                MultiSelectRequest::SetAll { selected: true },
                MultiSelectRequest::SetRange {
                    selected: false,
                    first: 2,
                    last: 99,
                    direction: MultiSelectRangeDirection::Forward,
                },
                MultiSelectRequest::SetRange {
                    selected: false,
                    first: -1,
                    last: 1,
                    direction: MultiSelectRangeDirection::Forward,
                },
            ],
            8,
        );
        let mut storage = RecordingStorage {
            selected: vec![false; 4],
            updates: Vec::new(),
        };

        requests.apply_requests_indexed(&mut storage);

        assert_eq!(storage.selected, vec![true, true, false, false]);
        assert_eq!(storage.updates, vec![0, 1, 2, 3, 2, 3]);
    }

    #[test]
    fn copied_result_does_not_borrow_native_request_storage() {
        let mut native_request = sys::ImGuiSelectionRequest {
            Type: sys::ImGuiSelectionRequestType_SetRange,
            Selected: true,
            RangeDirection: -1,
            RangeFirstItem: 1,
            RangeLastItem: 3,
        };
        let mut io = sys::ImGuiMultiSelectIO::default();
        io.Requests.Data = &mut native_request;
        io.Requests.Size = 1;
        io.Requests.Capacity = 1;
        io.RangeSrcItem = 7;
        io.NavIdItem = 9;
        io.NavIdSelected = true;
        io.ItemsCount = 5;

        let copied = unsafe { copy_multi_select_result(&io) };
        native_request.Selected = false;
        native_request.RangeFirstItem = 0;
        io.RangeSrcItem = 11;
        assert!(!native_request.Selected);
        assert_eq!(native_request.RangeFirstItem, 0);
        assert_eq!(io.RangeSrcItem, 11);

        assert_eq!(
            copied.requests(),
            &[MultiSelectRequest::SetRange {
                selected: true,
                first: 1,
                last: 3,
                direction: MultiSelectRangeDirection::Backward,
            }]
        );
        assert_eq!(copied.range_source_item(), Some(7));
        assert_eq!(copied.navigation_item(), Some(9));
        assert!(copied.navigation_item_selected());
        assert_eq!(copied.items_count(), 5);
    }
}
