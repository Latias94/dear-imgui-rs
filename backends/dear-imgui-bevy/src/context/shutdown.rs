use std::fmt;

use bevy_app::App;
use bevy_ecs::system::IntoSystem;

#[cfg(feature = "render")]
use bevy_render::RenderApp;
#[cfg(all(feature = "render", not(target_arch = "wasm32")))]
use bevy_render::pipelined_rendering::RenderExtractApp;

use super::{
    ImguiContexts, ImguiFrame, ImguiPass, ImguiPrimaryPass, ownership::ImguiContextRetirements,
};
use crate::schedule::ImguiContextDriver;

const MAX_SHUTDOWN_PASSES: usize = 8;

/// Application-level pass registration and shutdown operations for the Dear ImGui integration.
pub trait ImguiAppExt {
    /// Declare a new private Dear ImGui pass branded by `P`.
    ///
    /// Each call creates a distinct runtime pass. `P` prevents accidental cross-brand
    /// registration, while the private runtime key distinguishes multiple passes of one brand.
    fn declare_imgui_pass<P: 'static>(&mut self) -> ImguiPass<P>;

    /// Retrieve the private pass owned by the primary Dear ImGui Context.
    fn imgui_primary_pass(&mut self) -> ImguiPass<ImguiPrimaryPass>;

    /// Register one frame-input system in `pass`'s private runner.
    ///
    /// The system cannot be recovered as a unit-input Bevy system or registered in a public
    /// schedule after this call.
    fn add_imgui_system<P, S, M>(&mut self, pass: &ImguiPass<P>, system: S) -> &mut Self
    where
        P: 'static,
        S: IntoSystem<ImguiFrame<'static, P>, (), M> + 'static,
        M: 'static;

    /// Release every managed Context and its render/viewport resources without running user
    /// application schedules.
    ///
    /// This operation is idempotent. After it succeeds, the app no longer contains
    /// [`ImguiContexts`] and must not run Dear ImGui systems again.
    fn shutdown_imgui(&mut self) -> Result<(), ImguiShutdownError>;
}

/// Failure to converge all Context retirement handshakes during explicit shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImguiShutdownError {
    /// The plugin's private Context driver schedule was removed while retirement work remained.
    DriverScheduleUnavailable,
    /// One or more Contexts still depend on a render world that could not acknowledge release.
    RetirementPending {
        /// Number of complete Context owners retained by the retirement queue.
        contexts: usize,
        /// Whether a compatible Bevy render sub-app was available to pump acknowledgements.
        render_sub_app_available: bool,
    },
}

impl fmt::Display for ImguiShutdownError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DriverScheduleUnavailable => formatter.write_str(
                "the private Dear ImGui Context driver schedule is unavailable during shutdown",
            ),
            Self::RetirementPending {
                contexts,
                render_sub_app_available,
            } => write!(
                formatter,
                "{contexts} Dear ImGui Context retirement(s) remain pending after draining; render sub-app available: {render_sub_app_available}"
            ),
        }
    }
}

impl std::error::Error for ImguiShutdownError {}

impl ImguiAppExt for App {
    fn declare_imgui_pass<P: 'static>(&mut self) -> ImguiPass<P> {
        super::pass::declare_pass::<P>(self)
    }

    fn imgui_primary_pass(&mut self) -> ImguiPass<ImguiPrimaryPass> {
        super::pass::primary_pass(self)
    }

    fn add_imgui_system<P, S, M>(&mut self, pass: &ImguiPass<P>, system: S) -> &mut Self
    where
        P: 'static,
        S: IntoSystem<ImguiFrame<'static, P>, (), M> + 'static,
        M: 'static,
    {
        super::pass::add_system::<P, _, M>(self, pass, system);
        self
    }

    fn shutdown_imgui(&mut self) -> Result<(), ImguiShutdownError> {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let native_windows = crate::viewport::retire_native_viewport_windows(self.world_mut());
        let contexts = self.world_mut().remove_non_send::<ImguiContexts>();
        drop(contexts);

        let mut render_sub_app_available = false;
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if native_windows.requires_render_drain() {
            render_sub_app_available |= pump_render_sub_app(self);
        }
        for _ in 0..MAX_SHUTDOWN_PASSES {
            if pending_retirements(self) == 0 {
                return Ok(());
            }
            self.world_mut()
                .try_run_schedule(ImguiContextDriver)
                .map_err(|_| ImguiShutdownError::DriverScheduleUnavailable)?;
            if pending_retirements(self) == 0 {
                return Ok(());
            }
            render_sub_app_available |= pump_render_sub_app(self);
        }

        self.world_mut()
            .try_run_schedule(ImguiContextDriver)
            .map_err(|_| ImguiShutdownError::DriverScheduleUnavailable)?;
        let contexts = pending_retirements(self);
        if contexts == 0 {
            Ok(())
        } else {
            Err(ImguiShutdownError::RetirementPending {
                contexts,
                render_sub_app_available,
            })
        }
    }
}

fn pending_retirements(app: &App) -> usize {
    app.world()
        .get_non_send::<ImguiContextRetirements>()
        .map_or(0, ImguiContextRetirements::pending_len)
}

#[cfg(feature = "render")]
fn pump_render_sub_app(app: &mut App) -> bool {
    if app.get_sub_app(RenderApp).is_some() {
        app.sub_apps_mut().update_subapp_by_label(RenderApp);
        return true;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if app.get_sub_app(RenderExtractApp).is_some() {
        app.sub_apps_mut().update_subapp_by_label(RenderExtractApp);
        return true;
    }
    false
}

#[cfg(not(feature = "render"))]
fn pump_render_sub_app(_app: &mut App) -> bool {
    false
}
