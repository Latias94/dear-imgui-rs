use dear_app::{AddOnsConfig, RunnerConfig, run};
use dear_imgui_rs::*;

fn main() {
    // Basic info logs
    dear_imgui_rs::logging::init_tracing_with_filter(
        "dear_imgui=info,dear_app_quickstart=info,wgpu=warn",
    );

    let runner = RunnerConfig {
        window_title: "Dear App Quickstart".to_string(),
        window_size: (1280.0, 720.0),
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.1, 0.2, 0.3, 1.0],
        docking: Default::default(),
        ini_filename: None,
        restore_previous_geometry: true,
        redraw: dear_app::RedrawMode::Poll,
        io_config_flags: None,
        ..Default::default()
    };

    // Enable add-ons compiled into dear-app via features
    let addons = AddOnsConfig::auto();

    run(runner, addons, |ui, _addons| {
        ui.window("Dear App")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Hello from dear-app!");
                ui.separator();

                ui.text(format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));

                ui.bullet_text("Winit + WGPU backend");
                ui.bullet_text("Per-frame closure API");
                ui.bullet_text("Optional add-ons (ImPlot, ImNodes)");
            });
    })
    .unwrap();
}
