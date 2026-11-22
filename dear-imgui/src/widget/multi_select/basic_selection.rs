use crate::{ContextThreadLease, Id, ImGuiError, ImGuiResult, sys};
use std::ptr::NonNull;

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
