use super::callbacks::{StandardDrawCallback, classify_standard_draw_callback};
use crate::sys;
use crate::texture::TextureId;
use std::slice;

/// Iterator over draw commands
pub struct DrawCmdIterator<'a> {
    iter: slice::Iter<'a, sys::ImDrawCmd>,
}

impl<'a> DrawCmdIterator<'a> {
    pub(super) fn new(iter: slice::Iter<'a, sys::ImDrawCmd>) -> Self {
        Self { iter }
    }
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

            match classify_standard_draw_callback(cmd.UserCallback) {
                Some(StandardDrawCallback::ResetRenderState) => DrawCmd::ResetRenderState,
                Some(StandardDrawCallback::SetSamplerLinear) => DrawCmd::SetSamplerLinear,
                Some(StandardDrawCallback::SetSamplerNearest) => DrawCmd::SetSamplerNearest,
                None => match cmd.UserCallback {
                    Some(raw_callback) => DrawCmd::RawCallback {
                        callback: raw_callback,
                        raw_cmd: cmd,
                    },
                    None => DrawCmd::Elements {
                        count: cmd.ElemCount as usize,
                        cmd_params,
                        raw_cmd: cmd as *const sys::ImDrawCmd,
                    },
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
    /// Switch texture sampling to linear/filtering.
    SetSamplerLinear,
    /// Switch texture sampling to nearest/point.
    SetSamplerNearest,
    /// Raw callback
    RawCallback {
        callback: unsafe extern "C" fn(*const sys::ImDrawList, cmd: *const sys::ImDrawCmd),
        raw_cmd: *const sys::ImDrawCmd,
    },
}
