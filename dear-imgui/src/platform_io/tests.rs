use super::*;

fn new_platform_io() -> sys::ImGuiPlatformIO {
    unsafe {
        let p = sys::ImGuiPlatformIO_ImGuiPlatformIO();
        assert!(
            !p.is_null(),
            "ImGuiPlatformIO_ImGuiPlatformIO() returned null"
        );
        let v = *p;
        sys::ImGuiPlatformIO_destroy(p);
        v
    }
}

unsafe extern "C" fn draw_callback_marker(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
}

#[test]
fn platform_io_from_raw_matches_mut_wrapper() {
    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    let raw_ptr = (&mut raw) as *mut sys::ImGuiPlatformIO;

    let shared_ptr = unsafe { PlatformIo::from_raw(raw_ptr.cast_const()) }.as_raw();
    let mutable_ptr = unsafe { PlatformIo::from_raw_mut(raw_ptr) }.as_raw();

    assert_eq!(shared_ptr, mutable_ptr);
}

#[test]
fn platform_io_standard_draw_callback_accessors_roundtrip() {
    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    let pio = unsafe { PlatformIo::from_raw_mut((&mut raw) as *mut sys::ImGuiPlatformIO) };

    unsafe {
        pio.set_draw_callback_reset_render_state_raw(Some(draw_callback_marker));
        pio.set_draw_callback_set_sampler_linear_raw(Some(draw_callback_marker));
        pio.set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_marker));
    }

    assert_eq!(
        pio.draw_callback_reset_render_state_raw()
            .map(|f| f as usize),
        Some(draw_callback_marker as *const () as usize)
    );
    assert_eq!(
        pio.draw_callback_set_sampler_linear_raw()
            .map(|f| f as usize),
        Some(draw_callback_marker as *const () as usize)
    );
    assert_eq!(
        pio.draw_callback_set_sampler_nearest_raw()
            .map(|f| f as usize),
        Some(draw_callback_marker as *const () as usize)
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_io_clear_handlers_resets_platform_and_renderer_callbacks() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn platform_cb(_viewport: *mut sys::ImGuiViewport) {}
    unsafe extern "C" fn renderer_cb(_viewport: *mut sys::ImGuiViewport) {}
    unsafe extern "C" fn platform_dpi_scale_cb(_viewport: *mut sys::ImGuiViewport) -> f32 {
        1.0
    }

    let mut context = crate::Context::create();
    let pio = context.platform_io_mut();
    unsafe {
        pio.set_platform_create_window_raw(Some(platform_cb));
        pio.set_platform_destroy_window_raw(Some(platform_cb));
        pio.set_platform_get_window_dpi_scale_raw(Some(platform_dpi_scale_cb));
        pio.set_platform_on_changed_viewport_raw(Some(platform_cb));
        pio.set_renderer_create_window_raw(Some(renderer_cb));
        pio.set_renderer_destroy_window_raw(Some(renderer_cb));
        pio.clear_platform_handlers();
        assert!(pio.renderer_create_window_raw().is_some());
        pio.clear_renderer_handlers();
    }

    let raw = unsafe { &*pio.as_raw() };
    assert!(raw.Platform_CreateWindow.is_none());
    assert!(raw.Platform_DestroyWindow.is_none());
    assert!(raw.Platform_GetWindowDpiScale.is_none());
    assert!(raw.Platform_OnChangedViewport.is_none());
    assert!(raw.Renderer_CreateWindow.is_none());
    assert!(raw.Renderer_DestroyWindow.is_none());
}

