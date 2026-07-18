use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

use crate::AshRenderer;
use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextBinding, ContextTeardown, sys,
};

use super::callbacks::{
    renderer_create_window_sys, renderer_destroy_window_sys, renderer_probe_runtime_sys,
    renderer_render_window_sys, renderer_swap_buffers_sys,
};
use super::runtime::{AshViewportError, OwningViewportRuntime, RuntimeControl, RuntimeState};

struct TestPlatformMarker;

struct TestPlatformAttachment;

impl ContextAttachment for TestPlatformAttachment {
    fn release_platform_windows(&self, context: &ContextTeardown<'_>) {
        context.with_bound_context(clear_test_main_handle_raw);
    }
}

struct TestPlatformLease {
    binding: ContextBinding,
    _lease: ContextAttachmentLease,
}

impl Drop for TestPlatformLease {
    fn drop(&mut self) {
        let _ = self
            .binding
            .try_with_bound_context(clear_test_main_handle_raw);
    }
}

struct OrderingPlatformMarker;

struct OrderingPlatformAttachment {
    control: Rc<RefCell<Option<Rc<RuntimeControl>>>>,
    renderer_released_first: Rc<Cell<bool>>,
}

impl ContextAttachment for OrderingPlatformAttachment {
    fn release_platform_windows(&self, context: &ContextTeardown<'_>) {
        self.renderer_released_first.set(
            self.control
                .borrow()
                .as_ref()
                .is_some_and(|control| control.state() == RuntimeState::ResourceDropped),
        );
        context.with_bound_context(clear_test_main_handle_raw);
    }
}

unsafe extern "C" fn platform_unary(_viewport: *mut sys::ImGuiViewport) {}
unsafe extern "C" fn foreign_renderer_unary(_viewport: *mut sys::ImGuiViewport) {}
unsafe extern "C" fn foreign_renderer_render(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut c_void,
) {
}
unsafe extern "C" fn foreign_renderer_set_window_size(
    _viewport: *mut sys::ImGuiViewport,
    _size: *const sys::ImVec2,
) {
}
unsafe extern "C" fn foreign_draw_reset(
    _draw_list: *const sys::ImDrawList,
    _draw_command: *const sys::ImDrawCmd,
) {
}

fn claim_test_platform_callbacks(context: &mut Context) {
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::PLATFORM_HAS_VIEWPORTS);
    let platform_io = context.platform_io_mut();
    platform_io.set_platform_create_window_raw(Some(platform_unary));
    platform_io.set_platform_destroy_window_raw(Some(platform_unary));
    context
        .main_viewport()
        .set_platform_handle(std::ptr::dangling_mut::<c_void>());
}

fn clear_test_main_handle_raw() {
    let viewport = unsafe { sys::igGetMainViewport() };
    if !viewport.is_null() {
        unsafe {
            (*viewport).PlatformHandle = std::ptr::null_mut();
            (*viewport).PlatformHandleRaw = std::ptr::null_mut();
        }
    }
}

fn attach_test_platform(context: &mut Context) -> TestPlatformLease {
    let lease = context
        .register_attachment::<TestPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestPlatformAttachment),
        )
        .unwrap();
    claim_test_platform_callbacks(context);
    TestPlatformLease {
        binding: context.binding(),
        _lease: lease,
    }
}

#[test]
fn attach_requires_registered_platform_role_without_claiming_callbacks() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    claim_test_platform_callbacks(&mut context);

    let error = OwningViewportRuntime::attach_for_test(&mut context).unwrap_err();

    assert!(matches!(
        error,
        AshViewportError::Attachment(dear_imgui_rs::ContextAttachmentError::MissingPlatform)
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    clear_test_main_handle_raw();
}

#[test]
fn renderer_unconfigure_clears_only_its_flags_and_preserves_foreign_metadata() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    context
        .set_renderer_name(Some("foreign-renderer".to_string()))
        .unwrap();
    let prior_flags = BackendFlags::RENDERER_HAS_VTX_OFFSET;
    let added_flags = BackendFlags::RENDERER_HAS_TEXTURES;
    context
        .io_mut()
        .set_backend_flags(prior_flags | added_flags);
    context
        .platform_io_mut()
        .set_draw_callback_reset_render_state_raw(Some(foreign_draw_reset));

    AshRenderer::unconfigure_imgui_context(&mut context, added_flags);

    assert_eq!(context.io().backend_flags(), prior_flags);
    assert_eq!(
        context.io().backend_renderer_name().unwrap().to_bytes(),
        b"foreign-renderer"
    );
    assert!(
        context
            .platform_io()
            .draw_callback_reset_render_state_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_draw_reset
                        as unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
                )
            })
    );
}

