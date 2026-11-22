use super::*;

fn empty_draw_data(total_idx_count: i32, total_vtx_count: i32) -> DrawData {
    let mut raw = sys::ImDrawData::default();
    raw.TotalIdxCount = total_idx_count;
    raw.TotalVtxCount = total_vtx_count;
    raw.FramebufferScale = sys::ImVec2 { x: 1.0, y: 1.0 };
    DrawData(raw)
}

#[test]
fn draw_list_count_comes_from_the_vector_not_the_frame_counter() {
    let mut lists = [std::ptr::null_mut(), std::ptr::null_mut()];
    let mut raw = sys::ImDrawData::default();
    raw.FrameCount = 97;
    raw.CmdLists.Size = 2;
    raw.CmdLists.Capacity = 2;
    raw.CmdLists.Data = lists.as_mut_ptr();
    let draw_data = DrawData(raw);

    assert_eq!(draw_data.frame_count(), 97);
    assert_eq!(draw_data.draw_lists_count(), 2);
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
    let mut textures_vec = sys::ImVector_ImTextureDataPtr::default();

    let mut raw = sys::ImDrawData::default();
    raw.FramebufferScale = sys::ImVec2 { x: 1.0, y: 1.0 };
    raw.Textures = &mut textures_vec;
    let draw_data = DrawData(raw);

    assert_eq!(draw_data.textures().count(), 0);

    let mut textures_vec = sys::ImVector_ImTextureDataPtr {
        Size: 1,
        Data: std::ptr::null_mut(),
        ..sys::ImVector_ImTextureDataPtr::default()
    };
    let mut raw = sys::ImDrawData::default();
    raw.FramebufferScale = sys::ImVec2 { x: 1.0, y: 1.0 };
    raw.Textures = &mut textures_vec;
    let draw_data = DrawData(raw);
    assert_eq!(draw_data.textures().count(), 0);
}

#[test]
fn standalone_draw_list_uses_its_raw_owner_registrations() {
    unsafe extern "C" fn sampler(_: *const sys::ImDrawList, _: *const sys::ImDrawCmd) {}

    let mut owner = sys::ImGuiContext::default();
    owner.PlatformIO.DrawCallback_SetSamplerLinear = Some(sampler);
    let mut shared = sys::ImDrawListSharedData {
        Context: &raw mut owner,
        ..Default::default()
    };
    let raw_list = sys::ImDrawList {
        _Data: &raw mut shared,
        ..Default::default()
    };
    let ownerless = sys::ImDrawList::default();
    // The lists, shared data, and raw owner all outlive this address-only classification.
    let list = unsafe { DrawList::from_raw(&raw_list) };
    assert_eq!(
        classify_standard_draw_callback(list, Some(sampler)),
        Some(StandardDrawCallback::SetSamplerLinear)
    );
    let list = unsafe { DrawList::from_raw(&ownerless) };
    assert_eq!(classify_standard_draw_callback(list, Some(sampler)), None);
}

