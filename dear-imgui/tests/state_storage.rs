use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn state_storage_helpers_no_panic() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();

    // Access current window storage (requires an active window).
    let _ = ui.window("A").build(|| {
        let mut storage = ui.state_storage();
        let key = ui.get_id("k");
        storage.set_int(key, 123);
        assert_eq!(storage.get_int(key, 0), 123);
        storage.set_bool(key, true);
        assert!(storage.get_bool(key, false));
        storage.set_float(key, 1.5);
        assert!((storage.get_float(key, 0.0) - 1.5).abs() < 1e-6);
    });

    // Override storage with an owned instance.
    let mut owned = imgui::OwnedStateStorage::new();
    let _tok = ui.push_state_storage(owned.as_mut());
    ui.set_next_item_storage_id(ui.get_id("item"));
    let _ = ui.button("B");
}
