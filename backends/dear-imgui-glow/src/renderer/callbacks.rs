use dear_imgui_rs::sys;

#[used]
static RESET_RENDER_STATE_MARKER: u8 = 0;
#[used]
static SAMPLER_LINEAR_MARKER: u8 = 1;
#[used]
static SAMPLER_NEAREST_MARKER: u8 = 2;

#[inline(never)]
pub(super) unsafe extern "C" fn draw_callback_reset_render_state(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
    // The distinct volatile targets make callback identity survive LTO and linker ICF.
    let _ = unsafe { std::ptr::read_volatile(&RESET_RENDER_STATE_MARKER) };
}

#[inline(never)]
pub(super) unsafe extern "C" fn draw_callback_set_sampler_linear(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
    let _ = unsafe { std::ptr::read_volatile(&SAMPLER_LINEAR_MARKER) };
}

#[inline(never)]
pub(super) unsafe extern "C" fn draw_callback_set_sampler_nearest(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
    let _ = unsafe { std::ptr::read_volatile(&SAMPLER_NEAREST_MARKER) };
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::{Context, FramePrepareOptions, render::DrawCmd};

    use super::*;

    #[test]
    fn standard_draw_callbacks_have_distinct_addresses() {
        type RawDrawCallback = unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd);
        let callbacks: [RawDrawCallback; 3] = [
            draw_callback_reset_render_state,
            draw_callback_set_sampler_linear,
            draw_callback_set_sampler_nearest,
        ];
        for left in 0..callbacks.len() {
            for right in (left + 1)..callbacks.len() {
                assert!(
                    !std::ptr::fn_addr_eq(callbacks[left], callbacks[right]),
                    "standard Glow draw callbacks must retain distinct identities"
                );
            }
        }
    }

    #[test]
    fn standard_draw_callbacks_classify_as_three_distinct_commands() {
        let mut context = Context::create();
        let platform_io = context.platform_io_mut();
        unsafe {
            platform_io
                .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
            platform_io
                .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
            platform_io
                .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
        }

        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("the callback classification test uses legacy rendering")
            .build();
        context.prepare_frame(FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0));
        let ui = context.frame();
        let draw_list = ui.get_background_draw_list();
        draw_list.add_draw_cmd();
        unsafe {
            draw_list.add_callback(draw_callback_reset_render_state, std::ptr::null_mut(), 0);
            draw_list.add_callback(draw_callback_set_sampler_linear, std::ptr::null_mut(), 0);
            draw_list.add_callback(draw_callback_set_sampler_nearest, std::ptr::null_mut(), 0);
        }
        drop(draw_list);
        let frame = context.render_legacy();
        let commands = frame
            .draw_data()
            .draw_lists()
            .flat_map(|list| list.commands())
            .filter(|command| {
                matches!(
                    command,
                    DrawCmd::ResetRenderState
                        | DrawCmd::SetSamplerLinear
                        | DrawCmd::SetSamplerNearest
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(commands.len(), 3);
        assert!(matches!(commands[0], DrawCmd::ResetRenderState));
        assert!(matches!(commands[1], DrawCmd::SetSamplerLinear));
        assert!(matches!(commands[2], DrawCmd::SetSamplerNearest));
    }
}
