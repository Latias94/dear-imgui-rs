use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::Context;
use dear_implot::{Colormap, FloatFormat, HeatmapPlot, Plot, PlotContext};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn prepare_imgui(imgui: &mut Context) {
    let io = imgui.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
}

#[test]
fn public_numeric_format_apis_execute_with_validated_formats() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);

    let ui = imgui.frame();
    ui.window("numeric format APIs").build(|| {
        let plot_ui = plot.get_plot_ui(ui);
        let format = FloatFormat::new("%.2f").unwrap();

        plot_ui.colormap_scale_with_format("scale", 0.0, 1.0, 120.0, &format, Colormap::Viridis);

        let mut position = 0.5;
        let mut color = [0.0; 4];
        let _ = plot_ui.colormap_slider_with_format(
            "slider",
            &mut position,
            Some(&mut color),
            &format,
            Colormap::Viridis,
        );

        let values = [0.0, 0.25, 0.75, 1.0];
        if let Some(plot_token) = plot_ui.begin_plot("heatmap") {
            HeatmapPlot::new("values", &values, 2, 2)
                .with_label_format(format.borrowed())
                .plot(&plot_ui);
            HeatmapPlot::new("no labels", &values, 2, 2)
                .without_value_labels()
                .plot(&plot_ui);
            plot_token.end();
        }
    });
}
