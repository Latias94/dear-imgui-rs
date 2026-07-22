use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dear_imgui_rs::{BackendFlags, Context};
use winit::event::Event;
use winit::event_loop::ActiveEventLoop;

use super::WinitPlatformError;
use super::callbacks::{
    record_viewport_failure, run_callback, winit_create_window, winit_destroy_window,
    winit_get_window_pos_out,
};
use super::registry::preflight_viewport_ownership;
use super::runtime::{ConstructionStage, RuntimeState, WinitPlatformRuntime};
use crate::test_util::test_sync::lock_context;

unsafe extern "C" fn foreign_unary(_viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {}

static FOREIGN_DESTROY_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn foreign_destroy(_viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {
    FOREIGN_DESTROY_CALLS.fetch_add(1, Ordering::Relaxed);
}

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

unsafe extern "C" fn foreign_get_vec4_replacement(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    output: *mut dear_imgui_rs::sys::ImVec4,
) {
    if !output.is_null() {
        unsafe {
            *output = dear_imgui_rs::sys::ImVec4 {
                x: 5.0,
                y: 7.0,
                z: 11.0,
                w: 13.0,
            }
        };
    }
}

unsafe extern "C" fn foreign_set_window_alpha(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _alpha: f32,
) {
}

unsafe extern "C" fn foreign_set_window_alpha_replacement(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _alpha: f32,
) {
}

unsafe extern "C" fn foreign_create_vk_surface(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _instance: u64,
    _allocators: *const c_void,
    output: *mut u64,
) -> i32 {
    if !output.is_null() {
        unsafe { *output = 0 };
    }
    0
}

unsafe extern "C" fn foreign_create_vk_surface_replacement(
    _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _instance: u64,
    _allocators: *const c_void,
    output: *mut u64,
) -> i32 {
    if !output.is_null() {
        unsafe { *output = 0 };
    }
    0
}

#[derive(Clone, Copy)]
struct ContextPublicationSnapshot {
    monitor_data: *mut dear_imgui_rs::sys::ImGuiPlatformMonitor,
    monitor_size: i32,
    monitor_capacity: i32,
    backend_flags: BackendFlags,
    main_platform_user_data: *mut c_void,
    main_platform_handle: *mut c_void,
}

fn valid_test_monitor() -> dear_imgui_rs::sys::ImGuiPlatformMonitor {
    dear_imgui_rs::sys::ImGuiPlatformMonitor {
        MainPos: dear_imgui_rs::sys::ImVec2 { x: 3.0, y: 5.0 },
        MainSize: dear_imgui_rs::sys::ImVec2 {
            x: 1280.0,
            y: 720.0,
        },
        WorkPos: dear_imgui_rs::sys::ImVec2 { x: 3.0, y: 5.0 },
        WorkSize: dear_imgui_rs::sys::ImVec2 {
            x: 1280.0,
            y: 680.0,
        },
        DpiScale: 1.0,
        PlatformHandle: std::ptr::null_mut(),
    }
}

#[test]
fn logical_monitor_coordinates_reject_mixed_dpi_layouts() {
    assert_eq!(
        super::callbacks::validate_monitor_scale_factors_for_test(&[1.0, 1.0]),
        Ok(())
    );
    assert_eq!(
        super::callbacks::validate_monitor_scale_factors_for_test(&[1.0, 1.5]),
        Err(WinitPlatformError::MixedMonitorScaleFactorsUnsupported)
    );
}

#[test]
fn multi_viewport_rejects_custom_hidpi_coordinate_modes() {
    use super::runtime::validate_multi_viewport_hidpi_mode;

    assert_eq!(
        validate_multi_viewport_hidpi_mode(crate::HiDpiMode::Default),
        Ok(())
    );
    for mode in [crate::HiDpiMode::Rounded, crate::HiDpiMode::Locked(2.0)] {
        assert_eq!(
            validate_multi_viewport_hidpi_mode(mode),
            Err(WinitPlatformError::CustomHiDpiModeUnsupported)
        );
    }
}

#[test]
fn wayland_is_rejected_before_multi_viewport_publication() {
    assert_eq!(
        super::runtime::validate_window_system_for_test(true, true),
        Err(WinitPlatformError::WaylandUnsupported)
    );
    assert_eq!(
        super::runtime::validate_window_system_for_test(true, false),
        Ok(())
    );
    assert_eq!(
        super::runtime::validate_window_system_for_test(false, false),
        Err(WinitPlatformError::UnsupportedWindowSystem {
            target: std::env::consts::OS
        })
    );
}

fn snapshot_publication_state(context: &Context) -> ContextPublicationSnapshot {
    let platform_io = unsafe { &*context.platform_io().as_raw() };
    let main_viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    ContextPublicationSnapshot {
        monitor_data: platform_io.Monitors.Data,
        monitor_size: platform_io.Monitors.Size,
        monitor_capacity: platform_io.Monitors.Capacity,
        backend_flags: context.io().backend_flags(),
        main_platform_user_data: unsafe { (*main_viewport).PlatformUserData },
        main_platform_handle: unsafe { (*main_viewport).PlatformHandle },
    }
}

fn assert_publication_state_restored(context: &Context, expected: ContextPublicationSnapshot) {
    let platform_io = unsafe { &*context.platform_io().as_raw() };
    assert_eq!(platform_io.Monitors.Data, expected.monitor_data);
    assert_eq!(platform_io.Monitors.Size, expected.monitor_size);
    assert_eq!(platform_io.Monitors.Capacity, expected.monitor_capacity);
    assert_eq!(context.io().backend_flags(), expected.backend_flags);

    let main_viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    assert_eq!(
        unsafe { (*main_viewport).PlatformUserData },
        expected.main_platform_user_data
    );
    assert_eq!(
        unsafe { (*main_viewport).PlatformHandle },
        expected.main_platform_handle
    );

    assert!(platform_io.Platform_CreateWindow.is_none());
    assert!(platform_io.Platform_DestroyWindow.is_none());
    assert!(platform_io.Platform_ShowWindow.is_none());
    assert!(platform_io.Platform_SetWindowPos.is_none());
    assert!(platform_io.Platform_GetWindowPos.is_none());
    assert!(platform_io.Platform_SetWindowSize.is_none());
    assert!(platform_io.Platform_GetWindowSize.is_none());
    assert!(platform_io.Platform_GetWindowFramebufferScale.is_none());
    assert!(platform_io.Platform_SetWindowFocus.is_none());
    assert!(platform_io.Platform_GetWindowFocus.is_none());
    assert!(platform_io.Platform_GetWindowMinimized.is_none());
    assert!(platform_io.Platform_SetWindowTitle.is_none());
    assert!(platform_io.Platform_SetWindowAlpha.is_none());
    assert!(platform_io.Platform_UpdateWindow.is_none());
    assert!(platform_io.Platform_RenderWindow.is_none());
    assert!(platform_io.Platform_SwapBuffers.is_none());
    assert!(platform_io.Platform_GetWindowDpiScale.is_none());
    assert!(platform_io.Platform_OnChangedViewport.is_none());
    assert!(super::registry::runtime_for_context(context.as_raw()).is_none());
}

#[test]
fn invalid_monitor_preflight_is_typed_and_does_not_publish_context_state() {
    let _guard = lock_context();
    let mut context = Context::create();
    unsafe {
        context
            .platform_io_mut()
            .set_monitors(&[valid_test_monitor()]);
    }
    let before = snapshot_publication_state(&context);
    let checkpoint_reached = Cell::new(false);
    let mut invalid = valid_test_monitor();
    invalid.MainSize.x = 0.0;

    let error =
        match WinitPlatformRuntime::new_for_test_with(&mut context, vec![invalid], |_, _| {
            checkpoint_reached.set(true);
            Ok(())
        }) {
            Ok(_) => panic!("invalid monitor geometry must fail construction"),
            Err(error) => error,
        };

    assert_eq!(
        error,
        WinitPlatformError::InvalidMonitorGeometry {
            monitor: 0,
            reason: "MainSize must be positive",
        }
    );
    assert!(!checkpoint_reached.get());
    assert_publication_state_restored(&context, before);

    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_publication_state_restored(&context, before);
}

#[test]
fn construction_failure_rolls_back_every_publication_stage_and_allows_retry() {
    let _guard = lock_context();
    let stages = [
        ConstructionStage::Attachment,
        ConstructionStage::Registry,
        ConstructionStage::MainViewport,
        ConstructionStage::Callbacks,
        ConstructionStage::Monitors,
        ConstructionStage::BackendFlags,
    ];

    for failed_stage in stages {
        let mut context = Context::create();
        unsafe {
            context
                .platform_io_mut()
                .set_monitors(&[valid_test_monitor()]);
        }
        let before = snapshot_publication_state(&context);

        let error = match WinitPlatformRuntime::new_for_test_with(
            &mut context,
            vec![valid_test_monitor()],
            |stage, _| {
                if stage == failed_stage {
                    Err(WinitPlatformError::InjectedConstructionFailure {
                        stage: failed_stage.name(),
                    })
                } else {
                    Ok(())
                }
            },
        ) {
            Ok(_) => panic!("injected construction failure must abort the transaction"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            WinitPlatformError::InjectedConstructionFailure {
                stage: failed_stage.name(),
            }
        );
        assert_publication_state_restored(&context, before);

        let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
        runtime.shutdown(&mut context).unwrap();
        assert_publication_state_restored(&context, before);
    }
}

#[test]
fn construction_rollback_preserves_a_foreign_callback_replacement() {
    let _guard = lock_context();
    let mut context = Context::create();
    let before = snapshot_publication_state(&context);

    let error = match WinitPlatformRuntime::new_for_test_with(
        &mut context,
        vec![valid_test_monitor()],
        |stage, context| {
            if stage == ConstructionStage::BackendFlags {
                unsafe {
                    context
                        .platform_io_mut()
                        .set_platform_show_window_raw(Some(foreign_unary));
                }
                Err(WinitPlatformError::InjectedConstructionFailure {
                    stage: stage.name(),
                })
            } else {
                Ok(())
            }
        },
    ) {
        Ok(_) => panic!("injected construction failure must abort the transaction"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        WinitPlatformError::InjectedConstructionFailure {
            stage: ConstructionStage::BackendFlags.name(),
        }
    );
    let platform_io = unsafe { &*context.platform_io().as_raw() };
    let actual = platform_io.Platform_ShowWindow.unwrap();
    assert!(std::ptr::fn_addr_eq(
        actual,
        foreign_unary as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport)
    ));
    assert_eq!(platform_io.Monitors.Data, before.monitor_data);
    assert_eq!(platform_io.Monitors.Size, before.monitor_size);
    assert_eq!(platform_io.Monitors.Capacity, before.monitor_capacity);
    assert_eq!(context.io().backend_flags(), before.backend_flags);
    assert!(super::registry::runtime_for_context(context.as_raw()).is_none());

    unsafe { context.platform_io_mut().set_platform_show_window_raw(None) };
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_publication_state_restored(&context, before);
}

#[test]
fn construction_panic_rolls_back_target_context_and_restores_the_previous_current_context() {
    let _guard = lock_context();
    let mut context_a = Context::create();
    let raw_a = context_a.as_raw();
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };
    let context_b = Context::create();
    let raw_b = context_b.as_raw();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = WinitPlatformRuntime::new_for_test_with(
            &mut context_a,
            vec![valid_test_monitor()],
            |stage, _| {
                if stage == ConstructionStage::Monitors {
                    panic!("injected construction panic");
                }
                Ok(())
            },
        );
    }));

    assert!(panic.is_err());
    assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, raw_b);
    let platform_io_a = unsafe { &*dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_a) };
    assert!(platform_io_a.Platform_CreateWindow.is_none());
    assert!(platform_io_a.Platform_ShowWindow.is_none());
    assert!(platform_io_a.Monitors.Data.is_null());
    assert!(super::registry::runtime_for_context(raw_a).is_none());

    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context_a).unwrap();
    runtime.shutdown(&mut context_a).unwrap();
    assert_eq!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }, raw_b);

    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
    drop(context_a);
    unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
    drop(context_b);
}

