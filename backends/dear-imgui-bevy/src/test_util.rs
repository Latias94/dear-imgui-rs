use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(feature = "render")]
pub(crate) fn render_extraction_plugin() -> impl bevy_app::Plugin {
    use bevy_ecs::schedule::{ScheduleLabel, SystemSet};
    use bevy_render::{Render, RenderApp, RenderSystems, extract_plugin::ExtractPlugin};

    ExtractPlugin::<RenderApp>::new(
        |_, _| {},
        Render::base_schedule,
        Render.intern(),
        RenderSystems::ExtractCommands.intern(),
        RenderSystems::PostCleanup.intern(),
    )
}

pub(crate) fn imgui_context_guard() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
