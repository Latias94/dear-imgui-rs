//! Multi-select helpers (BeginMultiSelect/EndMultiSelect)
//!
//! This module provides a small, safe wrapper around Dear ImGui's multi-select
//! API introduced in 1.92 (`BeginMultiSelect` / `EndMultiSelect`), following
//! the "external storage" pattern described in the official docs:
//! https://github.com/ocornut/imgui/wiki/Multi-Select
//!
//! The main entry point is [`Ui::multi_select_indexed`], which:
//! - wraps `BeginMultiSelect()` / `EndMultiSelect()`
//! - wires `SetNextItemSelectionUserData()` for each item (index-based)
//! - applies selection requests to your storage using a simple trait.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::Ui;
use crate::sys;
use std::collections::HashSet;

bitflags::bitflags! {
    /// Flags controlling multi-selection behavior.
    ///
    /// These mirror Dear ImGui's `ImGuiMultiSelectFlags` and control how
    /// selection works (single vs multi, box-select, keyboard shortcuts, etc).
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MultiSelectFlags: i32 {
        /// No flags.
        const NONE = sys::ImGuiMultiSelectFlags_None as i32;
        /// Single-selection scope. Ctrl/Shift range selection is disabled.
        const SINGLE_SELECT = sys::ImGuiMultiSelectFlags_SingleSelect as i32;
        /// Disable `Ctrl+A` "select all" shortcut.
        const NO_SELECT_ALL = sys::ImGuiMultiSelectFlags_NoSelectAll as i32;
        /// Disable range selection (Shift+click / Shift+arrow).
        const NO_RANGE_SELECT = sys::ImGuiMultiSelectFlags_NoRangeSelect as i32;
        /// Disable automatic selection of newly focused items.
        const NO_AUTO_SELECT = sys::ImGuiMultiSelectFlags_NoAutoSelect as i32;
        /// Disable automatic clearing of selection when focus moves within the scope.
        const NO_AUTO_CLEAR = sys::ImGuiMultiSelectFlags_NoAutoClear as i32;
        /// Disable automatic clearing when reselecting the same range.
        const NO_AUTO_CLEAR_ON_RESELECT =
            sys::ImGuiMultiSelectFlags_NoAutoClearOnReselect as i32;
        /// Enable 1D box-select (same x, full-width rows).
        const BOX_SELECT_1D = sys::ImGuiMultiSelectFlags_BoxSelect1d as i32;
        /// Enable 2D box-select (arbitrary item layout).
        const BOX_SELECT_2D = sys::ImGuiMultiSelectFlags_BoxSelect2d as i32;
        /// Disable drag-scrolling when box-selecting near edges of the scope.
        const BOX_SELECT_NO_SCROLL = sys::ImGuiMultiSelectFlags_BoxSelectNoScroll as i32;
        /// Clear selection when pressing Escape while the scope is focused.
        const CLEAR_ON_ESCAPE = sys::ImGuiMultiSelectFlags_ClearOnEscape as i32;
        /// Clear selection when clicking on empty space (void) inside the scope.
        const CLEAR_ON_CLICK_VOID = sys::ImGuiMultiSelectFlags_ClearOnClickVoid as i32;
        /// Scope is the whole window (default).
        const SCOPE_WINDOW = sys::ImGuiMultiSelectFlags_ScopeWindow as i32;
        /// Scope is a rectangular region between `BeginMultiSelect()`/`EndMultiSelect()`.
        const SCOPE_RECT = sys::ImGuiMultiSelectFlags_ScopeRect as i32;
        /// Apply selection to items on mouse down.
        const SELECT_ON_CLICK = sys::ImGuiMultiSelectFlags_SelectOnClick as i32;
        /// Apply selection on mouse release (allows dragging without altering selection).
        const SELECT_ON_CLICK_RELEASE =
            sys::ImGuiMultiSelectFlags_SelectOnClickRelease as i32;
        /// Enable X-axis navigation wrap helper.
        const NAV_WRAP_X = sys::ImGuiMultiSelectFlags_NavWrapX as i32;
        /// Disable default right-click behavior that selects item before opening a context menu.
        const NO_SELECT_ON_RIGHT_CLICK =
            sys::ImGuiMultiSelectFlags_NoSelectOnRightClick as i32;
    }
}

/// Selection container backed by Dear ImGui's `ImGuiSelectionBasicStorage`.
///
/// This stores a set of selected `ImGuiID` values using the optimized helper
/// provided by Dear ImGui. It is suitable when items are naturally identified
/// by stable IDs (e.g. table rows, tree nodes).
#[derive(Debug)]
pub struct BasicSelection {
    raw: *mut sys::ImGuiSelectionBasicStorage,
}

