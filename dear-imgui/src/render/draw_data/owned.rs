use super::{DrawData, assert_draw_list_cloneable};
use crate::internal::RawWrapper;
use crate::sys;
use std::marker::PhantomData;
use std::rc::Rc;

/// A container for a heap-allocated deep copy of a `DrawData` struct.
///
/// Notes on thread-safety:
/// - This type intentionally does NOT implement `Send`/`Sync`. Although draw lists are cloned,
///   draw commands can still carry raw texture references and backend-specific assumptions.
/// - The context-owned texture request list is not copied. Use
///   [`crate::render::snapshot::FrameSnapshot`] when you need texture requests or thread-safe
///   rendering data detached from the ImGui context.
///
/// The underlying copy is released when this struct is dropped.
pub struct OwnedDrawData {
    draw_data: *mut sys::ImDrawData,
    // Keep this conservative until OwnedDrawData has a fully specified thread-safe contract.
    _no_send_sync: PhantomData<Rc<()>>,
}

impl OwnedDrawData {
    /// If this struct contains a `DrawData` object, then this function returns a reference to it.
    ///
    /// Otherwise, this struct is empty and so this function returns `None`.
    #[inline]
    pub fn draw_data(&self) -> Option<&DrawData> {
        if !self.draw_data.is_null() {
            Some(unsafe { &*(self.draw_data as *const DrawData) })
        } else {
            None
        }
    }
}

impl Default for OwnedDrawData {
    /// The default `OwnedDrawData` struct is empty.
    #[inline]
    fn default() -> Self {
        Self {
            draw_data: std::ptr::null_mut(),
            _no_send_sync: PhantomData,
        }
    }
}

impl From<&DrawData> for OwnedDrawData {
    /// Construct `OwnedDrawData` from `DrawData` by creating a heap-allocated deep copy of the given `DrawData`
    fn from(value: &DrawData) -> Self {
        unsafe {
            let source_ptr = RawWrapper::raw(value);
            if source_ptr.CmdListsCount > 0 && !source_ptr.CmdLists.Data.is_null() {
                for i in 0..(source_ptr.CmdListsCount as usize) {
                    let src_list = *source_ptr.CmdLists.Data.add(i);
                    assert_draw_list_cloneable(src_list.cast_const(), "OwnedDrawData::from");
                }
            }

            // Allocate a new ImDrawData using the constructor
            let result = sys::ImDrawData_ImDrawData();
            if result.is_null() {
                panic!("Failed to allocate ImDrawData for OwnedDrawData");
            }

            // Copy basic fields from the source
            (*result).Valid = source_ptr.Valid;
            (*result).TotalIdxCount = source_ptr.TotalIdxCount;
            (*result).TotalVtxCount = source_ptr.TotalVtxCount;
            (*result).DisplayPos = source_ptr.DisplayPos;
            (*result).DisplaySize = source_ptr.DisplaySize;
            (*result).FramebufferScale = source_ptr.FramebufferScale;
            (*result).OwnerViewport = source_ptr.OwnerViewport;

            // Copy draw lists by cloning each list to ensure OwnedDrawData owns its memory
            (*result).CmdListsCount = 0;
            if source_ptr.CmdListsCount > 0 && !source_ptr.CmdLists.Data.is_null() {
                for i in 0..(source_ptr.CmdListsCount as usize) {
                    let src_list = *source_ptr.CmdLists.Data.add(i);
                    if !src_list.is_null() {
                        // Clone the output of the draw list to own it
                        let cloned = sys::ImDrawList_CloneOutput(src_list);
                        if !cloned.is_null() {
                            sys::ImDrawData_AddDrawList(result, cloned);
                        }
                    }
                }
            }

            // The texture request list belongs to the originating ImGui context. Copying the raw
            // pointer would let a detached OwnedDrawData expose shared TextureData references while
            // the live context mutates the same objects through PlatformIo/DrawData.
            (*result).Textures = std::ptr::null_mut();

            OwnedDrawData {
                draw_data: result,
                _no_send_sync: PhantomData,
            }
        }
    }
}

impl From<&mut DrawData> for OwnedDrawData {
    /// Construct `OwnedDrawData` from mutable draw data by reborrowing it as shared draw data.
    fn from(value: &mut DrawData) -> Self {
        OwnedDrawData::from(&*value)
    }
}

impl Drop for OwnedDrawData {
    /// Releases any heap-allocated memory consumed by this `OwnedDrawData` object
    fn drop(&mut self) {
        unsafe {
            if !self.draw_data.is_null() {
                // Destroy cloned draw lists if any
                if !(*self.draw_data).CmdLists.Data.is_null() {
                    for i in 0..(*self.draw_data).CmdListsCount as usize {
                        let ptr = *(*self.draw_data).CmdLists.Data.add(i);
                        if !ptr.is_null() {
                            sys::ImDrawList_destroy(ptr);
                        }
                    }
                }

                // Destroy the ImDrawData itself
                sys::ImDrawData_destroy(self.draw_data);
                self.draw_data = std::ptr::null_mut();
            }
        }
    }
}
