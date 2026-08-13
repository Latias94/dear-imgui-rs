use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentRole,
    ContextPlatformAttachmentReleaseError,
};
use winit::event::{Event, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::WinitPlatformError;
use super::callbacks::{
    record_viewport_failure, run_callback, winit_create_window, winit_destroy_window,
    winit_get_window_pos_out,
};
use super::focus::{ContextFocusState, PlatformFocusState};
use super::registry::{ViewportIdentity, preflight_viewport_ownership};
use super::runtime::{
    ConstructionStage, InputOwnership, MouseLeaveState, RuntimeState, WinitPlatformRuntime,
    apply_raw_io_coordinate_contract_for_test,
};
use crate::test_util::test_sync::lock_context;

struct ActiveRendererMarker;
struct ActiveRendererAttachment;

impl ContextAttachment for ActiveRendererAttachment {}

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
fn coordinate_contract_transition_restores_metrics_and_invalidates_pointer_cache() {
    let _guard = lock_context();
    let context = Context::create();
    let io = unsafe { &mut *dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
    io.MousePos = dear_imgui_rs::sys::ImVec2 { x: 42.0, y: 27.0 };
    io.MouseHoveredViewport = 99;

    apply_raw_io_coordinate_contract_for_test(io, [800.0, 600.0], [1.5, 1.5]);

    assert_eq!(io.DisplaySize.x, 800.0);
    assert_eq!(io.DisplaySize.y, 600.0);
    assert_eq!(io.DisplayFramebufferScale.x, 1.5);
    assert_eq!(io.DisplayFramebufferScale.y, 1.5);
    assert_eq!(io.MousePos.x, -f32::MAX);
    assert_eq!(io.MousePos.y, -f32::MAX);
    assert_eq!(io.MouseHoveredViewport, 0);
}

#[test]
fn delayed_mouse_leave_invalidates_only_after_buttons_are_released() {
    let mut state = MouseLeaveState::default();

    state.note_cursor_left();
    assert!(state.take_invalidation_due());
    assert!(!state.take_invalidation_due());

    state.note_button(dear_imgui_rs::input::MouseButton::Left, true);
    state.note_cursor_left();
    assert!(!state.take_invalidation_due());
    state.note_button(dear_imgui_rs::input::MouseButton::Left, false);
    assert!(state.take_invalidation_due());
}

#[test]
fn entering_another_viewport_cancels_a_delayed_mouse_leave() {
    let mut state = MouseLeaveState::default();
    state.note_button(dear_imgui_rs::input::MouseButton::Left, true);
    state.note_cursor_left();

    state.note_cursor_available();
    state.note_button(dear_imgui_rs::input::MouseButton::Left, false);

    assert!(!state.take_invalidation_due());
}

#[test]
fn focus_transfer_between_owned_viewports_does_not_unfocus_the_context() {
    let first = WindowId::from(41_u64);
    let second = WindowId::from(42_u64);
    let mut state = ContextFocusState::default();

    assert!(state.note_window_focus(first, true));
    assert!(!state.note_window_focus(first, false));
    assert!(!state.note_window_focus(second, true));

    let owned = HashSet::from([first, second]);
    assert!(!state.reconcile_owned_windows(&owned, false));
}

#[test]
fn runtime_attached_while_unfocused_reports_loss_at_the_frame_boundary() {
    let mut state = ContextFocusState::with_focused_window(None);

    assert!(state.reconcile_owned_windows(&HashSet::new(), false));
    assert!(!state.reconcile_owned_windows(&HashSet::new(), false));
}

#[test]
fn losing_the_last_owned_viewport_focus_is_reconciled_once() {
    let window = WindowId::from(41_u64);
    let mut state = ContextFocusState::default();

    assert!(state.note_window_focus(window, true));
    assert!(!state.note_window_focus(window, false));

    let owned = HashSet::from([window]);
    assert!(state.reconcile_owned_windows(&owned, false));
    assert!(!state.reconcile_owned_windows(&owned, false));
}

#[test]
fn destroying_the_focused_viewport_reconciles_context_focus_loss() {
    let window = WindowId::from(41_u64);
    let mut state = ContextFocusState::default();

    assert!(state.note_window_focus(window, true));
    assert!(state.reconcile_owned_windows(&HashSet::new(), false));
}

#[test]
fn pending_platform_focus_overrides_stale_native_focus_until_confirmation() {
    let first = WindowId::from(41_u64);
    let second = WindowId::from(42_u64);
    let mut state = PlatformFocusState::default();
    let now = Instant::now();

    state.request(second, now);
    assert!(!state.effective_focus(now, first, true));
    assert!(state.effective_focus(now, second, false));

    state.note_native_event(false);
    assert!(state.effective_focus(now, second, false));

    state.note_native_event(true);
    assert!(!state.effective_focus(now, first, false));
    assert!(state.effective_focus(now, second, true));
}

#[test]
fn pending_platform_focus_retries_once_then_expires() {
    let window = WindowId::from(41_u64);
    let owned = HashSet::from([window]);
    let mut state = PlatformFocusState::default();
    let now = Instant::now();
    state.request(window, now);

    assert_eq!(state.advance(now, &owned), Some(window));
    assert!(state.effective_focus(now, window, false));
    assert_eq!(state.advance(now, &owned), None);
    let expired = now + Duration::from_secs(1);
    assert_eq!(state.advance(expired, &owned), None);
    assert!(!state.effective_focus(expired, window, false));
}

#[test]
fn native_focus_on_another_window_rejects_the_pending_target() {
    let first = WindowId::from(41_u64);
    let second = WindowId::from(42_u64);
    let mut state = PlatformFocusState::default();
    let now = Instant::now();
    state.request(second, now);

    state.note_native_event(true);

    assert!(state.effective_focus(now, first, true));
    assert!(!state.effective_focus(now, second, false));
}

#[test]
fn pending_platform_focus_keeps_context_focus_transfer_pending() {
    let first = WindowId::from(41_u64);
    let second = WindowId::from(42_u64);
    let mut platform = PlatformFocusState::default();
    let mut context = ContextFocusState::default();
    let now = Instant::now();

    assert!(context.note_window_focus(first, true));
    assert!(!context.note_window_focus(first, false));
    platform.request(second, now);
    let owned = HashSet::from([second]);
    let platform_focus_pending = platform.has_pending_for_owned_window(now, &owned);

    assert!(!context.reconcile_owned_windows(&owned, platform_focus_pending));
}

#[test]
fn context_focus_loss_invalidates_mouse_without_release_or_leave_events() {
    let mut state = MouseLeaveState::default();
    state.note_button(dear_imgui_rs::input::MouseButton::Left, true);
    assert!(!state.take_invalidation_due());

    state.note_context_focus_lost();

    assert!(state.take_invalidation_due());
    assert!(!state.take_invalidation_due());
}

#[test]
fn destroyed_viewport_releases_only_the_input_it_owns() {
    let first = WindowId::from(41_u64);
    let second = WindowId::from(42_u64);
    let mut ownership = InputOwnership::default();

    ownership.note_key(first, dear_imgui_rs::Key::A, true);
    ownership.note_key(second, dear_imgui_rs::Key::B, true);
    ownership.note_mouse_button(first, dear_imgui_rs::input::MouseButton::Left, true);
    ownership.note_mouse_button(second, dear_imgui_rs::input::MouseButton::Right, true);

    let released = ownership.retire_window(first, None);

    assert_eq!(released.keys, vec![dear_imgui_rs::Key::A]);
    assert_eq!(
        released.mouse_buttons,
        vec![dear_imgui_rs::input::MouseButton::Left]
    );
    assert_eq!(
        ownership.retire_window(second, None),
        super::runtime::ReleasedInput {
            keys: vec![dear_imgui_rs::Key::B],
            mouse_buttons: vec![dear_imgui_rs::input::MouseButton::Right],
            touch: false,
        }
    );
}

#[test]
fn latest_input_event_transfers_ownership_between_viewports() {
    let first = WindowId::from(41_u64);
    let second = WindowId::from(42_u64);
    let mut ownership = InputOwnership::default();

    ownership.note_key(first, dear_imgui_rs::Key::A, true);
    ownership.note_key(second, dear_imgui_rs::Key::A, true);
    ownership.note_mouse_button(first, dear_imgui_rs::input::MouseButton::Left, true);
    ownership.note_mouse_button(second, dear_imgui_rs::input::MouseButton::Left, true);

    assert_eq!(ownership.retire_window(first, None), Default::default());
    assert_eq!(
        ownership.retire_window(second, None),
        super::runtime::ReleasedInput {
            keys: vec![dear_imgui_rs::Key::A],
            mouse_buttons: vec![dear_imgui_rs::input::MouseButton::Left],
            touch: false,
        }
    );
}

#[test]
fn captured_mouse_buttons_follow_a_destroyed_viewport_to_the_main_window() {
    let viewport = WindowId::from(41_u64);
    let main = WindowId::from(42_u64);
    let mut ownership = InputOwnership::default();

    ownership.note_key(viewport, dear_imgui_rs::Key::A, true);
    ownership.note_mouse_button(viewport, dear_imgui_rs::input::MouseButton::Left, true);

    assert_eq!(
        ownership.retire_window(viewport, Some(main)),
        super::runtime::ReleasedInput {
            keys: vec![dear_imgui_rs::Key::A],
            mouse_buttons: Vec::new(),
            touch: false,
        }
    );
    assert_eq!(
        ownership.retire_window(main, None),
        super::runtime::ReleasedInput {
            keys: Vec::new(),
            mouse_buttons: vec![dear_imgui_rs::input::MouseButton::Left],
            touch: false,
        }
    );
}

#[test]
fn active_touch_follows_a_destroyed_viewport_handoff() {
    let viewport = WindowId::from(41_u64);
    let main = WindowId::from(42_u64);
    let mut ownership = InputOwnership::default();

    assert_eq!(
        ownership.note_touch(viewport, 7, winit::event::TouchPhase::Started),
        Some(crate::events::TouchAction::Press)
    );
    assert_eq!(
        ownership.retire_window(viewport, Some(main)),
        Default::default()
    );
    assert_eq!(
        ownership.retire_window(main, None),
        super::runtime::ReleasedInput {
            keys: Vec::new(),
            mouse_buttons: Vec::new(),
            touch: true,
        }
    );
}

#[test]
fn monitor_refresh_replaces_owned_storage_and_restores_the_prior_publication() {
    let _guard = lock_context();
    let mut context = Context::create();
    let before = snapshot_publication_state(&context);
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let previous_data = unsafe { (*context.platform_io().as_raw()).Monitors.Data };
    let mut replacement = valid_test_monitor();
    replacement.MainPos = dear_imgui_rs::sys::ImVec2 {
        x: -1600.0,
        y: 40.0,
    };
    replacement.WorkPos = replacement.MainPos;
    replacement.DpiScale = 1.5;

    assert_eq!(
        runtime
            .control()
            .refresh_monitors_for_test(&context, &[replacement]),
        Ok(true)
    );
    let monitors = unsafe { &(*context.platform_io().as_raw()).Monitors };
    assert_ne!(monitors.Data, previous_data);
    assert_eq!(monitors.Size, 1);
    assert_eq!(unsafe { (*monitors.Data).MainPos }, replacement.MainPos);
    assert_eq!(unsafe { (*monitors.Data).DpiScale }, 1.5);
    assert_eq!(
        runtime
            .control()
            .refresh_monitors_for_test(&context, &[replacement]),
        Ok(false)
    );

    runtime.shutdown(&mut context).unwrap();
    assert_publication_state_restored(&context, before);
}

#[test]
fn monitor_snapshot_refresh_tracks_work_and_provenance_without_partial_replacement() {
    use crate::multi_viewport::{WinitMonitorCollectionFailure, WinitMonitorPublicationState};
    use crate::native_support::{
        MonitorIdentity, MonitorSnapshot, PhysicalMonitorRect, WorkAreaProvenance,
    };

    let _guard = lock_context();
    let mut context = Context::create();
    let before = snapshot_publication_state(&context);
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let main = PhysicalMonitorRect::new([0.0, 0.0], [1920.0, 1080.0]).unwrap();
    let work = PhysicalMonitorRect::new([0.0, 40.0], [1920.0, 1040.0]).unwrap();
    let snapshot = MonitorSnapshot::from_test(
        MonitorIdentity::from_test_key("primary"),
        main,
        work,
        1.0,
        WorkAreaProvenance::WindowsRcWork,
    );

    assert_eq!(
        runtime
            .control()
            .refresh_monitor_snapshots_for_test(&context, Some(vec![snapshot.clone()])),
        Ok(true)
    );
    let first_data = unsafe { (*context.platform_io().as_raw()).Monitors.Data };
    assert_eq!(
        runtime
            .control()
            .refresh_monitor_snapshots_for_test(&context, Some(vec![snapshot.clone()])),
        Ok(false)
    );
    assert_eq!(
        unsafe { (*context.platform_io().as_raw()).Monitors.Data },
        first_data
    );

    let provenance_only = MonitorSnapshot::from_test(
        MonitorIdentity::from_test_key("primary"),
        main,
        work,
        1.0,
        WorkAreaProvenance::MacOsVisibleFrame,
    );
    assert_eq!(
        runtime
            .control()
            .refresh_monitor_snapshots_for_test(&context, Some(vec![provenance_only])),
        Ok(true)
    );
    let provenance_data = unsafe { (*context.platform_io().as_raw()).Monitors.Data };
    assert_ne!(provenance_data, first_data);

    let changed_work = PhysicalMonitorRect::new([0.0, 60.0], [1920.0, 1020.0]).unwrap();
    let work_only = MonitorSnapshot::from_test(
        MonitorIdentity::from_test_key("primary"),
        main,
        changed_work,
        1.0,
        WorkAreaProvenance::MacOsVisibleFrame,
    );
    let work_only_report = work_only.clone();
    let expected_work = super::coordinates::monitor_from_snapshot(&work_only).unwrap();
    assert_eq!(
        runtime
            .control()
            .refresh_monitor_snapshots_for_test(&context, Some(vec![work_only])),
        Ok(true)
    );
    let work_data = unsafe { (*context.platform_io().as_raw()).Monitors.Data };
    assert_ne!(work_data, provenance_data);
    let installed = unsafe { &*work_data };
    assert_eq!(installed.WorkPos, expected_work.WorkPos);
    assert_eq!(installed.WorkSize, expected_work.WorkSize);

    assert_eq!(
        runtime
            .control()
            .refresh_monitor_snapshots_for_test(&context, None),
        Ok(false)
    );
    assert_eq!(
        unsafe { (*context.platform_io().as_raw()).Monitors.Data },
        work_data
    );
    let report = runtime.control().monitor_publication_report().unwrap();
    assert_eq!(
        report.state(),
        WinitMonitorPublicationState::RetainedAfterCollectionFailure {
            reason: WinitMonitorCollectionFailure::Native(
                crate::native_support::MonitorCollectionError::MainFactsUnavailable { monitor: 0 },
            ),
        }
    );
    assert_eq!(report.snapshots().unwrap(), &[work_only_report]);

    let recovered = report.snapshots().unwrap()[0].clone();
    assert_eq!(
        runtime
            .control()
            .refresh_monitor_snapshots_for_test(&context, Some(vec![recovered.clone()])),
        Ok(false),
    );
    let recovered_report = runtime.control().monitor_publication_report().unwrap();
    assert_eq!(
        recovered_report.state(),
        WinitMonitorPublicationState::NativeSnapshot,
    );
    assert_eq!(recovered_report.snapshots().unwrap(), &[recovered]);

    runtime.shutdown(&mut context).unwrap();
    assert_publication_state_restored(&context, before);
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
        flag: "NoTaskBarIcon",
        operation: "window creation",
    };

    record_viewport_failure(runtime.control(), &mut viewport, error.clone());

    assert!(viewport.PlatformRequestClose);
    assert_eq!(runtime.poll_fault(), Err(error));
}

fn create_headless_secondary_viewport(
    context: &mut Context,
) -> *mut dear_imgui_rs::sys::ImGuiViewport {
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("the headless viewport test needs a built font atlas")
        .build();

    unsafe {
        context
            .platform_io_mut()
            .set_monitors(&[valid_test_monitor()]);
    }
    let _temporary_callbacks = super::callbacks::claim_platform_callbacks(context);
    let main_viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    assert!(!main_viewport.is_null());
    assert!(unsafe { (*main_viewport).PlatformUserData.is_null() });
    assert!(unsafe { (*main_viewport).PlatformHandle.is_null() });
    let temporary_handle = std::ptr::dangling_mut::<u8>().cast::<c_void>();
    unsafe { (*main_viewport).PlatformHandle = temporary_handle };

    let mut backend_flags = context.io().backend_flags();
    backend_flags
        .insert(BackendFlags::PLATFORM_HAS_VIEWPORTS | BackendFlags::RENDERER_HAS_VIEWPORTS);
    context.io_mut().set_backend_flags(backend_flags);
    context.enable_multi_viewport();
    context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
        [320.0, 240.0],
        1.0 / 60.0,
    ));
    context
        .frame()
        .window("Persistent failed viewport")
        .position([640.0, 480.0], dear_imgui_rs::Condition::Always)
        .size([160.0, 120.0], dear_imgui_rs::Condition::Always)
        .build(|| {});
    drop(context.render_legacy());

    unsafe {
        context.platform_io_mut().clear_platform_handlers();
        (*main_viewport).PlatformHandle = std::ptr::null_mut();
    }
    backend_flags.remove(BackendFlags::PLATFORM_HAS_VIEWPORTS);
    context.io_mut().set_backend_flags(backend_flags);

    let platform_io = context.platform_io().as_raw();
    unsafe {
        let viewports = &(*platform_io).Viewports;
        std::slice::from_raw_parts(viewports.Data, viewports.Size as usize)
            .iter()
            .copied()
            .find(|viewport| *viewport != main_viewport)
            .expect("the off-screen window should own a secondary viewport")
    }
}

