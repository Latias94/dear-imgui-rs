//! Experimental Bevy-native backend for `dear-imgui-rs`.
//!
//! This crate is the Bevy-side integration point for the Bevy Native Backend workstream. It is not
//! a wrapper around `dear-imgui-winit` or `dear-imgui-wgpu`: Bevy owns windows, input, WGPU device
//! state, render schedules, and camera targets. The backend owns only the Bevy plugin/resources that
//! adapt those systems to Dear ImGui.
//!
//! # Compatibility and gates
//!
//! The first proof target is Bevy `0.19.0-rc.2`, which declares Rust `1.95.0`. The root
//! `dear-imgui-rs` workspace currently remains on Rust `1.92`, so this crate has a dedicated
//! `rust-version = "1.95.0"` and should be validated with an explicit Bevy gate, for example:
//!
//! ```text
//! cargo +stable check -p dear-imgui-bevy --no-default-features
//! cargo +stable check -p dear-imgui-bevy --features render
//! cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --no-default-features
//! cargo +stable check -p dear-imgui-bevy --target wasm32-unknown-unknown --features render
//! cargo +stable nextest run -p dear-imgui-bevy
//! ```
//!
//! Core workspace gates should not silently rely on this crate until the repository-wide MSRV is
//! intentionally raised or CI has a dedicated Rust 1.95+ Bevy lane. The crate currently compiles
//! on `wasm32-unknown-unknown` for both the core and `render` feature sets; mobile targets remain a
//! platform-specific follow-on if a future Bevy target train needs a dedicated gate.
//!
//! The crate also exposes `configure_example_context` for the shared example/editor ImGui setup
//! pattern so the backend examples do not repeat the same initialization boilerplate.
//!
//! # Multi-viewport status
//!
//! `ImguiBackendConfig::multi_viewport` records an explicit request for Dear ImGui platform
//! windows. With the `multi-viewport` and `render` features on native targets, the backend installs
//! the PlatformIO lifecycle bridge, all-window input/platform feedback, and per-window render
//! routing before advertising full multi-viewport support.

pub mod context;
pub mod helpers;
pub mod input;
pub mod schedule;
pub mod texture;
pub mod viewport;

use bevy_app::{App, Plugin};
use bevy_ecs::resource::Resource;

pub use self::context::{ImguiContexts, ImguiFrameOutput, ImguiFrameState};
pub use self::helpers::configure_example_context;
pub use self::schedule::{ImguiBeginFrame, ImguiEndFrame, ImguiPrimaryContextPass};
#[cfg(feature = "render")]
pub use self::texture::ImguiBevyTextures;
pub use self::texture::ImguiTextureFeedbackQueue;
pub use self::viewport::{
    ImguiViewportBridge, ImguiViewportCamera, ImguiViewportCommand, ImguiViewportFeedback,
    ImguiViewportId, ImguiViewportSnapshot, ImguiViewportWindow,
};

const MULTI_VIEWPORT_FEATURE_ENABLED: bool = cfg!(feature = "multi-viewport");
const NATIVE_PLATFORM_TARGET: bool = !cfg!(target_arch = "wasm32");

/// Bevy plugin that installs the minimal Dear ImGui resources.
///
/// Later workstream tasks add input collection, frame scheduling, render extraction, and renderer
/// systems. For now the plugin establishes ownership boundaries and resource locations only.
#[derive(Debug, Clone, Default)]
pub struct ImguiPlugin {
    config: ImguiBackendConfig,
}

impl ImguiPlugin {
    /// Create a plugin with explicit backend configuration.
    #[must_use]
    pub fn new(config: ImguiBackendConfig) -> Self {
        Self { config }
    }

    /// Borrow the plugin configuration.
    #[must_use]
    pub fn config(&self) -> &ImguiBackendConfig {
        &self.config
    }
}

