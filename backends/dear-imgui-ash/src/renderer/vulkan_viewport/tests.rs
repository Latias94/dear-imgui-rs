use super::registry::{
    disable, has_renderer_state_for_context, render_callback_matches,
    try_install_renderer_callbacks, try_install_renderer_callbacks_after_preflight,
    unary_callback_matches, validate_empty_renderer_user_data,
    validate_no_created_platform_windows, validate_platform_backend, validate_platform_callbacks,
    validate_queue_family_selection, validate_vulkan_handles,
};
use super::*;
use ash::vk::Handle;
use std::ffi::c_void;
use std::mem::MaybeUninit;

fn lock_context() -> std::sync::MutexGuard<'static, ()> {
    super::test_context_guard()
}

unsafe extern "C" fn platform_slot_sentinel(
    _viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

unsafe extern "C" fn foreign_renderer_create_window(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_renderer_destroy_window(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_renderer_set_window_size_direct(
    _viewport: *mut sys::ImGuiViewport,
    _size: sys::ImVec2,
) {
}

unsafe extern "C" fn foreign_renderer_set_window_size_pointer(
    _viewport: *mut sys::ImGuiViewport,
    _size: *const sys::ImVec2,
) {
}

unsafe extern "C" fn foreign_renderer_render_window(
    _viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

unsafe extern "C" fn foreign_renderer_swap_buffers(
    _viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

fn set_window_size_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    expected: unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn try_install(ctx: &mut Context) -> Result<(), CallbackOwnershipError> {
    let raw = ctx.as_raw();
    try_install_renderer_callbacks(raw, ctx.platform_io_mut())
}

fn assert_ash_renderer_callbacks(ctx: &Context) {
    let platform_io = ctx.platform_io();
    assert!(unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys
    ));
    assert!(unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys
    ));
    assert!(
        platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
    );
    assert!(render_callback_matches(
        platform_io.renderer_render_window_raw(),
        renderer_render_window_sys
    ));
    assert!(render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        renderer_swap_buffers_sys
    ));
}

#[test]
fn renderer_callbacks_preserve_platform_render_slots() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(raw) };

    unsafe {
        (*platform_io).Platform_RenderWindow = Some(platform_slot_sentinel);
        (*platform_io).Platform_SwapBuffers = Some(platform_slot_sentinel);
    }

    try_install(&mut ctx).expect("empty renderer callback table");

    {
        assert_ash_renderer_callbacks(&ctx);
        unsafe {
            assert!(render_callback_matches(
                (*platform_io).Platform_RenderWindow,
                platform_slot_sentinel
            ));
            assert!(render_callback_matches(
                (*platform_io).Platform_SwapBuffers,
                platform_slot_sentinel
            ));
        }
    }

    disable(&mut ctx).unwrap();

    unsafe {
        assert!(ctx.platform_io().renderer_callbacks_are_empty());
        assert!(render_callback_matches(
            (*platform_io).Platform_RenderWindow,
            platform_slot_sentinel
        ));
        assert!(render_callback_matches(
            (*platform_io).Platform_SwapBuffers,
            platform_slot_sentinel
        ));

        (*platform_io).Platform_RenderWindow = None;
        (*platform_io).Platform_SwapBuffers = None;
    }
}

#[test]
fn foreign_renderer_callbacks_reject_install_without_mutation() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(raw) };

    macro_rules! assert_conflict {
        ($field:ident, $callback:ident, $matches:ident) => {{
            unsafe {
                (*platform_io).$field = Some($callback);
            }
            assert_eq!(
                try_install(&mut ctx),
                Err(CallbackOwnershipError::RendererCallbacksOccupied)
            );
            assert!(!has_renderer_state_for_context(raw));
            unsafe {
                assert!($matches((*platform_io).$field, $callback));
                (*platform_io).$field = None;
            }
            assert!(ctx.platform_io().renderer_callbacks_are_empty());
        }};
    }

    assert_conflict!(
        Renderer_CreateWindow,
        foreign_renderer_create_window,
        unary_callback_matches
    );
    assert_conflict!(
        Renderer_DestroyWindow,
        foreign_renderer_destroy_window,
        unary_callback_matches
    );
    assert_conflict!(
        Renderer_SetWindowSize,
        foreign_renderer_set_window_size_direct,
        set_window_size_callback_matches
    );
    assert_conflict!(
        Renderer_RenderWindow,
        foreign_renderer_render_window,
        render_callback_matches
    );
    assert_conflict!(
        Renderer_SwapBuffers,
        foreign_renderer_swap_buffers,
        render_callback_matches
    );
}

