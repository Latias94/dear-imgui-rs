//! Dear ImGui platform-viewport bridge for Bevy-owned windows.
//!
//! PlatformIO callbacks installed here only capture intent into an engine-owned queue. Bevy systems
//! drain that queue and mutate ECS-owned [`Window`] entities outside the C ABI callback boundary.

mod capability;
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
mod desktop;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod error;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
mod geometry;
mod identity;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) mod native_window;
mod protocol;
mod runtime;
mod window;

use bevy_app::App;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_app::{Last, PreUpdate};
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_camera::{Camera, Camera2d, RenderTarget, visibility::RenderLayers};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::message::{MessageReader, Messages};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::prelude::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::system::SystemParam;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_render::camera::CameraRenderGraph;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::Window;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::WindowPosition;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{
    CursorOptions, ExitSystems, PrimaryWindow, WindowCloseRequested, WindowClosing, WindowOccluded,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::WinitSettings;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_rs as imgui;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_rs::sys;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::cell::{Cell, RefCell};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::collections::HashMap;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::collections::HashSet;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::ffi::{CStr, c_char, c_void};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Rc;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::rc::Weak;

pub use capability::{ImguiNativeViewportStatus, ImguiNativeViewportSupport};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use desktop::ImguiMonitorPublication;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use desktop::{
    desktop_metrics_for_window, desktop_to_window_client_logical,
    monitor_publication_from_snapshot_set, viewport_feedback_from_window,
    window_client_logical_to_desktop,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use desktop::{
    feedback_from_window_for_entity, finite_desktop_pos, finite_desktop_size,
    physical_outer_pos_for_client_pos, physical_pos_from_desktop, positive_finite_or,
    set_window_desktop_size, winit_window_decoration_offset_desktop,
};
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
use desktop::{monitor_from_window, window_position_desktop};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use error::ImguiViewportCallbackInstallError;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub use error::{ImguiViewportCallbackOwnershipError, ImguiViewportRuntimeError};
pub(crate) use identity::ImguiViewportOwner;
pub use identity::{ImguiViewportCamera, ImguiViewportWindow};
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
pub(crate) use protocol::ImguiViewportSnapshot;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use protocol::{ImguiViewportCommand, ImguiViewportFeedback};
pub use protocol::{ImguiViewportId, ImguiViewportInstanceId};
pub(crate) use runtime::install_viewport_bridge;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use runtime::{
    ImguiViewportBridge, ImguiViewportBridgeAttachmentMarker, ImguiViewportBridgeKeepalive,
    ImguiViewportBridgeRegistration, ImguiViewportBridgeShared, NativeViewportFrameSupport,
    begin_owned_bridge_release, finish_owned_bridge_release, finish_viewport_ecs_release,
    install_owned_platform_callbacks, platform_callback_error, platform_callback_ownership,
    platform_capabilities_still_owned, preflight_owned_platform_callbacks,
    preflight_platform_callback_ownership, prepare_platform_viewports_for_frame,
    record_owned_platform_name, retire_native_viewport_windows,
    viewport_bridge_teardown_attachment, viewport_ecs_release_pending,
};
#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) use runtime::{
    ImguiViewportBridgeContext, settle_pending_client_placements,
    track_viewport_ecs_despawn_for_test,
};
#[cfg(test)]
pub(crate) use window::window_from_snapshot;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use window::window_from_snapshot_with_config;
pub use window::{ImguiViewportWindowConfig, ImguiViewportWindowConfigError};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use window::{
    apply_snapshot_to_window, apply_viewport_flags_to_cursor_options,
    apply_viewport_flags_to_window, feedback_from_snapshot,
};
