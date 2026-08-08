//! Minimal interactive multi-viewport sample using Winit + Ash (Vulkan).
//!
//! Run with:
//! ```text
//! cargo run -p dear-imgui-examples --bin multi_viewport_ash --features ash-winit-multi-viewport
//! cargo run -p dear-imgui-examples --bin multi_viewport_ash --features "ash-winit-multi-viewport,ash-dynamic-rendering"
//! ```
//!
//! This teaching entry point deliberately uses the interactive lifecycle only. The private
//! Vulkan validation contract has its own entry point and supplies validation configuration to
//! the same backend-specific lifecycle module.

// The shared lifecycle also exposes evidence consumed only by the private validation probe.
#[allow(dead_code)]
#[path = "../support/ash_multi_viewport.rs"]
mod ash_multi_viewport;

use ash_multi_viewport::{AshFrameUi, AshViewportScenario, ExampleResult};
use dear_imgui_rs::Condition;

struct InteractiveScenario;

impl AshViewportScenario for InteractiveScenario {
    type Evidence = ();

    fn draw_ui(&mut self, frame: AshFrameUi<'_>) -> ExampleResult<bool> {
        let ui = frame.ui;
        ui.window("Multi-Viewport (ash)")
            .size([460.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Renderer: dear-imgui-ash (Vulkan)");
                ui.separator();
                ui.text(format!("Swapchain format: {:?}", frame.surface_format));
                ui.text(format!(
                    "Framebuffer sRGB: {} (shader gamma path)",
                    frame.framebuffer_srgb
                ));
                ui.color_edit4("Clear color", frame.clear_color);
                if ui.button("Show Demo Window") {
                    *frame.demo_open = true;
                }
            });

        if *frame.demo_open {
            ui.show_demo_window(frame.demo_open);
        }
        Ok(false)
    }
}

fn main() -> ExampleResult {
    ash_multi_viewport::run(InteractiveScenario)
}
