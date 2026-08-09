//! Minimal interactive multi-viewport sample using Winit + Ash (Vulkan).
//!
//! Run with:
//! ```text
//! cargo run -p dear-imgui-examples --bin multi_viewport_ash --features ash-winit-multi-viewport
//! cargo run -p dear-imgui-examples --bin multi_viewport_ash --features "ash-winit-multi-viewport,ash-dynamic-rendering"
//! ```
//!
//! This teaching entry point contains only the normal interactive lifecycle. Private Vulkan
//! validation probes use a separate feature-gated adapter over the same backend runtime.

#[path = "../support/ash_multi_viewport.rs"]
mod ash_multi_viewport;

use ash_multi_viewport::{AshFrameUi, AshViewportScenario, ExampleResult, VulkanAdapterInfo};
use dear_imgui_rs::Condition;
use dear_imgui_rs::Context;

struct InteractiveScenario;

impl AshViewportScenario for InteractiveScenario {
    fn initialize(&mut self, _context: &mut Context, adapter: &VulkanAdapterInfo) -> ExampleResult {
        tracing::info!(
            name = %adapter.name,
            device_type = %adapter.device_type,
            driver = %adapter.driver,
            driver_info = %adapter.driver_info,
            vendor = adapter.vendor,
            device = adapter.device,
            "Selected Vulkan adapter"
        );
        Ok(())
    }

    fn draw_ui(&mut self, frame: AshFrameUi<'_>) -> ExampleResult {
        let ui = frame.ui;
        ui.window("Multi-Viewport (ash)")
            .size([460.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Renderer: dear-imgui-ash (Vulkan)");
                ui.text(format!("Viewports: {}", frame.viewport_count));
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
        Ok(())
    }
}

fn main() -> ExampleResult {
    ash_multi_viewport::run(InteractiveScenario)
}
