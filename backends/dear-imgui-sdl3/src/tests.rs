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
fn cpp_backend_uses_the_generated_imgui_io_layout() {
    let _guard = test_guard();
    let cpp_size = unsafe { ffi::dear_imgui_sdl3_backend_sizeof_imgui_io() };

    assert_eq!(
        cpp_size,
        std::mem::size_of::<dear_imgui_sys::ImGuiIO>(),
        "the SDL3 C++ backend and generated Rust bindings must use identical Dear ImGui defines"
    );
}
