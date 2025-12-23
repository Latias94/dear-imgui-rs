//! Draw data structures for Dear ImGui rendering
//!
//! This module provides safe Rust wrappers around Dear ImGui's draw data structures,
//! which contain all the information needed to render a frame.

use crate::internal::{RawCast, RawWrapper};
use crate::sys;
use crate::texture::TextureId;
use std::marker::PhantomData;
use std::rc::Rc;
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

    /// Returns an iterator over the textures that need to be updated
    ///
    /// This is used by renderer backends to process texture creation, updates, and destruction.
    /// Each item is an `ImTextureData*` carrying a `Status` which can be one of:
    /// - `OK`: nothing to do.
    /// - `WantCreate`: create a GPU texture and upload all pixels.
    /// - `WantUpdates`: upload specified `UpdateRect` regions.
    /// - `WantDestroy`: destroy the GPU texture (may be delayed until unused).
    /// Most of the time this list has only 1 texture and it doesn't need any update.
    pub fn textures(&self) -> TextureIterator<'_> {
        unsafe {
            if self.textures.is_null() {
                TextureIterator::new(std::ptr::null(), std::ptr::null())
            } else {
                let vector = &*self.textures;
                if vector.size <= 0 || vector.data.is_null() {
                    TextureIterator::new(std::ptr::null(), std::ptr::null())
                } else {
                    TextureIterator::new(vector.data, vector.data.add(vector.size as usize))
                }
            }
        }
    }

    /// Returns the number of textures in the texture list
    pub fn textures_count(&self) -> usize {
        unsafe {
            if self.textures.is_null() {
                0
            } else {
                let vector = &*self.textures;
                if vector.size <= 0 || vector.data.is_null() {
                    0
                } else {
                    vector.size as usize
                }
            }
        }
    }

    /// Get a specific texture by index
    ///
    /// Returns None if the index is out of bounds or no textures are available.
    pub fn texture(&self, index: usize) -> Option<&crate::texture::TextureData> {
        unsafe {
            if self.textures.is_null() {
                return None;
            }
            let vector = &*self.textures;
            if vector.data.is_null() {
                return None;
            }
            if index >= vector.size as usize {
                return None;
            }
            let texture_ptr = *vector.data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw(texture_ptr))
        }
    }

    /// Get a mutable reference to a specific texture by index
    ///
    /// Returns None if the index is out of bounds or no textures are available.
    pub fn texture_mut(&mut self, index: usize) -> Option<&mut crate::texture::TextureData> {
        unsafe {
            if self.textures.is_null() {
                return None;
            }
            let vector = &*self.textures;
            if vector.data.is_null() {
                return None;
            }
            if index >= vector.size as usize {
                return None;
            }
            let texture_ptr = *vector.data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(crate::texture::TextureData::from_raw(texture_ptr))
        }
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
        unsafe {
            if self.cmd_lists_count <= 0 || self.cmd_lists.data.is_null() {
                return &[];
            }
            slice::from_raw_parts(
                self.cmd_lists.data as *const *const DrawList,
                self.cmd_lists_count as usize,
            )
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
        unsafe {
            let cmd_buffer = &self.0.CmdBuffer;
            if cmd_buffer.Size <= 0 || cmd_buffer.Data.is_null() {
                return &[];
            }
            slice::from_raw_parts(cmd_buffer.Data, cmd_buffer.Size as usize)
        }
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
            if vtx_buffer.Size <= 0 || vtx_buffer.Data.is_null() {
                return &[];
            }
            slice::from_raw_parts(vtx_buffer.Data as *const DrawVert, vtx_buffer.Size as usize)
        }
    }

    /// Get index buffer as slice
    pub fn idx_buffer(&self) -> &[DrawIdx] {
        unsafe {
            let idx_buffer = &self.0.IdxBuffer;
            if idx_buffer.Size <= 0 || idx_buffer.Data.is_null() {
                return &[];
            }
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
                // Use raw field; backends may resolve effective TexID later
                texture_id: TextureId::from(cmd.TexRef._TexID),
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
                    raw_cmd: cmd as *const sys::ImDrawCmd,
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
    ///
    /// Notes:
    /// - For legacy paths (plain `TextureId`), this is the effective id.
    /// - With the modern texture system (ImTextureRef/ImTextureData), this may be 0.
    ///   Renderer backends should resolve the effective id at bind time using
    ///   `ImDrawCmd_GetTexID` with the `raw_cmd` pointer and the backend render state.
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
        /// Raw command pointer for backends
        ///
        /// Backend note: when using the modern texture system, resolve the effective
        /// texture id at bind time via `ImDrawCmd_GetTexID(raw_cmd)` together with your
        /// renderer state. This pointer is only valid during the `render_draw_data()`
        /// call that produced it; do not store it.
        raw_cmd: *const sys::ImDrawCmd,
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

impl DrawVert {
    /// Creates a new draw vertex with u32 color
    pub fn new(pos: [f32; 2], uv: [f32; 2], col: u32) -> Self {
        Self { pos, uv, col }
    }

    /// Creates a new draw vertex from RGBA bytes
    pub fn from_rgba(pos: [f32; 2], uv: [f32; 2], rgba: [u8; 4]) -> Self {
        let col = ((rgba[3] as u32) << 24)
            | ((rgba[2] as u32) << 16)
            | ((rgba[1] as u32) << 8)
            | (rgba[0] as u32);
        Self { pos, uv, col }
    }

    /// Extracts RGBA bytes from the packed color
    pub fn rgba(&self) -> [u8; 4] {
        [
            (self.col & 0xFF) as u8,
            ((self.col >> 8) & 0xFF) as u8,
            ((self.col >> 16) & 0xFF) as u8,
            ((self.col >> 24) & 0xFF) as u8,
        ]
    }
}

/// Index type used by Dear ImGui
pub type DrawIdx = u16;

/// A container for a heap-allocated deep copy of a `DrawData` struct.
///
/// Notes on thread-safety:
/// - This type intentionally does NOT implement `Send`/`Sync` because it currently retains
///   a pointer to the engine-managed textures list (`ImVector<ImTextureData*>`) instead of
///   deep-copying it. That list can be mutated by the UI thread across frames.
/// - You may move vertices/indices to another thread by extracting them into your own buffers
///   or by implementing a custom deep copy which snapshots the textures list as well.
///
/// The underlying copy is released when this struct is dropped.
pub struct OwnedDrawData {
    draw_data: *mut sys::ImDrawData,
    // Prevent Send/Sync: this struct retains a pointer to a shared textures list.
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
            // Allocate a new ImDrawData using the constructor
            let result = sys::ImDrawData_ImDrawData();
            if result.is_null() {
                panic!("Failed to allocate ImDrawData for OwnedDrawData");
            }

            // Copy basic fields from the source
            let source_ptr = RawWrapper::raw(value);
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

            // Textures list is shared, do not duplicate (renderer treats it as read-only)
            (*result).Textures = source_ptr.Textures;

            OwnedDrawData {
                draw_data: result,
                _no_send_sync: PhantomData,
            }
        }
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

/// Iterator over textures in draw data
pub struct TextureIterator<'a> {
    ptr: *const *mut sys::ImTextureData,
    end: *const *mut sys::ImTextureData,
    _phantom: std::marker::PhantomData<&'a crate::texture::TextureData>,
}

impl<'a> TextureIterator<'a> {
    /// Create a new texture iterator from raw pointers
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointers are valid and that the range
    /// [ptr, end) contains valid texture data pointers.
    pub(crate) unsafe fn new(
        ptr: *const *mut sys::ImTextureData,
        end: *const *mut sys::ImTextureData,
    ) -> Self {
        Self {
            ptr,
            end,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a> Iterator for TextureIterator<'a> {
    type Item = &'a mut crate::texture::TextureData;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr >= self.end {
            None
        } else {
            unsafe {
                let texture_ptr = *self.ptr;
                self.ptr = self.ptr.add(1);
                if texture_ptr.is_null() {
                    self.next() // Skip null pointers
                } else {
                    Some(crate::texture::TextureData::from_raw(texture_ptr))
                }
            }
        }
    }
}

impl<'a> std::iter::FusedIterator for TextureIterator<'a> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_data_textures_empty_is_safe() {
        let mut textures_vec: crate::internal::ImVector<*mut sys::ImTextureData> =
            crate::internal::ImVector::default();

        let draw_data = DrawData {
            valid: false,
            cmd_lists_count: 0,
            total_idx_count: 0,
            total_vtx_count: 0,
            cmd_lists: crate::internal::ImVector::default(),
            display_pos: [0.0, 0.0],
            display_size: [0.0, 0.0],
            framebuffer_scale: [1.0, 1.0],
            owner_viewport: std::ptr::null_mut(),
            textures: &mut textures_vec,
        };

        assert_eq!(draw_data.textures().count(), 0);
        assert_eq!(draw_data.textures_count(), 0);

        let mut textures_vec: crate::internal::ImVector<*mut sys::ImTextureData> =
            crate::internal::ImVector::default();
        textures_vec.size = 1;
        textures_vec.data = std::ptr::null_mut();
        let draw_data = DrawData {
            valid: false,
            cmd_lists_count: 0,
            total_idx_count: 0,
            total_vtx_count: 0,
            cmd_lists: crate::internal::ImVector::default(),
            display_pos: [0.0, 0.0],
            display_size: [0.0, 0.0],
            framebuffer_scale: [1.0, 1.0],
            owner_viewport: std::ptr::null_mut(),
            textures: &mut textures_vec,
        };
        assert_eq!(draw_data.textures().count(), 0);
        assert_eq!(draw_data.textures_count(), 0);
        assert!(draw_data.texture(0).is_none());
    }
}
