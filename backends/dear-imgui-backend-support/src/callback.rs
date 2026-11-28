use dear_imgui_rs::{DrawCmd, DrawCmdSnapshot};

/// The core RAII guard for publishing transient `Renderer_RenderState` during a draw.
///
/// The guard holds an exclusive borrow of the published state. Finish the guard to validate
/// the slot after drawing, and use `state_mut()` between raw callback invocations.
pub use dear_imgui_rs::RendererRenderStateGuard as ScopedRenderState;
/// Failure to publish or validate the transient renderer state slot.
pub use dear_imgui_rs::RendererRenderStateGuardError as ScopedRenderStateError;

/// A standard draw callback already recognized by Dear ImGui's draw command iterator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardDrawCallback {
    /// Restore the backend's draw state at this command position.
    ResetRenderState,
    /// Select the backend's linear sampler.
    SetSamplerLinear,
    /// Select the backend's nearest sampler.
    SetSamplerNearest,
    /// Use the bound texture's own sampler, where supported by the backend.
    SetSamplerFromTexture,
}

/// Routes standard callbacks from borrowed and detached draw commands to the same actions.
///
/// Backends apply each action in command order and choose how to handle unsupported samplers.
/// Borrowed raw callbacks still follow the backend's native callback contract.
pub struct DrawCallbackDispatcher;

impl DrawCallbackDispatcher {
    /// Returns the standard callback in a borrowed draw command, if present.
    ///
    /// Element draws and borrowed raw callbacks are handled by the caller.
    #[must_use]
    pub const fn from_draw_cmd(command: &DrawCmd<'_>) -> Option<StandardDrawCallback> {
        match command {
            DrawCmd::ResetRenderState => Some(StandardDrawCallback::ResetRenderState),
            DrawCmd::SetSamplerLinear => Some(StandardDrawCallback::SetSamplerLinear),
            DrawCmd::SetSamplerNearest => Some(StandardDrawCallback::SetSamplerNearest),
            DrawCmd::SetSamplerFromTexture => Some(StandardDrawCallback::SetSamplerFromTexture),
            DrawCmd::Elements { .. } | DrawCmd::RawCallback(_) => None,
        }
    }

    /// Returns the standard callback in a detached draw command, if present.
    #[must_use]
    pub const fn from_snapshot(command: &DrawCmdSnapshot) -> Option<StandardDrawCallback> {
        match command {
            DrawCmdSnapshot::ResetRenderState => Some(StandardDrawCallback::ResetRenderState),
            DrawCmdSnapshot::SetSamplerLinear => Some(StandardDrawCallback::SetSamplerLinear),
            DrawCmdSnapshot::SetSamplerNearest => Some(StandardDrawCallback::SetSamplerNearest),
            DrawCmdSnapshot::SetSamplerFromTexture => {
                Some(StandardDrawCallback::SetSamplerFromTexture)
            }
            DrawCmdSnapshot::Elements { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::{DrawCmdParams, TextureBinding, TextureId};

    use super::*;

    #[test]
    fn borrowed_and_detached_commands_preserve_standard_callback_actions() {
        let cases = [
            (
                DrawCmd::ResetRenderState,
                DrawCmdSnapshot::ResetRenderState,
                Some(StandardDrawCallback::ResetRenderState),
            ),
            (
                DrawCmd::SetSamplerLinear,
                DrawCmdSnapshot::SetSamplerLinear,
                Some(StandardDrawCallback::SetSamplerLinear),
            ),
            (
                DrawCmd::SetSamplerNearest,
                DrawCmdSnapshot::SetSamplerNearest,
                Some(StandardDrawCallback::SetSamplerNearest),
            ),
            (
                DrawCmd::SetSamplerFromTexture,
                DrawCmdSnapshot::SetSamplerFromTexture,
                Some(StandardDrawCallback::SetSamplerFromTexture),
            ),
            (
                DrawCmd::Elements {
                    count: 3,
                    cmd_params: DrawCmdParams {
                        clip_rect: [0.0, 0.0, 1.0, 1.0],
                        texture_id: TextureId::new(1),
                        vtx_offset: 0,
                        idx_offset: 0,
                    },
                },
                DrawCmdSnapshot::Elements {
                    count: 3,
                    clip_rect: [0.0, 0.0, 1.0, 1.0],
                    texture: TextureBinding::Legacy(TextureId::new(1)),
                    vtx_offset: 0,
                    idx_offset: 0,
                },
                None,
            ),
        ];

        for (command, snapshot, expected) in cases {
            assert_eq!(DrawCallbackDispatcher::from_draw_cmd(&command), expected);
            assert_eq!(DrawCallbackDispatcher::from_snapshot(&snapshot), expected);
        }
    }
}
