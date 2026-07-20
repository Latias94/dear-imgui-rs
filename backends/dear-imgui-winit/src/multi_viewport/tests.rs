use std::rc::Rc;

use dear_imgui_rs::{BackendFlags, Context};
use winit::event::Event;
use winit::event_loop::ActiveEventLoop;

use super::callbacks::{run_callback, winit_create_window, winit_destroy_window};
use super::runtime::{RuntimeState, WinitPlatformError, WinitPlatformRuntime};
use crate::test_util::test_sync::lock_context;

unsafe extern "C" fn foreign_unary(_viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_get_vec2(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    output: *mut dear_imgui_rs::sys::ImVec2,
) {
    if !output.is_null() {
        unsafe { *output = dear_imgui_rs::sys::ImVec2 { x: 7.0, y: 11.0 } };
    }
}

unsafe extern "C" fn foreign_direct_get_vec2(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> dear_imgui_rs::sys::ImVec2 {
    dear_imgui_rs::sys::ImVec2 { x: 7.0, y: 11.0 }
}

unsafe extern "C" fn foreign_get_vec4(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    output: *mut dear_imgui_rs::sys::ImVec4,
) {
    if !output.is_null() {
        unsafe {
            *output = dear_imgui_rs::sys::ImVec4 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                w: 4.0,
            }
        };
    }
}

#[test]
fn callback_claim_targets_the_passed_context_and_restores_the_previous_one() {
    let _guard = lock_context();
    let mut context_a = Context::create();
    let raw_a = context_a.as_raw();
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    let platform_io_b = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_b) };

    let runtime = WinitPlatformRuntime::new_for_test(&mut context_a).unwrap();
    let platform_io_a = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_a) };
    unsafe {
        assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
        assert!((*platform_io_a).Platform_CreateWindow.is_some());
        assert!((*platform_io_a).Platform_GetWindowPos.is_some());
        assert!((*platform_io_b).Platform_CreateWindow.is_none());
        assert!((*platform_io_b).Platform_GetWindowPos.is_none());
    }

    drop(runtime);
    assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, raw_b);
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
    drop(context_a);
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
    drop(context_b);
}

#[test]
fn callback_claim_is_transactional_when_a_foreign_slot_is_occupied() {
    let _guard = lock_context();
    let mut context = Context::create();
    unsafe {
        context
            .platform_io_mut()
            .set_platform_create_window_raw(Some(foreign_unary));
    }

    let error = match WinitPlatformRuntime::new_for_test(&mut context) {
        Ok(_) => panic!("foreign callback ownership must reject runtime attachment"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        WinitPlatformError::PlatformCallbackOccupied {
            callback: "Platform_CreateWindow"
        }
    );
    let actual = unsafe { (*context.platform_io().as_raw()).Platform_CreateWindow }.unwrap();
    assert!(std::ptr::fn_addr_eq(
        actual,
        foreign_unary as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport)
    ));

    unsafe {
        context
            .platform_io_mut()
            .set_platform_create_window_raw(None)
    };
}

#[test]
fn main_viewport_preflight_preserves_a_foreign_platform_handle() {
    let _guard = lock_context();
    let context = Context::create();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let foreign = std::ptr::dangling_mut::<u8>().cast();
    unsafe { (*viewport).PlatformHandle = foreign };

    assert_eq!(
        super::viewport_data::preflight_main_viewport(&context),
        Err(WinitPlatformError::ForeignPlatformUserData)
    );
    assert_eq!(unsafe { (*viewport).PlatformHandle }, foreign);
    unsafe { (*viewport).PlatformHandle = std::ptr::null_mut() };
}

#[test]
fn nested_event_loop_scopes_restore_the_outer_pointer_and_restore_after_panic() {
    let _guard = lock_context();
    let context = Context::create();
    let control = super::runtime::RuntimeControl::new_for_test(&context);
    let outer = 0x1000usize as *const ActiveEventLoop;
    let inner = 0x2000usize as *const ActiveEventLoop;

    control.enter_event_loop_pointer_for_test(outer, || {
        assert_eq!(control.event_loop_pointer_for_test(), outer);
        control.enter_event_loop_pointer_for_test(inner, || {
            assert_eq!(control.event_loop_pointer_for_test(), inner);
        });
        assert_eq!(control.event_loop_pointer_for_test(), outer);
    });
    assert!(control.event_loop_pointer_for_test().is_null());

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        control.enter_event_loop_pointer_for_test(outer, || panic!("injected scope panic"));
    }));
    assert!(control.event_loop_pointer_for_test().is_null());
}

#[test]
fn callback_panic_is_deferred_as_a_typed_fault() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();

    run_callback("InjectedCallback", (), |_| {
        panic!("injected callback panic")
    });
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "InjectedCallback"
        })
    );
    assert_eq!(runtime.poll_fault(), Ok(()));
}