#[test]
fn shutdown_preserves_a_foreign_monitor_replacement_and_allows_reopen() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let mut replacement = valid_test_monitor();
    replacement.MainPos.x = 41.0;
    replacement.WorkPos.x = 41.0;

    unsafe { context.platform_io_mut().set_monitors(&[replacement]) };
    let replacement_data = unsafe { (*context.platform_io().as_raw()).Monitors.Data };
    runtime.shutdown(&mut context).unwrap();

    let monitors = unsafe { &(*context.platform_io().as_raw()).Monitors };
    assert_eq!(monitors.Data, replacement_data);
    assert_eq!(monitors.Size, 1);
    assert_eq!(unsafe { (*monitors.Data).MainPos.x }, 41.0);

    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    runtime.shutdown(&mut context).unwrap();
    let monitors = unsafe { &(*context.platform_io().as_raw()).Monitors };
    assert_eq!(monitors.Data, replacement_data);
    assert_eq!(unsafe { (*monitors.Data).MainPos.x }, 41.0);
}

#[test]
fn shutdown_preserves_a_foreign_empty_monitor_replacement_without_double_free() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();

    unsafe { context.platform_io_mut().set_monitors(&[]) };
    runtime.shutdown(&mut context).unwrap();
    let monitors = unsafe { &(*context.platform_io().as_raw()).Monitors };
    assert!(monitors.Data.is_null());
    assert_eq!(monitors.Size, 0);
    assert_eq!(monitors.Capacity, 0);

    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    runtime.shutdown(&mut context).unwrap();
    let monitors = unsafe { &(*context.platform_io().as_raw()).Monitors };
    assert!(monitors.Data.is_null());
    assert_eq!(monitors.Size, 0);
    assert_eq!(monitors.Capacity, 0);
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

    let mut runtime = runtime;
    runtime.shutdown(&mut context_a).unwrap();
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
    let mut context = Context::create();
    let platform = crate::WinitPlatform::new(&mut context).unwrap();
    let platform_control = platform.control();
    let control = super::runtime::RuntimeControl::new_for_test(&context, &platform_control);
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
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();

    run_callback("InjectedCallback", (), |_| {
        panic!("injected callback panic")
    });
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "InjectedCallback"
        })
    );
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "InjectedCallback"
        })
    );
    assert_eq!(runtime.control().state(), RuntimeState::Faulted);
    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "InjectedCallback"
        })
    );
}

