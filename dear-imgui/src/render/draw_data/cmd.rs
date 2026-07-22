use super::callbacks::{StandardDrawCallback, classify_standard_draw_callback};
use crate::sys;
use crate::texture::{TextureId, effective_texture_id};
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
        self.iter.next().map(
            |cmd| match classify_standard_draw_callback(cmd.UserCallback) {
                Some(StandardDrawCallback::ResetRenderState) => DrawCmd::ResetRenderState,
                Some(StandardDrawCallback::SetSamplerLinear) => DrawCmd::SetSamplerLinear,
                Some(StandardDrawCallback::SetSamplerNearest) => DrawCmd::SetSamplerNearest,
                None => match cmd.UserCallback {
                    Some(raw_callback) => DrawCmd::RawCallback {
                        callback: raw_callback,
                        raw_cmd: cmd,
                    },
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
                            raw_cmd: cmd as *const sys::ImDrawCmd,
                        }
                    }
                },
            },
        )
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
#[derive(Clone, Debug)]
pub enum DrawCmd {
    /// Elements to draw
    Elements {
        /// The number of indices used for this draw command
        count: usize,
        cmd_params: DrawCmdParams,
        /// Raw command pointer for backends
        ///
        /// This pointer is only valid while iterating the source draw list; do not
        /// store it. Texture binding should use [`DrawCmdParams::texture_id`].
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_command_params_resolve_managed_texture_ids() {
        let mut texture = crate::texture::OwnedTextureData::new();
        let texture_id = TextureId::new(17);
        unsafe {
            // The test-owned texture and synthetic command remain alive for the assertion below.
            texture.set_tex_id(texture_id);
        }

        let mut command = sys::ImDrawCmd::default();
        command.TexRef = sys::ImTextureRef {
            _TexData: texture.as_raw_mut(),
            _TexID: 0,
        };
        command.ElemCount = 3;
        let commands = [command];

        let draw_command = DrawCmdIterator::new(commands.iter()).next().unwrap();
        let DrawCmd::Elements { cmd_params, .. } = draw_command else {
            panic!("expected an element draw command");
        };
        assert_eq!(cmd_params.texture_id, texture_id);
    }

    #[test]
    fn draw_command_iteration_allows_pending_managed_textures() {
        let mut texture = crate::texture::OwnedTextureData::new();
        assert!(texture.tex_id().is_null());

        let mut command = sys::ImDrawCmd::default();
        command.TexRef = sys::ImTextureRef {
            _TexData: texture.as_raw_mut(),
            _TexID: 0,
        };
        command.ElemCount = 3;
        let commands = [command];

        let draw_command = DrawCmdIterator::new(commands.iter()).next().unwrap();
        let DrawCmd::Elements { cmd_params, .. } = draw_command else {
            panic!("expected an element draw command");
        };
        assert!(cmd_params.texture_id.is_null());
    }
}