#[test]
fn viewport_failure_survives_imgui_clearing_request_flags() {
    let _guard = lock_context();
    let mut context = Context::create();
    let secondary_viewport = create_headless_secondary_viewport(&mut context);
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();

    runtime
        .control()
        .enter_event_loop_pointer_for_test(std::ptr::null(), || unsafe {
            winit_create_window(secondary_viewport);
            assert!((*secondary_viewport).PlatformRequestClose);
            (*secondary_viewport).PlatformRequestClose = false;
        });

    assert!(unsafe { (*secondary_viewport).PlatformRequestClose });
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::EventLoopUnavailable)
    );
}

#[test]
fn destroying_a_viewport_clears_its_persistent_failure() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let mut viewport = dear_imgui_rs::sys::ImGuiViewport::default();

    record_viewport_failure(
        runtime.control(),
        &mut viewport,
        WinitPlatformError::EventLoopUnavailable,
    );
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::EventLoopUnavailable)
    );
    unsafe { winit_destroy_window(&mut viewport) };
    viewport.PlatformRequestClose = false;

    runtime
        .control()
        .enter_event_loop_pointer_for_test(std::ptr::null(), || {});

    assert!(!viewport.PlatformRequestClose);
}

#[test]
fn persistent_failure_does_not_cross_a_native_ownership_change() {
    let _guard = lock_context();
    let mut context = Context::create();
    let viewport = create_headless_secondary_viewport(&mut context);
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();

    record_viewport_failure(
        runtime.control(),
        viewport,
        WinitPlatformError::EventLoopUnavailable,
    );
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::EventLoopUnavailable)
    );
    unsafe {
        (*viewport).PlatformRequestClose = false;
        (*viewport).PlatformUserData = std::ptr::dangling_mut::<u8>().cast();
    }

    runtime
        .control()
        .enter_event_loop_pointer_for_test(std::ptr::null(), || {});
    unsafe {
        (*viewport).PlatformUserData = std::ptr::null_mut();
    }
    runtime
        .control()
        .enter_event_loop_pointer_for_test(std::ptr::null(), || {});

    assert!(!unsafe { (*viewport).PlatformRequestClose });
}

