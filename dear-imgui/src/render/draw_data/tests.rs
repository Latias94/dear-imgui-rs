use super::*;

fn empty_draw_data(total_idx_count: i32, total_vtx_count: i32) -> DrawData {
    DrawData {
        valid: false,
        cmd_lists_count: 0,
        total_idx_count,
        total_vtx_count,
        cmd_lists: crate::internal::ImVector::default(),
        display_pos: [0.0, 0.0],
        display_size: [0.0, 0.0],
        framebuffer_scale: [1.0, 1.0],
        owner_viewport: std::ptr::null_mut(),
        textures: std::ptr::null_mut(),
    }
}

#[test]
fn draw_data_counts_are_checked_usize_counts() {
    let draw_data = empty_draw_data(7, 11);
    let total_idx_count: usize = draw_data.total_idx_count();
    let total_vtx_count: usize = draw_data.total_vtx_count();
    assert_eq!(total_idx_count, 7);
    assert_eq!(total_vtx_count, 11);

    let negative_idx_count = empty_draw_data(-1, 0);
    assert!(
        std::panic::catch_unwind(|| negative_idx_count.total_idx_count()).is_err(),
        "negative raw index counts must not cross the safe API boundary"
    );

    let negative_vtx_count = empty_draw_data(0, -1);
    assert!(
        std::panic::catch_unwind(|| negative_vtx_count.total_vtx_count()).is_err(),
        "negative raw vertex counts must not cross the safe API boundary"
    );
}

#[test]
fn draw_data_textures_empty_is_safe() {
    let mut textures_vec: crate::internal::ImVector<*mut sys::ImTextureData> =
        crate::internal::ImVector::default();

    let draw_data = DrawData {
        valid: false,
        cmd_lists_count: 0,
        total_idx_count: 0,
        total_vtx_count: 0,
        cmd_lists: crate::internal::ImVector::default(),
        display_pos: [0.0, 0.0],
        display_size: [0.0, 0.0],
        framebuffer_scale: [1.0, 1.0],
        owner_viewport: std::ptr::null_mut(),
        textures: &mut textures_vec,
    };

    assert_eq!(draw_data.textures().count(), 0);

    let mut textures_vec: crate::internal::ImVector<*mut sys::ImTextureData> =
        crate::internal::ImVector {
            size: 1,
            data: std::ptr::null_mut(),
            ..crate::internal::ImVector::default()
        };
    let draw_data = DrawData {
        valid: false,
        cmd_lists_count: 0,
        total_idx_count: 0,
        total_vtx_count: 0,
        cmd_lists: crate::internal::ImVector::default(),
        display_pos: [0.0, 0.0],
        display_size: [0.0, 0.0],
        framebuffer_scale: [1.0, 1.0],
        owner_viewport: std::ptr::null_mut(),
        textures: &mut textures_vec,
    };
    assert_eq!(draw_data.textures().count(), 0);
}

#[test]
fn platform_io_standard_draw_callbacks_are_classified() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn reset(_parent_list: *const sys::ImDrawList, _cmd: *const sys::ImDrawCmd) {}
    unsafe extern "C" fn linear(_parent_list: *const sys::ImDrawList, _cmd: *const sys::ImDrawCmd) {
    }
    unsafe extern "C" fn nearest(
        _parent_list: *const sys::ImDrawList,
        _cmd: *const sys::ImDrawCmd,
    ) {
    }

    let mut ctx = crate::Context::create();
    let platform_io = ctx.platform_io_mut();
    unsafe {
        platform_io.set_draw_callback_reset_render_state_raw(Some(reset));
        platform_io.set_draw_callback_set_sampler_linear_raw(Some(linear));
        platform_io.set_draw_callback_set_sampler_nearest_raw(Some(nearest));
    }

    assert_eq!(
        classify_standard_draw_callback(Some(reset)),
        Some(StandardDrawCallback::ResetRenderState)
    );
    assert_eq!(
        classify_standard_draw_callback(Some(linear)),
        Some(StandardDrawCallback::SetSamplerLinear)
    );
    assert_eq!(
        classify_standard_draw_callback(Some(nearest)),
        Some(StandardDrawCallback::SetSamplerNearest)
    );
}
