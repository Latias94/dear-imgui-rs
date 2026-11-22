use super::DrawList;
use crate::context::binding::rendered_draw_callbacks;
use crate::{internal::RawWrapper, sys};

type RawDrawCallback = unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd);

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