#[test]
fn callback_table_drift_blocks_remaining_callbacks_and_revokes_capability() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    unsafe {
        (*viewport).Pos = dear_imgui_rs::sys::ImVec2 { x: 41.0, y: 43.0 };
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(Some(foreign_destroy));
    }
    let mut output = dear_imgui_rs::sys::ImVec2 { x: -1.0, y: -2.0 };

    unsafe { winit_get_window_pos_out(viewport, &mut output) };

    assert_eq!(output.x, -1.0);
    assert_eq!(output.y, -2.0);
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_DestroyWindow"
        })
    );
    let flags = context.io().backend_flags();
    assert!(!flags.contains(BackendFlags::PLATFORM_HAS_VIEWPORTS));
    assert!(!flags.contains(BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT));

    unsafe {
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(Some(winit_destroy_window));
    }
    output = dear_imgui_rs::sys::ImVec2 { x: -3.0, y: -5.0 };
    unsafe { winit_get_window_pos_out(viewport, &mut output) };
    assert_eq!(output.x, -3.0);
    assert_eq!(output.y, -5.0);
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_DestroyWindow"
        })
    );
    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_DestroyWindow"
        })
    );
}

#[test]
fn create_without_event_loop_records_fault_without_unwinding() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };

    unsafe { winit_create_window(viewport) };
    assert!(unsafe { (*viewport).PlatformRequestClose });
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::EventLoopUnavailable)
    );
    unsafe { (*viewport).PlatformRequestClose = false };
}