#[cfg(feature = "multi-viewport")]
#[test]
fn clear_platform_handlers_clears_typed_get_window_callbacks() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn get_pos(_viewport: *mut Viewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 41.0, y: 42.0 };
        }
    }
    unsafe extern "C" fn get_size(_viewport: *mut Viewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 43.0, y: 44.0 };
        }
    }
    unsafe extern "C" fn get_scale(_viewport: *mut Viewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 45.0, y: 46.0 };
        }
    }
    unsafe extern "C" fn get_insets(_viewport: *mut Viewport, out: *mut sys::ImVec4) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec4::new(1.0, 2.0, 3.0, 4.0);
        }
    }
    unsafe extern "C" fn set_pos(_viewport: *mut Viewport, _pos: &sys::ImVec2) {}
    unsafe extern "C" fn set_size(_viewport: *mut Viewport, _size: &sys::ImVec2) {}

    let mut ctx = crate::Context::create();
    let viewport = ctx.main_viewport().as_raw_mut();
    let pio = ctx.platform_io_mut();
    unsafe {
        pio.set_platform_set_window_pos(Some(set_pos));
        pio.set_platform_get_window_pos(Some(get_pos));
        pio.set_platform_set_window_size(Some(set_size));
        pio.set_platform_get_window_size(Some(get_size));
        pio.set_platform_get_window_framebuffer_scale(Some(get_scale));
        pio.set_platform_get_window_work_area_insets(Some(get_insets));
        pio.clear_platform_handlers();
    }

    let raw = unsafe { &*pio.as_raw() };
    assert!(raw.Platform_GetWindowPos.is_none());
    assert!(raw.Platform_SetWindowPos.is_none());
    assert!(raw.Platform_GetWindowSize.is_none());
    assert!(raw.Platform_SetWindowSize.is_none());
    assert!(raw.Platform_GetWindowFramebufferScale.is_none());
    assert!(raw.Platform_GetWindowWorkAreaInsets.is_none());
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        0
    );

    let mut out = sys::ImVec2::default();
    unsafe {
        assert!(!sys::ImGuiPlatformIO_InvokePlatformGetWindowPos(
            raw, viewport, &mut out
        ));
        assert!(!sys::ImGuiPlatformIO_InvokePlatformGetWindowSize(
            raw, viewport, &mut out
        ));
        assert!(
            !sys::ImGuiPlatformIO_InvokePlatformGetWindowFramebufferScale(raw, viewport, &mut out)
        );
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn clear_renderer_handlers_clears_renderer_aggregate_callback_storage_only() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn platform_set_pos(
        _viewport: *mut sys::ImGuiViewport,
        _pos: *const sys::ImVec2,
    ) {
    }
    unsafe extern "C" fn renderer_set_size(
        _viewport: *mut sys::ImGuiViewport,
        _size: *const sys::ImVec2,
    ) {
    }

    let mut ctx = crate::Context::create();
    let pio = ctx.platform_io_mut();
    unsafe {
        pio.set_platform_set_window_pos_raw(Some(platform_set_pos));
        pio.set_renderer_set_window_size_raw(Some(renderer_set_size));
    }
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        1
    );

    unsafe { pio.clear_renderer_handlers() };

    let raw = unsafe { &*pio.as_raw() };
    assert!(raw.Platform_SetWindowPos.is_some());
    assert!(raw.Renderer_SetWindowSize.is_none());
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        1
    );

    unsafe { pio.clear_platform_handlers() };
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        0
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn conditional_renderer_clear_releases_owned_storage_but_preserves_replacement_slot() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn owned_pointer_callback(
        _viewport: *mut sys::ImGuiViewport,
        _size: *const sys::ImVec2,
    ) {
    }
    unsafe extern "C" fn foreign_direct_callback(
        _viewport: *mut sys::ImGuiViewport,
        _size: sys::ImVec2,
    ) {
    }

    let storage_count_before = unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() };
    let mut ctx = crate::Context::create();
    let platform_io = ctx.platform_io_mut();
    unsafe { platform_io.set_renderer_set_window_size_raw(Some(owned_pointer_callback)) };
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        storage_count_before + 1
    );

    unsafe {
        (*platform_io.as_raw_mut()).Renderer_SetWindowSize = Some(foreign_direct_callback);
    }
    // The test owns the callback table and no native call can run while it is mutated here.
    assert!(!unsafe {
        platform_io.clear_renderer_set_window_size_if_pointer_callback(owned_pointer_callback)
    });
    assert_eq!(
        unsafe { (*platform_io.as_raw()).Renderer_SetWindowSize }.map(|callback| callback as usize),
        Some(foreign_direct_callback as *const () as usize)
    );
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        storage_count_before
    );

    unsafe {
        (*platform_io.as_raw_mut()).Renderer_SetWindowSize = None;
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn conditional_platform_aggregate_clear_preserves_same_thunk_replacements() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn owned_vec2(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        unsafe { out.write(sys::ImVec2::new(1.0, 2.0)) };
    }
    unsafe extern "C" fn replacement_vec2(
        _viewport: *mut sys::ImGuiViewport,
        out: *mut sys::ImVec2,
    ) {
        unsafe { out.write(sys::ImVec2::new(3.0, 4.0)) };
    }
    unsafe extern "C" fn owned_vec4(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
        unsafe { out.write(sys::ImVec4::zero()) };
    }
    unsafe extern "C" fn owned_const_vec2(
        viewport: *mut sys::ImGuiViewport,
        value: *const sys::ImVec2,
    ) {
        unsafe { (*viewport).Pos = *value };
    }
    unsafe extern "C" fn replacement_const_vec2(
        viewport: *mut sys::ImGuiViewport,
        value: *const sys::ImVec2,
    ) {
        unsafe { (*viewport).Size = *value };
    }

    let storage_count_before = unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() };
    let mut ctx = crate::Context::create();
    let platform_io = ctx.platform_io_mut();
    unsafe {
        platform_io.set_platform_get_window_pos_raw(Some(owned_vec2));
        platform_io.set_platform_get_window_size_raw(Some(owned_vec2));
        platform_io.set_platform_get_window_framebuffer_scale_raw(Some(owned_vec2));
        platform_io.set_platform_get_window_work_area_insets_raw(Some(owned_vec4));
        platform_io.set_platform_set_window_pos_raw(Some(owned_const_vec2));
        platform_io.set_platform_set_window_size_raw(Some(owned_const_vec2));

        platform_io.set_platform_get_window_pos_raw(Some(replacement_vec2));
        platform_io.set_platform_set_window_pos_raw(Some(replacement_const_vec2));
    }
    // The test owns the callback table and no native call can run while it is mutated here.
    assert!(unsafe { !platform_io.clear_platform_get_window_pos_if_raw_callback(owned_vec2) });
    assert!(unsafe {
        !platform_io.clear_platform_set_window_pos_if_pointer_callback(owned_const_vec2)
    });
    let installed =
        unsafe { sys::ImGuiPlatformIO_PlatformGetWindowPosOutParam(platform_io.as_raw_mut()) }
            .unwrap();
    assert!(std::ptr::fn_addr_eq(
        installed,
        replacement_vec2 as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2)
    ));

    unsafe {
        assert!(platform_io.clear_platform_get_window_pos_if_raw_callback(replacement_vec2));
        assert!(
            platform_io.clear_platform_set_window_pos_if_pointer_callback(replacement_const_vec2)
        );
        assert!(platform_io.clear_platform_set_window_size_if_pointer_callback(owned_const_vec2));
        assert!(platform_io.clear_platform_get_window_size_if_raw_callback(owned_vec2));
        assert!(
            platform_io.clear_platform_get_window_framebuffer_scale_if_raw_callback(owned_vec2)
        );
        assert!(platform_io.clear_platform_get_window_work_area_insets_if_raw_callback(owned_vec4));
    }
    assert!(unsafe { (*platform_io.as_raw()).Platform_GetWindowPos }.is_none());
    assert!(unsafe { (*platform_io.as_raw()).Platform_GetWindowSize }.is_none());
    assert!(unsafe { (*platform_io.as_raw()).Platform_GetWindowFramebufferScale }.is_none());
    assert!(unsafe { (*platform_io.as_raw()).Platform_GetWindowWorkAreaInsets }.is_none());
    assert!(unsafe { (*platform_io.as_raw()).Platform_SetWindowPos }.is_none());
    assert!(unsafe { (*platform_io.as_raw()).Platform_SetWindowSize }.is_none());
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        storage_count_before
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn conditional_aggregate_setter_clear_targets_its_receiver_context() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn platform_set(
        _viewport: *mut sys::ImGuiViewport,
        _value: *const sys::ImVec2,
    ) {
    }
    unsafe extern "C" fn renderer_set(
        _viewport: *mut sys::ImGuiViewport,
        _value: *const sys::ImVec2,
    ) {
    }

    let storage_count_before = unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() };
    let mut first = crate::Context::create();
    let first_platform_io = {
        let platform_io = first.platform_io_mut();
        unsafe {
            platform_io.set_platform_set_window_pos_raw(Some(platform_set));
            platform_io.set_platform_set_window_size_raw(Some(platform_set));
            platform_io.set_renderer_set_window_size_raw(Some(renderer_set));
        }
        platform_io.as_raw_mut()
    };
    let first = first.suspend_or_panic();
    let second = crate::Context::create();
    let second_platform_io = second.platform_io().as_raw();

    let first_platform_io = unsafe { PlatformIo::from_raw_mut(first_platform_io) };
    unsafe {
        assert!(first_platform_io.clear_platform_set_window_pos_if_pointer_callback(platform_set));
        assert!(first_platform_io.clear_platform_set_window_size_if_pointer_callback(platform_set));
        assert!(first_platform_io.clear_renderer_set_window_size_if_pointer_callback(renderer_set));
    }

    assert!(unsafe { (*first_platform_io.as_raw()).Platform_SetWindowPos }.is_none());
    assert!(unsafe { (*first_platform_io.as_raw()).Platform_SetWindowSize }.is_none());
    assert!(unsafe { (*first_platform_io.as_raw()).Renderer_SetWindowSize }.is_none());
    assert!(unsafe { (*second_platform_io).Platform_SetWindowPos }.is_none());
    assert!(unsafe { (*second_platform_io).Platform_SetWindowSize }.is_none());
    assert!(unsafe { (*second_platform_io).Renderer_SetWindowSize }.is_none());
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        storage_count_before
    );

    drop(second);
    drop(first);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn conditional_platform_aggregate_clear_reports_direct_slot_replacements() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn owned_get(_viewport: *mut sys::ImGuiViewport, _out: *mut sys::ImVec2) {}
    unsafe extern "C" fn owned_set(_viewport: *mut sys::ImGuiViewport, _value: *const sys::ImVec2) {
    }
    unsafe extern "C" fn foreign_get(_viewport: *mut sys::ImGuiViewport) -> sys::ImVec2 {
        sys::ImVec2::zero()
    }
    unsafe extern "C" fn foreign_set(_viewport: *mut sys::ImGuiViewport, _value: sys::ImVec2) {}

    let storage_count_before = unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() };
    let mut ctx = crate::Context::create();
    let platform_io = ctx.platform_io_mut();
    unsafe {
        platform_io.set_platform_get_window_pos_raw(Some(owned_get));
        platform_io.set_platform_set_window_pos_raw(Some(owned_set));
    }

    unsafe {
        let raw = platform_io.as_raw_mut();
        (*raw).Platform_GetWindowPos = Some(foreign_get);
        (*raw).Platform_SetWindowPos = Some(foreign_set);
    }

    // The test owns the callback table and no native call can run while it is mutated here.
    assert!(unsafe { !platform_io.clear_platform_get_window_pos_if_raw_callback(owned_get) });
    assert!(unsafe { !platform_io.clear_platform_set_window_pos_if_pointer_callback(owned_set) });
    let raw = unsafe { &*platform_io.as_raw() };
    assert!(std::ptr::fn_addr_eq(
        raw.Platform_GetWindowPos.unwrap(),
        foreign_get as unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2
    ));
    assert!(std::ptr::fn_addr_eq(
        raw.Platform_SetWindowPos.unwrap(),
        foreign_set as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
    ));
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        storage_count_before
    );

    unsafe {
        let raw = platform_io.as_raw_mut();
        (*raw).Platform_GetWindowPos = None;
        (*raw).Platform_SetWindowPos = None;
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn context_drop_clears_aggregate_callback_storage() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn platform_set_pos(
        _viewport: *mut sys::ImGuiViewport,
        _pos: *const sys::ImVec2,
    ) {
    }
    unsafe extern "C" fn renderer_set_size(
        _viewport: *mut sys::ImGuiViewport,
        _size: *const sys::ImVec2,
    ) {
    }

    let storage_count_before = unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() };
    {
        let mut ctx = crate::Context::create();
        let pio = ctx.platform_io_mut();
        unsafe {
            pio.set_platform_set_window_pos_raw(Some(platform_set_pos));
            pio.set_renderer_set_window_size_raw(Some(renderer_set_size));
        }
        assert_eq!(
            unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
            storage_count_before + 1
        );
    }

    // The PlatformIO pointer was destroyed with the context. Only query shim-owned storage.
    assert_eq!(
        unsafe { sys::ImGuiPlatformIO_AggregateCallbackStorageCount() },
        storage_count_before
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn callback_registration_and_uninstall_target_the_receiving_context() {
    let _guard = crate::test_support::imgui_context_guard();

    unsafe extern "C" fn first_pos(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        unsafe { out.write(sys::ImVec2::new(11.0, 12.0)) };
    }
    unsafe extern "C" fn second_pos(_viewport: *mut Viewport, out: *mut sys::ImVec2) {
        unsafe { out.write(sys::ImVec2::new(21.0, 22.0)) };
    }
    unsafe extern "C" fn set_size(viewport: *mut Viewport, size: &sys::ImVec2) {
        unsafe { (*viewport).set_size([size.x, size.y]) };
    }
    fn query_position(context: &mut crate::Context) -> [f32; 2] {
        let viewport = context.main_viewport().as_raw_mut();
        let mut pos = sys::ImVec2::default();
        assert!(unsafe {
            sys::ImGuiPlatformIO_InvokePlatformGetWindowPos(
                context.platform_io().as_raw(),
                viewport,
                &mut pos,
            )
        });
        [pos.x, pos.y]
    }

    let mut first = crate::Context::create();
    let first_binding = first.binding();
    let first_raw = first.as_raw();
    unsafe {
        first
            .platform_io_mut()
            .set_platform_get_window_pos_raw(Some(first_pos))
    };
    let first = first.suspend_or_panic();
    let mut second = crate::Context::create();
    first_binding.with_bound_context(|| {
        unsafe {
            let target = second.platform_io_mut();
            target.set_platform_get_window_pos(Some(second_pos));
            target.set_renderer_set_window_size(Some(set_size));
        }
        assert_eq!(unsafe { sys::igGetCurrentContext() }, first_raw);
    });
    assert_eq!(query_position(&mut second), [21.0, 22.0]);
    let viewport = second.main_viewport().as_raw_mut();
    let size = sys::ImVec2::new(75.0, 28.0);
    assert!(unsafe {
        sys::ImGuiPlatformIO_InvokeRendererSetWindowSize(
            second.platform_io().as_raw(),
            viewport,
            &size,
        )
    });
    assert_eq!(second.main_viewport().size(), [75.0, 28.0]);

    first_binding.with_bound_context(|| unsafe {
        second.platform_io_mut().clear_platform_handlers();
        second.platform_io_mut().clear_renderer_handlers();
        assert_eq!(sys::igGetCurrentContext(), first_raw);
    });
    assert!(unsafe {
        (*second.platform_io().as_raw())
            .Platform_GetWindowPos
            .is_none()
    });
    assert!(second.platform_io().renderer_callbacks_are_empty());
    drop(second);
    let mut first = first.activate_or_panic();
    assert_eq!(query_position(&mut first), [11.0, 12.0]);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn typed_platform_callbacks_can_queue_engine_viewport_intent_via_backend_user_data() {
    let _guard = crate::test_support::imgui_context_guard();
    #[derive(Debug, PartialEq)]
    enum Command {
        Create {
            id: crate::Id,
            pos: [f32; 2],
            size: [f32; 2],
        },
        Move {
            id: crate::Id,
            pos: [f32; 2],
        },
    }

    #[derive(Default)]
    struct Queue {
        commands: Vec<Command>,
    }

    unsafe fn queue_from_current_io() -> &'static mut Queue {
        let io = unsafe { sys::igGetIO_Nil() };
        assert!(!io.is_null(), "test requires an active ImGui context");
        let user_data = unsafe { (*io).BackendPlatformUserData };
        assert!(
            !user_data.is_null(),
            "backend platform user data should point at the engine queue"
        );
        unsafe { &mut *(user_data as *mut Queue) }
    }

    unsafe extern "C" fn create_window(viewport: *mut Viewport) {
        let viewport = unsafe { viewport.as_ref() }.expect("viewport should be non-null");
        let queue = unsafe { queue_from_current_io() };
        queue.commands.push(Command::Create {
            id: viewport.id(),
            pos: viewport.pos(),
            size: viewport.size(),
        });
    }

    unsafe extern "C" fn set_window_pos(viewport: *mut Viewport, pos: &sys::ImVec2) {
        let viewport = unsafe { viewport.as_ref() }.expect("viewport should be non-null");
        let queue = unsafe { queue_from_current_io() };
        queue.commands.push(Command::Move {
            id: viewport.id(),
            pos: [pos.x, pos.y],
        });
    }

    let mut ctx = crate::Context::create();
    let mut queue = Queue::default();
    unsafe {
        // `queue` outlives every callback invocation below and is cleared before the test returns.
        ctx.io_mut()
            .set_backend_platform_user_data((&mut queue as *mut Queue).cast());
        ctx.platform_io_mut()
            .set_platform_create_window(Some(create_window));
        ctx.platform_io_mut()
            .set_platform_set_window_pos(Some(set_window_pos));
    }

    let viewport = ctx.main_viewport();
    viewport.set_pos([32.0, 48.0]);
    viewport.set_size([640.0, 360.0]);
    let id = viewport.id();
    let raw_viewport = viewport.as_raw_mut();
    let pio = ctx.platform_io().as_raw();
    unsafe {
        (*pio).Platform_CreateWindow.unwrap()(raw_viewport);
        let pos = sys::ImVec2::new(80.0, 96.0);
        assert!(sys::ImGuiPlatformIO_InvokePlatformSetWindowPos(
            pio,
            raw_viewport,
            &pos
        ));
    }

    assert_eq!(
        queue.commands,
        [
            Command::Create {
                id,
                pos: [32.0, 48.0],
                size: [640.0, 360.0],
            },
            Command::Move {
                id,
                pos: [80.0, 96.0],
            },
        ]
    );

    unsafe {
        // No callback is invoked after this point.
        ctx.io_mut()
            .set_backend_platform_user_data(std::ptr::null_mut());
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn saved_callbacks_restore_the_original_implementation_after_replacement() {
    let _guard = crate::test_support::imgui_context_guard();
    unsafe extern "C" fn original_dpi(_vp: *mut Viewport) -> f32 {
        1.25
    }
    unsafe extern "C" fn replacement_dpi(_vp: *mut Viewport) -> f32 {
        2.5
    }
    unsafe extern "C" fn original_get(_vp: *mut Viewport, out: *mut sys::ImVec2) {
        unsafe { out.write(sys::ImVec2::new(1.0, 1.5)) };
    }
    unsafe extern "C" fn replacement_get(_vp: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        unsafe { out.write(sys::ImVec2::new(2.0, 2.5)) };
    }
    unsafe extern "C" fn original_size(vp: *mut Viewport, size: &sys::ImVec2) {
        unsafe { (*vp).set_size([size.x, size.y]) };
    }
    unsafe extern "C" fn replacement_size(vp: *mut Viewport, size: &sys::ImVec2) {
        unsafe { (*vp).set_size([size.y, size.x]) };
    }

    let mut context = crate::Context::create();
    let viewport = context.main_viewport().as_raw_mut();
    let io = context.platform_io_mut();
    unsafe {
        io.set_platform_get_window_dpi_scale(Some(original_dpi));
        io.set_platform_get_window_framebuffer_scale(Some(original_get));
        io.set_renderer_set_window_size(Some(original_size));
        let saved_dpi = (*io.as_raw()).Platform_GetWindowDpiScale.unwrap();
        let saved_get =
            sys::ImGuiPlatformIO_PlatformGetWindowFramebufferScaleOutParam(io.as_raw_mut())
                .unwrap();
        let saved_size =
            sys::ImGuiPlatformIO_RendererSetWindowSizePointerParam(io.as_raw_mut()).unwrap();

        io.set_platform_get_window_dpi_scale(Some(replacement_dpi));
        io.set_platform_get_window_framebuffer_scale_raw(Some(replacement_get));
        io.set_renderer_set_window_size(Some(replacement_size));
        let mut density = sys::ImVec2::default();
        let size = sys::ImVec2::new(72.0, 48.0);
        assert!(!io.clear_platform_get_window_framebuffer_scale_if_raw_callback(saved_get));
        assert!(!io.clear_renderer_set_window_size_if_pointer_callback(saved_size));
        assert_eq!(
            (*io.as_raw()).Platform_GetWindowDpiScale.unwrap()(viewport),
            2.5
        );
        assert!(
            sys::ImGuiPlatformIO_InvokePlatformGetWindowFramebufferScale(
                io.as_raw(),
                viewport,
                &mut density
            )
        );
        assert_eq!([density.x, density.y], [2.0, 2.5]);
        assert!(sys::ImGuiPlatformIO_InvokeRendererSetWindowSize(
            io.as_raw(),
            viewport,
            &size
        ));
        assert_eq!([(*viewport).Size.x, (*viewport).Size.y], [48.0, 72.0]);

        io.set_platform_get_window_dpi_scale_raw(Some(saved_dpi));
        sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
            io.as_raw_mut(),
            Some(saved_get),
        );
        sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(
            io.as_raw_mut(),
            Some(saved_size),
        );
        assert_eq!(
            (*io.as_raw()).Platform_GetWindowDpiScale.unwrap()(viewport),
            1.25
        );
        assert!(
            sys::ImGuiPlatformIO_InvokePlatformGetWindowFramebufferScale(
                io.as_raw(),
                viewport,
                &mut density
            )
        );
        assert_eq!([density.x, density.y], [1.0, 1.5]);
        assert!(sys::ImGuiPlatformIO_InvokeRendererSetWindowSize(
            io.as_raw(),
            viewport,
            &size
        ));
        assert_eq!([(*viewport).Size.x, (*viewport).Size.y], [72.0, 48.0]);
        assert!(io.clear_platform_get_window_framebuffer_scale_if_raw_callback(saved_get));
        assert!(io.clear_renderer_set_window_size_if_pointer_callback(saved_size));
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn framebuffer_scale_hook_rejects_invalid_density() {
    use std::{cell::Cell, process::Command};

    const CASE: &str = "DEAR_IMGUI_TEST_PLATFORM_FRAMEBUFFER_SCALE";
    if let Ok(case) = std::env::var(CASE) {
        thread_local! { static SCALE: Cell<sys::ImVec2> = const { Cell::new(sys::ImVec2 { x: 1.0, y: 1.0 }) }; }
        unsafe extern "C" fn get_scale(_: *mut Viewport, out: *mut sys::ImVec2) {
            unsafe { out.write(SCALE.get()) };
        }

        let scale = match case.as_str() {
            "zero" => [0.0, 1.0],
            "negative" => [1.0, -1.0],
            "nan" => [1.0, f32::NAN],
            "infinite" => [f32::INFINITY, 1.0],
            _ => panic!("unknown density case {case}"),
        };
        SCALE.set(scale.into());
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = crate::Context::create();
        let viewport = context.main_viewport().as_raw_mut();
        let io = context.platform_io_mut();
        let mut out = sys::ImVec2::default();
        unsafe {
            io.set_platform_get_window_framebuffer_scale(Some(get_scale));
            assert!(
                sys::ImGuiPlatformIO_InvokePlatformGetWindowFramebufferScale(
                    io.as_raw(),
                    viewport,
                    &mut out
                )
            );
        }
        return;
    }

    // The installed hook is fail-stop across the C++ boundary. Observe its diagnostic from
    // a separate test process so an unrelated crash cannot satisfy the contract.
    let test_binary = std::env::current_exe().unwrap();
    let diagnostic =
        "Platform_GetWindowFramebufferScale must return finite positive values on both axes";
    for case in ["zero", "negative", "nan", "infinite"] {
        let output = Command::new(&test_binary)
            .args([
                "--exact",
                "platform_io::tests::framebuffer_scale_hook_rejects_invalid_density",
                "--nocapture",
            ])
            .env(CASE, case)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success() && stderr.contains(diagnostic),
            "{case}: {}\n{stderr}",
            output.status
        );
    }
}

#[cfg(feature = "multi-viewport")]
#[test]
fn cpp_platform_io_probe_round_trips_all_aggregate_slots() {
    let _guard = crate::test_support::imgui_context_guard();
    use std::cell::Cell;
    thread_local! {
        static POSITION: Cell<[f32; 2]> = const { Cell::new([0.0; 2]) };
        static SIZE: Cell<[f32; 2]> = const { Cell::new([0.0; 2]) };
        static RENDERER_SIZE: Cell<[f32; 2]> = const { Cell::new([0.0; 2]) };
    }

    unsafe extern "C" fn set_pos(_viewport: *mut Viewport, pos: &sys::ImVec2) {
        POSITION.set([pos.x, pos.y]);
    }

    unsafe extern "C" fn set_size(_viewport: *mut Viewport, size: &sys::ImVec2) {
        SIZE.set([size.x, size.y]);
    }

    unsafe extern "C" fn get_pos(viewport: *mut Viewport, out: *mut sys::ImVec2) {
        unsafe { out.write((*viewport).pos().into()) };
    }

    unsafe extern "C" fn get_size(_viewport: *mut Viewport, out: *mut sys::ImVec2) {
        unsafe { *out = sys::ImVec2 { x: 7.0, y: 8.0 } };
    }

    unsafe extern "C" fn get_framebuffer_scale(_viewport: *mut Viewport, out: *mut sys::ImVec2) {
        unsafe { *out = sys::ImVec2 { x: 9.0, y: 10.0 } };
    }

    unsafe extern "C" fn get_work_area_insets(_viewport: *mut Viewport, out: *mut sys::ImVec4) {
        unsafe { *out = sys::ImVec4::new(11.0, 12.0, 13.0, 14.0) };
    }

    unsafe extern "C" fn renderer_set_size(_viewport: *mut Viewport, size: &sys::ImVec2) {
        RENDERER_SIZE.set([size.x, size.y]);
    }

    let mut ctx = crate::Context::create();
    ctx.main_viewport().set_pos([5.0, 6.0]);
    let platform_io = ctx.platform_io_mut();
    unsafe {
        platform_io.set_platform_set_window_pos(Some(set_pos));
        platform_io.set_platform_set_window_size(Some(set_size));
        platform_io.set_platform_get_window_pos(Some(get_pos));
        platform_io.set_platform_get_window_size(Some(get_size));
        platform_io.set_platform_get_window_framebuffer_scale(Some(get_framebuffer_scale));
        platform_io.set_platform_get_window_work_area_insets(Some(get_work_area_insets));
        platform_io.set_renderer_set_window_size(Some(renderer_set_size));
    }
    let mut result = sys::DearImguiRsPlatformIoAggregateProbeResult::default();
    unsafe {
        assert!(sys::ImGuiPlatformIO_ProbeAggregateCallbacks(
            platform_io.as_raw_mut(),
            &mut result,
        ));
    }

    assert_eq!(POSITION.get(), [1.0, 2.0]);
    assert_eq!(SIZE.get(), [3.0, 4.0]);
    assert_eq!(RENDERER_SIZE.get(), [15.0, 16.0]);
    assert_eq!(
        (result.PlatformGetWindowPos.x, result.PlatformGetWindowPos.y),
        (5.0, 6.0)
    );
    assert_eq!(
        (
            result.PlatformGetWindowSize.x,
            result.PlatformGetWindowSize.y
        ),
        (7.0, 8.0)
    );
    assert_eq!(
        (
            result.PlatformGetWindowFramebufferScale.x,
            result.PlatformGetWindowFramebufferScale.y,
        ),
        (9.0, 10.0)
    );
    assert_eq!(
        (
            result.PlatformGetWindowWorkAreaInsets.x,
            result.PlatformGetWindowWorkAreaInsets.y,
            result.PlatformGetWindowWorkAreaInsets.z,
            result.PlatformGetWindowWorkAreaInsets.w,
        ),
        (11.0, 12.0, 13.0, 14.0)
    );
}
