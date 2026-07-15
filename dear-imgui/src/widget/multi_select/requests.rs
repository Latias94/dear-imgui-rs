use crate::{Id, sys};

use super::basic_selection::BasicSelection;
use super::storage::MultiSelectIndexStorage;

fn request_range(
    request: &sys::ImGuiSelectionRequest,
    items_count: usize,
) -> Option<std::ops::RangeInclusive<usize>> {
    let first = usize::try_from(request.RangeFirstItem).ok()?;
    let last = usize::try_from(request.RangeLastItem).ok()?;
    if first > last || first >= items_count {
        return None;
    }
    Some(first..=last.min(items_count.saturating_sub(1)))
}

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
                if let Some(range) = request_range(req, items_count) {
                    if req.RangeDirection < 0 {
                        for idx in range.rev() {
                            storage.set_selected(idx, req.Selected);
                        }
                    } else {
                        for idx in range {
                            storage.set_selected(idx, req.Selected);
                        }
                    }
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
                if let Some(range) = request_range(req, items_count) {
                    if req.RangeDirection < 0 {
                        for idx in range.rev() {
                            let id = id_at_index(idx);
                            selection.set_selected(id, req.Selected);
                        }
                    } else {
                        for idx in range {
                            let id = id_at_index(idx);
                            selection.set_selected(id, req.Selected);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn indexed_requests_preserve_backward_range_direction() {
        let mut request = sys::ImGuiSelectionRequest {
            Type: sys::ImGuiSelectionRequestType_SetRange,
            Selected: true,
            RangeDirection: -1,
            RangeFirstItem: 1,
            RangeLastItem: 3,
        };
        let mut io = sys::ImGuiMultiSelectIO::default();
        io.ItemsCount = 5;
        io.Requests.Data = &mut request;
        io.Requests.Size = 1;
        io.Requests.Capacity = 1;
        let mut storage = RecordingStorage {
            selected: vec![false; 5],
            updates: Vec::new(),
        };

        unsafe { apply_multi_select_requests_indexed(&mut io, &mut storage) };

        assert_eq!(storage.updates, vec![3, 2, 1]);
        assert_eq!(storage.selected, vec![false, true, true, true, false]);
    }
}
