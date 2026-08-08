use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

struct CurrentContextRestore(*mut imgui::sys::ImGuiContext);

impl Drop for CurrentContextRestore {
    fn drop(&mut self) {
        unsafe { imgui::sys::igSetCurrentContext(self.0) }
    }
}

#[test]
fn state_storage_helpers_no_panic() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();

    // Access current window storage (requires an active window).
    let _ = ui.window("A").build(|| {
        let key = ui.get_id("k");
        ui.with_current_state_storage(|mut storage| {
            storage.set_int(key, 123);
            assert_eq!(storage.get_int(key, 0), 123);
            storage.set_bool(key, true);
            assert!(storage.get_bool(key, false));
            storage.set_float(key, 1.5);
            assert!((storage.get_float(key, 0.0) - 1.5).abs() < 1e-6);
        });

        // Override storage with an owned instance.
        let mut owned = imgui::OwnedStateStorage::new();
        let replacement = owned.as_raw_mut();
        ui.with_state_storage(&mut owned, |storage| {
            assert_eq!(storage.as_raw(), replacement);
            ui.set_next_item_storage_id(ui.get_id("item"));
            let _ = ui.button("B");
        });
    });
}

#[test]
fn nested_state_storage_overrides_restore_lifo() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    ui.window("storage restore").build(|| {
        let original = ui.with_current_state_storage(|storage| storage.as_raw());
        let mut outer = imgui::OwnedStateStorage::new();
        let mut inner = imgui::OwnedStateStorage::new();
        let outer_ptr = outer.as_raw_mut();
        let inner_ptr = inner.as_raw_mut();

        ui.with_state_storage(&mut outer, |outer_storage| {
            assert_eq!(outer_storage.as_raw(), outer_ptr);
            assert_eq!(
                ui.with_current_state_storage(|storage| storage.as_raw()),
                outer_ptr
            );

            ui.with_state_storage(&mut inner, |inner_storage| {
                assert_eq!(inner_storage.as_raw(), inner_ptr);
                assert_eq!(
                    ui.with_current_state_storage(|storage| storage.as_raw()),
                    inner_ptr
                );
            });

            assert_eq!(
                ui.with_current_state_storage(|storage| storage.as_raw()),
                outer_ptr
            );
        });

        assert_eq!(
            ui.with_current_state_storage(|storage| storage.as_raw()),
            original
        );
    });
}

#[test]
fn nested_state_storage_overrides_restore_after_panic() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    ui.window("panic restore").build(|| {
        let original = ui.with_current_state_storage(|storage| storage.as_raw());
        let mut outer = imgui::OwnedStateStorage::new();
        let mut inner = imgui::OwnedStateStorage::new();
        let outer_ptr = outer.as_raw_mut();

        ui.with_state_storage(&mut outer, |_| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.with_state_storage(&mut inner, |_| {
                    panic!("forced panic in nested state storage override");
                });
            }));
            assert!(result.is_err());
            assert_eq!(
                ui.with_current_state_storage(|storage| storage.as_raw()),
                outer_ptr
            );
        });

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.with_state_storage(&mut outer, |_| {
                ui.with_state_storage(&mut inner, |_| {
                    panic!("forced panic through both state storage overrides");
                });
            });
        }));
        assert!(result.is_err());
        assert_eq!(
            ui.with_current_state_storage(|storage| storage.as_raw()),
            original
        );
    });
}

#[test]
fn state_storage_override_binds_its_owner_context() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    ui.window("context binding").build(|| {
        let owner = unsafe { imgui::sys::igGetCurrentContext() };
        assert!(!owner.is_null());

        unsafe { imgui::sys::igSetCurrentContext(std::ptr::null_mut()) };
        let restore_owner = CurrentContextRestore(owner);
        let mut storage = imgui::OwnedStateStorage::new();

        ui.with_state_storage(&mut storage, |_| {
            assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, owner);
        });
        assert!(unsafe { imgui::sys::igGetCurrentContext() }.is_null());

        drop(restore_owner);
    });
}