impl BasicSelection {
    /// Create an empty selection storage.
    pub fn new() -> Self {
        unsafe {
            let ptr = sys::ImGuiSelectionBasicStorage_ImGuiSelectionBasicStorage();
            if ptr.is_null() {
                panic!("ImGuiSelectionBasicStorage_ImGuiSelectionBasicStorage() returned null");
            }
            Self { raw: ptr }
        }
    }

    /// Return the number of selected items.
    pub fn len(&self) -> usize {
        unsafe {
            let size = (*self.raw).Size;
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
            sys::ImGuiSelectionBasicStorage_Clear(self.raw);
        }
    }

    /// Returns true if the given id is selected.
    pub fn contains(&self, id: crate::Id) -> bool {
        unsafe { sys::ImGuiSelectionBasicStorage_Contains(self.raw, id.raw()) }
    }

    /// Set selection state for a given id.
    pub fn set_selected(&mut self, id: crate::Id, selected: bool) {
        unsafe {
            sys::ImGuiSelectionBasicStorage_SetItemSelected(self.raw, id.raw(), selected);
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
        self.raw
    }
}

impl Default for BasicSelection {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BasicSelection {
    fn drop(&mut self) {
        unsafe {
            if !self.raw.is_null() {
                sys::ImGuiSelectionBasicStorage_destroy(self.raw);
                self.raw = std::ptr::null_mut();
            }
        }
    }
}

/// Iterator over selected ids stored in [`BasicSelection`].
pub struct BasicSelectionIter<'a> {
    storage: &'a BasicSelection,
    it: *mut std::os::raw::c_void,
}

impl<'a> Iterator for BasicSelectionIter<'a> {
    type Item = crate::Id;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut out_id: sys::ImGuiID = 0;
            let has_next = sys::ImGuiSelectionBasicStorage_GetNextSelectedItem(
                self.storage.as_raw(),
                &mut self.it,
                &mut out_id,
            );
            if has_next {
                Some(crate::Id::from(out_id))
            } else {
                None
            }
        }
    }
}

/// Index-based selection storage for multi-select helpers.
///
/// Implement this trait for your selection container (e.g. `Vec<bool>`,
/// `Vec<MyItem { selected: bool }>` or a custom type) to use
/// [`Ui::multi_select_indexed`].
pub trait MultiSelectIndexStorage {
    /// Total number of items in the selection scope.
    fn len(&self) -> usize;

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

impl<'a> MultiSelectIndexStorage for &'a mut [bool] {
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

/// Apply `ImGuiMultiSelectIO` requests to index-based selection storage.
///
/// This mirrors `ImGuiSelectionExternalStorage::ApplyRequests` from Dear ImGui,
/// but operates on the safe [`MultiSelectIndexStorage`] trait instead of relying
/// on C callbacks.
unsafe fn apply_multi_select_requests_indexed<S: MultiSelectIndexStorage>(
    ms_io: *mut sys::ImGuiMultiSelectIO,
    storage: &mut S,
) {
    unsafe {
        if ms_io.is_null() {
            return;
        }

        let io_ref: &mut sys::ImGuiMultiSelectIO = &mut *ms_io;
        let items_count = usize::try_from(io_ref.ItemsCount).unwrap_or(0);

        let requests = &mut io_ref.Requests;
        if requests.Data.is_null() || requests.Size <= 0 {
            return;
        }

        let len = match usize::try_from(requests.Size) {
            Ok(len) => len,
            Err(_) => return,
        };
        let slice = std::slice::from_raw_parts_mut(requests.Data, len);

        for req in slice {
            if req.Type == sys::ImGuiSelectionRequestType_SetAll {
                for idx in 0..items_count {
                    storage.set_selected(idx, req.Selected);
                }
            } else if req.Type == sys::ImGuiSelectionRequestType_SetRange {
                let first = req.RangeFirstItem as i32;
                let last = req.RangeLastItem as i32;
                if first < 0 || last < first {
                    continue;
                }
                let last_clamped = std::cmp::min(last as usize, items_count.saturating_sub(1));
                for idx in first as usize..=last_clamped {
                    storage.set_selected(idx, req.Selected);
                }
            }
        }
    }
}

/// RAII wrapper around `BeginMultiSelect()` / `EndMultiSelect()` for advanced users.
///
/// This gives direct, but scoped, access to the underlying `ImGuiMultiSelectIO`
/// struct. It does not perform any selection updates by itself; you are expected
/// to call helper methods or use the raw IO to drive your own storage.
pub struct MultiSelectScope<'ui> {
    ms_io_begin: *mut sys::ImGuiMultiSelectIO,
    items_count: i32,
    _marker: std::marker::PhantomData<&'ui Ui>,
}

impl<'ui> MultiSelectScope<'ui> {
    fn new(flags: MultiSelectFlags, selection_size: Option<i32>, items_count: usize) -> Self {
        let selection_size_i32 = selection_size.unwrap_or(-1);
        let items_count_i32 = i32::try_from(items_count).unwrap_or(i32::MAX);
        let ms_io_begin = unsafe {
            sys::igBeginMultiSelect(flags.bits(), selection_size_i32, items_count_i32)
        };
        Self {
            ms_io_begin,
            items_count: items_count_i32,
            _marker: std::marker::PhantomData,
        }
    }

