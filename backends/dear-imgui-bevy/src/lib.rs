//! Bevy-native integration for `dear-imgui-rs`.
//!
//! Dear ImGui Contexts remain main-thread owned and are driven serially through private typed
//! passes. Rendering, input, and native viewport support build on those Context identities.
//!
//! The default features provide the renderer and deterministic ordering with Bevy UI. A normal
//! primary-window application needs only a camera, the plugin, and a UI system:
//!
//! ```no_run
//! use bevy::prelude::*;
//! use dear_imgui_bevy::prelude::*;
//!
//! fn draw_ui(frame: ImguiFrame<'_>) {
//!     let ui = frame.ui();
//!     ui.window("Hello").build(|| ui.text("Dear ImGui in Bevy"));
//! }
//!
//! let mut app = App::new();
//! app.add_plugins(DefaultPlugins);
//! app.try_install_imgui(ImguiPlugin::default())
//!     .expect("the Dear ImGui configuration is valid");
//! app
//!     .add_systems(Startup, |mut commands: Commands| {
//!         commands.spawn(Camera2d);
//!     });
//! let primary_pass = app
//!     .imgui_primary_pass()
//!     .expect("the private pass registry must be active");
//! app.add_imgui_systems(&primary_pass, primary_pass.system(draw_ui))
//!     .expect("the system must be bound to this App and pass");
//! app.run();
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
//!     let context = contexts
//!         .primary_id()
//!         .expect("the Context registry must be active")
//!         .expect("the plugin installs a primary Context");
//!     let camera = cameras.single().expect("this example has one camera");
//!     commands.spawn((
//!         ImguiRenderRoute::new(context, camera),
//!         ImguiInputRoute::from_camera(context, camera),
//!     ));
//! }
//! ```
//!
//! ## Color Space and Compositing
//!
//! Dear ImGui vertex colors interpolate in sRGB across triangles and decode to linear light in the
//! fragment shader before texture modulation, while alpha remains linear. Output encoding follows
//! Bevy's resolved camera compositing space (`Linear`, `Srgb`, or `Oklab`).
//!
//! In Bevy `0.20.1`, `Camera3d` views (including HDR `Rgba16Float`) always resolve to `Linear`
//! compositing. This default keeps font anti-aliasing and translucent overlays over 3D scenes
//! physically accurate, though widgets authored for sRGB-space blending (such as the black
//! value-dimming overlay in `ColorPicker3` / `ColorPicker4`) look slightly brighter in dark
//! midtones. To blend Dear ImGui widgets in sRGB space over a `Camera3d`, spawn a dedicated
//! non-HDR overlay `Camera2d` with a higher `Camera::order`, transparent clear color, and
//! `CompositingSpace::Srgb`:
//!
//! ```no_run
//! use bevy::{
//!     camera::Hdr,
//!     prelude::*,
//!     render::view::Msaa,
//! };
//!
//! fn spawn_srgb_ui_overlay(mut commands: Commands) {
//!     // 3D scene camera on order 0.
//!     commands.spawn((Camera3d::default(), Hdr));
//!
//!     // Non-HDR Camera2d with CompositingSpace::Srgb allocates its own Rgba8Unorm target.
//!     // As the unique highest-order primary-window camera, the primary ImGui Context routes here
//!     // automatically.
//!     commands.spawn((
//!         Camera2d,
//!         Camera {
//!             order: 1,
//!             clear_color: ClearColorConfig::Custom(Color::NONE),
//!             ..Default::default()
//!         },
//!         CompositingSpace::Srgb,
//!         Msaa::Off,
//!     ));
//! }
//! ```
//!
//! Opaque ImGui windows (and translucent widgets inside them) blend cleanly in sRGB space in this
//! setup. Semi-transparent ImGui backgrounds directly over the 3D scene appear slightly darker
//! because Bevy `0.20.1`'s `upscaling` blit decodes sRGB without unpremultiplying RGB before
//! blending onto the window target.
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
//! ```compile_fail
//! use dear_imgui_bevy::input::ImguiInputSystems;
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

// Keep Bevy's transitive ECS macro implementation on the version pinned in Cargo.toml.
use bevy_ecs_macro_logic as _;

pub mod context;
pub mod input;
pub mod prelude;
pub mod texture;
pub mod viewport;

mod schedule;

#[cfg(feature = "render")]
pub mod route;

pub use dear_imgui_rs::ContextId;

#[cfg(feature = "render")]
pub use self::context::ImguiRendererOwnershipError;
pub use self::context::{
    ImguiAppExt, ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError,
    ImguiContextRemovalPendingReason, ImguiContextRetired, ImguiContextRetirementId,
    ImguiContextScopeError, ImguiContexts, ImguiFrame, ImguiPass, ImguiPassError, ImguiPlugin,
    ImguiPluginConfig, ImguiPluginInstallError, ImguiPrimaryChange, ImguiPrimaryPass,
    ImguiShutdownError, ImguiSystemConfigs, IntoImguiSystemConfigs,
};
#[cfg(feature = "render")]
pub use self::render::ImguiRenderSystems;
#[cfg(feature = "bevy-ui")]
pub use self::render::ImguiUiRenderOrder;
pub use self::schedule::{ImguiDriverScheduleError, ImguiDriverSchedulePlacement};
#[cfg(feature = "render")]
pub use self::texture::{ImguiBevyTextures, ImguiTexture, ImguiTextureRegistrationError};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use self::viewport::ImguiViewportBridge;
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use self::viewport::ImguiViewportFeedback;
#[cfg(test)]
pub(crate) use self::viewport::ImguiViewportSnapshot;
pub use self::viewport::{
    ImguiNativeViewportStatus, ImguiNativeViewportSupport, ImguiViewportCamera, ImguiViewportId,
    ImguiViewportInstanceId, ImguiViewportWindow, ImguiViewportWindowConfig,
    ImguiViewportWindowConfigError,
};

/// Current Bevy version targeted by this crate.
pub const BEVY_TARGET_VERSION: &str = "0.20.1";
/// Bevy reference commit used by the workstream.
pub const BEVY_TARGET_COMMIT: &str = "b3bdd75becaffe9a16677cf5bae5bd6d198f052b";
/// Rust version required by the current Bevy target train.
pub const RUST_TARGET_VERSION: &str = "1.97.1";
/// WGPU version used by Bevy `0.20.1`.
pub const WGPU_TARGET_VERSION: &str = "30.0.1";

#[cfg(feature = "render")]
mod render;
#[cfg(test)]
mod test_util;
