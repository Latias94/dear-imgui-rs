//! Bevy plugin installation and backend configuration.

use std::fmt;

use bevy_app::{App, Plugin, PluginsState};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::ScheduleLabel;

use super::ImguiContexts;
use super::backend_contract::BackendAttachment;
#[cfg(feature = "render")]
use crate::render;
#[cfg(feature = "render")]
use crate::route;
use crate::viewport::ImguiViewportWindowConfig;
use crate::{input, schedule, viewport};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ImguiPluginBuildMode {
    #[default]
    Install,
    AlreadyInstalled,
}

/// Bevy plugin that owns Dear ImGui Context, input, render, and viewport integration.
#[derive(Debug, Clone, Default)]
pub struct ImguiPlugin {
    config: ImguiPluginConfig,
    build_mode: ImguiPluginBuildMode,
    #[cfg(feature = "bevy-ui")]
    ui_render_order: render::ImguiUiRenderOrder,
}

impl ImguiPlugin {
    /// Create a plugin with explicit backend configuration.
    #[must_use]
    pub fn new(config: ImguiPluginConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Borrow the plugin configuration.
    #[must_use]
    pub fn config(&self) -> &ImguiPluginConfig {
        &self.config
    }

    /// Configure whether Dear ImGui or Bevy UI is drawn on top for the same camera.
    ///
    /// This setting takes effect when the `bevy-ui` Cargo feature is enabled.
    #[cfg(feature = "bevy-ui")]
    #[must_use]
    pub fn with_ui_render_order(mut self, order: render::ImguiUiRenderOrder) -> Self {
        self.ui_render_order = order;
        self
    }

    /// Return the configured Dear ImGui/Bevy UI draw order.
    #[cfg(feature = "bevy-ui")]
    #[must_use]
    pub fn ui_render_order(&self) -> render::ImguiUiRenderOrder {
        self.ui_render_order
    }

    #[cfg(feature = "render")]
    fn resolved_ui_render_order(&self) -> render::ImguiUiRenderOrder {
        #[cfg(feature = "bevy-ui")]
        {
            self.ui_render_order
        }
        #[cfg(not(feature = "bevy-ui"))]
        {
            render::ImguiUiRenderOrder::default()
        }
    }

    fn already_installed(mut self) -> Self {
        self.build_mode = ImguiPluginBuildMode::AlreadyInstalled;
        self
    }
}

/// Failure to validate a Dear ImGui installation before mutating the Bevy App.
#[derive(Debug)]
#[non_exhaustive]
pub enum ImguiPluginInstallError {
    /// The App already contains this integration.
    AlreadyInstalled,
    /// Bevy already finished or cleaned its plugin lifecycle.
    PluginLifecycleClosed,
    /// Native platform windows were requested by a build that cannot create them.
    NativeMultiViewportUnavailable,
    /// The secondary-window policy is internally inconsistent.
    ViewportWindow(crate::viewport::ImguiViewportWindowConfigError),
    /// The private Context driver was placed outside its valid main-schedule interval.
    DriverSchedule(schedule::ImguiDriverScheduleError),
    /// Explicit shutdown made the App terminal.
    AppTerminated,
    /// The App lifecycle still owns a registry that application code removed.
    ContextRegistryMissing,
    /// A registry exists without the App lifecycle claim that owns it.
    ContextRegistryOwnershipMissing,
    /// The App lifecycle still owns a private pass registry that application code removed.
    PassRegistryMissing,
    /// A private pass registry exists without its App lifecycle ownership claim.
    PassRegistryOwnershipMissing,
    /// The private pass registry could not be created or queried.
    PassRegistry(super::ImguiPassError),
    /// The Context registry and pass registry came from different Apps.
    ForeignPassRegistry,
    /// Core Context construction failed before App mutation began.
    ContextCreation(dear_imgui_rs::ImGuiError),
    /// Existing Contexts cannot admit this backend configuration.
    ContextPreflight(super::ImguiContextError),
}

impl fmt::Display for ImguiPluginInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInstalled => {
                formatter.write_str("Dear ImGui is already installed in this Bevy App")
            }
            Self::PluginLifecycleClosed => formatter.write_str(
                "Dear ImGui plugins cannot be installed after Bevy plugin finish or cleanup",
            ),
            Self::NativeMultiViewportUnavailable => formatter.write_str(
                "native Dear ImGui platform windows require a native build with the `multi-viewport` feature",
            ),
            Self::ViewportWindow(error) => write!(formatter, "invalid viewport window policy: {error}"),
            Self::DriverSchedule(error) => error.fmt(formatter),
            Self::AppTerminated => formatter
                .write_str("the Dear ImGui integration is terminal after explicit App shutdown"),
            Self::ContextRegistryMissing => formatter.write_str(
                "ImguiContexts was removed after admission; restore the original registry or shut the App down",
            ),
            Self::ContextRegistryOwnershipMissing => formatter.write_str(
                "ImguiContexts exists without the App lifecycle ownership claim",
            ),
            Self::PassRegistryMissing => formatter.write_str(
                "the private Dear ImGui pass registry was removed after App admission",
            ),
            Self::PassRegistryOwnershipMissing => formatter.write_str(
                "the private Dear ImGui pass registry exists without its App lifecycle ownership claim",
            ),
            Self::PassRegistry(error) => error.fmt(formatter),
            Self::ForeignPassRegistry => formatter.write_str(
                "ImguiContexts was created with pass handles from another Bevy App",
            ),
            Self::ContextCreation(error) => error.fmt(formatter),
            Self::ContextPreflight(error) => {
                write!(formatter, "existing Context rejected backend installation: {error}")
            }
        }
    }
}

