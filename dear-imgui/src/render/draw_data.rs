//! Draw data structures for Dear ImGui rendering
//!
//! This module provides safe Rust wrappers around Dear ImGui's draw data structures,
//! which contain all the information needed to render a frame.

use std::fmt;
use std::marker::PhantomData;
use std::slice;

use crate::context::binding::rendered_draw_callbacks;
use crate::draw::RawDrawCallback;
use crate::internal::{RawCast, RawWrapper};
use crate::sys;
use crate::texture::{TextureData, TextureId, effective_texture_id};

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

fn total_count_from_i32(caller: &str, raw: i32) -> usize {
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
            unsafe { draw_list.cmd_buffer() }.iter().any(|cmd| {
                cmd.UserCallback.is_some()
                    && classify_standard_draw_callback(draw_list, cmd.UserCallback).is_none()
            })
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

/// Iterator over draw lists.
///
/// A null list pointer in malformed native draw data panics when reached.
pub struct DrawListIterator<'a> {
    iter: std::slice::Iter<'a, *mut sys::ImDrawList>,
}

impl<'a> Iterator for DrawListIterator<'a> {
    type Item = &'a DrawList;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|&ptr| {
            assert!(!ptr.is_null(), "native draw list pointer must not be null");
            unsafe { DrawList::from_raw(ptr.cast_const()) }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl ExactSizeIterator for DrawListIterator<'_> {}
impl std::iter::FusedIterator for DrawListIterator<'_> {}

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
        unsafe { DrawCmdIterator::new(self, self.cmd_buffer().iter()) }
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

/// Iterator over draw commands
pub struct DrawCmdIterator<'a> {
    draw_list: &'a DrawList,
    iter: slice::Iter<'a, sys::ImDrawCmd>,
}

impl<'a> DrawCmdIterator<'a> {
    pub(super) fn new(draw_list: &'a DrawList, iter: slice::Iter<'a, sys::ImDrawCmd>) -> Self {
        Self { draw_list, iter }
    }
}

impl<'a> Iterator for DrawCmdIterator<'a> {
    type Item = DrawCmd<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|cmd| {
            match classify_standard_draw_callback(self.draw_list, cmd.UserCallback) {
                Some(StandardDrawCallback::ResetRenderState) => DrawCmd::ResetRenderState,
                Some(StandardDrawCallback::SetSamplerLinear) => DrawCmd::SetSamplerLinear,
                Some(StandardDrawCallback::SetSamplerNearest) => DrawCmd::SetSamplerNearest,
                Some(StandardDrawCallback::SetSamplerFromTexture) => DrawCmd::SetSamplerFromTexture,
                None => match cmd.UserCallback {
                    Some(callback) => DrawCmd::RawCallback(RawCallbackCommand {
                        callback,
                        draw_list: unsafe { self.draw_list.raw() },
                        command: cmd,
                    }),
                    None => {
                        let cmd_params = DrawCmdParams {
                            clip_rect: [
                                cmd.ClipRect.x,
                                cmd.ClipRect.y,
                                cmd.ClipRect.z,
                                cmd.ClipRect.w,
                            ],
                            texture_id: unsafe { effective_texture_id(&cmd.TexRef) },
                            vtx_offset: cmd.VtxOffset as usize,
                            idx_offset: cmd.IdxOffset as usize,
                        };
                        DrawCmd::Elements {
                            count: cmd.ElemCount as usize,
                            cmd_params,
                        }
                    }
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
    /// This is the effective ID resolved from either a legacy `TextureId` or a
    /// managed `ImTextureData` reference when the command is iterated.
    pub texture_id: TextureId,
    /// Vertex buffer offset
    pub vtx_offset: usize,
    /// Index buffer offset
    pub idx_offset: usize,
}

/// A draw command
#[derive(Debug)]
pub enum DrawCmd<'draw> {
    /// Elements to draw
    Elements {
        /// The number of indices used for this draw command
        count: usize,
        cmd_params: DrawCmdParams,
    },
    /// Reset render state
    ResetRenderState,
    /// Switch texture sampling to linear/filtering.
    SetSamplerLinear,
    /// Switch texture sampling to nearest/point.
    SetSamplerNearest,
    /// Use the sampler settings associated with each texture.
    SetSamplerFromTexture,
    /// Raw callback
    RawCallback(RawCallbackCommand<'draw>),
}

/// One borrowed, non-standard native draw callback at its exact command position.
///
/// This value is intentionally neither `Clone` nor detachable from the source draw list. Renderer
/// backends may either reject it before draw side effects or consume it exactly once through
/// [`Self::invoke`] while their documented transient render state is installed.
#[must_use = "a renderer must invoke or explicitly reject a raw callback command"]
pub struct RawCallbackCommand<'draw> {
    callback: RawDrawCallback,
    draw_list: &'draw sys::ImDrawList,
    command: &'draw sys::ImDrawCmd,
}

impl fmt::Debug for RawCallbackCommand<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawCallbackCommand")
            .field("callback", &(self.callback as usize))
            .field("draw_list", &(self.draw_list as *const sys::ImDrawList))
            .field("command", &(self.command as *const sys::ImDrawCmd))
            .finish()
    }
}

