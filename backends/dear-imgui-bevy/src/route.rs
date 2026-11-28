//! Main-world render and input routing for Dear ImGui Contexts.
//!
//! Route components are declarations placed on independent ECS entities. They do not own a Dear
//! ImGui Context or a Bevy camera. The resolver validates every declaration against the current
//! world and atomically replaces an internal immutable snapshot for the new epoch.
//!
//! Resolution bootstraps in `PostStartup`, then runs in `PostUpdate` after Bevy's camera update
//! set. Input and Context framing share the published immutable epoch in the next frame, and each
//! render snapshot carries it through extraction.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;

use bevy_app::{App, PostStartup, PostUpdate};
use bevy_asset::Assets;
use bevy_camera::{
    Camera, CameraOutputMode, CameraUpdateSystems, NormalizedRenderTarget, RenderTarget,
    RenderTargetInfo, Viewport,
};
use bevy_core_pipeline::{Core2d, Core3d};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{InternedScheduleLabel, IntoScheduleConfigs, ScheduleLabel};
use bevy_ecs::system::SystemParam;
use bevy_image::Image;
use bevy_math::{Rect, UVec2, Vec2};
use bevy_render::{
    camera::{CameraRenderGraph, MissingRenderTargetInfoError, NormalizedRenderTargetExt},
    texture::ManualTextureViews,
};
use bevy_window::{PrimaryWindow, Window};
use dear_imgui_rs::ContextId;

use crate::context::ImguiContexts;

mod diagnostics;
mod resolver;

pub use diagnostics::{
    ImguiDiagnostic, ImguiDiagnosticKind, ImguiDiagnosticOrigin, ImguiDiagnosticRecord,
    ImguiDiagnostics, ImguiNativeMonitorFailure, ImguiNativeMonitorFallback,
    ImguiNativeViewportPolicyFailure, ImguiRenderTargetKind,
};
#[cfg(test)]
#[path = "route/tests/routing.rs"]
mod routing_tests;

pub(crate) fn install_route_resolution(app: &mut App) {
    app.init_resource::<ImguiResolvedRoutes>()
        .init_resource::<ImguiDiagnostics>()
        .add_systems(PostStartup, resolver::resolve_imgui_routes)
        .add_systems(
            PostUpdate,
            resolver::resolve_imgui_routes.after(CameraUpdateSystems),
        );
}

/// An explicit main-viewport render association.
///
/// Put this component on a dedicated route entity. A Context may have at most one ordinary
/// main-viewport render route. Native Dear ImGui platform windows use backend-owned routes and do
/// not count toward that limit.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiRenderRoute {
    context_id: ContextId,
    camera: Entity,
    order: isize,
}

impl ImguiRenderRoute {
    /// Associate `context_id` with `camera` at the default overlay order.
    #[must_use]
    pub const fn new(context_id: ContextId, camera: Entity) -> Self {
        Self {
            context_id,
            camera,
            order: 0,
        }
    }

    /// Set the overlay order used when several Contexts draw on the same camera.
    #[must_use]
    pub const fn with_order(mut self, order: isize) -> Self {
        self.order = order;
        self
    }

    /// Return the Context selected by this declaration.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Return the Bevy camera selected by this declaration.
    #[must_use]
    pub const fn camera(&self) -> Entity {
        self.camera
    }

    /// Return the overlay order for this declaration.
    #[must_use]
    pub const fn order(&self) -> isize {
        self.order
    }
}

/// Camera-backed input source data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiCameraInputSource {
    camera: Entity,
}

impl ImguiCameraInputSource {
    /// Create a source whose host window and logical region are derived from a camera viewport.
    #[must_use]
    pub const fn new(camera: Entity) -> Self {
        Self { camera }
    }

    /// Return the camera used to derive the input region.
    #[must_use]
    pub const fn camera(&self) -> Entity {
        self.camera
    }
}

/// Explicit logical input region for an offscreen or otherwise custom presentation.
///
/// `region` is expressed in the host window's logical coordinates. Pointer positions inside this
/// region are mapped to the routed Context's display coordinates by the input backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImguiLogicalInputSource {
    window: Entity,
    region: Rect,
}

impl ImguiLogicalInputSource {
    /// Create a logical input region in `window`.
    #[must_use]
    pub const fn new(window: Entity, region: Rect) -> Self {
        Self { window, region }
    }

    /// Return the host OS window.
    #[must_use]
    pub const fn window(&self) -> Entity {
        self.window
    }

    /// Return the region in host-window logical coordinates.
    #[must_use]
    pub const fn region(&self) -> Rect {
        self.region
    }
}

/// Source from which one Context receives input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImguiInputSource {
    /// Derive the host window and logical input rectangle from a window-target camera.
    Camera(ImguiCameraInputSource),
    /// Use an explicit host-window logical rectangle.
    Logical(ImguiLogicalInputSource),
}

