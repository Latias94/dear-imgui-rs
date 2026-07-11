use dear_app::{AppConfig, Application, FrameContext, RunError, run};
use dear_imgui_rs::*;

struct Quickstart;

impl Application for Quickstart {
    fn frame(&mut self, context: &mut FrameContext<'_, '_>) -> Result<(), RunError> {
        let ui = context.ui();
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
                ui.bullet_text("Persistent Application state");
                ui.bullet_text("Generation-safe GPU recovery");
            });
        Ok(())
    }
}

fn main() {
    // Basic info logs
    dear_imgui_rs::logging::init_tracing_with_filter(
        "dear_imgui=info,dear_app_quickstart=info,wgpu=warn",
    );

    let config = AppConfig {
        window_title: "Dear App Quickstart".to_string(),
        window_size: (1280.0, 720.0),
        present_mode: wgpu::PresentMode::Fifo,
        clear_color: [0.1, 0.2, 0.3, 1.0],
        ..Default::default()
    };

    run(config, Quickstart).unwrap();
}