#[test]
fn failed_preflight_leaves_callback_table_registry_and_backend_flag_clean() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let initial_flags = ctx.io().backend_flags();

    let result = try_install_renderer_callbacks_after_preflight(raw, ctx.platform_io_mut(), || {
        Err(CallbackOwnershipError::SurfaceUnsupported(
            SurfaceSupportError::NullSurface,
        ))
    });

    assert_eq!(
        result,
        Err(CallbackOwnershipError::SurfaceUnsupported(
            SurfaceSupportError::NullSurface,
        ))
    );
    assert!(ctx.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state_for_context(raw));
    assert_eq!(ctx.io().backend_flags(), initial_flags);
}

#[test]
fn missing_platform_lifecycle_callbacks_fail_without_claiming_renderer_slots() {
    let _guard = lock_context();
    let ctx = Context::create();

    assert_eq!(
        validate_platform_callbacks(ctx.platform_io()),
        Err(CallbackOwnershipError::PlatformCallbacksUnavailable)
    );
    assert!(ctx.platform_io().renderer_callbacks_are_empty());
}

#[test]
fn missing_platform_capability_fails_without_mutating_renderer_state() {
    let _guard = lock_context();
    let context = Context::create();
    let initial_flags = context.io().backend_flags();

    assert_eq!(
        validate_platform_backend(&context),
        Err(CallbackOwnershipError::PlatformBackendUnavailable)
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state_for_context(context.as_raw()));
    assert_eq!(context.io().backend_flags(), initial_flags);
}

#[test]
fn foreign_renderer_user_data_preflight_is_transactional() {
    let _guard = lock_context();
    let context = Context::create();
    let foreign = 0x1234_usize as *mut c_void;

    assert_eq!(
        validate_empty_renderer_user_data([std::ptr::null_mut(), foreign]),
        Err(CallbackOwnershipError::RendererUserDataOccupied)
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state_for_context(context.as_raw()));
}

#[test]
fn existing_platform_windows_preflight_is_transactional() {
    let _guard = lock_context();
    let context = Context::create();

    assert_eq!(
        validate_no_created_platform_windows([false, true]),
        Err(CallbackOwnershipError::PlatformWindowsAlreadyCreated)
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state_for_context(context.as_raw()));
}

#[test]
fn invalid_vulkan_config_is_rejected_without_callback_mutation() {
    let _guard = lock_context();
    let context = Context::create();
    let physical_device = vk::PhysicalDevice::from_raw(1);
    let present_queue = vk::Queue::from_raw(2);

    assert_eq!(
        validate_vulkan_handles(vk::PhysicalDevice::null(), present_queue),
        Err(CallbackOwnershipError::NullPhysicalDevice)
    );
    assert_eq!(
        validate_vulkan_handles(physical_device, vk::Queue::null()),
        Err(CallbackOwnershipError::NullPresentQueue)
    );

    let queue_families = [vk::QueueFamilyProperties {
        queue_flags: vk::QueueFlags::COMPUTE,
        queue_count: 1,
        ..Default::default()
    }];
    assert_eq!(
        validate_queue_family_selection(&queue_families, 0, 0),
        Err(CallbackOwnershipError::GraphicsQueueFamilyUnsupported {
            queue_family_index: 0,
        })
    );
    assert_eq!(
        validate_queue_family_selection(&queue_families, 1, 0),
        Err(CallbackOwnershipError::GraphicsQueueFamilyOutOfRange {
            queue_family_index: 1,
            queue_family_count: 1,
        })
    );
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(!has_renderer_state_for_context(context.as_raw()));
}

#[test]
fn live_viewport_data_blocks_callback_rebind_and_is_context_owned() {
    let _guard = lock_context();
    let mut ctx_a = Context::create();
    let raw_a = ctx_a.as_raw();
    let data = std::ptr::NonNull::<ViewportAshData>::dangling().as_ptr();
    register_viewport_data(data);
    assert!(is_ash_viewport_data(data));

    unsafe { sys::igSetCurrentContext(std::ptr::null_mut()) };
    let ctx_b = Context::create();
    let raw_b = ctx_b.as_raw();
    assert!(!is_ash_viewport_data(data));
    unsafe { sys::igSetCurrentContext(raw_a) };

    assert_eq!(
        try_install_renderer_callbacks_after_preflight(raw_a, ctx_a.platform_io_mut(), || Ok(()),),
        Err(CallbackOwnershipError::LiveViewportResources)
    );
    assert!(ctx_a.platform_io().renderer_callbacks_are_empty());

    unregister_viewport_data(data);
    unsafe { sys::igSetCurrentContext(raw_b) };
    drop(ctx_b);
    unsafe { sys::igSetCurrentContext(raw_a) };
    drop(ctx_a);
}

