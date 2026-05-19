use crate::sys;

use super::basic_selection::BasicSelection;
use super::options::MultiSelectOptions;
use super::requests::{apply_multi_select_requests_basic, apply_multi_select_requests_indexed};
use super::storage::MultiSelectIndexStorage;

fn usize_to_i32(name: &str, value: usize) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| panic!("{name} exceeded ImGui's i32 range"))
}

/// RAII wrapper around `BeginMultiSelect()` / `EndMultiSelect()` for advanced users.
///
/// This gives direct, but scoped, access to the underlying `ImGuiMultiSelectIO`
/// struct. It does not perform any selection updates by itself; you are expected
/// to call helper methods or use the raw IO to drive your own storage.
pub struct MultiSelectScope<'ui> {
    pub(super) ms_io_begin: *mut sys::ImGuiMultiSelectIO,
    items_count: i32,
    ended: bool,
    _marker: std::marker::PhantomData<&'ui crate::Ui>,
}

impl<'ui> MultiSelectScope<'ui> {
    pub(super) fn new(
        flags: impl Into<MultiSelectOptions>,
        selection_size: Option<i32>,
        items_count: usize,
    ) -> Self {
        let options = flags.into();
        let selection_size_i32 = selection_size.unwrap_or(-1);
        let items_count_i32 = usize_to_i32("items_count", items_count);
        let ms_io_begin =
            unsafe { sys::igBeginMultiSelect(options.raw(), selection_size_i32, items_count_i32) };
        Self {
            ms_io_begin,
            items_count: items_count_i32,
            ended: false,
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
    pub fn end(mut self) -> MultiSelectEnd<'ui> {
        let ms_io_end = unsafe { sys::igEndMultiSelect() };
        self.ended = true;
        MultiSelectEnd {
            ms_io_end,
            items_count: self.items_count,
            _marker: std::marker::PhantomData,
        }
    }
}

impl Drop for MultiSelectScope<'_> {
    fn drop(&mut self) {
        if !self.ended {
            unsafe {
                sys::igEndMultiSelect();
            }
            self.ended = true;
        }
    }
}

/// IO view returned after calling `EndMultiSelect()` via [`MultiSelectScope::end`].
pub struct MultiSelectEnd<'ui> {
    ms_io_end: *mut sys::ImGuiMultiSelectIO,
    items_count: i32,
    _marker: std::marker::PhantomData<&'ui crate::Ui>,
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