impl ImguiInputSource {
    /// Derive input from a window-target camera.
    #[must_use]
    pub const fn camera(camera: Entity) -> Self {
        Self::Camera(ImguiCameraInputSource::new(camera))
    }

    /// Map an explicit host-window logical rectangle to the Context.
    #[must_use]
    pub const fn logical(window: Entity, region: Rect) -> Self {
        Self::Logical(ImguiLogicalInputSource::new(window, region))
    }

    /// Return the camera source, if this source is camera-backed.
    #[must_use]
    pub const fn as_camera(&self) -> Option<&ImguiCameraInputSource> {
        match self {
            Self::Camera(source) => Some(source),
            Self::Logical(_) => None,
        }
    }

    /// Return the logical source, if this source has an explicit logical region.
    #[must_use]
    pub const fn as_logical(&self) -> Option<&ImguiLogicalInputSource> {
        match self {
            Self::Camera(_) => None,
            Self::Logical(source) => Some(source),
        }
    }
}

/// Arbitration policy for Contexts whose input regions share a host window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiInputPolicy {
    /// Give input to the unique highest-priority exclusive route under the pointer.
    Exclusive {
        /// Priority used when exclusive regions overlap.
        priority: i32,
    },
    /// Observe the same input as every other matching shared route.
    Shared,
    /// Disable input for this Context while retaining an explicit declaration.
    Disabled,
}

impl Default for ImguiInputPolicy {
    fn default() -> Self {
        Self::Exclusive { priority: 0 }
    }
}

impl ImguiInputPolicy {
    /// Construct an exclusive policy with `priority`.
    #[must_use]
    pub const fn exclusive(priority: i32) -> Self {
        Self::Exclusive { priority }
    }

    /// Return the exclusive priority, or `None` for shared and disabled policies.
    #[must_use]
    pub const fn priority(self) -> Option<i32> {
        match self {
            Self::Exclusive { priority } => Some(priority),
            Self::Shared | Self::Disabled => None,
        }
    }
}

/// An explicit Context input association.
///
/// Input routing is independent from render routing. In particular, image and manual texture
/// targets never acquire OS input merely because they have a render route.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ImguiInputRoute {
    context_id: ContextId,
    source: ImguiInputSource,
    policy: ImguiInputPolicy,
}

impl ImguiInputRoute {
    /// Create an input declaration with the default exclusive policy.
    #[must_use]
    pub const fn new(context_id: ContextId, source: ImguiInputSource) -> Self {
        Self {
            context_id,
            source,
            policy: ImguiInputPolicy::Exclusive { priority: 0 },
        }
    }

    /// Create a camera-derived input declaration.
    #[must_use]
    pub const fn from_camera(context_id: ContextId, camera: Entity) -> Self {
        Self::new(context_id, ImguiInputSource::camera(camera))
    }

    /// Create an explicit logical input declaration.
    #[must_use]
    pub const fn logical(context_id: ContextId, window: Entity, region: Rect) -> Self {
        Self::new(context_id, ImguiInputSource::logical(window, region))
    }

    /// Set how this route arbitrates overlapping input.
    #[must_use]
    pub const fn with_policy(mut self, policy: ImguiInputPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Return the Context selected by this declaration.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Borrow the input source.
    #[must_use]
    pub const fn source(&self) -> &ImguiInputSource {
        &self.source
    }

    /// Return the arbitration policy.
    #[must_use]
    pub const fn policy(&self) -> ImguiInputPolicy {
        self.policy
    }
}

/// Whether a resolved render route was selected automatically or explicitly.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImguiRenderRouteSource {
    /// The primary Context was assigned to the unique eligible primary-window camera.
    AutoPrimary,
    /// A user-owned route entity declared the association.
    Explicit,
}

/// One immutable main-world render route for an epoch.
#[derive(Clone, Debug)]
pub(crate) struct ImguiResolvedRenderRoute {
    context_id: ContextId,
    route_entity: Option<Entity>,
    camera: Entity,
    order: isize,
    camera_order: isize,
    camera_schedule: InternedScheduleLabel,
    #[cfg(test)]
    source: ImguiRenderRouteSource,
    target: NormalizedRenderTarget,
    target_info: RenderTargetInfo,
    camera_viewport: Option<Viewport>,
}

impl ImguiResolvedRenderRoute {
    /// Return the routed Context.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Return the user-owned declaration entity, or `None` for `AutoPrimary`.
    #[must_use]
    pub const fn route_entity(&self) -> Option<Entity> {
        self.route_entity
    }

    /// Return the routed Bevy camera.
    #[must_use]
    pub const fn camera(&self) -> Entity {
        self.camera
    }

    /// Return the overlay order among Contexts routed to this camera.
    #[must_use]
    pub const fn order(&self) -> isize {
        self.order
    }

