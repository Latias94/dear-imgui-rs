//! Observable `dear-app` lifecycle using one persistent application value.

use dear_app::{
    AppConfig, Application, EventContext, FrameContext, GpuContext, InitContext,
    PrepareFrameContext, RunError, ShutdownContext, imgui::Condition, run,
};

/// Long-lived state owned by `Application`; it is not recreated with GPU resources.
#[derive(Default)]
struct LifecycleApp {
    configure_imgui_calls: u64,
    initialized_calls: u64,
    event_calls: u64,
    prepare_frame_calls: u64,
    frame_calls: u64,
    gpu_lost_calls: u64,
    gpu_recreated_calls: u64,
    shutdown_calls: u64,
    persistent_clicks: u64,
    gpu_generation: Option<u64>,
    last_event: String,
}

impl LifecycleApp {
    fn print_summary(&self) {
        eprintln!(
            "[lifecycle] configure_imgui={} initialized={} event={} prepare_frame={} frame={} \
             gpu_lost={} gpu_recreated={} shutdown={} generation={:?}",
            self.configure_imgui_calls,
            self.initialized_calls,
            self.event_calls,
            self.prepare_frame_calls,
            self.frame_calls,
            self.gpu_lost_calls,
            self.gpu_recreated_calls,
            self.shutdown_calls,
            self.gpu_generation,
        );
    }
}

impl Application for LifecycleApp {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        self.configure_imgui_calls += 1;
        context.imgui().style_mut().set_window_rounding(4.0);
        eprintln!("[lifecycle] configure_imgui");
        Ok(())
    }

    fn initialized(
        &mut self,
        _init: &mut InitContext<'_>,
        gpu: &mut GpuContext<'_>,
    ) -> Result<(), RunError> {
        self.initialized_calls += 1;
        self.gpu_generation = Some(gpu.generation().get());
        eprintln!(
            "[lifecycle] initialized at GPU generation {}",
            gpu.generation().get()
        );
        Ok(())
    }

    fn event(&mut self, context: &mut EventContext<'_>) -> Result<(), RunError> {
        self.event_calls += 1;
        self.last_event = format!("{:?}", context.event()).chars().take(96).collect();
        Ok(())
    }

    fn prepare_frame(&mut self, _context: &mut PrepareFrameContext<'_>) -> Result<(), RunError> {
        self.prepare_frame_calls += 1;
        Ok(())
    }

    fn gpu_lost(&mut self, context: &mut GpuContext<'_>) -> Result<(), RunError> {
        self.gpu_lost_calls += 1;
        self.gpu_generation = Some(context.generation().get());
        eprintln!(
            "[lifecycle] GPU generation {} lost",
            context.generation().get()
        );
        Ok(())
    }

    fn gpu_recreated(&mut self, context: &mut GpuContext<'_>) -> Result<(), RunError> {
        self.gpu_recreated_calls += 1;
        self.gpu_generation = Some(context.generation().get());
        eprintln!(
            "[lifecycle] GPU generation {} recreated",
            context.generation().get()
        );
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        self.frame_calls += 1;
        self.gpu_generation = Some(context.gpu().generation().get());
        let ui = context.ui();

        ui.window("Application Lifecycle")
            .size([500.0, 430.0], Condition::FirstUseEver)
            .build(|| {
                if ui.button("Update persistent state") {
                    self.persistent_clicks += 1;
                }
                ui.same_line();
                ui.text(format!("Persistent clicks: {}", self.persistent_clicks));

                ui.separator();
                match self.gpu_generation {
                    Some(generation) => ui.text(format!("GPU generation: {generation}")),
                    None => ui.text("GPU generation: not ready"),
                }
                if self.last_event.is_empty() {
                    ui.text("Last event: none");
                } else {
                    ui.text(format!("Last event: {}", self.last_event));
                }

                ui.separator();
                ui.text("Lifecycle calls");
                ui.text(format!(
                    "configure_imgui: {}    initialized: {}",
                    self.configure_imgui_calls, self.initialized_calls
                ));
                ui.text(format!(
                    "event: {}    prepare_frame: {}    frame: {}",
                    self.event_calls, self.prepare_frame_calls, self.frame_calls
                ));
                ui.text(format!(
                    "gpu_lost: {}    gpu_recreated: {}    shutdown: {}",
                    self.gpu_lost_calls, self.gpu_recreated_calls, self.shutdown_calls
                ));
            });
        Ok(())
    }

    fn shutdown(&mut self, context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        self.shutdown_calls += 1;
        self.gpu_generation = context.gpu_generation().map(|generation| generation.get());
        eprintln!("[lifecycle] shutdown");
        self.print_summary();
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Application Lifecycle".to_owned(),
        window_size: (720.0, 520.0),
        ..Default::default()
    };

    run(config, LifecycleApp::default())
}
