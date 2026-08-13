use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn create_context() -> imgui::Context {
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
    ctx
}

#[test]
fn misc_query_helpers_no_panic() {
    let _guard = test_guard();

    let mut ctx = create_context();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();
    let _ = ui.window("Queries").build(|| {
        let _ = ui.window_viewport();
        let _ = ui.tree_node_to_label_spacing();

        let _ = ui.button("Btn");
        let _ = ui.item_id();
        let _ = ui.is_item_edited();
    });
}

#[test]
fn window_viewport_uses_the_implicit_fallback_window_after_new_frame() {
    let _guard = test_guard();
    let mut ctx = create_context();

    let ui = ctx.frame();

    assert_eq!(ui.window_viewport().id(), ui.main_viewport().id());
}

#[test]
fn window_viewport_restores_the_implicit_fallback_after_an_explicit_window() {
    let _guard = test_guard();
    let mut ctx = create_context();

    let ui = ctx.frame();
    let fallback_viewport = ui.window_viewport().id();
    let main_viewport = ui.main_viewport().id();
    ui.set_next_window_viewport(main_viewport);

    ui.window("Explicit viewport query").build(|| {
        assert_eq!(ui.window_viewport().id(), main_viewport);
    });

    assert_eq!(ui.window_viewport().id(), fallback_viewport);
}
