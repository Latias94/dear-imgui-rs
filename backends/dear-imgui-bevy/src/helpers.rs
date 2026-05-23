//! Shared helper utilities for the backend's example and editor-facing setup.

use crate::ImguiContext;
use dear_imgui_rs::ConfigFlags;

/// Configure a Dear ImGui context for the backend examples.
///
/// The helper keeps the repeated example setup in one place:
/// - disables input-event trickling;
/// - optionally enables docking;
/// - builds the default font atlas;
/// - disables `.ini` persistence.
pub fn configure_example_context(imgui: &mut ImguiContext, enable_docking: bool) {
    let context = imgui.context_mut();
    context.io_mut().set_config_input_trickle_event_queue(false);

    let mut flags = context.io().config_flags();
    if enable_docking {
        flags.insert(ConfigFlags::DOCKING_ENABLE);
    } else {
        flags.remove(ConfigFlags::DOCKING_ENABLE);
    }
    context.io_mut().set_config_flags(flags);

    let _ = context.font_atlas_mut().build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
}
