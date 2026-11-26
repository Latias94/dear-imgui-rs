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

#[cfg(feature = "render")]
pub(crate) struct RenderTestApp(bevy_app::App);

#[cfg(feature = "render")]
impl RenderTestApp {
    pub(crate) fn new() -> Self {
        Self(bevy_app::App::new())
    }
}

#[cfg(feature = "render")]
impl std::ops::Deref for RenderTestApp {
    type Target = bevy_app::App;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "render")]
impl std::ops::DerefMut for RenderTestApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "render")]
impl Drop for RenderTestApp {
    fn drop(&mut self) {
        use crate::ImguiAppExt as _;

        if std::thread::panicking() {
            let _ =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.0.shutdown_imgui()));
        } else {
            self.0
                .shutdown_imgui()
                .expect("test must retire ImGui contexts before dropping the render world");
        }
    }
}

#[cfg(all(feature = "render", not(target_arch = "wasm32")))]
pub(crate) fn run_isolated_test(full_name: &str, body: impl FnOnce()) {
    const CHILD: &str = "DEAR_IMGUI_BEVY_ISOLATED_TEST";
    let name = full_name.split_once("::").unwrap().1;
    if std::env::var(CHILD).as_deref() == Ok(name) {
        body();
        return;
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name])
        .env(CHILD, name)
        .output()
        .expect("run the intentional context leak in a separate process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "isolated test failed: {}\n{stdout}\n{stderr}",
        output.status
    );
}

pub(crate) fn imgui_context_guard() -> MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
