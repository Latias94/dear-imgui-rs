use crate::{Id, sys};

use super::basic_selection::BasicSelection;
use super::options::MultiSelectOptions;
use super::requests::{MultiSelectResult, MultiSelectUserData, copy_multi_select_result};
use super::storage::MultiSelectIndexStorage;

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
    ended: bool,
}

impl<'ui> MultiSelectScope<'ui> {
    pub(super) fn new(
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

    pub(super) fn finish(mut self) -> MultiSelectResult {
        let native_io = self.end_native();
        self.ended = true;
        let mut result = unsafe { copy_multi_select_result(native_io) };
        result.record_range_source_reset(self.range_source_reset);
        result
    }

    fn end_native(&mut self) -> *mut sys::ImGuiMultiSelectIO {
        self.ui.run_with_bound_context(|| unsafe {
            if self.range_source_reset {
                let context = sys::igGetCurrentContext();
                let active = context.as_ref().and_then(|context| {
                    context
                        .CurrentMultiSelect
                        .cast::<sys::ImGuiMultiSelectTempData>()
                        .as_mut()
                });
                if let Some(active) = active {
                    active.IO.RangeSrcReset = true;
                }
            }
            sys::igEndMultiSelect()
        })
    }
}

impl Drop for MultiSelectScope<'_> {
    fn drop(&mut self) {
        if self.ended {
            return;
        }

        self.end_native();
        self.ended = true;
    }
}