#[test]
fn create_without_event_loop_records_fault_without_unwinding() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };

    unsafe { winit_create_window(viewport) };
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::EventLoopUnavailable)
    );
    unsafe { (*viewport).PlatformRequestClose = false };
}

#[test]
fn destroy_preserves_foreign_platform_user_data_and_reports_it() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let foreign = std::ptr::dangling_mut::<u8>().cast();
    unsafe { (*viewport).PlatformUserData = foreign };

    unsafe { winit_destroy_window(viewport) };
    assert_eq!(unsafe { (*viewport).PlatformUserData }, foreign);
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::ForeignPlatformUserData)
    );

    unsafe { (*viewport).PlatformUserData = std::ptr::null_mut() };
}

#[test]
fn shutdown_preserves_a_replaced_direct_callback_and_is_idempotent() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    unsafe {
        context
            .platform_io_mut()
            .set_platform_show_window_raw(Some(foreign_unary));
    }

    assert_eq!(
        runtime.shutdown(),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_ShowWindow"
        })
    );
    let actual = unsafe { (*context.platform_io().as_raw()).Platform_ShowWindow }.unwrap();
    assert!(std::ptr::fn_addr_eq(
        actual,
        foreign_unary as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport)
    ));
    assert_eq!(runtime.shutdown(), Ok(()));
    assert_eq!(
        runtime.route_secondary_event(&mut context, &Event::<()>::AboutToWait),
        Err(WinitPlatformError::RuntimeDetached)
    );
    unsafe { context.platform_io_mut().set_platform_show_window_raw(None) };
}

#[test]
fn shutdown_preserves_a_replaced_aggregate_callback() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    unsafe {
        context
            .platform_io_mut()
            .set_platform_get_window_pos_raw(Some(foreign_get_vec2));
    }

    assert_eq!(
        runtime.shutdown(),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_GetWindowPos"
        })
    );
    // The runtime has stopped and this test exclusively owns the replacement callback.
    assert!(unsafe {
        context
            .platform_io_mut()
            .clear_platform_get_window_pos_if_raw_callback(foreign_get_vec2)
    });
}

#[test]
fn shutdown_reports_a_direct_aggregate_slot_replacement() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    unsafe {
        (*context.platform_io_mut().as_raw_mut()).Platform_GetWindowPos =
            Some(foreign_direct_get_vec2);
    }

    assert_eq!(
        runtime.shutdown(),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_GetWindowPos"
        })
    );
    let actual = unsafe { (*context.platform_io().as_raw()).Platform_GetWindowPos }.unwrap();
    assert!(std::ptr::fn_addr_eq(
        actual,
        foreign_direct_get_vec2
            as unsafe extern "C" fn(
                *mut dear_imgui_rs::sys::ImGuiViewport,
            ) -> dear_imgui_rs::sys::ImVec2
    ));
    unsafe {
        (*context.platform_io_mut().as_raw_mut()).Platform_GetWindowPos = None;
    }
}

#[test]
fn runtime_does_not_claim_or_clear_an_unimplemented_foreign_callback() {
    let _guard = lock_context();
    let mut context = Context::create();
    unsafe {
        context
            .platform_io_mut()
            .set_platform_get_window_work_area_insets_raw(Some(foreign_get_vec4));
    }

    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    assert_eq!(runtime.shutdown(), Ok(()));
    // The runtime has stopped and this test exclusively owns the replacement callback.
    assert!(unsafe {
        context
            .platform_io_mut()
            .clear_platform_get_window_work_area_insets_if_raw_callback(foreign_get_vec4)
    });
}

#[test]
fn shutdown_restores_viewport_flags_that_predated_the_runtime() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut flags = context.io().backend_flags();
    flags.insert(BackendFlags::PLATFORM_HAS_VIEWPORTS);
    context.io_mut().set_backend_flags(flags);

    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    runtime.shutdown().unwrap();
    let flags = context.io().backend_flags();
    assert!(flags.contains(BackendFlags::PLATFORM_HAS_VIEWPORTS));
    assert!(!flags.contains(BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT));
}

#[test]
fn wrapper_first_shutdown_detaches_once_and_clears_capability_flags() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let control = Rc::clone(runtime.control());
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
    );

    drop(runtime);
    assert_eq!(control.state(), RuntimeState::Detached);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
    );
    drop(context);
    assert_eq!(control.state(), RuntimeState::Detached);
}

#[test]
fn context_first_shutdown_tombstones_the_wrapper_without_native_access_after_drop() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let control = Rc::clone(runtime.control());

    drop(context);
    assert_eq!(control.state(), RuntimeState::ContextDestroyed);
    assert_eq!(runtime.poll_fault(), Ok(()));
    drop(runtime);
    assert_eq!(control.state(), RuntimeState::ContextDestroyed);
}
