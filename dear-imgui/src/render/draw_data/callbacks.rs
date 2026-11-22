use super::DrawList;
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

    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(context) };
    if platform_io.is_null() {
        return None;
    }

    let platform_io = unsafe { &*platform_io };
    if callback_matches(callback, platform_io.DrawCallback_ResetRenderState) {
        Some(StandardDrawCallback::ResetRenderState)
    } else if callback_matches(callback, platform_io.DrawCallback_SetSamplerLinear) {
        Some(StandardDrawCallback::SetSamplerLinear)
    } else if callback_matches(callback, platform_io.DrawCallback_SetSamplerNearest) {
        Some(StandardDrawCallback::SetSamplerNearest)
    } else if callback_matches(callback, platform_io.DrawCallback_SetSamplerFromTex) {
        Some(StandardDrawCallback::SetSamplerFromTexture)
    } else {
        None
    }
}