#[test]
fn create_rejects_foreign_platform_handle_raw_before_allocating_or_publishing() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let mut viewport = dear_imgui_rs::sys::ImGuiViewport::default();
    let foreign_raw = std::ptr::dangling_mut::<u8>().cast();
    viewport.PlatformHandleRaw = foreign_raw;

    // The callback must reject the viewport before it dereferences the scoped event-loop pointer
    // to create a native window. A dangling non-null value is therefore sufficient for this
    // regression test.
    runtime.control().enter_event_loop_pointer_for_test(
        std::ptr::NonNull::<ActiveEventLoop>::dangling().as_ptr(),
        || unsafe { winit_create_window(&mut viewport) },
    );

    assert!(viewport.PlatformRequestClose);
    assert!(viewport.PlatformUserData.is_null());
    assert!(viewport.PlatformHandle.is_null());
    assert_eq!(viewport.PlatformHandleRaw, foreign_raw);
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::ForeignPlatformUserData)
    );
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn unsupported_viewport_policy_fault_requests_close() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let mut viewport = dear_imgui_rs::sys::ImGuiViewport::default();
    let error = WinitPlatformError::UnsupportedViewportFlag {
        flag: "NoFocusOnClick",
        operation: "window creation",
    };

    record_viewport_failure(runtime.control(), &mut viewport, error.clone());

    assert!(viewport.PlatformRequestClose);
    assert_eq!(runtime.poll_fault(), Err(error));
}