    /// Access the IO struct returned by `BeginMultiSelect()`.
    pub fn begin_io(&self) -> &sys::ImGuiMultiSelectIO {
        unsafe { &*self.ms_io_begin }
    }

    /// Mutable access to the IO struct returned by `BeginMultiSelect()`.
    pub fn begin_io_mut(&mut self) -> &mut sys::ImGuiMultiSelectIO {
        unsafe { &mut *self.ms_io_begin }
    }

    /// Apply selection requests from `BeginMultiSelect()` to index-based storage.
    pub fn apply_begin_requests_indexed<S: MultiSelectIndexStorage>(&mut self, storage: &mut S) {
        unsafe {
            apply_multi_select_requests_indexed(self.ms_io_begin, storage);
        }
    }

    /// Finalize the multi-select scope and return an IO view for the end state.
    ///
    /// This calls `EndMultiSelect()` and returns a `MultiSelectEnd` wrapper
    /// that can be used to apply the final selection requests.
    pub fn end(self) -> MultiSelectEnd<'ui> {
        let ms_io_end = unsafe { sys::igEndMultiSelect() };
        MultiSelectEnd {
            ms_io_end,
            items_count: self.items_count,
            _marker: std::marker::PhantomData,
        }
    }
}

/// IO view returned after calling `EndMultiSelect()` via [`MultiSelectScope::end`].
pub struct MultiSelectEnd<'ui> {
    ms_io_end: *mut sys::ImGuiMultiSelectIO,
    items_count: i32,
    _marker: std::marker::PhantomData<&'ui Ui>,
}

impl<'ui> MultiSelectEnd<'ui> {
    /// Access the IO struct returned by `EndMultiSelect()`.
    pub fn io(&self) -> &sys::ImGuiMultiSelectIO {
        unsafe { &*self.ms_io_end }
    }

    /// Mutable access to the IO struct returned by `EndMultiSelect()`.
    pub fn io_mut(&mut self) -> &mut sys::ImGuiMultiSelectIO {
        unsafe { &mut *self.ms_io_end }
    }

    /// Apply selection requests from `EndMultiSelect()` to index-based storage.
    pub fn apply_requests_indexed<S: MultiSelectIndexStorage>(&mut self, storage: &mut S) {
        unsafe {
            apply_multi_select_requests_indexed(self.ms_io_end, storage);
        }
    }

    /// Apply selection requests from `EndMultiSelect()` to a [`BasicSelection`].
    pub fn apply_requests_basic<G>(&mut self, selection: &mut BasicSelection, mut id_at_index: G)
    where
        G: FnMut(usize) -> crate::Id,
    {
        unsafe {
            apply_multi_select_requests_basic(
                self.ms_io_end,
                selection,
                self.items_count as usize,
                &mut id_at_index,
            );
        }
    }
}

