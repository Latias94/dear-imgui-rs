use dear_imgui_reflect as reflect;
use dear_imgui_reflect::imgui::Context;
use reflect::ImGuiReflect;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

/// Simple struct to exercise container-level events with stable field paths.
#[derive(ImGuiReflect, Default)]
struct ResponseDemo {
    samples: Vec<i32>,
    offsets: [i32; 3],
    map: HashMap<String, i32>,
}

#[test]
fn reflect_response_tracks_container_events_with_paths() {
    let _guard = test_guard();

    let mut ctx = Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();

    let mut demo = ResponseDemo::default();
    let mut resp = reflect::ReflectResponse::default();

    // First pass: no changes should produce no events.
    let _changed = reflect::input_with_response(&ui, "ResponseDemo", &mut demo, &mut resp);
    assert!(resp.is_empty());

    // Mutate the data before the next frame so that container editors have
    // something to operate on.
    demo.samples.extend_from_slice(&[1, 2]);
    demo.offsets = [0, 1, 2];
    demo.map.insert("a".to_owned(), 1);

    // End the first frame to satisfy Dear ImGui's frame lifecycle assertions.
    ctx.render();

    let ui = ctx.frame();
    resp.clear();

    // Second pass: containers have elements; we still don't simulate clicks,
    // but this ensures that simply reflecting does not spuriously emit events.
    let _changed = reflect::input_with_response(&ui, "ResponseDemo", &mut demo, &mut resp);
    assert!(resp.is_empty());
}