#[test]
fn destroy_preserves_foreign_platform_user_data_and_reports_it() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let foreign = std::ptr::dangling_mut::<u8>().cast();
    unsafe { (*viewport).PlatformUserData = foreign };

    unsafe { winit_destroy_window(viewport) };
    assert_eq!(unsafe { (*viewport).PlatformUserData }, foreign);
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::ViewportOwnershipLost {
            viewport_id: unsafe { (*viewport).ID },
            field: "PlatformUserData",
        })
    );

    unsafe { (*viewport).PlatformUserData = std::ptr::null_mut() };
    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::ViewportOwnershipLost {
            viewport_id: unsafe { (*viewport).ID },
            field: "PlatformUserData",
        })
    );
    assert_eq!(runtime.control().state(), RuntimeState::Detached);
}

#[test]
fn shutdown_rejects_foreign_viewport_fields_before_aggregate_destroy() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let foreign = std::ptr::dangling_mut::<u8>().cast();
    unsafe { (*viewport).PlatformHandle = foreign };

    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::ViewportOwnershipLost {
            viewport_id: unsafe { (*viewport).ID },
            field: "PlatformHandle",
        })
    );
    assert_eq!(runtime.control().state(), RuntimeState::Attached);
    assert_eq!(unsafe { (*viewport).PlatformHandle }, foreign);

    unsafe { (*viewport).PlatformHandle = std::ptr::null_mut() };
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn preflight_keeps_owned_viewports_filtered_from_the_public_snapshot() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let platform_io = context.platform_io_mut().as_raw_mut();
    let original_size = unsafe { (*platform_io).Viewports.Size };

    // Dear ImGui rebuilds `PlatformIO.Viewports` as a filtered public snapshot. An owned
    // viewport can be hidden in that snapshot while still remaining live in its internal list.
    unsafe { (*platform_io).Viewports.Size = 0 };
    let result = unsafe { preflight_viewport_ownership(runtime.control(), platform_io) };
    unsafe { (*platform_io).Viewports.Size = original_size };

    assert_eq!(result, Ok(()));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn shutdown_preserves_a_replaced_direct_callback_and_is_idempotent() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let mut flags = context.io().backend_flags();
    flags.insert(BackendFlags::HAS_GAMEPAD);
    context.io_mut().set_backend_flags(flags);
    unsafe {
        context
            .platform_io_mut()
            .set_platform_show_window_raw(Some(foreign_unary));
    }

    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_ShowWindow"
        })
    );
    let actual = unsafe { (*context.platform_io().as_raw()).Platform_ShowWindow }.unwrap();
    assert!(std::ptr::fn_addr_eq(
        actual,
        foreign_unary as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport)
    ));
    let flags = context.io().backend_flags();
    assert!(!flags.contains(BackendFlags::PLATFORM_HAS_VIEWPORTS));
    assert!(!flags.contains(BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT));
    assert!(flags.contains(BackendFlags::HAS_GAMEPAD));
    assert_eq!(runtime.shutdown(&mut context), Ok(()));
    assert_eq!(
        runtime.route_secondary_event(&mut context, &Event::<()>::AboutToWait),
        Err(WinitPlatformError::RuntimeDetached)
    );
    unsafe { context.platform_io_mut().set_platform_show_window_raw(None) };

    let mut reopened = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    reopened.shutdown(&mut context).unwrap();
    let flags = context.io().backend_flags();
    assert!(!flags.contains(BackendFlags::PLATFORM_HAS_VIEWPORTS));
    assert!(!flags.contains(BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT));
    assert!(flags.contains(BackendFlags::HAS_GAMEPAD));
}

