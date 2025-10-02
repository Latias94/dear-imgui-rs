use once_cell::sync::Lazy;
use std::sync::Mutex;

static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[test]
fn owned_draw_data_survives_context_drop() {
    let _guard = TEST_MUTEX.lock().unwrap();

    let mut ctx = dear_imgui_rs::Context::create();
    // Minimal IO setup required by Dear ImGui assertions
    ctx.io_mut().set_display_size([1.0, 1.0]);
    // Avoid font atlas assertions by signaling renderer supports textures
    {
        use dear_imgui_rs::BackendFlags;
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
    let owned: dear_imgui_rs::render::draw_data::OwnedDrawData = draw_data.into();

    // Destroy original context; owned data must remain valid
    drop(ctx);

    let dd = owned
        .draw_data()
        .expect("owned draw data should be present");
    // Access a couple of fields to ensure memory is still valid
    let _ = dd.draw_lists_count();
}

// NOTE: OwnedDrawData intentionally does NOT implement Send/Sync (see implementation docs).
// Thread-safety guarantees are covered by thread_safety.rs using static assertions.
