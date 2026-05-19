use crate::{Id, sys};

use super::basic_selection::BasicSelection;
use super::storage::MultiSelectIndexStorage;

/// Apply `ImGuiMultiSelectIO` requests to index-based selection storage.
///
/// This mirrors `ImGuiSelectionExternalStorage::ApplyRequests` from Dear ImGui,
/// but operates on the safe [`MultiSelectIndexStorage`] trait instead of relying
/// on C callbacks.
pub(super) unsafe fn apply_multi_select_requests_indexed<S: MultiSelectIndexStorage>(
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

/// Apply multi-select requests to a `BasicSelection` using an index→id mapping.
pub(super) unsafe fn apply_multi_select_requests_basic<G>(
    ms_io: *mut sys::ImGuiMultiSelectIO,
    selection: &mut BasicSelection,
    items_count: usize,
    id_at_index: &mut G,
) where
    G: FnMut(usize) -> Id,
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
