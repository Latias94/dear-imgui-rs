//! Winit + WGPU docking and multi-viewport quickstart.
//!
//! Run with:
//! ```text
//! cargo run -p dear-imgui-examples --bin multi_viewport_wgpu --features multi-viewport
//! ```
//!
//! Drag either Dear ImGui window outside the main window to create a secondary
//! native window. The `Game View` also demonstrates an external WGPU texture
//! rendered through the same viewport route.

// The shared lifecycle also exposes evidence consumed only by the private runtime probe.
#[allow(dead_code)]
#[path = "../support/wgpu_multi_viewport_runtime.rs"]
mod wgpu_multi_viewport_runtime;

use dear_imgui_rs::FrameToken;
use wgpu_multi_viewport_runtime::{
    MainSurfaceFrameDriver, MainSurfaceRenderOutcome, ViewportScenario, run,
};

struct InteractiveScenario;

impl ViewportScenario for InteractiveScenario {
    type Output = ();

    fn drive_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
        driver: &mut MainSurfaceFrameDriver<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let prepared = driver.prepare_frame(frame)?;
        if matches!(
            driver.render_main_frame(prepared)?,
            MainSurfaceRenderOutcome::ReadyToPresent
        ) {
            driver.present_frame()?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let _ = run(InteractiveScenario)?;
    Ok(())
}
