//! Bevy plugin installation and backend configuration.

use bevy_app::{App, Plugin};
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

/// Bevy plugin that owns Dear ImGui Context, input, render, and viewport integration.
#[derive(Debug, Clone, Default)]
pub struct ImguiPlugin {
    config: ImguiPluginConfig,
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
}

impl Plugin for ImguiPlugin {
    fn build(&self, app: &mut App) {
        assert!(
            !self.config.multi_viewport() || cfg!(feature = "multi-viewport"),
            "ImguiPluginConfig requested native platform windows; enable the `multi-viewport` Cargo feature"
        );
        self.config
            .viewport_window()
            .validate()
            .unwrap_or_else(|error| panic!("invalid Dear ImGui viewport window policy: {error}"));
        super::pass::install_pass_registry(app);
        let primary_pass = super::pass::primary_pass(app);
        let lifecycle = super::pass::lifecycle(app.world());
        if app.world().get_non_send::<ImguiContexts>().is_none() {
            assert!(
                !lifecycle.registry_claimed(),
                "ImguiContexts was removed after installation; reinsert the original registry or shut the App down"
            );
            app.insert_non_send(ImguiContexts::with_primary(
                dear_imgui_rs::SuspendedContext::create(),
                primary_pass,
                lifecycle,
            ));
        } else {
            assert!(
                lifecycle.registry_claimed(),
                "ImguiContexts exists without the App lifecycle ownership claim"
            );
            let registry_id = super::pass::registry_id(app.world());
            assert_eq!(
                app.world()
                    .get_non_send::<ImguiContexts>()
                    .expect("the Context registry was just checked")
                    .pass_registry_id(),
                registry_id,
                "ImguiContexts was created with pass handles from another App"
            );
        }
        schedule::install_imgui_schedules(app, self.config.driver_schedule());
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
        refresh_backend_contract(app, self.config.clone(), render_integration_installed);
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "render")]
        {
            let render_integration_installed =
                render::install_render_extraction(_app, self.resolved_ui_render_order());
            refresh_backend_contract(_app, self.config.clone(), render_integration_installed);
        }
    }
}

fn refresh_backend_contract(
    app: &mut App,
    config: ImguiPluginConfig,
    render_integration_installed: bool,
) {
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
    let mut contexts = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin must retain its Context registry");
    contexts.set_primary_contract(config.docking(), config.multi_viewport());
    contexts.attach_backend(attachment).unwrap_or_else(|error| {
        panic!("ImguiPlugin could not attach the Dear ImGui Context registry: {error}")
    });
    app.insert_resource(ImguiBackendRuntime::new(
        config,
        render_integration_installed,
    ));
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
    /// begins. Plugin installation panics when the anchor places the driver outside that interval.
    #[must_use]
    pub fn with_driver_before(mut self, anchor: impl ScheduleLabel) -> Self {
        self.driver_schedule = schedule::ImguiDriverSchedulePlacement::before(anchor);
        self
    }

    /// Run the serial Context driver immediately after `anchor` in Bevy's main schedule order.
    ///
    /// The resulting placement must remain after `PreUpdate` completes and before `PostUpdate`
    /// begins. Plugin installation panics when the anchor places the driver outside that interval.
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
