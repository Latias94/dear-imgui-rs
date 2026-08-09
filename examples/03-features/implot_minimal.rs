//! Minimal interactive ImPlot line chart using the high-level `dear-app` runtime.

use dear_app::{AddOnsConfig, AppConfig, Application, FrameContext, RunError, run};
use dear_imgui_rs::Condition;
use dear_implot::{AxisFlags, LinePlot, Plot, PlotCond};

const SAMPLE_COUNT: usize = 128;

struct ImPlotMinimal {
    amplitude: f32,
    x: Vec<f64>,
    y: Vec<f64>,
}

impl Default for ImPlotMinimal {
    fn default() -> Self {
        let x = (0..SAMPLE_COUNT)
            .map(|index| index as f64 * std::f64::consts::TAU / (SAMPLE_COUNT - 1) as f64)
            .collect();

        Self {
            amplitude: 1.0,
            x,
            y: vec![0.0; SAMPLE_COUNT],
        }
    }
}

impl Application for ImPlotMinimal {
    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let plot_context = context.addons().implot();
        let ui = context.ui();
        let Some(plot_context) = plot_context else {
            ui.text("ImPlot add-on not enabled");
            return Ok(());
        };
        let plot_ui = plot_context.get_plot_ui(ui);

        ui.window("ImPlot Minimal")
            .size([680.0, 430.0], Condition::FirstUseEver)
            .build(|| {
                ui.slider("Amplitude", 0.1, 2.0, &mut self.amplitude);
                ui.text("Drag inside the plot to pan; use the wheel to zoom.");

                let amplitude = f64::from(self.amplitude);
                for (&x, y) in self.x.iter().zip(&mut self.y) {
                    *y = amplitude * x.sin();
                }

                if let Some(_plot) = plot_ui.begin_plot_with_size("Sine wave", [-1.0, 320.0]) {
                    plot_ui.setup_axes(
                        Some("x"),
                        Some("amplitude × sin(x)"),
                        AxisFlags::NONE,
                        AxisFlags::NONE,
                    );
                    plot_ui.setup_axes_limits(
                        0.0,
                        std::f64::consts::TAU,
                        -2.2,
                        2.2,
                        PlotCond::Once,
                    );
                    LinePlot::new("signal", &self.x, &self.y).plot(&plot_ui);
                }
            });

        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - ImPlot Minimal".to_owned(),
        window_size: (900.0, 600.0),
        addons: AddOnsConfig {
            with_implot: true,
            ..Default::default()
        },
        ..Default::default()
    };

    run(config, ImPlotMinimal::default())
}
