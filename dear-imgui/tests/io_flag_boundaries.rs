use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn first_unknown_bit(known_bits: i32) -> i32 {
    (0..31)
        .map(|shift| 1_i32 << shift)
        .find(|candidate| known_bits & candidate == 0)
        .expect("test requires at least one spare positive flag bit")
}

#[test]
fn io_flag_types_include_current_public_upstream_bits() {
    assert_eq!(
        imgui::ConfigFlags::NO_KEYBOARD.bits(),
        imgui::sys::ImGuiConfigFlags_NoKeyboard
    );
    assert_eq!(
        imgui::BackendFlags::HAS_PARENT_VIEWPORT.bits(),
        imgui::sys::ImGuiBackendFlags_HasParentViewport
    );
}

#[test]
fn io_flag_setters_reject_unsupported_bits_before_storing_them() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    let io = ctx.io_mut();

    let config_flags = imgui::ConfigFlags::NAV_ENABLE_KEYBOARD | imgui::ConfigFlags::NO_KEYBOARD;
    io.set_config_flags(config_flags);
    assert_eq!(io.config_flags(), config_flags);

    let backend_flags =
        imgui::BackendFlags::RENDERER_HAS_TEXTURES | imgui::BackendFlags::HAS_PARENT_VIEWPORT;
    io.set_backend_flags(backend_flags);
    assert_eq!(io.backend_flags(), backend_flags);

    let unsupported_config = imgui::ConfigFlags::from_bits_retain(1 << 2);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            io.set_config_flags(unsupported_config);
        }))
        .is_err()
    );
    assert_eq!(io.config_flags(), config_flags);

    let unsupported_backend = imgui::BackendFlags::from_bits_retain(1 << 9);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            io.set_backend_flags(unsupported_backend);
        }))
        .is_err()
    );
    assert_eq!(io.backend_flags(), backend_flags);
}

#[test]
fn io_flag_getters_preserve_unknown_raw_bits() {
    let _guard = test_guard();

    let ctx = imgui::Context::create();
    let config_unknown = first_unknown_bit(imgui::ConfigFlags::all().bits());
    let backend_unknown = first_unknown_bit(imgui::BackendFlags::all().bits());

    unsafe {
        let io = imgui::sys::igGetIO_ContextPtr(ctx.as_raw());
        (*io).ConfigFlags = imgui::ConfigFlags::NO_KEYBOARD.bits() | config_unknown;
        (*io).BackendFlags = imgui::BackendFlags::RENDERER_HAS_TEXTURES.bits() | backend_unknown;
    }

    let io = ctx.io();
    assert_eq!(
        io.config_flags().bits(),
        imgui::ConfigFlags::NO_KEYBOARD.bits() | config_unknown
    );
    assert_eq!(
        io.backend_flags().bits(),
        imgui::BackendFlags::RENDERER_HAS_TEXTURES.bits() | backend_unknown
    );
}