#[test]
fn persistent_failure_does_not_cross_a_viewport_id_generation_change() {
    let _guard = lock_context();
    let mut context = Context::create();
    let viewport = create_headless_secondary_viewport(&mut context);
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();

    record_viewport_failure(
        runtime.control(),
        viewport,
        WinitPlatformError::EventLoopUnavailable,
    );
    assert_eq!(
        runtime.poll_fault(),
        Err(WinitPlatformError::EventLoopUnavailable)
    );
    let original_id = unsafe { (*viewport).ID };
    unsafe {
        (*viewport).PlatformRequestClose = false;
        (*viewport).ID = original_id.wrapping_add(1);
    }

    runtime
        .control()
        .enter_event_loop_pointer_for_test(std::ptr::null(), || {});

    assert!(!unsafe { (*viewport).PlatformRequestClose });
    unsafe { (*viewport).ID = original_id };
}

#[test]
fn callback_fault_queue_drains_every_failure_in_observation_order() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let first = WinitPlatformError::WindowOperation {
        operation: "first callback",
        message: "first failure".to_owned(),
    };
    let second = WinitPlatformError::WindowOperation {
        operation: "second callback",
        message: "second failure".to_owned(),
    };

    runtime.control().record_fault(first.clone());
    runtime.control().record_fault(first.clone());
    runtime.control().record_fault(second.clone());

    assert_eq!(
        runtime.control().drain_faults(),
        vec![first.clone(), first, second]
    );
    assert_eq!(runtime.poll_fault(), Ok(()));
}