impl Ui {
    /// Low-level entry point: begin a multi-select scope and return a RAII wrapper.
    ///
    /// This is the closest safe wrapper to the raw `BeginMultiSelect()` /
    /// `EndMultiSelect()` pair. It does not drive any selection storage by
    /// itself; use `begin_io()` / `end().io()` and the helper methods to
    /// implement custom patterns.
    pub fn begin_multi_select_raw(
        &self,
        flags: MultiSelectFlags,
        selection_size: Option<i32>,
        items_count: usize,
    ) -> MultiSelectScope<'_> {
        MultiSelectScope::new(flags, selection_size, items_count)
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
    /// ui.multi_select_indexed(&mut selected, MultiSelectFlags::NONE, |ui, idx, is_selected| {
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
    pub fn multi_select_indexed<S, F>(
        &self,
        storage: &mut S,
        flags: MultiSelectFlags,
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

        // Begin multi-select scope.
        let ms_io_begin = unsafe {
            sys::igBeginMultiSelect(flags.bits(), selection_size_i32, items_count as i32)
        };

        // Apply SetAll requests (if any) before submitting items.
        unsafe {
            apply_multi_select_requests_indexed(ms_io_begin, storage);
        }

        // Submit items: for each index we set SelectionUserData and let user
        // draw widgets, passing the current selection state as `is_selected`.
        for idx in 0..items_count {
            unsafe {
                sys::igSetNextItemSelectionUserData(idx as sys::ImGuiSelectionUserData);
            }
            let is_selected = storage.is_selected(idx);
            render_item(self, idx, is_selected);
        }

        // End scope and apply requests generated during item submission.
        let ms_io_end = unsafe { sys::igEndMultiSelect() };
        unsafe {
            apply_multi_select_requests_indexed(ms_io_end, storage);
        }
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
        flags: MultiSelectFlags,
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

        let ms_io_begin =
            unsafe { sys::igBeginMultiSelect(flags.bits(), selection_size_i32, row_count as i32) };

        unsafe {
            apply_multi_select_requests_indexed(ms_io_begin, storage);
        }

        for row in 0..row_count {
            unsafe {
                sys::igSetNextItemSelectionUserData(row as sys::ImGuiSelectionUserData);
            }
            // Start a new table row and move to first column.
            self.table_next_row();
            self.table_next_column();

            let is_selected = storage.is_selected(row);
            build_row(self, row, is_selected);
        }

        let ms_io_end = unsafe { sys::igEndMultiSelect() };
        unsafe {
            apply_multi_select_requests_indexed(ms_io_end, storage);
        }
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
        flags: MultiSelectFlags,
        items_count: usize,
        mut id_at_index: G,
        mut render_item: F,
    ) where
        G: FnMut(usize) -> crate::Id,
        F: FnMut(&Ui, usize, crate::Id, bool),
    {
        let selection_size_i32 = i32::try_from(selection.len()).unwrap_or(-1);

        let ms_io_begin = unsafe {
            sys::igBeginMultiSelect(flags.bits(), selection_size_i32, items_count as i32)
        };

        unsafe {
            apply_multi_select_requests_basic(
                ms_io_begin,
                selection,
                items_count,
                &mut id_at_index,
            );
        }

        for idx in 0..items_count {
            unsafe {
                sys::igSetNextItemSelectionUserData(idx as sys::ImGuiSelectionUserData);
            }
            let id = id_at_index(idx);
            let is_selected = selection.contains(id);
            render_item(self, idx, id, is_selected);
        }

        let ms_io_end = unsafe { sys::igEndMultiSelect() };
        unsafe {
            apply_multi_select_requests_basic(ms_io_end, selection, items_count, &mut id_at_index);
        }
    }
}

/// Apply multi-select requests to a `BasicSelection` using an index→id mapping.
unsafe fn apply_multi_select_requests_basic<G>(
    ms_io: *mut sys::ImGuiMultiSelectIO,
    selection: &mut BasicSelection,
    items_count: usize,
    id_at_index: &mut G,
) where
    G: FnMut(usize) -> crate::Id,
{
    unsafe {
        if ms_io.is_null() {
            return;
        }

        let io_ref: &mut sys::ImGuiMultiSelectIO = &mut *ms_io;
        let requests = &mut io_ref.Requests;
        if requests.Data.is_null() || requests.Size <= 0 {
            return;
        }

        let len = match usize::try_from(requests.Size) {
            Ok(len) => len,
            Err(_) => return,
        };
        let slice = std::slice::from_raw_parts_mut(requests.Data, len);

        for req in slice {
            if req.Type == sys::ImGuiSelectionRequestType_SetAll {
                for idx in 0..items_count {
                    let id = id_at_index(idx);
                    selection.set_selected(id, req.Selected);
                }
            } else if req.Type == sys::ImGuiSelectionRequestType_SetRange {
                let first = req.RangeFirstItem as i32;
                let last = req.RangeLastItem as i32;
                if first < 0 || last < first {
                    continue;
                }
                let last_clamped = std::cmp::min(last as usize, items_count.saturating_sub(1));
                for idx in first as usize..=last_clamped {
                    let id = id_at_index(idx);
                    selection.set_selected(id, req.Selected);
                }
            }
        }
    }
}
