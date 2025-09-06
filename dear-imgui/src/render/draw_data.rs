//! Draw data structures for Dear ImGui rendering
//!
//! This module provides safe Rust wrappers around Dear ImGui's draw data structures,
//! which contain all the information needed to render a frame.

use crate::internal::{RawCast, RawWrapper};
use crate::render::renderer::TextureId;
use crate::sys;
use std::slice;

/// All draw data to render a Dear ImGui frame.
#[repr(C)]
pub struct DrawData {
    /// Only valid after render() is called and before the next new frame() is called.
    valid: bool,
    /// Number of DrawList to render.
    cmd_lists_count: i32,
    /// For convenience, sum of all draw list index buffer sizes.
    pub total_idx_count: i32,
    /// For convenience, sum of all draw list vertex buffer sizes.
    pub total_vtx_count: i32,
    // Array of DrawList.
    cmd_lists: crate::internal::ImVector<DrawList>,
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
    owner_viewport: *mut sys::ImGuiViewport,
    /// Texture data (internal use)
    textures: *mut crate::internal::ImVector<*mut sys::ImTextureData>,
}

unsafe impl RawCast<sys::ImDrawData> for DrawData {}

impl RawWrapper for DrawData {
    type Raw = sys::ImDrawData;

    unsafe fn raw(&self) -> &Self::Raw {
        std::mem::transmute(self)
    }

    unsafe fn raw_mut(&mut self) -> &mut Self::Raw {
        std::mem::transmute(self)
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
        unsafe {
            DrawListIterator {
                iter: self.cmd_lists().iter(),
            }
        }
    }
    /// Returns the number of draw lists included in the draw data.
    #[inline]
    pub fn draw_lists_count(&self) -> usize {
        self.cmd_lists_count.try_into().unwrap()
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

    #[inline]
    pub(crate) unsafe fn cmd_lists(&self) -> &[*const DrawList] {
        if self.cmd_lists_count <= 0 || self.cmd_lists.data.is_null() {
            return &[];
        }
        slice::from_raw_parts(
            self.cmd_lists.data as *const *const DrawList,
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
    iter: std::slice::Iter<'a, *const DrawList>,
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

/// Draw command list
#[repr(transparent)]
pub struct DrawList(sys::ImDrawList);

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
    pub(crate) unsafe fn cmd_buffer(&self) -> &[sys::ImDrawCmd] {
        let cmd_buffer = &self.0.CmdBuffer;
        if cmd_buffer.Size <= 0 || cmd_buffer.Data.is_null() {
            return &[];
        }
        slice::from_raw_parts(cmd_buffer.Data, cmd_buffer.Size as usize)
    }

    /// Returns an iterator over the draw commands in this draw list
    pub fn commands(&self) -> DrawCmdIterator<'_> {
        unsafe {
            DrawCmdIterator {
                iter: self.cmd_buffer().iter(),
            }
        }
    }

    /// Get vertex buffer as slice
    pub fn vtx_buffer(&self) -> &[DrawVert] {
        unsafe {
            let vtx_buffer = &self.0.VtxBuffer;
            slice::from_raw_parts(vtx_buffer.Data as *const DrawVert, vtx_buffer.Size as usize)
        }
    }

    /// Get index buffer as slice
    pub fn idx_buffer(&self) -> &[DrawIdx] {
        unsafe {
            let idx_buffer = &self.0.IdxBuffer;
            slice::from_raw_parts(idx_buffer.Data, idx_buffer.Size as usize)
        }
    }
}

/// Iterator over draw commands
pub struct DrawCmdIterator<'a> {
    iter: slice::Iter<'a, sys::ImDrawCmd>,
}

impl<'a> Iterator for DrawCmdIterator<'a> {
    type Item = DrawCmd;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|cmd| {
            let cmd_params = DrawCmdParams {
                clip_rect: [
                    cmd.ClipRect.x,
                    cmd.ClipRect.y,
                    cmd.ClipRect.z,
                    cmd.ClipRect.w,
                ],
                texture_id: TextureId::from(cmd.TexRef._TexID as usize),
                vtx_offset: cmd.VtxOffset as usize,
                idx_offset: cmd.IdxOffset as usize,
            };

            // Check for special callback values
            match cmd.UserCallback {
                Some(raw_callback) if raw_callback as usize == (-1isize) as usize => {
                    DrawCmd::ResetRenderState
                }
                Some(raw_callback) => DrawCmd::RawCallback {
                    callback: raw_callback,
                    raw_cmd: cmd,
                },
                None => DrawCmd::Elements {
                    count: cmd.ElemCount as usize,
                    cmd_params,
                },
            }
        })
    }
}

/// Parameters for a draw command
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DrawCmdParams {
    /// Clipping rectangle [left, top, right, bottom]
    pub clip_rect: [f32; 4],
    /// Texture ID to use for rendering
    pub texture_id: TextureId,
    /// Vertex buffer offset
    pub vtx_offset: usize,
    /// Index buffer offset
    pub idx_offset: usize,
}

/// A draw command
#[derive(Clone, Debug)]
pub enum DrawCmd {
    /// Elements to draw
    Elements {
        /// The number of indices used for this draw command
        count: usize,
        cmd_params: DrawCmdParams,
    },
    /// Reset render state
    ResetRenderState,
    /// Raw callback
    RawCallback {
        callback: unsafe extern "C" fn(*const sys::ImDrawList, cmd: *const sys::ImDrawCmd),
        raw_cmd: *const sys::ImDrawCmd,
    },
}

/// Vertex format used by Dear ImGui
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
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