#[test]
fn terminal_fault_is_queued_once_and_remains_sticky() {
    let _guard = lock_context();
    let mut context = Context::create();
    let runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let first = WinitPlatformError::CallbackPanicked {
        callback: "first terminal callback",
    };
    let ignored = WinitPlatformError::CallbackPanicked {
        callback: "later terminal callback",
    };

    runtime.control().record_terminal_fault(first.clone());
    runtime.control().record_terminal_fault(ignored);

    assert_eq!(runtime.control().drain_faults(), vec![first.clone()]);
    assert_eq!(runtime.poll_fault(), Err(first));
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
fn direct_context_platform_teardown_detaches_the_winit_runtime_and_allows_reopen() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("test runtime should attach to the platform");

    context
        .destroy_platform_windows()
        .expect("the core transaction should release the attached Winit runtime");

    assert_eq!(runtime.control().state(), RuntimeState::Detached);
    assert!(!runtime.control().teardown_callbacks_active());
    assert!(runtime.control().platform_callback_contract().is_none());
    assert!(super::registry::runtime_for_context(context.as_raw()).is_none());
    let platform_io = unsafe { &*context.platform_io().as_raw() };
    assert!(platform_io.Platform_DestroyWindow.is_none());
    assert_eq!(
        runtime.route_secondary_event(&mut context, &Event::<()>::AboutToWait),
        Err(WinitPlatformError::RuntimeDetached)
    );

    let mut reopened = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("a core platform teardown should release the Winit runtime slot");
    reopened.shutdown(&mut context).unwrap();
    platform.shutdown(&mut context).unwrap();
}

