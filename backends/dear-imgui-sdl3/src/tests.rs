use super::*;
use std::sync::{Mutex, MutexGuard};

static TEST_MUTEX: Mutex<()> = Mutex::new(());

pub(crate) fn test_guard() -> MutexGuard<'static, ()> {
    TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn backend_error_display_is_stable() {
    let _guard = test_guard();
    assert_eq!(
        Sdl3BackendError::InvalidGlslVersion.to_string(),
        "Invalid GLSL version string"
    );
}

#[test]
fn opengl_viewport_swap_interval_policies_have_stable_native_values() {
    assert_eq!(
        Sdl3OpenGlViewportSwapInterval::Immediate.native_policy(),
        (0, 0)
    );
    assert_eq!(
        Sdl3OpenGlViewportSwapInterval::VSync.native_policy(),
        (0, 1)
    );
    assert_eq!(
        Sdl3OpenGlViewportSwapInterval::Adaptive.native_policy(),
        (0, -1)
    );
    assert_eq!(
        Sdl3OpenGlViewportSwapInterval::MatchMain.native_policy(),
        (1, 0)
    );
}

#[test]
fn cpp_backend_uses_the_generated_imgui_io_layout() {
    let _guard = test_guard();
    let cpp_size = unsafe { ffi::dear_imgui_sdl3_backend_sizeof_imgui_io() };

    assert_eq!(
        cpp_size,
        std::mem::size_of::<dear_imgui_sys::ImGuiIO>(),
        "the SDL3 C++ backend and generated Rust bindings must use identical Dear ImGui defines"
    );
}

#[test]
#[cfg(debug_assertions)]
fn native_viewport_transactions_fail_closed_and_restore_state() {
    let _guard = test_guard();
    let failed_scenarios = unsafe { ffi::dear_imgui_sdl3_native_contract_self_test() };

    assert_eq!(
        failed_scenarios, 0,
        "SDL3 native viewport transaction scenarios failed: {failed_scenarios:#x}"
    );
}

#[test]
fn delayed_mouse_leave_remains_due_after_its_target_frame() {
    let _guard = test_guard();
    unsafe {
        assert!(!ffi::dear_imgui_sdl3_mouse_leave_due_for_test(11, 10, 0));
        assert!(ffi::dear_imgui_sdl3_mouse_leave_due_for_test(11, 11, 0));
        assert!(ffi::dear_imgui_sdl3_mouse_leave_due_for_test(11, 14, 0));
        assert!(!ffi::dear_imgui_sdl3_mouse_leave_due_for_test(11, 14, 1));
        assert!(!ffi::dear_imgui_sdl3_mouse_leave_due_for_test(0, 14, 0));
    }
}
