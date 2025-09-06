//! Draw data structures for Dear ImGui rendering
//!
//! This module provides safe Rust wrappers around Dear ImGui's draw data structures,
//! which contain all the information needed to render a frame.

use crate::internal::{RawCast, RawWrapper};
use crate::render::renderer::TextureId;
use crate::sys;
use std::slice;

/// All draw data to render a Dear ImGui frame
///
/// This structure contains all the draw lists and associated data needed
/// to render a complete Dear ImGui frame. It's returned by `Context::render()`.
#[repr(C)]
pub struct DrawData {
    /// Only valid after render() is called and before the next new_frame() is called
    valid: bool,
    /// Number of DrawList to render
    cmd_lists_count: i32,
    /// For convenience, sum of all draw list index buffer sizes
    pub total_idx_count: i32,
    /// For convenience, sum of all draw list vertex buffer sizes
    pub total_vtx_count: i32,
    /// Array of DrawList pointers
    cmd_lists: *mut *mut sys::ImDrawList,
    /// Upper-left position of the viewport to render
    ///
    /// (= upper-left corner of the orthogonal projection matrix to use)
    pub display_pos: [f32; 2],
    /// Size of the viewport to render
    ///
    /// (= display_pos + display_size == lower-right corner of the orthogonal matrix to use)
    pub display_size: [f32; 2],
    /// Amount of pixels for each unit of display_size
    ///
    /// Based on io.display_frame_buffer_scale. Typically [1.0, 1.0] on normal displays, and
    /// [2.0, 2.0] on Retina displays, but fractional values are also possible.
    pub framebuffer_scale: [f32; 2],
    /// Viewport carrying the DrawData instance, might be of use to the renderer (generally not)
    owner_viewport: *mut sys::ImGuiViewport,
}

unsafe impl RawCast<sys::ImDrawData> for DrawData {}

impl RawWrapper for DrawData {
    type Raw = sys::ImDrawData;

    unsafe fn raw(&self) -> &Self::Raw {
        &*(self as *const _ as *const Self::Raw)
    }

    unsafe fn raw_mut(&mut self) -> &mut Self::Raw {
        &mut *(self as *mut _ as *mut Self::Raw)
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

    /// Returns an iterator over the draw lists included in the draw data
    #[inline]
    pub fn draw_lists(&self) -> DrawListIterator<'_> {
        unsafe {
            DrawListIterator {
                iter: self.cmd_lists_slice().iter(),
            }
        }
    }

    /// Returns the number of draw lists included in the draw data
    #[inline]
    pub fn draw_lists_count(&self) -> usize {
        self.cmd_lists_count.max(0) as usize
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

    /// Get command lists as slice
    #[inline]
    unsafe fn cmd_lists_slice(&self) -> &[*const DrawList] {
        if self.cmd_lists_count <= 0 || self.cmd_lists.is_null() {
            return &[];
        }
        slice::from_raw_parts(
            self.cmd_lists as *const *const DrawList,
            self.cmd_lists_count as usize,
        )
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
            sys::ImDrawData_ScaleClipRects(RawWrapper::raw_mut(self), &scale);
        }
    }
}

/// Iterator over draw lists
pub struct DrawListIterator<'a> {
    iter: slice::Iter<'a, *const DrawList>,
}

impl<'a> Iterator for DrawListIterator<'a> {
    type Item = &'a DrawList;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|&ptr| unsafe { &*ptr })
    }
}

impl<'a> ExactSizeIterator for DrawListIterator<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

/// A single draw list (generally one per window + extra overlay draw lists)
///
/// This contains the ImGui-generated geometry for a single draw list.
/// All positions are in screen coordinates (0,0=top-left, 1 pixel per unit).
/// Primitives are always added in 2D (we don't do 3D clipping).
#[repr(transparent)]
pub struct DrawList(*const sys::ImDrawList);

impl DrawList {
    /// Create DrawList from raw pointer
    ///
    /// # Safety
    /// The pointer must be valid and point to a valid ImDrawList
    #[inline]
    pub unsafe fn from_raw(ptr: *const sys::ImDrawList) -> &'static Self {
        &*(ptr as *const Self)
    }

    /// Get the raw pointer to the underlying ImDrawList
    #[inline]
    pub fn as_raw(&self) -> *const sys::ImDrawList {
        self.0
    }

    /// Returns an iterator over the draw commands in this draw list
    pub fn commands(&self) -> DrawCmdIterator<'_> {
        unsafe {
            let cmd_buffer = &(*self.0).CmdBuffer;
            DrawCmdIterator {
                iter: slice::from_raw_parts(cmd_buffer.Data, cmd_buffer.Size as usize).iter(),
            }
        }
    }

    /// Get vertex buffer as slice
    pub fn vtx_buffer(&self) -> &[DrawVert] {
        unsafe {
            let vtx_buffer = &(*self.0).VtxBuffer;
            slice::from_raw_parts(
                vtx_buffer.Data as *const DrawVert,
                vtx_buffer.Size as usize,
            )
        }
    }

    /// Get index buffer as slice
    pub fn idx_buffer(&self) -> &[DrawIdx] {
        unsafe {
            let idx_buffer = &(*self.0).IdxBuffer;
            slice::from_raw_parts(idx_buffer.Data, idx_buffer.Size as usize)
        }
    }
}

/// Iterator over draw commands
pub struct DrawCmdIterator<'a> {
    iter: slice::Iter<'a, sys::ImDrawCmd>,
}

impl<'a> Iterator for DrawCmdIterator<'a> {
    type Item = DrawCmd<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(DrawCmd::from_raw)
    }
}

/// A single draw command within a draw list
///
/// Generally corresponds to 1 GPU draw call, unless it's a callback.
pub struct DrawCmd<'a> {
    raw: &'a sys::ImDrawCmd,
}

impl<'a> DrawCmd<'a> {
    #[inline]
    fn from_raw(raw: &'a sys::ImDrawCmd) -> Self {
        Self { raw }
    }

    /// Get the texture ID for this draw command
    #[inline]
    pub fn texture_id(&self) -> TextureId {
        TextureId::from(self.raw.TexRef._TexID as usize)
    }

    /// Get the clipping rectangle (x1, y1, x2, y2)
    #[inline]
    pub fn clip_rect(&self) -> [f32; 4] {
        [
            self.raw.ClipRect.x,
            self.raw.ClipRect.y,
            self.raw.ClipRect.z,
            self.raw.ClipRect.w,
        ]
    }

    /// Get the number of indices for this draw command
    #[inline]
    pub fn elem_count(&self) -> u32 {
        self.raw.ElemCount
    }

    /// Get the vertex offset for this draw command
    #[inline]
    pub fn vtx_offset(&self) -> u32 {
        self.raw.VtxOffset
    }

    /// Get the index offset for this draw command
    #[inline]
    pub fn idx_offset(&self) -> u32 {
        self.raw.IdxOffset
    }

    /// Check if this is a user callback command
    #[inline]
    pub fn is_user_callback(&self) -> bool {
        self.raw.UserCallback.is_some()
    }
}

/// Vertex format used by Dear ImGui
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DrawVert {
    /// Position (2D)
    pub pos: [f32; 2],
    /// UV coordinates
    pub uv: [f32; 2],
    /// Color (packed RGBA)
    pub col: u32,
}

/// Index type used by Dear ImGui
pub type DrawIdx = u16;