#[test]
fn direct_context_platform_teardown_releases_the_runtime_slot_after_postflight_error() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("test runtime should attach to the platform");
    unsafe {
        context
            .platform_io_mut()
            .set_platform_show_window_raw(Some(foreign_unary));
    }

    let error = context
        .destroy_platform_windows()
        .expect_err("postflight callback drift should be reported after native teardown");
    assert!(matches!(
        error,
        dear_imgui_rs::ContextPlatformWindowTeardownError::AttachmentPostflight(_)
    ));
    assert_eq!(runtime.control().state(), RuntimeState::Detached);
    assert!(super::registry::runtime_for_context(context.as_raw()).is_none());

    unsafe { context.platform_io_mut().set_platform_show_window_raw(None) };
    let mut reopened = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("a postflight failure must not retain the detached runtime slot");
    reopened.shutdown(&mut context).unwrap();
    platform.shutdown(&mut context).unwrap();
}

#[test]
fn renderer_owner_validation_rejects_a_shutdown_runtime_with_the_same_context_id() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();

    assert_eq!(runtime.validate_renderer_owner(&context), Ok(()));
    runtime.shutdown(&mut context).unwrap();

    assert_eq!(
        runtime.validate_renderer_owner(&context),
        Err(WinitPlatformError::RuntimeDetached)
    );
}

