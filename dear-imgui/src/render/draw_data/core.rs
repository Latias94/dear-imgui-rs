use super::{DrawCmd, DrawListIterator};
use crate::internal::{RawCast, RawWrapper};
use crate::sys;
use std::slice;

/// All draw data to render a Dear ImGui frame.
#[repr(transparent)]
pub struct DrawData(pub(super) sys::ImDrawData);

/// Pointer-free capabilities required to consume one frame's draw commands.
///
/// Renderers can inspect this summary before applying managed-texture side effects. It does not
/// borrow draw lists or expose Context-owned native pointers.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct DrawRequirements {
    raw_callbacks: bool,
}

impl DrawRequirements {
    /// Whether the frame contains at least one non-standard native draw callback.
    #[must_use]
    pub const fn requires_raw_callback_support(self) -> bool {
        self.raw_callbacks
    }
}

unsafe impl RawCast<sys::ImDrawData> for DrawData {}

pub(super) fn total_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}

impl RawWrapper for DrawData {
    type Raw = sys::ImDrawData;

    unsafe fn raw(&self) -> &Self::Raw {
        unsafe { <Self as RawCast<Self::Raw>>::raw(self) }
    }

    unsafe fn raw_mut(&mut self) -> &mut Self::Raw {
        unsafe { <Self as RawCast<Self::Raw>>::raw_mut(self) }
    }
}

impl DrawData {
    /// Check if the draw data is valid
    ///
    /// Draw data is only valid after `Context::render()` is called and before
    /// the next `Context::new_frame()` is called.
    #[inline]
    pub fn valid(&self) -> bool {
        self.0.Valid
    }

    /// Returns the frame counter of the Context that emitted this draw data.
    ///
    /// This is primarily useful for diagnostics and correlating renderer work with a UI frame.
    #[inline]
    pub fn frame_count(&self) -> usize {
        total_count_from_i32("DrawData::frame_count()", self.0.FrameCount)
    }

    /// Returns an iterator over the draw lists included in the draw data.
    #[inline]
    pub fn draw_lists(&self) -> DrawListIterator<'_> {
        unsafe { DrawListIterator::new(self.cmd_lists().iter()) }
    }
    /// Returns the number of draw lists included in the draw data.
    #[inline]
    pub fn draw_lists_count(&self) -> usize {
        unsafe { self.cmd_lists().len() }
    }

    /// Summarizes renderer capabilities needed by this draw data without exposing native borrows.
    #[must_use]
    pub fn requirements(&self) -> DrawRequirements {
        let raw_callbacks = self.draw_lists().any(|draw_list| {
            draw_list
                .commands()
                .any(|command| matches!(command, DrawCmd::RawCallback(_)))
        });
        DrawRequirements { raw_callbacks }
    }

    /// Returns the total number of index-buffer elements across all draw lists.
    #[inline]
    pub fn total_idx_count(&self) -> usize {
        total_count_from_i32("DrawData::total_idx_count()", self.0.TotalIdxCount)
    }

    /// Returns the total number of vertex-buffer elements across all draw lists.
    #[inline]
    pub fn total_vtx_count(&self) -> usize {
        total_count_from_i32("DrawData::total_vtx_count()", self.0.TotalVtxCount)
    }

    /// Get the display position as an array
    #[inline]
    pub fn display_pos(&self) -> [f32; 2] {
        [self.0.DisplayPos.x, self.0.DisplayPos.y]
    }

    /// Get the display size as an array
    #[inline]
    pub fn display_size(&self) -> [f32; 2] {
        [self.0.DisplaySize.x, self.0.DisplaySize.y]
    }

    /// Get the framebuffer scale as an array
    #[inline]
    pub fn framebuffer_scale(&self) -> [f32; 2] {
        [self.0.FramebufferScale.x, self.0.FramebufferScale.y]
    }

    /// Raw owner viewport pointer for this draw data.
    ///
    /// This is primarily useful for integrations that snapshot multiple Dear ImGui platform
    /// viewports. The pointer belongs to the current ImGui context and must not be stored beyond
    /// the draw data lifetime.
    #[inline]
    pub fn owner_viewport(&self) -> *mut sys::ImGuiViewport {
        self.0.OwnerViewport
    }

    #[inline]
    pub(crate) unsafe fn cmd_lists(&self) -> &[*mut sys::ImDrawList] {
        unsafe {
            if self.0.CmdLists.Size <= 0 || self.0.CmdLists.Data.is_null() {
                return &[];
            }
            let len = match usize::try_from(self.0.CmdLists.Size) {
                Ok(len) => len,
                Err(_) => return &[],
            };
            slice::from_raw_parts(self.0.CmdLists.Data, len)
        }
    }

    /// Converts all buffers from indexed to non-indexed, in case you cannot render indexed buffers
    ///
    /// **This is slow and most likely a waste of resources. Always prefer indexed rendering!**
    #[doc(alias = "DeIndexAllBuffers")]
    pub fn deindex_all_buffers(&mut self) {
        unsafe {
            sys::ImDrawData_DeIndexAllBuffers(RawWrapper::raw_mut(self));
        }
    }

    /// Scales the clip rect of each draw command
    ///
    /// Can be used if your final output buffer is at a different scale than Dear ImGui expects,
    /// or if there is a difference between your window resolution and framebuffer resolution.
    #[doc(alias = "ScaleClipRects")]
    pub fn scale_clip_rects(&mut self, fb_scale: [f32; 2]) {
        unsafe {
            let scale = sys::ImVec2 {
                x: fb_scale[0],
                y: fb_scale[1],
            };
            sys::ImDrawData_ScaleClipRects(RawWrapper::raw_mut(self), scale);
        }
    }
}
