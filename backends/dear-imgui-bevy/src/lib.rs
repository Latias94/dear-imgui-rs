//! Bevy-native integration for `dear-imgui-rs`.
//!
//! Dear ImGui Contexts remain main-thread owned and are driven serially through explicit Bevy
//! schedules. Rendering, input, and native viewport support build on those Context identities.

pub mod context;
pub mod helpers;
pub mod input;
pub mod schedule;
pub mod texture;
pub mod viewport;

pub use dear_imgui_rs::ContextId;

#[cfg(feature = "render")]
pub use self::context::ownership::ImguiRendererOwnershipError;
pub use self::context::ownership::{
    ImguiBackendConfig, ImguiBackendStatus, ImguiContextIntoInnerErrorReason, ImguiPlugin,
};
pub use self::context::{
    ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError, ImguiContexts,
    ImguiFrameOutput, ImguiFrameState, ImguiUi,
};
pub use self::helpers::configure_example_context;
pub use self::schedule::ImguiPrimaryContextPass;
#[cfg(feature = "render")]
pub use self::texture::ImguiBevyTextures;
pub use self::viewport::{
    ImguiViewportBridge, ImguiViewportCamera, ImguiViewportCommand, ImguiViewportFeedback,
    ImguiViewportId, ImguiViewportSnapshot, ImguiViewportWindow, ImguiViewportWindowConfig,
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
pub mod render;