#[test]
fn public_platform_owner_drives_the_installed_viewport_runtime() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("test runtime should attach to the platform owner");

    assert!(platform.viewports_enabled());
    assert_eq!(platform.context_id(), context.id());
    assert_eq!(platform.drain_viewport_faults(), Ok(Vec::new()));
    assert!(platform.viewport_renderer_adapter(&context).is_ok());

    drop(runtime);
    platform.disable_viewports(&mut context).unwrap();
    assert!(!platform.viewports_enabled());
    platform.shutdown(&mut context).unwrap();
}

#[test]
fn public_platform_shutdown_returns_retryable_faults_before_detaching() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("test runtime should attach to the platform owner");
    runtime
        .control()
        .record_fault(WinitPlatformError::CallbackPanicked {
            callback: "first queued shutdown fault",
        });
    runtime
        .control()
        .record_fault(WinitPlatformError::CallbackPanicked {
            callback: "second queued shutdown fault",
        });

    assert!(matches!(
        platform.disable_viewports(&mut context),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "first queued shutdown fault"
        })
    ));
    assert!(platform.viewports_enabled());
    assert!(matches!(
        platform.disable_viewports(&mut context),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "second queued shutdown fault"
        })
    ));
    assert!(platform.viewports_enabled());

    platform.disable_viewports(&mut context).unwrap();
    assert!(!platform.viewports_enabled());
    platform.disable_viewports(&mut context).unwrap();
    platform.shutdown(&mut context).unwrap();
    drop(runtime);
}