impl std::error::Error for ImguiPluginInstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ViewportWindow(error) => Some(error),
            Self::DriverSchedule(error) => Some(error),
            Self::ContextCreation(error) => Some(error),
            Self::ContextPreflight(error) => Some(error),
            Self::PassRegistry(error) => Some(error),
            _ => None,
        }
    }
}

struct PreparedImguiInstallation {
    primary: Option<dear_imgui_rs::SuspendedContext>,
    driver_schedule: schedule::ValidatedImguiDriverSchedulePlacement,
}

impl ImguiPlugin {
    fn prepare_installation(
        &self,
        app: &mut App,
    ) -> Result<PreparedImguiInstallation, ImguiPluginInstallError> {
        let lifecycle = app
            .world()
            .get_resource::<super::lifecycle::ImguiAppLifecycle>()
            .cloned();
        let pass_registry_exists = super::pass::existing_registry_id(app.world()).is_some();
        match lifecycle.as_ref() {
            Some(lifecycle) if pass_registry_exists && !lifecycle.pass_registry_claimed() => {
                return Err(ImguiPluginInstallError::PassRegistryOwnershipMissing);
            }
            Some(lifecycle) if !pass_registry_exists && lifecycle.pass_registry_claimed() => {
                return Err(ImguiPluginInstallError::PassRegistryMissing);
            }
            None if pass_registry_exists => {
                return Err(ImguiPluginInstallError::PassRegistryOwnershipMissing);
            }
            _ => {}
        }
        if lifecycle
            .as_ref()
            .is_some_and(super::lifecycle::ImguiAppLifecycle::is_terminal)
        {
            return Err(ImguiPluginInstallError::AppTerminated);
        }
        if app.world().contains_resource::<ImguiBackendRuntime>() {
            return Err(ImguiPluginInstallError::AlreadyInstalled);
        }
        if self.config.multi_viewport()
            && !cfg!(all(feature = "multi-viewport", not(target_arch = "wasm32")))
        {
            return Err(ImguiPluginInstallError::NativeMultiViewportUnavailable);
        }
        self.config
            .viewport_window()
            .validate()
            .map_err(ImguiPluginInstallError::ViewportWindow)?;
        let driver_schedule =
            schedule::validate_imgui_schedule_placement(app, self.config.driver_schedule())
                .map_err(ImguiPluginInstallError::DriverSchedule)?;

        let primary = if app.world().get_non_send::<ImguiContexts>().is_some() {
            let lifecycle = lifecycle
                .as_ref()
                .ok_or(ImguiPluginInstallError::ContextRegistryOwnershipMissing)?;
            if !lifecycle.context_registry_claimed() {
                return Err(ImguiPluginInstallError::ContextRegistryOwnershipMissing);
            }
            let pass_registry_id = super::pass::existing_registry_id(app.world())
                .ok_or(ImguiPluginInstallError::PassRegistryMissing)?;
            if app
                .world()
                .get_non_send::<ImguiContexts>()
                .expect("the Context registry was just checked")
                .pass_registry_id()
                != pass_registry_id
            {
                return Err(ImguiPluginInstallError::ForeignPassRegistry);
            }

            #[cfg(feature = "render")]
            let render_integration_installed = render::render_integration_available(app);
            #[cfg(not(feature = "render"))]
            let render_integration_installed = false;
            let attachment = BackendAttachment {
                render_integration_installed,
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                viewport_bridge_registration: Some(
                    viewport::ImguiViewportBridge::default().registration(),
                ),
                #[cfg(feature = "render")]
                renderer_releases: None,
            };
            app.world_mut()
                .get_non_send_mut::<ImguiContexts>()
                .expect("the Context registry was just checked")
                .preflight_backend_attachment(
                    &attachment,
                    Some((self.config.docking(), self.config.multi_viewport())),
                )
                .map_err(ImguiPluginInstallError::ContextPreflight)?;
            None
        } else {
            if lifecycle
                .as_ref()
                .is_some_and(super::lifecycle::ImguiAppLifecycle::context_registry_claimed)
            {
                return Err(ImguiPluginInstallError::ContextRegistryMissing);
            }
            Some(
                dear_imgui_rs::SuspendedContext::try_create()
                    .map_err(ImguiPluginInstallError::ContextCreation)?,
            )
        };

        Ok(PreparedImguiInstallation {
            primary,
            driver_schedule,
        })
    }