#[test]
fn existing_ash_callback_table_cannot_rebind_renderer_state() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let mut renderer = MaybeUninit::<AshRenderer>::uninit();

    try_install(&mut ctx).expect("empty renderer callback table");
    insert_renderer_state(raw, renderer.as_mut_ptr(), None).unwrap();

    assert_eq!(
        try_install(&mut ctx),
        Err(CallbackOwnershipError::RendererCallbacksOccupied)
    );
    assert_ash_renderer_callbacks(&ctx);

    disable(&mut ctx).unwrap();
    assert!(!has_renderer_state_for_context(raw));
}

#[test]
fn disable_preserves_renderer_callbacks_replaced_by_another_backend() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let mut renderer = MaybeUninit::<AshRenderer>::uninit();

    try_install(&mut ctx).expect("empty renderer callback table");
    insert_renderer_state(raw, renderer.as_mut_ptr(), None).unwrap();
    {
        let platform_io = ctx.platform_io_mut();
        platform_io.set_renderer_create_window_raw(Some(foreign_renderer_create_window));
        platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_destroy_window));
        platform_io
            .set_renderer_set_window_size_raw(Some(foreign_renderer_set_window_size_pointer));
        platform_io.set_renderer_render_window_raw(Some(foreign_renderer_render_window));
        platform_io.set_renderer_swap_buffers_raw(Some(foreign_renderer_swap_buffers));
    }
    let io = ctx.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    disable(&mut ctx).unwrap();
    assert!(!has_renderer_state_for_context(raw));
    assert!(
        ctx.io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    {
        let platform_io = ctx.platform_io();
        assert!(unary_callback_matches(
            platform_io.renderer_create_window_raw(),
            foreign_renderer_create_window
        ));
        assert!(unary_callback_matches(
            platform_io.renderer_destroy_window_raw(),
            foreign_renderer_destroy_window
        ));
        assert!(
            platform_io.renderer_set_window_size_matches_pointer_callback(
                foreign_renderer_set_window_size_pointer
            )
        );
        assert!(render_callback_matches(
            platform_io.renderer_render_window_raw(),
            foreign_renderer_render_window
        ));
        assert!(render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            foreign_renderer_swap_buffers
        ));
    }

    let platform_io = ctx.platform_io_mut();
    platform_io.set_renderer_create_window_raw(None);
    platform_io.set_renderer_destroy_window_raw(None);
    platform_io.set_renderer_set_window_size_raw(None);
    platform_io.set_renderer_render_window_raw(None);
    platform_io.set_renderer_swap_buffers_raw(None);
}

#[test]
fn shutdown_is_a_noop_for_a_foreign_renderer_context() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let platform_io = ctx.platform_io_mut();
    platform_io.set_renderer_create_window_raw(Some(foreign_renderer_create_window));
    let io = ctx.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    shutdown_multi_viewport_support(&mut ctx).unwrap();

    assert!(unary_callback_matches(
        ctx.platform_io().renderer_create_window_raw(),
        foreign_renderer_create_window
    ));
    assert!(
        ctx.io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    ctx.platform_io_mut().set_renderer_create_window_raw(None);
}

#[test]
fn renderer_state_is_context_local() {
    let _guard = lock_context();
    let ctx_a = Context::create();
    let raw_a = ctx_a.as_raw();
    let mut renderer_a = MaybeUninit::<AshRenderer>::uninit();
    let renderer_a_ptr = renderer_a.as_mut_ptr();
    insert_renderer_state(raw_a, renderer_a_ptr, None).unwrap();

    unsafe {
        sys::igSetCurrentContext(std::ptr::null_mut());
    }

    let ctx_b = Context::create();
    let raw_b = ctx_b.as_raw();
    let mut renderer_b = MaybeUninit::<AshRenderer>::uninit();
    let renderer_b_ptr = renderer_b.as_mut_ptr();
    insert_renderer_state(raw_b, renderer_b_ptr, None).unwrap();

    unsafe {
        sys::igSetCurrentContext(raw_a);
        {
            let borrowed = borrow_renderer().expect("renderer for context A");
            assert_eq!(borrowed.renderer, renderer_a_ptr);
        }

        sys::igSetCurrentContext(raw_b);
        {
            let borrowed = borrow_renderer().expect("renderer for context B");
            assert_eq!(borrowed.renderer, renderer_b_ptr);
        }
    }

    remove_renderer_state_for_context(raw_b);
    unsafe {
        sys::igSetCurrentContext(raw_b);
        assert!(borrow_renderer().is_none());

        sys::igSetCurrentContext(raw_a);
        assert!(borrow_renderer().is_some());
    }

    remove_renderer_state_for_context(raw_a);
    unsafe {
        sys::igSetCurrentContext(raw_a);
    }
    drop(ctx_a);
    unsafe {
        sys::igSetCurrentContext(raw_b);
    }
    drop(ctx_b);
}