impl Plugin for ImguiPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<ImguiBackendConfig>() {
            app.insert_resource(self.config.clone());
        }
        if app.world().get_non_send::<ImguiContext>().is_none() {
            app.insert_non_send(ImguiContext::new(dear_imgui_rs::Context::create()));
        }
        schedule::install_imgui_schedules(app);
        input::install_input_mapping(app);
        context::install_context_lifecycle(app);
        viewport::install_viewport_bridge(app);
        #[cfg(feature = "render")]
        let render_integration_installed = render::install_render_extraction(app);
        #[cfg(not(feature = "render"))]
        let render_integration_installed = false;
        refresh_backend_status(app, render_integration_installed);
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "render")]
        {
            let render_integration_installed = render::install_render_extraction(_app);
            refresh_backend_status(_app, render_integration_installed);
        }
    }
}

fn refresh_backend_status(app: &mut App, render_integration_installed: bool) {
    let effective_config = app.world().resource::<ImguiBackendConfig>().clone();
    sync_backend_context_config(app, &effective_config, render_integration_installed);
    app.insert_resource(ImguiBackendStatus::from_config(
        &effective_config,
        render_integration_installed,
    ));
}

fn sync_backend_context_config(
    app: &mut App,
    config: &ImguiBackendConfig,
    render_integration_installed: bool,
) {
    let Some(mut imgui_context) = app.world_mut().get_non_send_mut::<ImguiContext>() else {
        return;
    };
    let context = imgui_context.context_mut();
    let mut config_flags = context.io().config_flags();
    if config.docking {
        config_flags.insert(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
    } else {
        config_flags.remove(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
    }
    context.io_mut().set_config_flags(config_flags);

    let _ = context.set_platform_name(Some(config.name.clone()));
    let renderer_name = render_integration_installed.then(|| config.name.clone());
    let _ = context.set_renderer_name(renderer_name);
}

/// Static configuration for the Bevy backend.
#[derive(Resource, Debug, Clone, Eq, PartialEq)]
pub struct ImguiBackendConfig {
    /// User-facing label recorded in the Dear ImGui context and diagnostics.
    pub name: String,
    /// Whether the backend should request docking support when lifecycle code wires IO flags.
    pub docking: bool,
    /// Whether the user requested Dear ImGui docking multi-viewport OS windows.
    ///
    /// This is recorded in [`ImguiBackendStatus::multi_viewport_requested`]. Full support is only
    /// advertised after the native PlatformIO lifecycle bridge, all-window input feedback, and
    /// secondary viewport render routing are all available.
    pub multi_viewport: bool,
}

impl Default for ImguiBackendConfig {
    fn default() -> Self {
        Self {
            name: "dear-imgui-bevy".to_owned(),
            docking: true,
            multi_viewport: false,
        }
    }
}

/// Observable backend state installed by [`ImguiPlugin`].
#[derive(Resource, Debug, Clone, Eq, PartialEq)]
pub struct ImguiBackendStatus {
    /// First Bevy version targeted by this crate skeleton.
    pub bevy_target: &'static str,
    /// Rust version required by the Bevy target train.
    pub rust_target: &'static str,
    /// Whether render integration has been compiled in.
    pub render_feature_enabled: bool,
    /// Whether render-world extraction and overlay systems were installed into Bevy's `RenderApp`.
    pub render_integration_installed: bool,
    /// Whether the current backend configuration requested Dear ImGui platform windows.
    pub multi_viewport_requested: bool,
    /// Whether the Cargo feature needed to compile PlatformIO viewport callbacks is enabled.
    pub multi_viewport_feature_enabled: bool,
    /// Whether the current target can use native Bevy OS windows for Dear ImGui platform windows.
    pub native_platform_target: bool,
    /// Whether PlatformIO lifecycle callbacks can be connected to Bevy-owned window entities.
    pub viewport_lifecycle_bridge_enabled: bool,
    /// Whether input, focus, cursor, DPI, and IME feedback covers all Dear ImGui platform windows.
    pub viewport_input_feedback_enabled: bool,
    /// Whether secondary Dear ImGui viewport draw data is routed to matching Bevy window targets.
    pub viewport_render_routing_enabled: bool,
    /// Whether the backend currently wires the required Bevy OS-window platform callbacks.
    ///
    /// This remains `false` until lifecycle, input feedback, and renderer routing are all wired.
    /// The `multi-viewport` feature may install an internal lifecycle bridge before the backend is
    /// ready to advertise full Dear ImGui OS-level viewport support.
    pub multi_viewport_supported: bool,
}

impl ImguiBackendStatus {
    fn from_config(config: &ImguiBackendConfig, render_integration_installed: bool) -> Self {
        let viewport_lifecycle_bridge_enabled =
            config.multi_viewport && MULTI_VIEWPORT_FEATURE_ENABLED && NATIVE_PLATFORM_TARGET;
        let viewport_input_feedback_enabled =
            config.multi_viewport && MULTI_VIEWPORT_FEATURE_ENABLED && NATIVE_PLATFORM_TARGET;
        let viewport_render_routing_enabled = config.multi_viewport
            && MULTI_VIEWPORT_FEATURE_ENABLED
            && NATIVE_PLATFORM_TARGET
            && render_integration_installed;

        Self {
            bevy_target: BEVY_TARGET_VERSION,
            rust_target: RUST_TARGET_VERSION,
            render_feature_enabled: cfg!(feature = "render"),
            render_integration_installed,
            multi_viewport_requested: config.multi_viewport,
            multi_viewport_feature_enabled: MULTI_VIEWPORT_FEATURE_ENABLED,
            native_platform_target: NATIVE_PLATFORM_TARGET,
            viewport_lifecycle_bridge_enabled,
            viewport_input_feedback_enabled,
            viewport_render_routing_enabled,
            multi_viewport_supported: viewport_lifecycle_bridge_enabled
                && viewport_input_feedback_enabled
                && viewport_render_routing_enabled,
        }
    }
}

impl Default for ImguiBackendStatus {
    fn default() -> Self {
        Self::from_config(&ImguiBackendConfig::default(), false)
    }
}

/// Non-send Bevy resource that owns the Dear ImGui context.
///
/// Dear ImGui has process-global current-context state and `dear_imgui_rs::Context` is intentionally
/// not `Send`/`Sync`. Storing it as a Bevy non-send resource keeps UI lifecycle work on the main
/// thread until later tasks add schedule-specific accessors.
pub struct ImguiContext {
    context: dear_imgui_rs::Context,
}

impl ImguiContext {
    /// Wrap an existing Dear ImGui context for insertion into a Bevy world.
    #[must_use]
    pub fn new(context: dear_imgui_rs::Context) -> Self {
        Self { context }
    }

    /// Borrow the inner Dear ImGui context.
    #[must_use]
    pub fn context(&self) -> &dear_imgui_rs::Context {
        &self.context
    }

    /// Mutably borrow the inner Dear ImGui context.
    #[must_use]
    pub fn context_mut(&mut self) -> &mut dear_imgui_rs::Context {
        &mut self.context
    }

    /// Consume the wrapper and return the Dear ImGui context.
    #[must_use]
    pub fn into_inner(mut self) -> dear_imgui_rs::Context {
        self.clear_platform_backend_data();
        let this = std::mem::ManuallyDrop::new(self);
        // SAFETY: `this` will not run `Drop`, and we return ownership of the inner context to the
        // caller exactly once.
        unsafe { std::ptr::read(&this.context) }
    }

    fn clear_platform_backend_data(&mut self) {
        #[cfg(feature = "multi-viewport")]
        {
            self.context.destroy_platform_windows();
            self.context
                .io_mut()
                .set_backend_platform_user_data(std::ptr::null_mut());
            self.context.platform_io_mut().clear_platform_handlers();
        }
    }
}

impl Drop for ImguiContext {
    fn drop(&mut self) {
        self.clear_platform_backend_data();
    }
}

/// First Bevy version targeted by this crate.
pub const BEVY_TARGET_VERSION: &str = "0.19.0-rc.2";
/// Bevy reference commit used by the workstream.
pub const BEVY_TARGET_COMMIT: &str = "a389b928aee5906928a16a7d4e66cb02c7362901";
/// Rust version required by the first Bevy target train.
pub const RUST_TARGET_VERSION: &str = "1.95.0";
/// WGPU version used by Bevy `0.19.0-rc.2`.
pub const WGPU_TARGET_VERSION: &str = "29.0.3";

#[cfg(feature = "render")]
pub mod render;
