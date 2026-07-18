use crate::{Id, sys};

use super::basic_selection::BasicSelection;
use super::storage::MultiSelectIndexStorage;

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

        for request in &self.requests {
            match *request {
                MultiSelectRequest::SetAll { selected } => {
                    for index in 0..items_count {
                        storage.set_selected(index, selected);
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
                                storage.set_selected(index, selected);
                            }
                        }
                        MultiSelectRangeDirection::Backward => {
                            for index in range.rev() {
                                storage.set_selected(index, selected);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Apply these requests to a [`BasicSelection`] using an index-to-ID mapping.
    pub fn apply_requests_basic<G>(&self, selection: &mut BasicSelection, mut id_at_index: G)
    where
        G: FnMut(usize) -> Id,
    {
        for request in &self.requests {
            match *request {
                MultiSelectRequest::SetAll { selected } => {
                    for index in 0..self.items_count {
                        selection.set_selected(id_at_index(index), selected);
                    }
                }
                MultiSelectRequest::SetRange {
                    selected,
                    first,
                    last,
                    direction,
                } => {
                    let Some(range) = indexed_range(first, last, self.items_count) else {
                        continue;
                    };
                    match direction {
                        MultiSelectRangeDirection::Forward => {
                            for index in range {
                                selection.set_selected(id_at_index(index), selected);
                            }
                        }
                        MultiSelectRangeDirection::Backward => {
                            for index in range.rev() {
                                selection.set_selected(id_at_index(index), selected);
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn record_range_source_reset(&mut self, reset: bool) {
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

pub(super) unsafe fn copy_multi_select_result(
    io: *const sys::ImGuiMultiSelectIO,
) -> MultiSelectResult {
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
