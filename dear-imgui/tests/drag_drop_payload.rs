use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn drag_drop_payload_query_no_panic() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();

    // No drag operation in this headless test, but the query should be safe.
    assert!(ui.drag_drop_payload().is_none());

    struct PayloadName<'a>(&'a std::cell::Cell<usize>);
    impl AsRef<str> for PayloadName<'_> {
        fn as_ref(&self) -> &str {
            self.0.set(self.0.get() + 1);
            "APP_ITEM"
        }
    }

    let conversions = std::cell::Cell::new(0);
    let source = ui
        .drag_drop_source_config(PayloadName(&conversions))
        .flags(imgui::DragDropSourceFlags::EXTERN | imgui::DragDropSourceFlags::NO_PREVIEW_TOOLTIP)
        .begin_payload(42_u32)
        .expect("external source");
    assert_eq!(conversions.get(), 1);
    assert!(ui.drag_drop_payload().is_some());
    source.end();
}
