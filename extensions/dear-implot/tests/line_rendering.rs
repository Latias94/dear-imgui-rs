#![cfg(not(target_arch = "wasm32"))]

use dear_imgui_rs::{Condition, Context};
use dear_implot::PlotContext;
use dear_implot_sys as sys;

#[test]
fn native_lines_match_imgui_texture_and_fringe_contract_at_each_pixel_density() {
    let mut imgui = Context::create();
    imgui.io_mut().set_display_size([800.0, 600.0]);
    imgui.io_mut().set_delta_time(1.0 / 60.0);
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let plot = PlotContext::create(&imgui);
    let binding = imgui.binding();

    for anti_aliased in [false, true] {
        for pixel_density in [0.5_f32, 1.0, 2.0] {
            for weight in [0.5_f32, 1.5, 2.0, 3.7, 128.0] {
                let mut positions = [f32::NAN; 8];
                let mut uvs = [f32::NAN; 8];
                let mut expected_uvs = [f32::NAN; 4];
                let mut expected_width = f32::NAN;
                let mut count = -1;
                {
                    let ui = imgui.frame();
                    ui.window("native line texture regression")
                        .size([650.0, 500.0], Condition::Always)
                        .build(|| {
                            count = binding.with_bound_context(|| unsafe {
                                // Every output points to the fixed-capacity storage required by
                                // this test-only native probe; both contexts remain alive.
                                sys::native_test_probe::dear_implot_line_probe_plot(
                                    plot.raw(),
                                    weight,
                                    pixel_density,
                                    anti_aliased,
                                    positions.as_mut_ptr(),
                                    uvs.as_mut_ptr(),
                                    expected_uvs.as_mut_ptr(),
                                    &mut expected_width,
                                )
                            });
                        });
                }
                let _ = imgui.render_legacy();
                let case = format!("AA={anti_aliased}, density={pixel_density}, weight={weight}");
                assert_eq!(count, 4, "{case}");
                assert!(
                    positions.into_iter().chain(uvs).all(f32::is_finite),
                    "{case}"
                );
                let width = (positions[0] - positions[6]).hypot(positions[1] - positions[7]);
                // Plot renderers use an approximate SSE reciprocal square root, then add
                // f32 offsets to screen coordinates. UV selection itself remains exact.
                let coordinate_scale = positions.into_iter().map(f32::abs).fold(1.0_f32, f32::max);
                let width_tolerance =
                    expected_width * 0.0005 + coordinate_scale * (4.0 * f32::EPSILON);
                assert!(
                    (width - expected_width).abs() <= width_tolerance,
                    "{case}: width {width}, expected {expected_width}, tolerance {width_tolerance}"
                );
                for index in 0..4 {
                    let expected = if index < 2 {
                        &expected_uvs[..2]
                    } else {
                        &expected_uvs[2..]
                    };
                    assert_eq!(
                        &uvs[index * 2..index * 2 + 2],
                        expected,
                        "{case}: vertex {index}"
                    );
                }
                assert_eq!(expected_uvs[1], expected_uvs[3], "{case}: one atlas row");
                if !anti_aliased {
                    assert_eq!(
                        &expected_uvs[..2],
                        &expected_uvs[2..],
                        "{case}: white-pixel UVs"
                    );
                    assert!(
                        (expected_width - weight.max(1.0).max(1.0 / pixel_density)).abs()
                            < f32::EPSILON,
                        "{case}: no AA fringe"
                    );
                }
            }
        }
    }
}

#[test]
fn native_digital_heatmap_and_candles_emit_visible_rectangles() {
    let mut imgui = Context::create();
    imgui.io_mut().set_display_size([800.0, 600.0]);
    imgui.io_mut().set_delta_time(1.0 / 60.0);
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let plot = PlotContext::create(&imgui);
    let binding = imgui.binding();
    for (kind, invert_x, invert_y) in [
        (0, false, false),
        (1, false, false),
        (1, false, true),
        (1, true, false),
        (1, true, true),
        (2, false, false),
    ] {
        let mut bounds = [f32::NAN; 4];
        let mut count = -1;
        {
            let ui = imgui.frame();
            ui.window("native rectangle regression")
                .size([650.0, 500.0], Condition::Always)
                .build(|| {
                    count = binding.with_bound_context(|| unsafe {
                        sys::native_test_probe::dear_implot_rectangle_probe_plot(
                            plot.raw(),
                            kind,
                            invert_x,
                            invert_y,
                            bounds.as_mut_ptr(),
                        )
                    });
                });
        }
        let _ = imgui.render_legacy();
        let case = format!("kind={kind}, invert_x={invert_x}, invert_y={invert_y}");
        assert!(count >= 4, "{case}: produced {count} vertices");
        assert!(bounds.into_iter().all(f32::is_finite), "{case}");
        assert!(
            bounds[2] > bounds[0] && bounds[3] > bounds[1],
            "{case}: {bounds:?}"
        );
    }
}
