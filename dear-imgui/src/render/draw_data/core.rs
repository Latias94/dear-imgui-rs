use super::DrawListIterator;
use crate::internal::{RawCast, RawWrapper};
use crate::sys;
use std::slice;

/// All draw data to render a Dear ImGui frame.
#[repr(C)]
pub struct DrawData {
    /// Only valid after render() is called and before the next new frame() is called.
    pub(super) valid: bool,
    /// Number of DrawList to render.
    pub(super) cmd_lists_count: i32,
    /// For convenience, sum of all draw list index buffer sizes.
    pub(super) total_idx_count: i32,
    /// For convenience, sum of all draw list vertex buffer sizes.
    pub(super) total_vtx_count: i32,
    // Array of DrawList.
    pub(super) cmd_lists: crate::internal::ImVector<*mut sys::ImDrawList>,
    /// Upper-left position of the viewport to render.
    ///
    /// (= upper-left corner of the orthogonal projection matrix to use)
    pub display_pos: [f32; 2],
    /// Size of the viewport to render.
    ///
    /// (= display_pos + display_size == lower-right corner of the orthogonal matrix to use)
    pub display_size: [f32; 2],
    /// Amount of pixels for each unit of display_size.
    ///
    /// Based on io.display_frame_buffer_scale. Typically [1.0, 1.0] on normal displays, and
    /// [2.0, 2.0] on Retina displays, but fractional values are also possible.
    pub framebuffer_scale: [f32; 2],

    /// Viewport carrying the DrawData instance, might be of use to the renderer (generally not).
    pub(super) owner_viewport: *mut sys::ImGuiViewport,
    /// Texture data (internal use)
    pub(super) textures: *mut crate::internal::ImVector<*mut sys::ImTextureData>,
}

// Keep this struct layout-compatible with the sys bindings (`ImDrawData`).
const _: [(); std::mem::size_of::<sys::ImDrawData>()] = [(); std::mem::size_of::<DrawData>()];
const _: [(); std::mem::align_of::<sys::ImDrawData>()] = [(); std::mem::align_of::<DrawData>()];

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
        self.valid
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

    /// Returns the total number of index-buffer elements across all draw lists.
    #[inline]
    pub fn total_idx_count(&self) -> usize {
        total_count_from_i32("DrawData::total_idx_count()", self.total_idx_count)
    }

    /// Returns the total number of vertex-buffer elements across all draw lists.
    #[inline]
    pub fn total_vtx_count(&self) -> usize {
        total_count_from_i32("DrawData::total_vtx_count()", self.total_vtx_count)
    }

    /// Get the display position as an array
    #[inline]
    pub fn display_pos(&self) -> [f32; 2] {
        self.display_pos
    }

    /// Get the display size as an array
    #[inline]
    pub fn display_size(&self) -> [f32; 2] {
        self.display_size
    }

    /// Get the framebuffer scale as an array
    #[inline]
    pub fn framebuffer_scale(&self) -> [f32; 2] {
        self.framebuffer_scale
    }

    /// Raw owner viewport pointer for this draw data.
    ///
    /// This is primarily useful for integrations that snapshot multiple Dear ImGui platform
    /// viewports. The pointer belongs to the current ImGui context and must not be stored beyond
    /// the draw data lifetime.
    #[inline]
    pub fn owner_viewport(&self) -> *mut sys::ImGuiViewport {
        self.owner_viewport
    }

    #[inline]
    pub(crate) unsafe fn cmd_lists(&self) -> &[*mut sys::ImDrawList] {
        unsafe {
            if self.cmd_lists_count <= 0 || self.cmd_lists.data.is_null() {
                return &[];
            }
            let len = match usize::try_from(self.cmd_lists_count) {
                Ok(len) => len,
                Err(_) => return &[],
            };
            slice::from_raw_parts(self.cmd_lists.data, len)
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
