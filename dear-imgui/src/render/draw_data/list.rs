use super::{DrawCmdIterator, DrawIdx, DrawVert};
use crate::internal::RawWrapper;
use crate::sys;
use std::slice;

/// Iterator over draw lists
pub struct DrawListIterator<'a> {
    iter: std::slice::Iter<'a, *mut sys::ImDrawList>,
}

impl<'a> Iterator for DrawListIterator<'a> {
    type Item = &'a DrawList;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().and_then(|&ptr| {
            if ptr.is_null() {
                None
            } else {
                Some(unsafe { DrawList::from_raw(ptr.cast_const()) })
            }
        })
    }
}

impl<'a> ExactSizeIterator for DrawListIterator<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> DrawListIterator<'a> {
    pub(super) fn new(iter: std::slice::Iter<'a, *mut sys::ImDrawList>) -> Self {
        Self { iter }
    }
}

/// Draw command list
#[repr(transparent)]
pub struct DrawList(sys::ImDrawList);

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImDrawList>()] = [(); std::mem::size_of::<DrawList>()];
const _: [(); std::mem::align_of::<sys::ImDrawList>()] = [(); std::mem::align_of::<DrawList>()];

impl RawWrapper for DrawList {
    type Raw = sys::ImDrawList;
    #[inline]
    unsafe fn raw(&self) -> &sys::ImDrawList {
        &self.0
    }
    #[inline]
    unsafe fn raw_mut(&mut self) -> &mut sys::ImDrawList {
        &mut self.0
    }
}

impl DrawList {
    #[inline]
    pub(crate) unsafe fn from_raw<'a>(raw: *const sys::ImDrawList) -> &'a Self {
        unsafe { &*(raw as *const Self) }
    }

    #[inline]
    pub(crate) unsafe fn cmd_buffer(&self) -> &[sys::ImDrawCmd] {
        unsafe {
            let cmd_buffer = &self.0.CmdBuffer;
            if cmd_buffer.Size <= 0 || cmd_buffer.Data.is_null() {
                return &[];
            }
            let len = match usize::try_from(cmd_buffer.Size) {
                Ok(len) => len,
                Err(_) => return &[],
            };
            slice::from_raw_parts(cmd_buffer.Data, len)
        }
    }

    /// Returns an iterator over the draw commands in this draw list
    pub fn commands(&self) -> DrawCmdIterator<'_> {
        unsafe { DrawCmdIterator::new(self.cmd_buffer().iter()) }
    }

    /// Get vertex buffer as slice
    pub fn vtx_buffer(&self) -> &[DrawVert] {
        unsafe {
            let vtx_buffer = &self.0.VtxBuffer;
            if vtx_buffer.Size <= 0 || vtx_buffer.Data.is_null() {
                return &[];
            }
            let len = match usize::try_from(vtx_buffer.Size) {
                Ok(len) => len,
                Err(_) => return &[],
            };
            slice::from_raw_parts(vtx_buffer.Data as *const DrawVert, len)
        }
    }

    /// Get index buffer as slice
    pub fn idx_buffer(&self) -> &[DrawIdx] {
        unsafe {
            let idx_buffer = &self.0.IdxBuffer;
            if idx_buffer.Size <= 0 || idx_buffer.Data.is_null() {
                return &[];
            }
            let len = match usize::try_from(idx_buffer.Size) {
                Ok(len) => len,
                Err(_) => return &[],
            };
            slice::from_raw_parts(idx_buffer.Data, len)
        }
    }
}

/// Owned draw list returned by `CloneOutput`.
///
/// This owns an independent copy of a draw list and will free it on drop.
pub struct OwnedDrawList(*mut sys::ImDrawList);

impl Drop for OwnedDrawList {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_destroy(self.0) }
    }
}

impl OwnedDrawList {
    /// Create from raw pointer.
    ///
    /// Safety: `ptr` must be a valid pointer returned by `ImDrawList_CloneOutput` or `ImDrawList_ImDrawList`.
    pub(crate) unsafe fn from_raw(ptr: *mut sys::ImDrawList) -> Self {
        Self(ptr)
    }

    /// Borrow as a read-only draw list view.
    pub fn as_view(&self) -> &DrawList {
        unsafe { DrawList::from_raw(self.0) }
    }

    /// Clear free memory held by the draw list (release heap allocations).
    pub fn clear_free_memory(&mut self) {
        unsafe { sys::ImDrawList__ClearFreeMemory(self.0) }
    }

    /// Reset for new frame (not commonly needed for cloned lists).
    pub fn reset_for_new_frame(&mut self) {
        unsafe { sys::ImDrawList__ResetForNewFrame(self.0) }
    }
}