#[test]
fn shutdown_never_passes_winit_viewports_to_a_foreign_destroy_callback() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    FOREIGN_DESTROY_CALLS.store(0, Ordering::Relaxed);
    unsafe {
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(Some(foreign_destroy));
    }

    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::PlatformCallbackReplaced {
            callback: "Platform_DestroyWindow"
        })
    );
    assert_eq!(runtime.control().state(), RuntimeState::Attached);
    assert_eq!(FOREIGN_DESTROY_CALLS.load(Ordering::Relaxed), 0);

    unsafe {
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(Some(winit_destroy_window));
    }
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn platform_shutdown_requires_the_renderer_destroy_callback_to_be_released_first() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    FOREIGN_DESTROY_CALLS.store(0, Ordering::Relaxed);
    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(Some(foreign_destroy));
    }

    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::RendererShutdownRequired {
            field: "Renderer_DestroyWindow"
        })
    );
    assert_eq!(runtime.control().state(), RuntimeState::Attached);
    assert_eq!(FOREIGN_DESTROY_CALLS.load(Ordering::Relaxed), 0);

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(None);
        (*dear_imgui_rs::sys::igGetMainViewport()).RendererUserData =
            std::ptr::dangling_mut::<u8>().cast();
    }
    assert_eq!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::RendererShutdownRequired {
            field: "RendererUserData"
        })
    );
    assert_eq!(runtime.control().state(), RuntimeState::Attached);
    unsafe {
        (*dear_imgui_rs::sys::igGetMainViewport()).RendererUserData = std::ptr::null_mut();
    }
    runtime.shutdown(&mut context).unwrap();
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
        runtime.shutdown(&mut context),
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
        runtime.shutdown(&mut context),
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
fn runtime_ignores_foreign_callbacks_it_does_not_claim_after_they_change() {
    let _guard = lock_context();
    let mut context = Context::create();
    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_platform_set_window_alpha_raw(Some(foreign_set_window_alpha));
        platform_io.set_platform_get_window_work_area_insets_raw(Some(foreign_get_vec4));
        platform_io.set_platform_create_vk_surface_raw(Some(foreign_create_vk_surface));
    }

    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_platform_set_window_alpha_raw(Some(foreign_set_window_alpha_replacement));
        platform_io
            .set_platform_get_window_work_area_insets_raw(Some(foreign_get_vec4_replacement));
        platform_io.set_platform_create_vk_surface_raw(Some(foreign_create_vk_surface_replacement));
    }

    assert_eq!(
        runtime.route_secondary_event(&mut context, &Event::<()>::AboutToWait),
        Ok(false)
    );
    assert_eq!(runtime.poll_fault(), Ok(()));
    assert_eq!(runtime.shutdown(&mut context), Ok(()));

    let raw = unsafe { &*context.platform_io().as_raw() };
    assert!(std::ptr::fn_addr_eq(
        raw.Platform_SetWindowAlpha
            .expect("foreign alpha callback remains installed"),
        foreign_set_window_alpha_replacement
            as unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, f32)
    ));
    assert!(std::ptr::fn_addr_eq(
        raw.Platform_CreateVkSurface
            .expect("foreign Vulkan-surface callback remains installed"),
        foreign_create_vk_surface_replacement
            as unsafe extern "C" fn(
                *mut dear_imgui_rs::sys::ImGuiViewport,
                u64,
                *const c_void,
                *mut u64,
            ) -> i32
    ));

    // The runtime has stopped and this test exclusively owns each foreign callback.
    assert!(unsafe {
        context
            .platform_io_mut()
            .clear_platform_get_window_work_area_insets_if_raw_callback(
                foreign_get_vec4_replacement,
            )
    });
    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_platform_set_window_alpha_raw(None);
        platform_io.set_platform_create_vk_surface_raw(None);
    }
}