#[test]
fn every_occupied_renderer_slot_rejects_attach_transactionally() {
    let _guard = super::test_context_guard();
    for slot in 0..5 {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let platform_io = context.platform_io_mut();
        match slot {
            0 => platform_io.set_renderer_create_window_raw(Some(foreign_renderer_unary)),
            1 => platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_unary)),
            2 => {
                platform_io.set_renderer_set_window_size_raw(Some(foreign_renderer_set_window_size))
            }
            3 => platform_io.set_renderer_render_window_raw(Some(foreign_renderer_render)),
            4 => platform_io.set_renderer_swap_buffers_raw(Some(foreign_renderer_render)),
            _ => unreachable!(),
        }

        let error = OwningViewportRuntime::attach_for_test(&mut context).unwrap_err();
        assert!(matches!(
            error,
            AshViewportError::RendererCallbackOccupied { .. }
        ));
        let occupied = match slot {
            0 => context.platform_io().renderer_create_window_raw().is_some(),
            1 => context
                .platform_io()
                .renderer_destroy_window_raw()
                .is_some(),
            2 => unsafe {
                (*context.platform_io().as_raw())
                    .Renderer_SetWindowSize
                    .is_some()
            },
            3 => context.platform_io().renderer_render_window_raw().is_some(),
            4 => context.platform_io().renderer_swap_buffers_raw().is_some(),
            _ => unreachable!(),
        };
        assert!(occupied);
        context.platform_io_mut().clear_renderer_handlers();
    }
}

#[test]
fn moving_wrapper_keeps_runtime_owned_storage_stable_and_shutdown_is_once() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let address = runtime.renderer_address_for_test();

    fn move_runtime(runtime: OwningViewportRuntime) -> OwningViewportRuntime {
        runtime
    }

    let mut runtime = move_runtime(runtime);
    assert_eq!(runtime.renderer_address_for_test(), address);
    unsafe { renderer_probe_runtime_sys() };
    assert_eq!(runtime.callback_probe_count_for_test(), 1);
    runtime.shutdown(&mut context).unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert_eq!(
        runtime.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
}

#[test]
fn callback_panic_and_reentry_are_deferred_to_rust_entry() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();

    runtime.panic_next_callback_for_test();
    unsafe { renderer_render_window_sys(std::ptr::null_mut(), std::ptr::null_mut()) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::CallbackPanicked {
            callback: "Renderer_RenderWindow"
        })
    ));

    runtime.trigger_reentrant_entry_for_test();
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::CallbackReentered { .. })
    ));
}

#[test]
fn every_foreign_callback_replacement_is_preserved_during_shutdown() {
    let _guard = super::test_context_guard();
    for (slot, expected_name) in [
        "Renderer_CreateWindow",
        "Renderer_DestroyWindow",
        "Renderer_SetWindowSize",
        "Renderer_RenderWindow",
        "Renderer_SwapBuffers",
    ]
    .into_iter()
    .enumerate()
    {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
        let platform_io = context.platform_io_mut();
        match slot {
            0 => platform_io.set_renderer_create_window_raw(Some(foreign_renderer_unary)),
            1 => platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_unary)),
            2 => {
                platform_io.set_renderer_set_window_size_raw(Some(foreign_renderer_set_window_size))
            }
            3 => platform_io.set_renderer_render_window_raw(Some(foreign_renderer_render)),
            4 => platform_io.set_renderer_swap_buffers_raw(Some(foreign_renderer_render)),
            _ => unreachable!(),
        }

        assert!(matches!(
            runtime.shutdown(&mut context),
            Err(AshViewportError::RendererCallbackReplaced { callback })
                if callback == expected_name
        ));
        let platform_io = context.platform_io();
        let preserved = match slot {
            0 => platform_io
                .renderer_create_window_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                    )
                }),
            1 => platform_io
                .renderer_destroy_window_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                    )
                }),
            2 => platform_io.renderer_set_window_size_matches_pointer_callback(
                foreign_renderer_set_window_size,
            ),
            3 => platform_io
                .renderer_render_window_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_render
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                    )
                }),
            4 => platform_io
                .renderer_swap_buffers_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_render
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                    )
                }),
            _ => unreachable!(),
        };
        assert!(preserved, "foreign callback slot {slot} was overwritten");
        context.platform_io_mut().clear_renderer_handlers();
    }
}

#[test]
fn foreign_renderer_user_data_is_reported_without_taking_or_typing_it() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let foreign = 0x1234_usize as *mut c_void;
    let viewport = unsafe { sys::igGetMainViewport() };
    assert!(!viewport.is_null());
    unsafe {
        (*viewport).RendererUserData = foreign;
        renderer_destroy_window_sys(viewport);
    }

    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert_eq!(unsafe { (*viewport).RendererUserData }, foreign);

    unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn context_first_teardown_releases_renderer_before_platform_and_tombstones_wrapper() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let control_slot = Rc::new(RefCell::new(None));
    let renderer_released_first = Rc::new(Cell::new(false));
    let lease = context
        .register_attachment::<OrderingPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(OrderingPlatformAttachment {
                control: Rc::clone(&control_slot),
                renderer_released_first: Rc::clone(&renderer_released_first),
            }),
        )
        .unwrap();
    let _platform = TestPlatformLease {
        binding: context.binding(),
        _lease: lease,
    };
    claim_test_platform_callbacks(&mut context);
    let runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    control_slot
        .borrow_mut()
        .replace(runtime.control_for_test());

    drop(context);

    assert!(renderer_released_first.get());
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RuntimeDetached)
    ));
}

#[test]
fn owned_callback_table_is_complete_after_publish() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let platform_io = context.platform_io();

    assert!(
        platform_io
            .renderer_create_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_create_window_sys as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            })
    );
    assert!(
        platform_io
            .renderer_destroy_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_destroy_window_sys as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            })
    );
    assert!(
        platform_io
            .renderer_render_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_render_window_sys
                        as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                )
            })
    );
    assert!(
        platform_io
            .renderer_swap_buffers_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_swap_buffers_sys
                        as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                )
            })
    );

    runtime.shutdown(&mut context).unwrap();
}
