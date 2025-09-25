use once_cell::sync::Lazy;
use std::sync::Mutex;

static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[test]
fn owned_draw_data_survives_context_drop() {
    let _guard = TEST_MUTEX.lock().unwrap();

    let mut ctx = dear_imgui::Context::create();
    // Minimal IO setup required by Dear ImGui assertions
    ctx.io_mut().set_display_size([1.0, 1.0]);
    // Avoid font atlas assertions by signaling renderer supports textures
    {
        use dear_imgui::BackendFlags;
        let flags = ctx.io().backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES;
        ctx.io_mut().set_backend_flags(flags);
    }

    // Build a minimal frame (no draw calls needed for this test)
    {
        let ui = ctx.frame();
        let _ = ui; // keep scope for frame
    }
    let draw_data = ctx.render();
    let _ = draw_data.draw_lists_count();

    // Deep copy
    let owned: dear_imgui::render::draw_data::OwnedDrawData = draw_data.into();

    // Destroy original context; owned data must remain valid
    drop(ctx);

    let dd = owned.draw_data().expect("owned draw data should be present");
    // Access a couple of fields to ensure memory is still valid
    let _ = dd.draw_lists_count();
}

#[test]
fn owned_draw_data_can_move_to_thread() {
    let _guard = TEST_MUTEX.lock().unwrap();

    let mut ctx = dear_imgui::Context::create();
    // Minimal IO setup required by Dear ImGui assertions
    ctx.io_mut().set_display_size([1.0, 1.0]);
    // Avoid font atlas assertions by signaling renderer supports textures
    {
        use dear_imgui::BackendFlags;
        let flags = ctx.io().backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES;
        ctx.io_mut().set_backend_flags(flags);
    }

    // Build a minimal frame (no draw calls needed for this test)
    {
        let ui = ctx.frame();
        let _ = ui;
    }
    let draw_data = ctx.render();
    let owned: dear_imgui::render::draw_data::OwnedDrawData = draw_data.into();

    // Drop context before moving across threads
    drop(ctx);

    std::thread::spawn(move || {
        let dd = owned.draw_data().expect("owned draw data should be present");
        let _ = dd.draw_lists_count();
    })
    .join()
    .expect("thread joined");
}
