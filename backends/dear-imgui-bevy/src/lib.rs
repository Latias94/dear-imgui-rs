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
//! cargo +stable nextest run -p dear-imgui-bevy
//! ```
//!
//! Core workspace gates should not silently rely on this crate until the repository-wide MSRV is
//! intentionally raised or CI has a dedicated Rust 1.95+ Bevy lane.

pub mod context;
pub mod input;
pub mod schedule;
pub mod texture;

use bevy_app::{App, Plugin};
use bevy_ecs::resource::Resource;

pub use self::context::{ImguiContexts, ImguiFrameOutput, ImguiFrameState};
pub use self::schedule::{ImguiBeginFrame, ImguiEndFrame, ImguiPrimaryContextPass};
#[cfg(feature = "render")]
pub use self::texture::ImguiBevyTextures;
pub use self::texture::ImguiTextureFeedbackQueue;

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
        app.init_resource::<ImguiBackendStatus>();
        if app.world().get_non_send::<ImguiContext>().is_none() {
            app.insert_non_send(ImguiContext::new(dear_imgui_rs::Context::create()));
        }
        schedule::install_imgui_schedules(app);
        input::install_input_mapping(app);
        context::install_context_lifecycle(app);
        #[cfg(feature = "render")]
        render::install_render_extraction(app);
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "render")]
        render::install_render_extraction(_app);
    }
}

/// Static configuration for the Bevy backend.
#[derive(Resource, Debug, Clone, Eq, PartialEq)]
pub struct ImguiBackendConfig {
    /// User-facing label recorded in the Dear ImGui context and diagnostics.
    pub name: String,
    /// Whether the backend should request docking support when lifecycle code wires IO flags.
    pub docking: bool,
    /// Whether the backend should eventually wire multi-viewport support.
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
}

impl Default for ImguiBackendStatus {
    fn default() -> Self {
        Self {
            bevy_target: BEVY_TARGET_VERSION,
            rust_target: RUST_TARGET_VERSION,
            render_feature_enabled: cfg!(feature = "render"),
        }
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
    pub fn into_inner(self) -> dear_imgui_rs::Context {
        self.context
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
