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

#[test]
fn scalar_array_widgets_reject_invalid_component_counts() {
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
    let mut empty: [f32; 0] = [];
    let mut five = [0.0f32; 5];

    let _ = ui.window("Invalid scalar arrays").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("empty_slider", 0.0f32, 1.0)
                    .build_array(&mut empty);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.input_scalar_n("empty_input", &mut empty).build();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = imgui::Drag::<f32, _>::new("empty_drag").build_array(ui, &mut empty);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("marker_slider", 0.0f32, 1.0)
                    .flags(imgui::SliderFlags::COLOR_MARKERS)
                    .build_array(&mut five);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = imgui::Drag::<f32, _>::new("marker_drag")
                    .flags(imgui::DragFlags::COLOR_MARKERS)
                    .build_array(ui, &mut five);
            }))
            .is_err()
        );
    });
}

#[test]
fn slider_widgets_reject_unsupported_flags_and_ranges() {
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
    let wrap_around = imgui::SliderFlags::from_bits_retain(imgui::sys::ImGuiSliderFlags_WrapAround);
    let legacy_power = imgui::SliderFlags::from_bits_retain(1);
    let private_read_only =
        imgui::SliderFlags::from_bits_retain(imgui::sys::ImGuiSliderFlags_ReadOnly);
    let mut i32_value = 0i32;
    let mut u32_value = 0u32;
    let mut i64_value = 0i64;
    let mut u64_value = 0u64;
    let mut f32_value = 0.0f32;
    let mut f64_value = 0.0f64;
    let mut f32_pair = [0.0f32, 1.0];
    let mut angle = 0.0f32;

    let _ = ui.window("Invalid sliders").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("wrap_slider", 0.0f32, 1.0)
                    .flags(wrap_around)
                    .build(&mut f32_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("legacy_slider", 0.0f32, 1.0)
                    .flags(legacy_power)
                    .build(&mut f32_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("private_slider", 0.0f32, 1.0)
                    .flags(private_read_only)
                    .build(&mut f32_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("array_range_slider", -f32::MAX, f32::MAX)
                    .build_array(&mut f32_pair);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = imgui::VerticalSlider::new(
                    "vertical_range_slider",
                    [20.0, 100.0],
                    i32::MIN,
                    i32::MAX,
                )
                .build(ui, &mut i32_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("reversed_extreme_slider", i32::MAX, i32::MIN)
                    .build(&mut i32_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = imgui::AngleSlider::new("angle_wrap_slider")
                    .flags(wrap_around)
                    .build(ui, &mut angle);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = imgui::AngleSlider::new("angle_nan_slider")
                    .range_degrees(f32::NAN, 360.0)
                    .build(ui, &mut angle);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("u32_range_slider", 0u32, u32::MAX)
                    .build(&mut u32_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("i64_range_slider", i64::MIN, i64::MAX)
                    .build(&mut i64_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("u64_range_slider", 0u64, u64::MAX)
                    .build(&mut u64_value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .slider_config("f64_range_slider", -f64::MAX, f64::MAX)
                    .build(&mut f64_value);
            }))
            .is_err()
        );
    });
}

#[test]
fn drag_widgets_reject_unsupported_flags() {
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
    let unsupported = imgui::DragFlags::from_bits_retain(imgui::sys::ImGuiSliderFlags_ReadOnly);
    let mut value = 0.0f32;
    let mut values = [0.0f32, 1.0];
    let mut min = 0.0f32;
    let mut max = 1.0f32;

    let _ = ui.window("Invalid drags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = imgui::Drag::<f32, _>::new("unsupported_drag")
                    .flags(unsupported)
                    .build(ui, &mut value);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = imgui::Drag::<f32, _>::new("unsupported_drag_array")
                    .flags(unsupported)
                    .build_array(ui, &mut values);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .drag_float_range2_config("unsupported_drag_range")
                    .flags(unsupported)
                    .build(ui, &mut min, &mut max);
            }))
            .is_err()
        );
    });
}
