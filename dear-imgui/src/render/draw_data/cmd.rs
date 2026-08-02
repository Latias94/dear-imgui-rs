use super::callbacks::{StandardDrawCallback, classify_standard_draw_callback};
use crate::draw::RawDrawCallback;
use crate::sys;
use crate::texture::{TextureId, effective_texture_id};
use std::fmt;
use std::slice;

/// Iterator over draw commands
pub struct DrawCmdIterator<'a> {
    draw_list: &'a sys::ImDrawList,
    iter: slice::Iter<'a, sys::ImDrawCmd>,
}

impl<'a> DrawCmdIterator<'a> {
    pub(super) fn new(
        draw_list: &'a sys::ImDrawList,
        iter: slice::Iter<'a, sys::ImDrawCmd>,
    ) -> Self {
        Self { draw_list, iter }
    }
}

impl<'a> Iterator for DrawCmdIterator<'a> {
    type Item = DrawCmd<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(
            |cmd| match classify_standard_draw_callback(cmd.UserCallback) {
                Some(StandardDrawCallback::ResetRenderState) => DrawCmd::ResetRenderState,
                Some(StandardDrawCallback::SetSamplerLinear) => DrawCmd::SetSamplerLinear,
                Some(StandardDrawCallback::SetSamplerNearest) => DrawCmd::SetSamplerNearest,
                None => match cmd.UserCallback {
                    Some(callback) => DrawCmd::RawCallback(RawCallbackCommand {
                        callback,
                        draw_list: self.draw_list,
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

#[cfg(test)]
mod tests {
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

        let draw_list = sys::ImDrawList::default();
        let draw_command = DrawCmdIterator::new(&draw_list, commands.iter())
            .next()
            .unwrap();
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

        let draw_list = sys::ImDrawList::default();
        let draw_command = DrawCmdIterator::new(&draw_list, commands.iter())
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
        let DrawCmd::RawCallback(raw) = DrawCmdIterator::new(&draw_list, commands.iter())
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
