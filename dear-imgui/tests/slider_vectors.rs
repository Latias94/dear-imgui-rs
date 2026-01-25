use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn slider_vector_helpers_no_panic() {
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
    let mut f2 = [0.0f32, 1.0];
    let mut f3 = [0.0f32, 1.0, 2.0];
    let mut f4 = [0.0f32, 1.0, 2.0, 3.0];
    let mut i2 = [0i32, 1];
    let mut i3 = [0i32, 1, 2];
    let mut i4 = [0i32, 1, 2, 3];

    let _ = ui.window("Sliders").build(|| {
        let _ = ui.slider_float2("f2", &mut f2, 0.0, 10.0);
        let _ = ui.slider_float3("f3", &mut f3, 0.0, 10.0);
        let _ = ui.slider_float4("f4", &mut f4, 0.0, 10.0);

        let _ = ui.slider_int2("i2", &mut i2, 0, 10);
        let _ = ui.slider_int3("i3", &mut i3, 0, 10);
        let _ = ui.slider_int4("i4", &mut i4, 0, 10);
    });
}
