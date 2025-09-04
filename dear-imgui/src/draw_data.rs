//! High-level wrapper for Dear ImGui draw data

use dear_imgui_sys as sys;
use std::slice;

/// High-level wrapper for Dear ImGui draw data
///
/// This provides a safe, Rust-friendly interface to Dear ImGui's rendering data.
#[derive(Debug)]
pub struct DrawData<'a> {
    raw: &'a sys::ImDrawData,
}

impl<'a> DrawData<'a> {
    /// Create a new DrawData wrapper from raw ImDrawData
    ///
    /// # Safety
    ///
    /// The caller must ensure that the raw pointer is valid for the lifetime 'a
    pub unsafe fn from_raw(raw: &'a sys::ImDrawData) -> Self {
        Self { raw }
    }

    /// Get the raw ImDrawData pointer
    pub fn raw(&self) -> &sys::ImDrawData {
        self.raw
    }

    /// Check if the draw data is valid for rendering
    pub fn valid(&self) -> bool {
        self.raw.Valid && self.raw.CmdListsCount > 0 && self.raw.TotalVtxCount > 0
    }

    /// Get the display position
    pub fn display_pos(&self) -> [f32; 2] {
        [self.raw.DisplayPos.x, self.raw.DisplayPos.y]
    }

    /// Get the display size
    pub fn display_size(&self) -> [f32; 2] {
        [self.raw.DisplaySize.x, self.raw.DisplaySize.y]
    }

    /// Get the framebuffer scale
    pub fn framebuffer_scale(&self) -> [f32; 2] {
        [self.raw.FramebufferScale.x, self.raw.FramebufferScale.y]
    }

    /// Get the total vertex count
    pub fn total_vtx_count(&self) -> i32 {
        self.raw.TotalVtxCount
    }

    /// Get the total index count
    pub fn total_idx_count(&self) -> i32 {
        self.raw.TotalIdxCount
    }

    /// Get an iterator over the draw lists
    pub fn draw_lists(&self) -> DrawListIterator {
        unsafe {
            let cmd_lists = slice::from_raw_parts(
                self.raw.CmdLists.Data as *const *const sys::ImDrawList,
                self.raw.CmdLists.Size as usize,
            );
            DrawListIterator {
                cmd_lists,
                index: 0,
            }
        }
    }
}

/// Iterator over draw lists
pub struct DrawListIterator<'a> {
    cmd_lists: &'a [*const sys::ImDrawList],
    index: usize,
}

impl<'a> Iterator for DrawListIterator<'a> {
    type Item = DrawList<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.cmd_lists.len() {
            let draw_list = unsafe { DrawList::from_raw(&*self.cmd_lists[self.index]) };
            self.index += 1;
            Some(draw_list)
        } else {
            None
        }
    }
}

/// High-level wrapper for Dear ImGui draw list
#[derive(Debug)]
pub struct DrawList<'a> {
    raw: &'a sys::ImDrawList,
}

impl<'a> DrawList<'a> {
    /// Create a new DrawList wrapper from raw ImDrawList
    ///
    /// # Safety
    ///
    /// The caller must ensure that the raw pointer is valid for the lifetime 'a
    pub unsafe fn from_raw(raw: &'a sys::ImDrawList) -> Self {
        Self { raw }
    }

    /// Get the raw ImDrawList pointer
    pub fn raw(&self) -> &sys::ImDrawList {
        self.raw
    }

    /// Get the vertex buffer
    pub fn vtx_buffer(&self) -> &[sys::ImDrawVert] {
        unsafe { slice::from_raw_parts(self.raw.VtxBuffer.Data, self.raw.VtxBuffer.Size as usize) }
    }

    /// Get the index buffer
    pub fn idx_buffer(&self) -> &[sys::ImDrawIdx] {
        unsafe { slice::from_raw_parts(self.raw.IdxBuffer.Data, self.raw.IdxBuffer.Size as usize) }
    }

    /// Get an iterator over the draw commands
    pub fn commands(&self) -> DrawCmdIterator {
        unsafe {
            let cmd_buffer =
                slice::from_raw_parts(self.raw.CmdBuffer.Data, self.raw.CmdBuffer.Size as usize);
            DrawCmdIterator {
                cmd_buffer,
                index: 0,
            }
        }
    }
}

/// Iterator over draw commands
pub struct DrawCmdIterator<'a> {
    cmd_buffer: &'a [sys::ImDrawCmd],
    index: usize,
}

impl<'a> Iterator for DrawCmdIterator<'a> {
    type Item = &'a sys::ImDrawCmd;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.cmd_buffer.len() {
            let cmd = &self.cmd_buffer[self.index];
            self.index += 1;
            Some(cmd)
        } else {
            None
        }
    }
}

/// High-level wrapper for Dear ImGui draw command
#[derive(Debug)]
pub struct DrawCmd<'a> {
    raw: &'a sys::ImDrawCmd,
}

impl<'a> DrawCmd<'a> {
    /// Create a new DrawCmd wrapper from raw ImDrawCmd
    pub fn new(raw: &'a sys::ImDrawCmd) -> Self {
        Self { raw }
    }

    /// Get the raw ImDrawCmd pointer
    pub fn raw(&self) -> &sys::ImDrawCmd {
        self.raw
    }

    /// Get the element count
    pub fn elem_count(&self) -> u32 {
        self.raw.ElemCount
    }

    /// Get the clip rectangle
    pub fn clip_rect(&self) -> [f32; 4] {
        [
            self.raw.ClipRect.x,
            self.raw.ClipRect.y,
            self.raw.ClipRect.z,
            self.raw.ClipRect.w,
        ]
    }

    /// Get the texture ID
    pub fn texture_id(&self) -> sys::ImTextureRef {
        self.raw.TexRef
    }

    /// Get the vertex offset
    pub fn vtx_offset(&self) -> u32 {
        self.raw.VtxOffset
    }

    /// Get the index offset
    pub fn idx_offset(&self) -> u32 {
        self.raw.IdxOffset
    }

    /// Check if this command has a user callback
    pub fn has_user_callback(&self) -> bool {
        self.raw.UserCallback.is_some()
    }
}