#[test]
fn rendered_draw_commands_keep_their_owner_callbacks_after_backend_teardown() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn reset(_parent_list: *const sys::ImDrawList, _cmd: *const sys::ImDrawCmd) {
        std::hint::black_box(0);
    }
    unsafe extern "C" fn linear(_parent_list: *const sys::ImDrawList, _cmd: *const sys::ImDrawCmd) {
        std::hint::black_box(1);
    }
    unsafe extern "C" fn nearest(
        _parent_list: *const sys::ImDrawList,
        _cmd: *const sys::ImDrawCmd,
    ) {
        std::hint::black_box(2);
    }
    unsafe extern "C" fn from_texture(
        _parent_list: *const sys::ImDrawList,
        _cmd: *const sys::ImDrawCmd,
    ) {
        std::hint::black_box(3);
    }

    let mut other = crate::Context::create();
    unsafe {
        other
            .platform_io_mut()
            .set_draw_callback_set_sampler_linear_raw(Some(nearest))
    };
    let other_binding = other.binding();
    let _other = other.suspend_or_panic();

    let mut ctx = crate::Context::create();
    ctx.prepare_frame(
        crate::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
    );
    let platform_io = ctx.platform_io_mut();
    unsafe {
        platform_io.set_draw_callback_reset_render_state_raw(Some(reset));
        platform_io.set_draw_callback_set_sampler_linear_raw(Some(linear));
        platform_io.set_draw_callback_set_sampler_nearest_raw(Some(nearest));
        platform_io.set_draw_callback_set_sampler_from_texture_raw(Some(from_texture));
    }
    let owner_platform_io = platform_io.as_raw_mut();

    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    let frame = ctx.begin_frame();
    let list = frame.ui().get_foreground_draw_list();
    unsafe { list.add_callback(reset, std::ptr::null_mut(), 0) };
    list.set_sampler_linear();
    list.set_sampler_nearest();
    list.set_sampler_from_texture();
    drop(list);
    let pending = frame.render(&consumer);
    let requirements = pending.draw_requirements();
    let feedback: Vec<_> = pending
        .texture_requests()
        .iter()
        .map(|r| r.retry())
        .collect();
    let frame = pending.reconcile_texture_feedback(feedback).unwrap();

    // This headless renderer owns no viewport resources. Its static callback functions remain
    // valid while the native backend shutdown operation clears their registrations.
    unsafe { sys::ImGuiPlatformIO_ClearRendererHandlers(owner_platform_io) };

    other_binding.with_bound_context(|| {
        assert!(!requirements.requires_raw_callback_support());
        assert_eq!(frame.requirements(), requirements);
        let commands: Vec<_> = frame
            .draw_lists()
            .flat_map(DrawList::commands)
            .filter(|cmd| !matches!(cmd, DrawCmd::Elements { .. }))
            .collect();
        assert!(matches!(
            commands.as_slice(),
            [
                DrawCmd::ResetRenderState,
                DrawCmd::SetSamplerLinear,
                DrawCmd::SetSamplerNearest,
                DrawCmd::SetSamplerFromTexture
            ]
        ));
    });
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_recapture_keeps_callback_meanings_until_the_next_frame() {
    use crate::render::DrawCmdSnapshot;

    unsafe extern "C" fn sampler(_: *const sys::ImDrawList, _: *const sys::ImDrawCmd) {}

    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = crate::Context::create();
    ctx.prepare_frame(
        crate::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
    );
    let consumer = ctx.create_detached_renderer_consumer().unwrap();
    // The headless backend uses a static marker and has no viewport resources to release.
    unsafe {
        ctx.platform_io_mut()
            .set_draw_callback_set_sampler_linear_raw(Some(sampler))
    };
    let frame = ctx.begin_frame();
    frame.ui().get_foreground_draw_list().set_sampler_linear();
    let main = frame.render_snapshot(&consumer).unwrap();
    unsafe { ctx.platform_io_mut().clear_renderer_handlers() };
    let recaptured = ctx.platform_viewport_snapshot(&consumer).unwrap();

    for snapshot in [main, recaptured] {
        assert!(
            snapshot
                .draw_data()
                .draw_lists
                .iter()
                .flat_map(|list| &list.commands)
                .any(|cmd| matches!(cmd, DrawCmdSnapshot::SetSamplerLinear))
        );
        let feedback: Vec<_> = snapshot
            .texture_requests()
            .iter()
            .map(|r| r.retry())
            .collect();
        snapshot.commit(feedback).unwrap();
    }

    unsafe {
        ctx.platform_io_mut()
            .set_draw_callback_set_sampler_nearest_raw(Some(sampler))
    };
    let frame = ctx.begin_frame();
    frame.ui().get_foreground_draw_list().set_sampler_nearest();
    let next = frame.render_snapshot(&consumer).unwrap();
    assert!(
        next.draw_data()
            .draw_lists
            .iter()
            .flat_map(|list| &list.commands)
            .any(|cmd| matches!(cmd, DrawCmdSnapshot::SetSamplerNearest))
    );
}
