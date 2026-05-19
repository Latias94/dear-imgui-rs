use crate::{Id, sys};

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
    pub fn contains(&self, id: Id) -> bool {
        unsafe { sys::ImGuiSelectionBasicStorage_Contains(self.raw, id.raw()) }
    }

    /// Set selection state for a given id.
    pub fn set_selected(&mut self, id: Id, selected: bool) {
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