#[test]
fn runtime_locks_hidpi_configuration_without_state_changes() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let platform = runtime.owned_platform_for_test_mut();
    let mode = platform.hidpi_mode();
    let factor = platform.hidpi_factor();

    assert_eq!(
        platform.set_hidpi_mode(crate::HiDpiMode::Locked(2.0)),
        Err(WinitPlatformError::RuntimeConfigurationLocked)
    );
    // `attach_window` uses the same guard before it observes or updates the supplied window.
    assert_eq!(
        platform.ensure_runtime_configuration_mutable(),
        Err(WinitPlatformError::RuntimeConfigurationLocked)
    );
    assert_eq!(platform.hidpi_mode(), mode);
    assert_eq!(platform.hidpi_factor(), factor);
}

#[test]
fn platform_owner_rejects_reserved_foreign_flags_transactionally() {
    let _guard = lock_context();
    for flag in [
        BackendFlags::PLATFORM_HAS_VIEWPORTS,
        BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
    ] {
        let mut context = Context::create();
        let mut flags = context.io().backend_flags();
        flags.insert(flag);
        context.io_mut().set_backend_flags(flags);
        let before = snapshot_publication_state(&context);

        let error = match WinitPlatformRuntime::new_for_test(&mut context) {
            Ok(_) => panic!("an occupied platform capability must reject attachment"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            WinitPlatformError::PlatformStateOccupied {
                field: "BackendFlags"
            }
        );
        assert_publication_state_restored(&context, before);

        let mut flags = context.io().backend_flags();
        flags.remove(flag);
        context.io_mut().set_backend_flags(flags);
        let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
        runtime.shutdown(&mut context).unwrap();
        let mut expected = before;
        expected.backend_flags.remove(flag);
        assert_publication_state_restored(&context, expected);
    }
}

#[test]
fn explicit_shutdown_closes_an_open_frame_before_platform_teardown() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
        [320.0, 240.0],
        1.0 / 60.0,
    ));
    let _ = context.font_atlas().build();

    context.frame().text("close before Winit teardown");
    assert_eq!(
        context.frame_lifecycle_state(),
        dear_imgui_rs::FrameLifecycleState::InFrame
    );
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(
        context.frame_lifecycle_state(),
        dear_imgui_rs::FrameLifecycleState::Idle
    );

    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
        [320.0, 240.0],
        1.0 / 60.0,
    ));
    context.frame().text("context remains reusable");
    assert!(context.end_frame());
}

#[test]
fn shutdown_rejects_a_foreign_context_before_changing_runtime_state() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let suspended = context.suspend();
    let mut foreign = Context::create();

    assert_eq!(
        runtime.shutdown(&mut foreign),
        Err(WinitPlatformError::ContextMismatch)
    );
    assert_eq!(runtime.control().state(), RuntimeState::Attached);

    drop(foreign);
    let mut context = suspended.activate().unwrap();
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn wrapper_drop_defers_native_shutdown_to_context_attachment() {
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
    assert_eq!(control.state(), RuntimeState::Attached);
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
    );
    drop(context);
    assert_eq!(control.state(), RuntimeState::ContextDestroyed);
}

#[test]
fn dropping_the_external_platform_wrapper_keeps_runtime_teardown_context_owned() {
    let _guard = lock_context();
    let mut context = Context::create();
    let platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime =
        WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform).unwrap();
    let control = Rc::clone(runtime.control());

    // The runtime retains the Context-bound control. Dropping the public base wrapper must defer
    // the attachment instead of clearing callbacks or platform state out from under that runtime.
    drop(platform);
    assert_eq!(control.state(), RuntimeState::Attached);
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
    );

    drop(context);
    assert_eq!(control.state(), RuntimeState::ContextDestroyed);
    drop(runtime);
    assert_eq!(control.state(), RuntimeState::ContextDestroyed);
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