impl RawCallbackCommand<'_> {
    /// Invokes the callback once with the exact parent draw list and native command.
    ///
    /// # Safety
    ///
    /// The caller must support the callback's native ABI and install any transient renderer state
    /// promised by its backend before invoking it. The registered callback must satisfy the
    /// contract of [`crate::DrawListMut::add_callback`], including never unwinding or throwing
    /// across the C ABI boundary. Renderer state modified by the callback remains modified until a
    /// later explicit reset command; backends must not invent an implicit reset.
    pub unsafe fn invoke(self) {
        unsafe {
            (self.callback)(
                self.draw_list as *const sys::ImDrawList,
                self.command as *const sys::ImDrawCmd,
            );
        }
    }
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

// Ensure our Rust-side vertex/index types stay layout-compatible with the raw sys bindings.
const _: [(); std::mem::size_of::<sys::ImDrawVert>()] = [(); std::mem::size_of::<DrawVert>()];
const _: [(); std::mem::align_of::<sys::ImDrawVert>()] = [(); std::mem::align_of::<DrawVert>()];

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

const _: [(); std::mem::size_of::<sys::ImDrawIdx>()] = [(); std::mem::size_of::<DrawIdx>()];
const _: [(); std::mem::align_of::<sys::ImDrawIdx>()] = [(); std::mem::align_of::<DrawIdx>()];

const LEGACY_RESET_RENDER_STATE_CALLBACK_VALUE: usize = usize::MAX;
const RESET_RENDER_STATE_CALLBACK_VALUE: usize = usize::MAX - 7;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum StandardDrawCallback {
    ResetRenderState,
    SetSamplerLinear,
    SetSamplerNearest,
    SetSamplerFromTexture,
}

#[derive(Copy, Clone)]
pub(crate) struct StandardDrawCallbacks {
    reset: sys::ImDrawCallback,
    linear: sys::ImDrawCallback,
    nearest: sys::ImDrawCallback,
    from_texture: sys::ImDrawCallback,
}

impl From<&sys::ImGuiPlatformIO> for StandardDrawCallbacks {
    fn from(io: &sys::ImGuiPlatformIO) -> Self {
        Self {
            reset: io.DrawCallback_ResetRenderState,
            linear: io.DrawCallback_SetSamplerLinear,
            nearest: io.DrawCallback_SetSamplerNearest,
            from_texture: io.DrawCallback_SetSamplerFromTex,
        }
    }
}

#[inline]
fn callback_matches(callback: RawDrawCallback, standard: sys::ImDrawCallback) -> bool {
    standard.is_some_and(|standard| standard as usize == callback as usize)
}

#[inline]
pub(crate) fn classify_standard_draw_callback(
    draw_list: &DrawList,
    callback: sys::ImDrawCallback,
) -> Option<StandardDrawCallback> {
    let callback = callback?;
    let callback_value = callback as usize;

    if callback_value == RESET_RENDER_STATE_CALLBACK_VALUE
        || callback_value == LEGACY_RESET_RENDER_STATE_CALLBACK_VALUE
    {
        return Some(StandardDrawCallback::ResetRenderState);
    }

    // A reconciled draw list can be read while a different Context is bound. Dear ImGui
    // sets this back-pointer when it initializes the list owner's shared draw data.
    let context = unsafe { draw_list.raw()._Data.as_ref()?.Context };
    if context.is_null() {
        return None;
    }

    // Managed draw data is valid through the end of its rendered frame. A raw context or
    // a list inspected before Render still uses its owner's current registrations.
    let frame = unsafe { (*context).FrameCount };
    let callbacks = rendered_draw_callbacks(context, frame)
        .unwrap_or_else(|| StandardDrawCallbacks::from(unsafe { &(*context).PlatformIO }));
    if callback_matches(callback, callbacks.reset) {
        Some(StandardDrawCallback::ResetRenderState)
    } else if callback_matches(callback, callbacks.linear) {
        Some(StandardDrawCallback::SetSamplerLinear)
    } else if callback_matches(callback, callbacks.nearest) {
        Some(StandardDrawCallback::SetSamplerNearest)
    } else if callback_matches(callback, callbacks.from_texture) {
        Some(StandardDrawCallback::SetSamplerFromTexture)
    } else {
        None
    }
}

