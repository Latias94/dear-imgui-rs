use crate::sys;
use std::slice;

type RawDrawCallback = unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd);

const LEGACY_RESET_RENDER_STATE_CALLBACK_VALUE: usize = usize::MAX;
const RESET_RENDER_STATE_CALLBACK_VALUE: usize = usize::MAX - 7;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum StandardDrawCallback {
    ResetRenderState,
    SetSamplerLinear,
    SetSamplerNearest,
}

#[inline]
fn callback_matches(callback: RawDrawCallback, standard: sys::ImDrawCallback) -> bool {
    standard.is_some_and(|standard| standard as usize == callback as usize)
}

#[inline]
pub(crate) fn classify_standard_draw_callback(
    callback: sys::ImDrawCallback,
) -> Option<StandardDrawCallback> {
    let callback = callback?;
    let callback_value = callback as usize;

    if callback_value == RESET_RENDER_STATE_CALLBACK_VALUE
        || callback_value == LEGACY_RESET_RENDER_STATE_CALLBACK_VALUE
    {
        return Some(StandardDrawCallback::ResetRenderState);
    }

    let context = unsafe { sys::igGetCurrentContext() };
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
    } else {
        None
    }
}

#[inline]
fn is_standard_draw_callback(callback: sys::ImDrawCallback) -> bool {
    classify_standard_draw_callback(callback).is_some()
}

pub(crate) unsafe fn draw_list_has_uncloneable_callbacks(raw: *const sys::ImDrawList) -> bool {
    if raw.is_null() {
        return false;
    }

    let cmd_buffer = unsafe { &(*raw).CmdBuffer };
    if cmd_buffer.Size <= 0 || cmd_buffer.Data.is_null() {
        return false;
    }

    let len = match usize::try_from(cmd_buffer.Size) {
        Ok(len) => len,
        Err(_) => return true,
    };

    unsafe { slice::from_raw_parts(cmd_buffer.Data, len) }
        .iter()
        .any(|cmd| cmd.UserCallback.is_some() && !is_standard_draw_callback(cmd.UserCallback))
}

pub(crate) unsafe fn assert_draw_list_cloneable(raw: *const sys::ImDrawList, caller: &str) {
    assert!(
        !unsafe { draw_list_has_uncloneable_callbacks(raw) },
        "{caller} cannot clone draw lists containing user callbacks; \
         callback userdata is opaque and cannot be duplicated safely"
    );
}
