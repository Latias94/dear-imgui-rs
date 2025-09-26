use dear_imgui as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn input_text_growable_buffer_no_panic() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    // Build font atlas so text widgets don't assert
    let _ = ctx.font_atlas_mut().build();
    // No ini persistence to avoid filesystem
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    // Long initial string to exercise our owned buffer path
    let mut text = String::from_iter(std::iter::repeat('a').take(2048));

    let ui = ctx.frame();
    let _ = ui.input_text("LongText", &mut text).build();
    // ImString variant
    let mut im = imgui::ImString::new("hello");
    let _ = ui.input_text_imstr("ImStr", &mut im).build();
    // Ensure we can render without crashes
    // No render required in headless tests
}

#[test]
fn input_text_multiline_growable_buffer_no_panic() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let mut text = String::from_iter(std::iter::repeat('b').take(8192));

    let ui = ctx.frame();
    let _ = ui
        .input_text_multiline("LongMultiline", &mut text, [300.0, 120.0])
        .build();
    // ImString variant
    let mut im = imgui::ImString::new(String::from_iter(std::iter::repeat('c').take(4096)));
    let _ = ui
        .input_text_multiline_imstr("ImStrMultiline", &mut im, [300.0, 120.0])
        .build();
    // No render required in headless tests
}

#[test]
fn input_scalars_build_no_panic() {
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

    let mut i = 42i32;
    let mut f = 3.14f32;
    let mut d = 2.71828f64;
    let mut i2 = [1i32, 2];
    let mut i3 = [3i32, 4, 5];
    let mut f2 = [1.0f32, 2.0];
    let mut f3 = [3.0f32, 4.0, 5.0];

    let _ = ui.input_int("int", &mut i);
    let _ = ui.input_float("float", &mut f);
    let _ = ui.input_double("double", &mut d);
    let _ = ui.input_int2("int2", &mut i2).build();
    let _ = ui.input_int3("int3", &mut i3).build();
    let _ = ui.input_float2("float2", &mut f2).build();
    let _ = ui.input_float3("float3", &mut f3).build();

    // No render required in headless tests
}
