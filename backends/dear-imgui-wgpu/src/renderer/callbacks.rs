use dear_imgui_rs::sys;

pub(super) unsafe extern "C" fn draw_callback_reset_render_state(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
}

pub(super) unsafe extern "C" fn draw_callback_set_sampler_linear(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
}

pub(super) unsafe extern "C" fn draw_callback_set_sampler_nearest(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
}