impl DrawData {
    /// Returns a shared iterator over textures attached to this draw data.
    ///
    /// This crate-internal view is used only while collecting Context-owned renderer requests.
    pub(crate) fn textures(&self) -> TextureIterator<'_> {
        unsafe {
            if self.0.Textures.is_null() {
                TextureIterator::new(std::ptr::null(), std::ptr::null())
            } else {
                let vector = &*self.0.Textures;
                if vector.Size <= 0 || vector.Data.is_null() {
                    TextureIterator::new(std::ptr::null(), std::ptr::null())
                } else {
                    TextureIterator::new(vector.Data, vector.Data.add(vector.Size as usize))
                }
            }
        }
    }
}

/// Iterator over textures in draw data
pub(crate) struct TextureIterator<'a> {
    ptr: *const *mut sys::ImTextureData,
    end: *const *mut sys::ImTextureData,
    _phantom: PhantomData<&'a TextureData>,
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
            _phantom: PhantomData,
        }
    }
}

impl<'a> Iterator for TextureIterator<'a> {
    type Item = &'a TextureData;

    fn next(&mut self) -> Option<Self::Item> {
        while self.ptr < self.end {
            let texture_ptr = unsafe { *self.ptr };
            self.ptr = unsafe { self.ptr.add(1) };
            if texture_ptr.is_null() {
                continue;
            }

            return Some(unsafe { TextureData::from_raw_ref(texture_ptr as *const _) });
        }

        None
    }
}

impl<'a> std::iter::FusedIterator for TextureIterator<'a> {}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cmd_tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        static OBSERVED_CALLBACK: Cell<(usize, usize, usize)> = const { Cell::new((0, 0, 0)) };
    }

    unsafe extern "C" fn observe_raw_callback(
        draw_list: *const sys::ImDrawList,
        command: *const sys::ImDrawCmd,
    ) {
        let user_data = unsafe { (*command).UserCallbackData as usize };
        OBSERVED_CALLBACK.set((draw_list as usize, command as usize, user_data));
    }

    #[test]
    fn draw_command_params_resolve_managed_texture_ids() {
        let mut texture = crate::texture::OwnedTextureData::empty();
        let texture_id = TextureId::new(17);
        unsafe {
            // The test-owned texture and synthetic command remain alive for the assertion below.
            texture.set_tex_id(texture_id);
        }

        let command = sys::ImDrawCmd {
            TexRef: sys::ImTextureRef {
                _TexData: texture.as_raw_mut(),
                _TexID: 0,
            },
            ElemCount: 3,
            ..Default::default()
        };
        let commands = [command];

        let draw_list = sys::ImDrawList::default();
        let draw_command =
            DrawCmdIterator::new(unsafe { DrawList::from_raw(&draw_list) }, commands.iter())
                .next()
                .unwrap();
        let DrawCmd::Elements { cmd_params, .. } = draw_command else {
            panic!("expected an element draw command");
        };
        assert_eq!(cmd_params.texture_id, texture_id);
    }

    #[test]
    fn draw_command_iteration_allows_pending_managed_textures() {
        let mut texture = crate::texture::OwnedTextureData::empty();
        assert!(texture.tex_id().is_null());

        let command = sys::ImDrawCmd {
            TexRef: sys::ImTextureRef {
                _TexData: texture.as_raw_mut(),
                _TexID: 0,
            },
            ElemCount: 3,
            ..Default::default()
        };
        let commands = [command];

        let draw_list = sys::ImDrawList::default();
        let draw_command =
            DrawCmdIterator::new(unsafe { DrawList::from_raw(&draw_list) }, commands.iter())
                .next()
                .unwrap();
        let DrawCmd::Elements { cmd_params, .. } = draw_command else {
            panic!("expected an element draw command");
        };
        assert!(cmd_params.texture_id.is_null());
    }

    #[test]
    fn raw_callback_command_invokes_once_with_exact_parent_and_command() {
        OBSERVED_CALLBACK.set((0, 0, 0));
        let draw_list = sys::ImDrawList::default();
        let mut command = sys::ImDrawCmd {
            UserCallback: Some(observe_raw_callback),
            UserCallbackData: 0x1234usize as *mut _,
            ..Default::default()
        };
        let command_address = &raw const command as usize;
        let commands = std::slice::from_mut(&mut command);
        let DrawCmd::RawCallback(raw) =
            DrawCmdIterator::new(unsafe { DrawList::from_raw(&draw_list) }, commands.iter())
                .next()
                .expect("raw callback command")
        else {
            panic!("expected raw callback command");
        };

        unsafe { raw.invoke() };

        assert_eq!(
            OBSERVED_CALLBACK.get(),
            (&raw const draw_list as usize, command_address, 0x1234)
        );
    }
}