    fn commit_installation(
        &self,
        app: &mut App,
        prepared: PreparedImguiInstallation,
    ) -> Result<(), super::ImguiPassError> {
        super::pass::install_pass_registry(app)?;
        if let Some(primary) = prepared.primary {
            let primary_pass = super::pass::primary_pass(app)?;
            let lifecycle = super::pass::lifecycle(app.world());
            app.insert_non_send(ImguiContexts::with_primary(
                primary,
                primary_pass,
                lifecycle,
            ));
        }
        schedule::install_imgui_schedules(app, prepared.driver_schedule);
        #[cfg(feature = "render")]
        route::install_route_resolution(app);
        input::install_input_mapping(app);
        crate::context::install_context_lifecycle(app);
        #[cfg(feature = "render")]
        crate::texture::install_texture_leases(app);
        #[cfg(feature = "render")]
        let render_integration_available = render::render_integration_available(app);
        #[cfg(not(feature = "render"))]
        let render_integration_available = false;
        #[cfg(feature = "render")]
        let render_integration_installed =
            render::install_render_extraction(app, self.resolved_ui_render_order());
        #[cfg(not(feature = "render"))]
        let render_integration_installed = false;
        debug_assert_eq!(render_integration_installed, render_integration_available);
        viewport::install_viewport_bridge(app);
        refresh_backend_contract(app, self.config.clone(), render_integration_installed)
            .expect("installation preflight must make backend attachment infallible");
        Ok(())
    }
}

impl Plugin for ImguiPlugin {
    fn build(&self, app: &mut App) {
        if self.build_mode == ImguiPluginBuildMode::AlreadyInstalled {
            debug_assert!(app.world().contains_resource::<ImguiBackendRuntime>());
            return;
        }
        let prepared = self
            .prepare_installation(app)
            .unwrap_or_else(|error| panic!("ImguiPlugin installation failed: {error}"));
        self.commit_installation(app, prepared)
            .unwrap_or_else(|error| panic!("ImguiPlugin installation failed: {error}"));
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "render")]
        {
            let render_integration_installed =
                render::install_render_extraction(_app, self.resolved_ui_render_order());
            refresh_backend_contract(_app, self.config.clone(), render_integration_installed)
                .unwrap_or_else(|error| panic!("ImguiPlugin finish failed: {error}"));
        }
    }
}

pub(crate) fn try_install_imgui(
    app: &mut App,
    plugin: ImguiPlugin,
) -> Result<&mut App, ImguiPluginInstallError> {
    if app
        .world()
        .get_resource::<super::lifecycle::ImguiAppLifecycle>()
        .is_some_and(super::lifecycle::ImguiAppLifecycle::is_terminal)
    {
        return Err(ImguiPluginInstallError::AppTerminated);
    }
    if app.is_plugin_added::<ImguiPlugin>()
        || app.world().contains_resource::<ImguiBackendRuntime>()
    {
        return Err(ImguiPluginInstallError::AlreadyInstalled);
    }
    if matches!(
        app.plugins_state(),
        PluginsState::Finished | PluginsState::Cleaned
    ) {
        return Err(ImguiPluginInstallError::PluginLifecycleClosed);
    }
    let prepared = plugin.prepare_installation(app)?;
    plugin
        .commit_installation(app, prepared)
        .map_err(ImguiPluginInstallError::PassRegistry)?;
    app.add_plugins(plugin.already_installed());
    Ok(app)
}

