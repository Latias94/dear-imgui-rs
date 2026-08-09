//! Minimal interactive ImPlot3D helix using the high-level `dear-app` runtime.

use dear_app::{AddOnsConfig, AppConfig, Application, FrameContext, RunError, run};
use dear_imgui_rs::Condition;
use dear_implot3d::{Axis3DFlags, Line3D, Plot3D, Plot3DCond};

const SAMPLE_COUNT: usize = 160;

struct ImPlot3dMinimal {
    turns: f32,
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
}

impl Default for ImPlot3dMinimal {
    fn default() -> Self {
        Self {
            turns: 2.0,
            x: vec![0.0; SAMPLE_COUNT],
            y: vec![0.0; SAMPLE_COUNT],
            z: vec![0.0; SAMPLE_COUNT],
        }
    }
}

impl Application for ImPlot3dMinimal {
    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let plot_context = context.addons().implot3d();
        let ui = context.ui();
        let Some(plot_context) = plot_context else {
            ui.text("ImPlot3D add-on not enabled");
            return Ok(());
        };
        let plot_ui = plot_context.get_plot_ui(ui);

        ui.window("ImPlot3D Minimal")
            .size([680.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                ui.slider("Turns", 1.0, 5.0, &mut self.turns);
                ui.text("Drag to rotate the plot; use the wheel to zoom.");

                let denominator = (SAMPLE_COUNT - 1) as f32;
                for index in 0..SAMPLE_COUNT {
                    let progress = index as f32 / denominator;
                    let angle = progress * self.turns * std::f32::consts::TAU;
                    self.x[index] = angle.cos();
                    self.y[index] = angle.sin();
                    self.z[index] = progress * 2.0 - 1.0;
                }

                if let Some(_plot) = plot_ui.begin_plot("Helix").size([-1.0, 390.0]).build() {
                    plot_ui.setup_axes(
                        "x",
                        "y",
                        "z",
                        Axis3DFlags::NONE,
                        Axis3DFlags::NONE,
                        Axis3DFlags::NONE,
                    );
                    plot_ui.setup_axes_limits(
                        -1.25,
                        1.25,
                        -1.25,
                        1.25,
                        -1.25,
                        1.25,
                        Plot3DCond::Once,
                    );
                    Line3D::f32("helix", &self.x, &self.y, &self.z).plot(&plot_ui);
                }
            });

        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - ImPlot3D Minimal".to_owned(),
        window_size: (900.0, 680.0),
        addons: AddOnsConfig {
            with_implot3d: true,
            ..Default::default()
        },
        ..Default::default()
    };

    run(config, ImPlot3dMinimal::default())
}
