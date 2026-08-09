use std::sync::{Mutex, MutexGuard, OnceLock};

use dear_imgui_rs::Context;
use dear_implot3d::{
    Plot3D, Plot3DContext, Plot3DDataLayout, Plot3DError, Surface3D, Surface3DFlags,
};

fn test_guard() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn configured_context() -> Context {
    let mut context = Context::create();
    let io = context.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    context
}

#[test]
fn safe_surface_paths_submit_to_a_real_context() {
    let _guard = test_guard();
    let mut imgui = configured_context();
    let plot_context = Plot3DContext::create(&imgui);

    let frame = imgui.begin_frame();
    let plot_ui = plot_context.get_plot_ui(frame.ui());
    let _plot = plot_ui
        .begin_plot("surface native probe")
        .build()
        .expect("surface probe plot should begin");

    let xs = [0.0, 1.0];
    let ys = [0.0, 1.0];
    let zs = [0.0, 1.0, 1.0, 2.0];
    let xs_flat = [0.0, 1.0, 0.0, 1.0];
    let ys_flat = [0.0, 0.0, 1.0, 1.0];

    Surface3D::new("Surface3D", &xs, &ys, &zs)
        .try_plot(&plot_ui)
        .unwrap();
    plot_ui
        .surface_f32("Surface3DBuilder", &xs, &ys, &zs)
        .plot()
        .unwrap();
    plot_ui
        .surface_f32_flat(
            "flattened surface",
            &xs_flat,
            &ys_flat,
            &zs,
            2,
            2,
            0.0,
            0.0,
            Surface3DFlags::NONE,
        )
        .unwrap();
}

#[test]
fn every_surface_entry_point_rejects_nul_before_shape_work() {
    let _guard = test_guard();
    let mut imgui = configured_context();
    let plot_context = Plot3DContext::create(&imgui);

    let frame = imgui.begin_frame();
    let plot_ui = plot_context.get_plot_ui(frame.ui());
    let expected = Err(Plot3DError::StringConversion("surface label contains NUL"));

    assert_eq!(
        Surface3D::new("invalid\0surface", &[0.0], &[0.0], &[]).try_plot(&plot_ui),
        expected
    );
    assert_eq!(
        plot_ui
            .surface_f32("invalid\0builder", &[0.0], &[0.0], &[])
            .plot(),
        expected
    );
    assert_eq!(
        plot_ui.surface_f32_flat(
            "invalid\0flat",
            &[],
            &[],
            &[],
            usize::MAX,
            2,
            0.0,
            0.0,
            Surface3DFlags::NONE,
        ),
        expected
    );

    // SAFETY: The invalid label is rejected before shape validation or pointer access.
    let raw_result = unsafe {
        plot_ui.surface_f32_raw(
            "invalid\0raw",
            &[],
            &[],
            &[],
            usize::MAX,
            2,
            0.0,
            0.0,
            Surface3DFlags::NONE,
            Plot3DDataLayout::DEFAULT,
        )
    };
    assert_eq!(raw_result, expected);
}