fn refresh_backend_contract(
    app: &mut App,
    config: ImguiPluginConfig,
    render_integration_installed: bool,
) -> Result<(), super::ImguiContextError> {
    #[cfg(feature = "render")]
    let renderer_releases = render_integration_installed.then(|| {
        app.world()
            .resource::<render::ImguiRendererReleases>()
            .clone()
    });
    let attachment = BackendAttachment {
        render_integration_installed,
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        viewport_bridge_registration: app
            .world()
            .get_non_send::<viewport::ImguiViewportBridge>()
            .map(viewport::ImguiViewportBridge::registration),
        #[cfg(feature = "render")]
        renderer_releases,
    };
    {
        let mut contexts = app
            .world_mut()
            .get_non_send_mut::<ImguiContexts>()
            .expect("ImguiPlugin must retain its Context registry");
        contexts.set_primary_contract(config.docking(), config.multi_viewport());
        contexts.attach_backend(attachment)?;
    }
    app.insert_resource(ImguiBackendRuntime::new(
        config,
        render_integration_installed,
    ));
    Ok(())
}

/// Configuration applied when [`ImguiPlugin`] attaches the primary Context.
#[derive(Debug, Clone, PartialEq)]
pub struct ImguiPluginConfig {
    docking: bool,
    multi_viewport: bool,
    viewport_window: ImguiViewportWindowConfig,
    driver_schedule: schedule::ImguiDriverSchedulePlacement,
}

impl ImguiPluginConfig {
    /// Enable or disable docking for the primary Context.
    #[must_use]
    pub const fn with_docking(mut self, enabled: bool) -> Self {
        self.docking = enabled;
        self
    }

    /// Return whether docking is enabled for the primary Context.
    #[must_use]
    pub const fn docking(&self) -> bool {
        self.docking
    }

    /// Enable or disable native platform windows for the primary Context.
    ///
    /// This requires the native-only `multi-viewport` Cargo feature.
    #[must_use]
    pub const fn with_multi_viewport(mut self, enabled: bool) -> Self {
        self.multi_viewport = enabled;
        self
    }

    /// Return whether native platform windows are enabled for the primary Context.
    #[must_use]
    pub const fn multi_viewport(&self) -> bool {
        self.multi_viewport
    }

    /// Set the window policy applied to native Dear ImGui platform windows.
    #[must_use]
    pub fn with_viewport_window(mut self, config: ImguiViewportWindowConfig) -> Self {
        self.viewport_window = config;
        self
    }

    /// Borrow the native platform-window policy.
    #[must_use]
    pub const fn viewport_window(&self) -> &ImguiViewportWindowConfig {
        &self.viewport_window
    }

    /// Run the serial Context driver immediately before `anchor` in Bevy's main schedule order.
    ///
    /// The resulting placement must remain after `PreUpdate` completes and before `PostUpdate`
    /// begins. [`crate::ImguiAppExt::try_install_imgui`] returns
    /// [`ImguiPluginInstallError::DriverSchedule`] when the anchor is invalid; direct Bevy plugin
    /// installation is the explicit panic convenience path.
    #[must_use]
    pub fn with_driver_before(mut self, anchor: impl ScheduleLabel) -> Self {
        self.driver_schedule = schedule::ImguiDriverSchedulePlacement::before(anchor);
        self
    }

    /// Run the serial Context driver immediately after `anchor` in Bevy's main schedule order.
    ///
    /// The resulting placement must remain after `PreUpdate` completes and before `PostUpdate`
    /// begins. [`crate::ImguiAppExt::try_install_imgui`] returns
    /// [`ImguiPluginInstallError::DriverSchedule`] when the anchor is invalid; direct Bevy plugin
    /// installation is the explicit panic convenience path.
    #[must_use]
    pub fn with_driver_after(mut self, anchor: impl ScheduleLabel) -> Self {
        self.driver_schedule = schedule::ImguiDriverSchedulePlacement::after(anchor);
        self
    }

    /// Return the configured main-schedule placement of the serial Context driver.
    #[must_use]
    pub const fn driver_schedule(&self) -> schedule::ImguiDriverSchedulePlacement {
        self.driver_schedule
    }
}

impl Default for ImguiPluginConfig {
    fn default() -> Self {
        Self {
            docking: true,
            multi_viewport: false,
            viewport_window: ImguiViewportWindowConfig::default(),
            driver_schedule: schedule::ImguiDriverSchedulePlacement::default(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(crate) struct ImguiBackendRuntime {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    config: ImguiPluginConfig,
    #[cfg(feature = "render")]
    render_integration_installed: bool,
}

impl ImguiBackendRuntime {
    pub(crate) fn new(config: ImguiPluginConfig, render_integration_installed: bool) -> Self {
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let _ = config;
        #[cfg(not(feature = "render"))]
        let _ = render_integration_installed;
        Self {
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            config,
            #[cfg(feature = "render")]
            render_integration_installed,
        }
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) const fn config(&self) -> &ImguiPluginConfig {
        &self.config
    }

    #[cfg(feature = "render")]
    pub(crate) const fn render_integration_installed(&self) -> bool {
        self.render_integration_installed
    }
}