    /// Return the camera's Bevy render order.
    #[must_use]
    pub const fn camera_order(&self) -> isize {
        self.camera_order
    }

    /// Return the camera render-graph schedule captured for this epoch.
    #[must_use]
    pub const fn camera_schedule(&self) -> InternedScheduleLabel {
        self.camera_schedule
    }

    /// Return how this route was selected.
    #[cfg(test)]
    #[must_use]
    pub const fn source(&self) -> ImguiRenderRouteSource {
        self.source
    }

    /// Borrow the normalized target captured for this epoch.
    #[must_use]
    pub const fn target(&self) -> &NormalizedRenderTarget {
        &self.target
    }

    /// Borrow the target dimensions and scale captured for this epoch.
    #[must_use]
    pub const fn target_info(&self) -> &RenderTargetInfo {
        &self.target_info
    }

    /// Borrow the physical camera viewport captured for this epoch.
    #[must_use]
    pub const fn camera_viewport(&self) -> Option<&Viewport> {
        self.camera_viewport.as_ref()
    }

    /// Return the actual physical output size after applying a camera viewport.
    #[must_use]
    pub fn physical_output_size(&self) -> UVec2 {
        self.camera_viewport
            .as_ref()
            .map_or(self.target_info.physical_size, |viewport| {
                viewport.physical_size
            })
    }

    pub(crate) fn host_window(&self) -> Option<Entity> {
        match &self.target {
            NormalizedRenderTarget::Window(window) => Some(window.entity()),
            _ => None,
        }
    }
}

/// Immutable render-route topology captured for one driven frame.
#[derive(Clone, Debug, Default)]
pub(crate) struct ImguiRenderRouteEpoch {
    epoch: u64,
    routes: Arc<[ImguiResolvedRenderRoute]>,
}

impl ImguiRenderRouteEpoch {
    /// Return the resolver epoch that produced these routes.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Borrow the render routes that were active when the frame was driven.
    #[must_use]
    pub fn render_routes(&self) -> &[ImguiResolvedRenderRoute] {
        &self.routes
    }
}

/// One immutable main-world input route for an epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImguiResolvedInputRoute {
    context_id: ContextId,
    route_entity: Option<Entity>,
    source: ImguiInputSource,
    policy: ImguiInputPolicy,
    host_window: Entity,
    logical_region: Rect,
}

impl ImguiResolvedInputRoute {
    /// Return the routed Context.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Return the source declaration.
    #[must_use]
    pub const fn source(&self) -> ImguiInputSource {
        self.source
    }

    /// Return the arbitration policy.
    #[must_use]
    pub const fn policy(&self) -> ImguiInputPolicy {
        self.policy
    }

    /// Return the host OS window.
    #[must_use]
    pub const fn host_window(&self) -> Entity {
        self.host_window
    }

    /// Return the effective input region in host-window logical coordinates.
    #[must_use]
    pub const fn logical_region(&self) -> Rect {
        self.logical_region
    }
}

/// Atomic main-world route snapshot produced for one resolver epoch.
#[derive(Resource, Default, Debug)]
pub(crate) struct ImguiResolvedRoutes {
    epoch: u64,
    render_routes: Arc<[ImguiResolvedRenderRoute]>,
    input_routes: Arc<[ImguiResolvedInputRoute]>,
}

impl ImguiResolvedRoutes {
    /// Return the latest resolver epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Borrow resolved render routes in stable camera/overlay order.
    #[cfg(test)]
    #[must_use]
    pub fn render_routes(&self) -> &[ImguiResolvedRenderRoute] {
        &self.render_routes
    }

    /// Borrow resolved input routes in stable host-window/Context order.
    #[must_use]
    pub fn input_routes(&self) -> &[ImguiResolvedInputRoute] {
        &self.input_routes
    }

    /// Find the ordinary main-viewport render route for one Context.
    #[must_use]
    pub fn render_route(&self, context_id: ContextId) -> Option<&ImguiResolvedRenderRoute> {
        self.render_routes
            .iter()
            .find(|route| route.context_id == context_id)
    }

    /// Find the active input route for one Context.
    #[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    #[must_use]
    pub fn input_route(&self, context_id: ContextId) -> Option<&ImguiResolvedInputRoute> {
        self.input_routes
            .iter()
            .find(|route| route.context_id == context_id)
    }

    /// Capture the render topology used by a frame before later schedules can resolve a new epoch.
    #[must_use]
    pub(crate) fn render_epoch(&self) -> ImguiRenderRouteEpoch {
        ImguiRenderRouteEpoch {
            epoch: self.epoch,
            routes: Arc::clone(&self.render_routes),
        }
    }

    fn replace(
        &mut self,
        render_routes: Vec<ImguiResolvedRenderRoute>,
        input_routes: Vec<ImguiResolvedInputRoute>,
    ) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.render_routes = render_routes.into();
        self.input_routes = input_routes.into();
        self.epoch
    }
}
