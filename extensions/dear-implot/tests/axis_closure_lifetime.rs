use dear_imgui_rs::Context;
use dear_implot::{AxisFlags, PlotContext, PlotError, XAxis, YAxisConfig};
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn axis_closure_is_plot_scoped_even_if_token_is_dropped() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    {
        use dear_imgui_rs::BackendFlags;
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
    }
    let plot_ctx = PlotContext::create(&imgui);

    {
        let ui = imgui.frame();
        let plot_ui = plot_ctx.get_plot_ui(&ui);
        let plot = plot_ui.begin_plot("t").expect("failed to begin plot");

        let _ = plot_ui.setup_x_axis_format_closure(XAxis::X1, |_v| "x".to_string());
        let _ = plot_ui.setup_x_axis_transform_closure(XAxis::X1, |v| v, |v| v);
        plot_ui.plot_line("l", &[0.0, 1.0], &[0.0, 1.0]);

        drop(plot);
    }
    let _ = imgui.render();

    // Start a second plot to ensure plot-scoped storage was cleaned up properly.
    {
        let ui = imgui.frame();
        let plot_ui = plot_ctx.get_plot_ui(&ui);
        let _plot = plot_ui.begin_plot("t2").expect("failed to begin plot");
    }
    let _ = imgui.render();
}

#[test]
fn multi_axis_plot_rejects_interior_nul_labels() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    {
        use dear_imgui_rs::BackendFlags;
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
    }
    let plot_ctx = PlotContext::create(&imgui);

    let ui = imgui.frame();
    let _plot_ui = plot_ctx.get_plot_ui(&ui);

    let plot = dear_implot::MultiAxisPlot::new("t").add_y_axis(YAxisConfig {
        label: Some("a\0b"),
        flags: AxisFlags::NONE,
        range: None,
    });

    match plot.begin() {
        Ok(_) => panic!("expected interior NUL label to be rejected"),
        Err(err) => assert!(matches!(err, PlotError::StringConversion(_))),
    }
    let _ = imgui.render();
}