#[test]
fn public_platform_owner_rejects_a_foreign_context_without_detaching() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("test runtime should attach to the platform owner");
    let suspended = context.suspend_or_panic();
    let mut foreign = Context::create();

    assert_eq!(
        platform.disable_viewports(&mut foreign),
        Err(WinitPlatformError::ContextMismatch)
    );
    assert!(platform.viewports_enabled());

    drop(foreign);
    let mut context = suspended.activate().unwrap();
    platform.disable_viewports(&mut context).unwrap();
    platform.shutdown(&mut context).unwrap();
    drop(runtime);
}

#[test]
fn full_event_route_rejects_a_foreign_context_before_consuming_faults() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("test runtime should attach to the platform owner");
    runtime
        .control()
        .record_fault(WinitPlatformError::CallbackPanicked {
            callback: "queued before foreign full event",
        });
    let suspended = context.suspend_or_panic();
    let mut foreign = Context::create();
    let foreign_flags = foreign.io().backend_flags();

    assert!(matches!(
        super::events::handle_event(
            runtime.control(),
            &mut platform,
            &mut foreign,
            &Event::<()>::AboutToWait,
        ),
        Err(WinitPlatformError::ContextMismatch)
    ));
    assert_eq!(foreign.io().backend_flags(), foreign_flags);
    assert!(matches!(
        runtime.poll_fault(),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "queued before foreign full event"
        })
    ));

    drop(foreign);
    let mut context = suspended.activate().unwrap();
    platform.disable_viewports(&mut context).unwrap();
    platform.shutdown(&mut context).unwrap();
    drop(runtime);
}

