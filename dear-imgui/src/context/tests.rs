use super::{Context, binding::with_bound_context};

#[test]
fn platform_io_shared_and_mut_views_match() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let shared = ctx.platform_io().as_raw();
    let mutable = ctx.platform_io_mut().as_raw();
    assert_eq!(shared, mutable);
}

#[test]
fn with_bound_context_restores_previous_context_after_panic() {
    let _guard = crate::test_support::imgui_context_guard();
    let ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_a = ctx_a.suspend();
    let ctx_b = Context::create();
    let raw_b = ctx_b.raw;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_bound_context(raw_a, || panic!("forced panic while context is rebound"));
    }));

    assert!(result.is_err());
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
fn io_and_platform_io_accessors_use_self_context_not_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let marker_a = std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
    ctx_a.io_mut().set_backend_language_user_data(marker_a);
    let pio_a = ctx_a.platform_io().as_raw();
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = Context::create();
    let marker_b = std::ptr::NonNull::<u16>::dangling().as_ptr().cast();
    ctx_b.io_mut().set_backend_language_user_data(marker_b);
    let pio_b = ctx_b.platform_io().as_raw();

    assert_ne!(marker_a, marker_b);
    assert_ne!(pio_a, pio_b);

    let ctx_a = suspended_a.activate().expect_err("ctx_b is still active");
    assert_eq!(ctx_a.0.io().backend_language_user_data(), marker_a);
    assert_eq!(ctx_a.0.platform_io().as_raw(), pio_a);
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(ctx_a);
}

#[test]
fn style_and_main_viewport_accessors_use_self_context_not_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    ctx_a.style_mut().set_alpha(0.25);
    let viewport_a = ctx_a.main_viewport().as_raw();
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = Context::create();
    ctx_b.style_mut().set_alpha(0.75);
    let viewport_b = ctx_b.main_viewport().as_raw();

    assert_ne!(viewport_a, viewport_b);

    let mut ctx_a = suspended_a.activate().expect_err("ctx_b is still active");
    assert_eq!(ctx_a.0.style().alpha(), 0.25);
    assert_eq!(ctx_a.0.main_viewport().as_raw(), viewport_a);
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(ctx_a);
}

#[test]
fn io_font_global_scale_uses_owner_context_not_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    ctx_a.style_mut().set_font_scale_main(1.25);
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = Context::create();
    ctx_b.style_mut().set_font_scale_main(2.0);

    let mut ctx_a = suspended_a.activate().expect_err("ctx_b is still active");
    assert_eq!(ctx_a.0.io().font_global_scale(), 1.25);

    ctx_a.0.io_mut().set_font_global_scale(1.5);

    assert_eq!(ctx_a.0.style().font_scale_main(), 1.5);
    assert_eq!(ctx_b.style().font_scale_main(), 2.0);
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(ctx_a);
}

#[test]
fn frame_lifecycle_requires_receiver_to_be_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let ctx_a = Context::create();
    let suspended_a = ctx_a.suspend();
    let ctx_b = Context::create();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = suspended_a.0.draw_data();
    }));

    assert!(result.is_err());
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, ctx_b.raw);

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
fn ui_stack_tokens_drop_on_owner_context_and_restore_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_b = super::SuspendedContext::create();
    let raw_b = suspended_b.0.raw;

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.font_atlas_mut().build();
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);

    {
        let ui_a = ctx_a.frame();
        let style_alpha =
            |raw| with_bound_context(raw, || unsafe { (*crate::sys::igGetStyle()).Alpha });
        let original_alpha_a = style_alpha(raw_a);
        let original_alpha_b = style_alpha(raw_b);
        let token = ui_a.push_style_var(crate::StyleVar::Alpha(0.25));
        assert_eq!(style_alpha(raw_a), 0.25);

        unsafe { crate::sys::igSetCurrentContext(raw_b) };
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);
        assert_eq!(style_alpha(raw_b), original_alpha_b);

        drop(token);

        assert_eq!(style_alpha(raw_a), original_alpha_a);
        assert_eq!(style_alpha(raw_b), original_alpha_b);
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);
    }

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.render();

    drop(ctx_a);
    drop(suspended_b);
}

#[test]
fn ui_methods_run_on_owner_context_and_restore_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_b = super::SuspendedContext::create();
    let raw_b = suspended_b.0.raw;

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.font_atlas_mut().build();
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);

    {
        let ui_a = ctx_a.frame();

        let color_a = [0.1, 0.2, 0.3, 1.0];
        let color_b = [0.8, 0.7, 0.6, 1.0];
        unsafe {
            with_bound_context(raw_a, || {
                (&mut *(crate::sys::igGetStyle() as *mut crate::Style))
                    .set_color(crate::StyleColor::Text, color_a)
            });
            with_bound_context(raw_b, || {
                (&mut *(crate::sys::igGetStyle() as *mut crate::Style))
                    .set_color(crate::StyleColor::Text, color_b)
            });
        }

        unsafe { crate::sys::igSetCurrentContext(raw_b) };
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

        assert_eq!(ui_a.style_color(crate::StyleColor::Text), color_a);
        assert_eq!(ui_a.clone_style().color(crate::StyleColor::Text), color_a);
        ui_a.style_colors_dark();

        let owner_color = unsafe {
            with_bound_context(raw_a, || {
                (&*(crate::sys::igGetStyle() as *const crate::Style)).color(crate::StyleColor::Text)
            })
        };
        let current_color = unsafe {
            with_bound_context(raw_b, || {
                (&*(crate::sys::igGetStyle() as *const crate::Style)).color(crate::StyleColor::Text)
            })
        };

        assert_ne!(owner_color, color_a);
        assert_eq!(current_color, color_b);
        assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);
    }

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.render();

    drop(ctx_a);
    drop(suspended_b);
}

