use std::fmt;

use bevy_app::App;
use bevy_ecs::schedule::{InternedSystemSet, IntoScheduleConfigs};
use bevy_ecs::system::ScheduleSystem;

#[cfg(feature = "render")]
use bevy_render::RenderApp;
#[cfg(all(feature = "render", not(target_arch = "wasm32")))]
use bevy_render::pipelined_rendering::RenderExtractApp;

use super::{
    ImguiContextAdmissionError, ImguiContextError, ImguiContexts, ImguiPass, ImguiPrimaryPass,
    ownership::{ImguiContextRemovalPendingReason, ImguiContextRetirements},
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

    /// Adopt a custom suspended Context as this App's initial primary Context.
    ///
    /// Call this before adding [`crate::ImguiPlugin`]. Admission is App-scoped so an explicitly
    /// shut down App cannot construct a fresh, unattached Context registry.
    fn adopt_imgui_primary_context(
        &mut self,
        context: dear_imgui_rs::SuspendedContext,
    ) -> Result<&mut Self, ImguiContextAdmissionError>;

    /// Register configured unit-input systems in `pass`'s private runner.
    ///
    /// Bind each frame-input function with [`ImguiPass::system`] first. The supplied Bevy
    /// [`IntoScheduleConfigs`] is preserved, including ordering, run conditions, system sets, and
    /// automatically inserted deferred-command barriers.
    fn add_imgui_systems<P, M>(
        &mut self,
        pass: &ImguiPass<P>,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self
    where
        P: 'static;

    /// Configure system sets in `pass`'s private runner with Bevy's native set configuration.
    fn configure_imgui_sets<P, M>(
        &mut self,
        pass: &ImguiPass<P>,
        sets: impl IntoScheduleConfigs<InternedSystemSet, M>,
    ) -> &mut Self
    where
        P: 'static;

    /// Release every managed Context and its render/viewport resources without running user
    /// application schedules.
    ///
    /// Renderer and viewport callback ownership is validated for every Context before this method
    /// removes native window mappings or the Context registry. A
    /// [`ImguiShutdownError::ContextTeardownBlocked`] error leaves both intact so the conflicting
    /// field can be repaired through [`ImguiContexts::configure`] before retrying.
    ///
    /// This operation is idempotent. After it succeeds, the app no longer contains
    /// [`ImguiContexts`] and must not run Dear ImGui systems again.
    fn shutdown_imgui(&mut self) -> Result<(), ImguiShutdownError>;
}

/// Failure to converge all Context retirement handshakes during explicit shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImguiShutdownError {
    /// A Context still owns part of the backend contract, but another integration replaced one of
    /// the fields needed for deterministic teardown.
    ContextTeardownBlocked {
        /// Context whose teardown ownership validation failed.
        context_id: dear_imgui_rs::ContextId,
        /// Exact renderer or viewport ownership conflict observed by the preflight.
        reason: ImguiContextRemovalPendingReason,
    },
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
            Self::ContextTeardownBlocked { context_id, reason } => write!(
                formatter,
                "Context {context_id:?} cannot begin terminal Dear ImGui teardown: {reason}"
            ),
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

impl std::error::Error for ImguiShutdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ContextTeardownBlocked { reason, .. } => Some(reason),
            Self::DriverScheduleUnavailable | Self::RetirementPending { .. } => None,
        }
    }
}

impl ImguiAppExt for App {
    fn declare_imgui_pass<P: 'static>(&mut self) -> ImguiPass<P> {
        super::pass::declare_pass::<P>(self)
    }

    fn imgui_primary_pass(&mut self) -> ImguiPass<ImguiPrimaryPass> {
        super::pass::primary_pass(self)
    }

    fn adopt_imgui_primary_context(
        &mut self,
        context: dear_imgui_rs::SuspendedContext,
    ) -> Result<&mut Self, ImguiContextAdmissionError> {
        if self
            .world()
            .get_resource::<super::lifecycle::ImguiAppLifecycle>()
            .is_some_and(super::lifecycle::ImguiAppLifecycle::is_terminal)
        {
            return Err(ImguiContextAdmissionError::new(
                ImguiContextError::AppTerminated,
                context,
            ));
        }
        super::pass::install_pass_registry(self);
        let lifecycle = super::pass::lifecycle(self.world());
        if self.world().get_non_send::<ImguiContexts>().is_some() || lifecycle.registry_claimed() {
            return Err(ImguiContextAdmissionError::new(
                ImguiContextError::ContextRegistryAlreadyInstalled,
                context,
            ));
        }
        let primary_pass = super::pass::primary_pass(self);
        self.insert_non_send(ImguiContexts::with_primary(
            context,
            primary_pass,
            lifecycle,
        ));
        Ok(self)
    }

    fn add_imgui_systems<P, M>(
        &mut self,
        pass: &ImguiPass<P>,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self
    where
        P: 'static,
    {
        super::pass::add_systems(self, pass, systems);
        self
    }

    fn configure_imgui_sets<P, M>(
        &mut self,
        pass: &ImguiPass<P>,
        sets: impl IntoScheduleConfigs<InternedSystemSet, M>,
    ) -> &mut Self
    where
        P: 'static,
    {
        super::pass::configure_sets(self, pass, sets);
        self
    }

    fn shutdown_imgui(&mut self) -> Result<(), ImguiShutdownError> {
        if let Some(mut contexts) = self.world_mut().get_non_send_mut::<ImguiContexts>()
            && let Err((context_id, reason)) = contexts.preflight_backend_detach()
        {
            return Err(ImguiShutdownError::ContextTeardownBlocked { context_id, reason });
        }

        self.init_resource::<super::lifecycle::ImguiAppLifecycle>();
        self.world()
            .resource::<super::lifecycle::ImguiAppLifecycle>()
            .commit_terminal();

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