#[test]
fn secondary_window_route_rejects_a_foreign_context_before_consuming_faults() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut platform = crate::WinitPlatform::new(&mut context).unwrap();
    let runtime = WinitPlatformRuntime::new_for_test_with_platform(&mut context, &platform)
        .expect("test runtime should attach to the platform owner");
    runtime
        .control()
        .record_fault(WinitPlatformError::CallbackPanicked {
            callback: "queued before foreign window event",
        });
    let suspended = context.suspend_or_panic();
    let mut foreign = Context::create();
    let foreign_flags = foreign.io().backend_flags();

    assert!(matches!(
        super::events::route_secondary_window_event(
            runtime.control(),
            &mut foreign,
            WindowId::dummy(),
            &WindowEvent::CloseRequested,
        ),
        Err(WinitPlatformError::ContextMismatch)
    ));
    assert_eq!(foreign.io().backend_flags(), foreign_flags);
    assert!(matches!(
        runtime.poll_fault(),
        Err(WinitPlatformError::CallbackPanicked {
            callback: "queued before foreign window event"
        })
    ));

    drop(foreign);
    let mut context = suspended.activate().unwrap();
    platform.disable_viewports(&mut context).unwrap();
    platform.shutdown(&mut context).unwrap();
    drop(runtime);
}

#[test]
fn direct_context_platform_teardown_rejects_renderer_state_before_native_destroy() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    FOREIGN_DESTROY_CALLS.store(0, Ordering::Relaxed);
    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(Some(foreign_destroy));
    }

    let error = context
        .destroy_platform_windows()
        .expect_err("renderer callbacks must reject core platform teardown before native destroy");
    assert!(matches!(
        error,
        dear_imgui_rs::ContextPlatformWindowTeardownError::AttachmentPreflight(_)
    ));
    assert_eq!(runtime.control().state(), RuntimeState::Attached);
    assert!(!runtime.control().teardown_callbacks_active());
    assert_eq!(FOREIGN_DESTROY_CALLS.load(Ordering::Relaxed), 0);

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(None);
    }
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
fn viewport_identity_follows_a_live_viewport_when_docking_changes_its_id() {
    let _guard = lock_context();
    let context = Context::create();
    let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let original_id = unsafe { (*viewport).ID };
    let identity = ViewportIdentity::capture(context.as_raw(), viewport);
    unsafe { (*viewport).ID = original_id.wrapping_add(1) };

    let resolved_after_id_change = unsafe { identity.resolve() };
    unsafe { (*viewport).ID = original_id };

    assert_eq!(resolved_after_id_change, Some(viewport));
    drop(context);
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
fn runtime_shutdown_rejects_an_active_renderer_attachment_before_frame_or_native_mutation() {
    let _guard = lock_context();
    let mut context = Context::create();
    let mut runtime = WinitPlatformRuntime::new_for_test(&mut context).unwrap();
    let mut renderer = context
        .register_attachment::<ActiveRendererMarker>(
            ContextAttachmentRole::Renderer,
            Rc::new(ActiveRendererAttachment),
        )
        .unwrap();

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(WinitPlatformError::PlatformAttachmentRelease(
            ContextPlatformAttachmentReleaseError::RendererActive
        ))
    ));
    assert_eq!(runtime.control().state(), RuntimeState::Attached);
    assert!(runtime.validate_renderer_owner(&context).is_ok());

    assert_eq!(renderer.detach(), Ok(true));
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
    ]
    .into_iter()
    .filter(|flag| crate::platform::WINIT_VIEWPORT_FLAGS.contains(*flag))
    {
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
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("the platform teardown test uses headless legacy rendering")
        .build();

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
    let suspended = context.suspend_or_panic();
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
