//! Bevy-native integration for `dear-imgui-rs`.
//!
//! Dear ImGui Contexts remain main-thread owned and are driven serially through explicit Bevy
//! schedules. Rendering, input, and native viewport support build on those Context identities.
//!
//! The default features provide the renderer and deterministic ordering with Bevy UI. A normal
//! primary-window application needs only a camera, the plugin, and a UI system:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use dear_imgui_bevy::prelude::*;
//!
//! fn draw_ui(imgui: ImguiUi) {
//!     let Ok(ui) = imgui.ui() else {
//!         return;
//!     };
//!     ui.window("Hello").build(|| ui.text("Dear ImGui in Bevy"));
//! }
//!
//! App::new()
//!     .add_plugins((DefaultPlugins, ImguiPlugin::default()))
//!     .add_systems(Startup, |mut commands: Commands| {
//!         commands.spawn(Camera2d);
//!     })
//!     .add_systems(ImguiPrimaryContextPass, draw_ui)
//!     .run();
//! ```
//!
//! Advanced targets use explicit render and input route entities:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use dear_imgui_bevy::prelude::*;
//!
//! fn route_primary_context(
//!     mut commands: Commands,
//!     contexts: NonSend<ImguiContexts>,
//!     cameras: Query<Entity, With<Camera>>,
//! ) {
//!     let context = contexts.primary_id().expect("the plugin installs a primary Context");
//!     let camera = cameras.single().expect("this example has one camera");
//!     commands.spawn((
//!         ImguiRenderRoute::new(context, camera),
//!         ImguiInputRoute::from_camera(context, camera),
//!     ));
//! }
//! ```
//!
//! Superseded configuration, camera-marker, helper, and renderer-storage APIs are intentionally
//! unavailable:
//!
//! ```compile_fail
//! use dear_imgui_bevy::{ImguiBackendConfig, ImguiBackendStatus, configure_example_context};
//! ```
//!
//! ```compile_fail
//! use dear_imgui_bevy::render::{ImguiOverlayCamera, ImguiOverlayDisabled};
//! ```
//!
//! ```compile_fail
//! use dear_imgui_bevy::{ImguiBeginFrame, ImguiEndFrame, RenderFeature};
//! ```
//!
//! ```compile_fail
//! use dear_imgui_bevy::render::ImguiPreparedRenderFrame;
//! ```
//!
//! Input translation is plugin-owned. Consumers read capture snapshots instead of mutating
//! backend state:
//!
//! ```no_run
//! use bevy_ecs::prelude::Res;
//! use dear_imgui_bevy::input::ImguiInputCapture;
//!
//! fn gameplay_enabled(capture: Res<ImguiInputCapture>) -> bool {
//!     let snapshot = capture.aggregate();
//!     !snapshot.wants_keyboard_input()
//! }
//! ```
//!
//! ```compile_fail
//! use dear_imgui_bevy::input::{ImguiInputState, primary_window_input_system};
//! ```
//!
//! Native viewport markers are read through accessors rather than copied or field-projected:
//!
//! ```compile_fail
//! use dear_imgui_bevy::{ImguiViewportId, ImguiViewportWindow};
//!
//! fn old_viewport_id(marker: &ImguiViewportWindow) -> ImguiViewportId {
//!     marker.viewport_id
//! }
//! ```

#[cfg(test)]
extern crate self as dear_imgui_bevy;

pub mod context;
pub mod input;
pub mod prelude;
pub mod schedule;
pub mod texture;
pub mod viewport;

#[cfg(feature = "render")]
pub mod route;

pub use dear_imgui_rs::ContextId;

#[cfg(feature = "render")]
pub use self::context::ownership::ImguiRendererOwnershipError;
pub use self::context::ownership::{
    ImguiContextRemovalPendingReason, ImguiPlugin, ImguiPluginConfig,
};
pub use self::context::{
    ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError, ImguiContexts, ImguiUi,
};
#[cfg(feature = "render")]
pub use self::render::ImguiRenderSystems;
#[cfg(feature = "bevy-ui")]
pub use self::render::ImguiUiRenderOrder;
pub use self::schedule::ImguiPrimaryContextPass;
#[cfg(feature = "render")]
pub use self::texture::{ImguiBevyTextures, ImguiTexture, ImguiTextureRegistrationError};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use self::viewport::ImguiViewportBridge;
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use self::viewport::ImguiViewportFeedback;
#[cfg(test)]
pub(crate) use self::viewport::ImguiViewportSnapshot;
pub use self::viewport::{
    ImguiViewportCamera, ImguiViewportId, ImguiViewportWindow, ImguiViewportWindowConfig,
    ImguiViewportWindowConfigError,
};

/// Current Bevy version targeted by this crate.
pub const BEVY_TARGET_VERSION: &str = "0.19.0";
/// Bevy reference commit used by the workstream.
pub const BEVY_TARGET_COMMIT: &str = "c6f634ca9f406d68ba5109d921247b654cb42c10";
/// Rust version required by the current Bevy target train.
pub const RUST_TARGET_VERSION: &str = "1.95.0";
/// WGPU version used by Bevy `0.19.0`.
pub const WGPU_TARGET_VERSION: &str = "29.0.3";

#[cfg(feature = "render")]
mod render;
#[cfg(test)]
mod test_util;
