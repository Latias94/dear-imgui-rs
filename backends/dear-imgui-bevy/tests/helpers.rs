use dear_imgui_bevy::{ImguiContext, configure_example_context};
use dear_imgui_rs::ConfigFlags;
use std::sync::{Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn configure_example_context_applies_shared_example_settings() {
    let _guard = imgui_context_guard();

    let mut imgui = ImguiContext::new(dear_imgui_rs::Context::create());

    configure_example_context(&mut imgui, true);
    {
        let io = imgui.context().io();
        assert!(!io.config_input_trickle_event_queue());
        assert!(io.config_flags().contains(ConfigFlags::DOCKING_ENABLE));
        assert!(io.ini_filename().is_none());
    }

    configure_example_context(&mut imgui, false);
    let io = imgui.context().io();
    assert!(!io.config_flags().contains(ConfigFlags::DOCKING_ENABLE));
    assert!(io.ini_filename().is_none());
}