#[test]
fn one_renderer_cannot_be_registered_to_two_contexts() {
    let _guard = lock_context();
    let ctx_a = Context::create();
    let raw_a = ctx_a.as_raw();
    let mut renderer = MaybeUninit::<AshRenderer>::uninit();
    let renderer = renderer.as_mut_ptr();
    insert_renderer_state(raw_a, renderer, None).unwrap();

    unsafe { sys::igSetCurrentContext(std::ptr::null_mut()) };
    let ctx_b = Context::create();
    let raw_b = ctx_b.as_raw();
    assert_eq!(
        insert_renderer_state(raw_b, renderer, None),
        Err(CallbackOwnershipError::RendererAlreadyRegistered)
    );
    assert!(!has_renderer_state_for_context(raw_b));

    remove_renderer_state_for_context(raw_a);
    unsafe { sys::igSetCurrentContext(raw_b) };
    drop(ctx_b);
    unsafe { sys::igSetCurrentContext(raw_a) };
    drop(ctx_a);
}

#[test]
fn clear_for_drop_removes_renderer_state() {
    let _guard = lock_context();
    let ctx = Context::create();
    let raw = ctx.as_raw();
    let mut renderer = MaybeUninit::<AshRenderer>::uninit();
    let renderer_ptr = renderer.as_mut_ptr();

    insert_renderer_state(raw, renderer_ptr, None).unwrap();
    unsafe {
        sys::igSetCurrentContext(raw);
        assert!(borrow_renderer().is_some());
    }

    clear_for_drop(renderer_ptr);
    unsafe {
        sys::igSetCurrentContext(raw);
        assert!(borrow_renderer().is_none());
    }

    drop(ctx);
}

#[test]
fn take_viewport_data_ignores_foreign_renderer_user_data() {
    let _guard = lock_context();
    let mut viewport = sys::ImGuiViewport::default();
    let foreign = 0x1234usize as *mut c_void;
    viewport.RendererUserData = foreign;

    let viewport = unsafe { Viewport::from_raw_mut(&mut viewport) };
    let data = unsafe { take_viewport_data(viewport) };

    assert!(data.is_none());
    assert_eq!(viewport.renderer_user_data(), foreign);
}

#[test]
fn viewport_user_data_mut_ignores_unregistered_renderer_user_data() {
    let _guard = lock_context();
    let mut viewport = sys::ImGuiViewport::default();
    let foreign = 0x1234usize as *mut c_void;
    viewport.RendererUserData = foreign;

    let viewport = unsafe { Viewport::from_raw_mut(&mut viewport) };
    let data = unsafe { viewport_user_data_mut(viewport) };

    assert!(data.is_none());
    assert_eq!(viewport.renderer_user_data(), foreign);
}

#[test]
fn only_active_viewport_state_can_acquire() {
    assert!(ViewportRuntimeState::Active.can_acquire());
    assert!(!ViewportRuntimeState::Paused.can_acquire());
    assert!(!ViewportRuntimeState::RebuildRequired.can_acquire());
    assert!(!ViewportRuntimeState::Failed.can_acquire());
}

#[test]
fn present_semaphores_are_selected_by_acquired_image() {
    let image_zero = vk::Semaphore::from_raw(11);
    let image_one = vk::Semaphore::from_raw(22);
    let semaphores = [image_zero, image_one];

    assert_eq!(
        present_semaphore_for_image(&semaphores, 0),
        Some(image_zero)
    );
    assert_eq!(present_semaphore_for_image(&semaphores, 1), Some(image_one));
    assert_eq!(present_semaphore_for_image(&semaphores, 2), None);
}

#[test]
fn zero_extent_pauses_and_variable_extent_is_clamped() {
    assert_eq!(
        swapchain::desired_extent_from_size_and_scale([0.0, 24.0], [1.0, 1.0]),
        None
    );
    assert_eq!(
        swapchain::desired_extent_from_size_and_scale([12.0, 8.0], [2.0, 1.5]),
        Some(vk::Extent2D {
            width: 24,
            height: 12,
        })
    );

    let capabilities = vk::SurfaceCapabilitiesKHR {
        current_extent: vk::Extent2D {
            width: u32::MAX,
            height: u32::MAX,
        },
        min_image_extent: vk::Extent2D {
            width: 64,
            height: 48,
        },
        max_image_extent: vk::Extent2D {
            width: 1920,
            height: 1080,
        },
        ..Default::default()
    };
    assert_eq!(
        swapchain::select_swapchain_extent(
            &capabilities,
            Some(vk::Extent2D {
                width: 16,
                height: 4096,
            })
        ),
        Some(vk::Extent2D {
            width: 64,
            height: 1080,
        })
    );
    assert_eq!(
        swapchain::select_swapchain_extent(&capabilities, None),
        None
    );
}