#[test]
fn context_stack_tokens_drop_on_owner_context_and_restore_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = Context::create();
    let raw_a = ctx_a.raw;
    let suspended_b = super::SuspendedContext::create();
    let raw_b = suspended_b.0.raw;

    unsafe { crate::sys::igSetCurrentContext(raw_a) };
    let font = ctx_a.font_atlas_mut().add_font_default(None) as *const crate::Font;
    let _ = ctx_a.font_atlas_mut().build();

    let token = {
        let font = unsafe { &*font };
        ctx_a.push_font(font)
    };

    unsafe { crate::sys::igSetCurrentContext(raw_b) };
    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

    drop(token);

    assert_eq!(unsafe { crate::sys::igGetCurrentContext() }, raw_b);

    unsafe { crate::sys::igSetCurrentContext(raw_a) };

    drop(ctx_a);
    drop(suspended_b);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_viewport_snapshot_requires_rendered_frame_and_reuses_current_draw_data() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let _ = ctx.font_atlas_mut().build();
    ctx.prepare_frame(super::FramePrepareOptions::new([320.0, 240.0], 1.0 / 60.0));

    let before_render = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.platform_viewport_snapshot(crate::render::snapshot::SnapshotOptions::default());
    }));
    assert!(before_render.is_err());

    let frame = ctx.begin_frame();
    frame.ui().text("snapshot after render");
    let _ = frame.render();

    let snapshot = ctx
        .platform_viewport_snapshot(crate::render::snapshot::SnapshotOptions::default())
        .expect("rendered platform viewport draw data should snapshot");

    assert_eq!(snapshot.draw.display_size, [320.0, 240.0]);
    assert!(
        snapshot
            .viewports
            .iter()
            .any(|viewport| viewport.draw.display_size == [320.0, 240.0]),
        "platform viewport snapshot should include the rendered main viewport"
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_io_get_window_pos_and_size_setters_install_handlers() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn get_pos(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_pos: *mut crate::sys::ImVec2,
    ) {
        if let Some(out_pos) = unsafe { out_pos.as_mut() } {
            *out_pos = crate::sys::ImVec2 { x: 10.0, y: 20.0 };
        }
    }
    unsafe extern "C" fn get_size(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_size: *mut crate::sys::ImVec2,
    ) {
        if let Some(out_size) = unsafe { out_size.as_mut() } {
            *out_size = crate::sys::ImVec2 { x: 30.0, y: 40.0 };
        }
    }
    unsafe extern "C" fn get_scale(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_scale: *mut crate::sys::ImVec2,
    ) {
        if let Some(out_scale) = unsafe { out_scale.as_mut() } {
            *out_scale = crate::sys::ImVec2 { x: 1.0, y: 2.0 };
        }
    }
    unsafe extern "C" fn get_insets(
        _viewport: *mut crate::sys::ImGuiViewport,
        out_insets: *mut crate::sys::ImVec4,
    ) {
        if let Some(out_insets) = unsafe { out_insets.as_mut() } {
            *out_insets = crate::sys::ImVec4::new(1.0, 2.0, 3.0, 4.0);
        }
    }

    let mut ctx = Context::create();

    {
        let pio = ctx.platform_io_mut();
        pio.set_platform_get_window_pos_raw(Some(get_pos));
        pio.set_platform_get_window_size_raw(Some(get_size));
        pio.set_platform_get_window_framebuffer_scale_raw(Some(get_scale));
        pio.set_platform_get_window_work_area_insets_raw(Some(get_insets));

        let raw = unsafe { &*pio.as_raw() };
        assert!(raw.Platform_GetWindowPos.is_some());
        assert!(raw.Platform_GetWindowSize.is_some());
        assert!(raw.Platform_GetWindowFramebufferScale.is_some());
        assert!(raw.Platform_GetWindowWorkAreaInsets.is_some());
    }
    assert!(
        ctx.io().backend_language_user_data().is_null(),
        "PlatformIO out-param helpers must not occupy BackendLanguageUserData"
    );

    let pio = ctx.platform_io_mut();
    pio.set_platform_get_window_pos_raw(None);
    pio.set_platform_get_window_size_raw(None);
    pio.set_platform_get_window_framebuffer_scale_raw(None);
    pio.set_platform_get_window_work_area_insets_raw(None);

    let raw = unsafe { &*pio.as_raw() };
    assert!(raw.Platform_GetWindowPos.is_none());
    assert!(raw.Platform_GetWindowSize.is_none());
    assert!(raw.Platform_GetWindowFramebufferScale.is_none());
    assert!(raw.Platform_GetWindowWorkAreaInsets.is_none());
}

#[test]
fn registered_user_texture_token_survives_context_drop() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut texture = crate::texture::OwnedTextureData::new();

    let token = ctx.register_user_texture_token(&mut texture);
    drop(ctx);
    drop(token);
    drop(texture);
}

#[test]
fn registered_user_texture_token_survives_texture_drop() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let token = {
        let mut texture = crate::texture::OwnedTextureData::new();
        ctx.register_user_texture_token(&mut texture)
    };

    drop(token);
    drop(ctx);
}

#[test]
fn user_texture_registration_is_idempotent_and_unregister_is_noop_when_missing() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    let mut texture = crate::texture::OwnedTextureData::new();

    ctx.register_user_texture(&mut texture);
    ctx.register_user_texture(&mut texture);
    ctx.unregister_user_texture(&mut texture);
    ctx.unregister_user_texture(&mut texture);
}
