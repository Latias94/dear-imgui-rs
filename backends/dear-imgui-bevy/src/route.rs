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
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use bevy_app::{App, PostStartup, PostUpdate};
use bevy_asset::{AssetId, Assets};
use bevy_camera::{
    Camera, CameraOutputMode, CameraUpdateSystems, ManualTextureViewHandle, NormalizedRenderTarget,
    RenderTarget, RenderTargetInfo, Viewport,
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

mod resolver;
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

/// Producer identity for an atomic diagnostic batch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ImguiDiagnosticOrigin {
    /// Main-world render route resolution.
    RenderRouting,
    /// Main-world input route resolution.
    InputRouting,
    /// Main-to-render-world extraction.
    RenderExtraction,
    /// Bevy image and Dear ImGui texture integration.
    Texture,
    /// Native monitor collection and work-area provenance.
    NativeMonitor,
    /// Native Dear ImGui viewport mapping and window policy.
    NativeViewport,
}

/// Stable reason that a complete native monitor publication could not be formed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ImguiNativeMonitorFailure {
    MainFactsUnavailable,
    InvalidMainGeometry,
    InvalidScaleFactor,
    InvalidWorkGeometry,
    EmptyCollection,
    ProjectionInvalid,
    Unclassified,
}

/// Stable reason that a native monitor uses its full rectangle as work area.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ImguiNativeMonitorFallback {
    Wayland,
    UnsupportedWindowSystem,
    SourceUnavailable,
    InvalidNativeData,
    MainThreadUnavailable,
    AmbiguousDesktopScope,
    Unclassified,
}

/// Stable reason for a native Dear ImGui viewport policy failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ImguiNativeViewportPolicyFailure {
    NativeWindowPending,
    WindowHandleUnavailable,
    UnexpectedHandleKind,
    WindowOwnerUnavailable,
    WrongWindowThread,
    InstallFailed,
    HookDetached,
    WindowDestroyed,
}

/// Render target category used by route diagnostics.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ImguiRenderTargetKind {
    Window,
    Image,
    ManualTextureView,
    None,
}

/// Structured reason that a backend operation was skipped or rejected.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ImguiDiagnosticKind {
    /// No entity is marked as the primary window.
    MissingPrimaryWindow,
    /// More than one entity is marked as the primary window.
    AmbiguousPrimaryWindow { count: usize },
    /// No eligible primary-window camera remained after validation.
    NoEligibleAutoPrimaryCamera,
    /// Several eligible cameras shared the highest Bevy camera order.
    AmbiguousAutoPrimary { order: isize },
    /// The declaration references a Context not owned by this registry.
    UnknownContext,
    /// More than one ordinary render route declares the same Context.
    DuplicateRenderRoute { declarations: usize },
    /// More than one input route declares the same Context.
    DuplicateInputRoute { declarations: usize },
    /// The declaration references a camera entity that no longer exists.
    MissingCamera,
    /// The camera is inactive.
    InactiveCamera,
    /// The camera does not write its completed output to the target.
    CameraDoesNotWrite,
    /// The camera does not run the Bevy Core2d or Core3d schedule.
    UnsupportedCameraSchedule,
    /// A `WindowRef::Primary` target cannot be normalized in the current world.
    UnresolvedPrimaryWindowTarget { candidates: usize },
    /// The target window entity is unavailable.
    MissingWindowTarget { window: Entity },
    /// The target image asset is unavailable in the main world.
    MissingImageTarget { image: AssetId<Image> },
    /// A registered Bevy image is unavailable or cannot be sampled by the ImGui renderer.
    UnavailableBevyImageTexture { image: AssetId<Image> },
    /// The manual texture view handle is unavailable.
    MissingManualTextureViewTarget {
        texture_view: ManualTextureViewHandle,
    },
    /// `RenderTarget::None` has no color attachment on which an overlay can compose.
    UnsupportedRenderTargetNone,
    /// The selected target or camera viewport has no drawable area.
    ZeroSizedRenderTarget { target: ImguiRenderTargetKind },
    /// The target scale factor is non-finite or not positive.
    InvalidRenderTargetScaleFactor { target: ImguiRenderTargetKind },
    /// The main-world camera did not produce a corresponding render-world view.
    MissingExtractedView,
    /// The render-world view no longer matches the immutable main-world route epoch.
    StaleExtractedView,
    /// Camera-derived input is available only for a window render target.
    InputCameraRequiresWindowTarget { target: ImguiRenderTargetKind },
    /// The explicit logical input host is not a live Window entity.
    MissingLogicalInputWindow { window: Entity },
    /// The explicit logical input rectangle is non-finite or has no area.
    InvalidLogicalInputRegion,
    /// Equal-priority exclusive input regions overlap on one host window.
    AmbiguousExclusiveInput { priority: i32 },
    /// Native monitor collection failed before a complete publication could be formed.
    NativeMonitorCollectionFailed { reason: ImguiNativeMonitorFailure },
    /// The monitor batch could not prove one unique detached primary identity.
    NativeMonitorPrimaryUnproven,
    /// A monitor uses its full rectangle because an exact work area was unavailable.
    NativeMonitorWorkAreaFallback { reason: ImguiNativeMonitorFallback },
    /// A viewport ECS window exists but its exact native Winit mapping is not ready.
    NativeViewportWindowPending {
        viewport: crate::viewport::ImguiViewportInstanceId,
    },
    /// Installing or updating the native viewport window policy failed.
    NativeViewportPolicyInstallFailed {
        viewport: crate::viewport::ImguiViewportInstanceId,
        reason: ImguiNativeViewportPolicyFailure,
    },
}

/// One diagnostic payload before or after it is published in a batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImguiDiagnostic {
    kind: ImguiDiagnosticKind,
    context_id: Option<ContextId>,
    route_entity: Option<Entity>,
    camera: Option<Entity>,
    viewport_instance: Option<crate::viewport::ImguiViewportInstanceId>,
}

impl ImguiDiagnostic {
    /// Create a diagnostic without optional ownership identities.
    #[must_use]
    pub const fn new(kind: ImguiDiagnosticKind) -> Self {
        Self {
            kind,
            context_id: None,
            route_entity: None,
            camera: None,
            viewport_instance: None,
        }
    }

    /// Attach the affected Context identity.
    #[must_use]
    pub const fn with_context(mut self, context_id: ContextId) -> Self {
        self.context_id = Some(context_id);
        self
    }

    /// Attach the affected declaration entity.
    #[must_use]
    pub const fn with_route(mut self, route_entity: Entity) -> Self {
        self.route_entity = Some(route_entity);
        self
    }

    /// Attach the affected camera entity.
    #[must_use]
    pub const fn with_camera(mut self, camera: Entity) -> Self {
        self.camera = Some(camera);
        self
    }

    /// Attach the stable native viewport instance identity.
    #[must_use]
    pub const fn with_viewport_instance(
        mut self,
        viewport_instance: crate::viewport::ImguiViewportInstanceId,
    ) -> Self {
        self.viewport_instance = Some(viewport_instance);
        self
    }

    /// Return the structured diagnostic reason.
    #[must_use]
    pub const fn kind(&self) -> &ImguiDiagnosticKind {
        &self.kind
    }

    /// Return the affected Context, if any.
    #[must_use]
    pub const fn context_id(&self) -> Option<ContextId> {
        self.context_id
    }

    /// Return the affected declaration entity, if any.
    #[must_use]
    pub const fn route_entity(&self) -> Option<Entity> {
        self.route_entity
    }

    /// Return the affected camera, if any.
    #[must_use]
    pub const fn camera(&self) -> Option<Entity> {
        self.camera
    }

    /// Return the affected stable native viewport instance, if any.
    #[must_use]
    pub const fn viewport_instance(&self) -> Option<crate::viewport::ImguiViewportInstanceId> {
        self.viewport_instance
    }

    fn stable_cmp(&self, other: &Self) -> Ordering {
        resolver::optional_context_key(self.context_id)
            .cmp(&resolver::optional_context_key(other.context_id))
            .then_with(|| self.route_entity.cmp(&other.route_entity))
            .then_with(|| self.camera.cmp(&other.camera))
            .then_with(|| self.viewport_instance.cmp(&other.viewport_instance))
            .then_with(|| self.kind.cmp(&other.kind))
    }
}

/// Published diagnostic with its producer and atomic replacement epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImguiDiagnosticRecord {
    origin: ImguiDiagnosticOrigin,
    epoch: u64,
    diagnostic: ImguiDiagnostic,
}

impl ImguiDiagnosticRecord {
    /// Return the producer that owns this diagnostic batch.
    #[must_use]
    pub const fn origin(&self) -> ImguiDiagnosticOrigin {
        self.origin
    }

    /// Return the producer-local replacement epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Borrow the structured diagnostic.
    #[must_use]
    pub const fn diagnostic(&self) -> &ImguiDiagnostic {
        &self.diagnostic
    }
}

#[derive(Clone, Debug, Default)]
struct ImguiDiagnosticBatch {
    epoch: u64,
    diagnostics: Vec<ImguiDiagnostic>,
}

/// Stable, queryable diagnostics published by backend subsystems.
///
/// Each origin owns one complete batch. Replacing a batch is atomic, and an epoch older than the
/// latest accepted epoch for that origin is ignored. This prevents delayed render-world feedback
/// from overwriting newer main-world state. Applications should access this resource read-only;
/// publication is reserved for backend subsystems.
#[derive(Resource, Clone, Debug)]
pub struct ImguiDiagnostics {
    state: Arc<RwLock<ImguiDiagnosticsState>>,
}

#[derive(Debug, Default)]
struct ImguiDiagnosticsState {
    batches: BTreeMap<ImguiDiagnosticOrigin, ImguiDiagnosticBatch>,
}

impl Default for ImguiDiagnostics {
    fn default() -> Self {
        Self {
            state: Arc::new(RwLock::new(ImguiDiagnosticsState::default())),
        }
    }
}

impl ImguiDiagnostics {
    pub(crate) fn replace(
        &self,
        origin: ImguiDiagnosticOrigin,
        epoch: u64,
        diagnostics: impl IntoIterator<Item = ImguiDiagnostic>,
    ) -> bool {
        let mut state = self.write_state();
        if state
            .batches
            .get(&origin)
            .is_some_and(|batch| epoch < batch.epoch)
        {
            return false;
        }

        let mut diagnostics = diagnostics.into_iter().collect::<Vec<_>>();
        diagnostics.sort_by(ImguiDiagnostic::stable_cmp);
        state
            .batches
            .insert(origin, ImguiDiagnosticBatch { epoch, diagnostics });
        true
    }

    /// Return the latest accepted epoch for one producer.
    #[must_use]
    pub fn epoch(&self, origin: ImguiDiagnosticOrigin) -> Option<u64> {
        self.read_state()
            .batches
            .get(&origin)
            .map(|batch| batch.epoch)
    }

    /// Return all diagnostics in stable origin/identity/kind order.
    #[must_use]
    pub fn entries(&self) -> Vec<ImguiDiagnosticRecord> {
        self.read_state()
            .batches
            .iter()
            .flat_map(|(origin, batch)| {
                batch
                    .diagnostics
                    .iter()
                    .cloned()
                    .map(|diagnostic| ImguiDiagnosticRecord {
                        origin: *origin,
                        epoch: batch.epoch,
                        diagnostic,
                    })
            })
            .collect()
    }

    /// Iterate an owned snapshot of one producer's diagnostics in stable identity/kind order.
    pub fn entries_for(
        &self,
        origin: ImguiDiagnosticOrigin,
    ) -> impl ExactSizeIterator<Item = ImguiDiagnostic> {
        self.read_state()
            .batches
            .get(&origin)
            .map_or_else(Vec::new, |batch| batch.diagnostics.clone())
            .into_iter()
    }

    /// Return whether every producer currently reports an empty batch.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.read_state()
            .batches
            .values()
            .all(|batch| batch.diagnostics.is_empty())
    }

    fn read_state(&self) -> RwLockReadGuard<'_, ImguiDiagnosticsState> {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, ImguiDiagnosticsState> {
        self.state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_payload_order(lower: ImguiDiagnosticKind, higher: ImguiDiagnosticKind) {
        let lower = ImguiDiagnostic::new(lower);
        let higher = ImguiDiagnostic::new(higher);
        assert_eq!(
            lower.stable_cmp(&higher),
            Ordering::Less,
            "{lower:?} should sort before {higher:?}",
        );
        assert_eq!(
            higher.stable_cmp(&lower),
            Ordering::Greater,
            "{higher:?} should sort after {lower:?}",
        );
    }

    #[test]
    fn diagnostic_publication_is_atomic_and_sorts_every_variant_payload() {
        let diagnostics = ImguiDiagnostics::default();
        let first_window =
            Entity::from_raw_u32(1).expect("test entity index should be representable");
        let second_window =
            Entity::from_raw_u32(9).expect("test entity index should be representable");
        let (low_window, high_window) = if first_window < second_window {
            (first_window, second_window)
        } else {
            (second_window, first_window)
        };
        let mut images = Assets::<Image>::default();
        let first_image = images.add(Image::default()).id();
        let second_image = images.add(Image::default()).id();
        let (low_image, high_image) = if first_image < second_image {
            (first_image, second_image)
        } else {
            (second_image, first_image)
        };

        assert_payload_order(
            ImguiDiagnosticKind::AmbiguousPrimaryWindow { count: 2 },
            ImguiDiagnosticKind::AmbiguousPrimaryWindow { count: 9 },
        );
        assert_payload_order(
            ImguiDiagnosticKind::AmbiguousAutoPrimary { order: -5 },
            ImguiDiagnosticKind::AmbiguousAutoPrimary { order: 4 },
        );
        assert_payload_order(
            ImguiDiagnosticKind::DuplicateRenderRoute { declarations: 2 },
            ImguiDiagnosticKind::DuplicateRenderRoute { declarations: 7 },
        );
        assert_payload_order(
            ImguiDiagnosticKind::DuplicateInputRoute { declarations: 3 },
            ImguiDiagnosticKind::DuplicateInputRoute { declarations: 8 },
        );
        assert_payload_order(
            ImguiDiagnosticKind::UnresolvedPrimaryWindowTarget { candidates: 1 },
            ImguiDiagnosticKind::UnresolvedPrimaryWindowTarget { candidates: 4 },
        );
        assert_payload_order(
            ImguiDiagnosticKind::MissingWindowTarget { window: low_window },
            ImguiDiagnosticKind::MissingWindowTarget {
                window: high_window,
            },
        );
        assert_payload_order(
            ImguiDiagnosticKind::MissingImageTarget { image: low_image },
            ImguiDiagnosticKind::MissingImageTarget { image: high_image },
        );
        assert_payload_order(
            ImguiDiagnosticKind::MissingManualTextureViewTarget {
                texture_view: ManualTextureViewHandle(2),
            },
            ImguiDiagnosticKind::MissingManualTextureViewTarget {
                texture_view: ManualTextureViewHandle(7),
            },
        );
        assert_payload_order(
            ImguiDiagnosticKind::ZeroSizedRenderTarget {
                target: ImguiRenderTargetKind::Window,
            },
            ImguiDiagnosticKind::ZeroSizedRenderTarget {
                target: ImguiRenderTargetKind::None,
            },
        );
        assert_payload_order(
            ImguiDiagnosticKind::InvalidRenderTargetScaleFactor {
                target: ImguiRenderTargetKind::Window,
            },
            ImguiDiagnosticKind::InvalidRenderTargetScaleFactor {
                target: ImguiRenderTargetKind::None,
            },
        );
        assert_payload_order(
            ImguiDiagnosticKind::InputCameraRequiresWindowTarget {
                target: ImguiRenderTargetKind::Window,
            },
            ImguiDiagnosticKind::InputCameraRequiresWindowTarget {
                target: ImguiRenderTargetKind::None,
            },
        );
        assert_payload_order(
            ImguiDiagnosticKind::MissingLogicalInputWindow { window: low_window },
            ImguiDiagnosticKind::MissingLogicalInputWindow {
                window: high_window,
            },
        );
        assert_payload_order(
            ImguiDiagnosticKind::AmbiguousExclusiveInput { priority: -3 },
            ImguiDiagnosticKind::AmbiguousExclusiveInput { priority: 8 },
        );
        assert_payload_order(
            ImguiDiagnosticKind::NativeMonitorCollectionFailed {
                reason: ImguiNativeMonitorFailure::MainFactsUnavailable,
            },
            ImguiDiagnosticKind::NativeMonitorCollectionFailed {
                reason: ImguiNativeMonitorFailure::ProjectionInvalid,
            },
        );
        assert_payload_order(
            ImguiDiagnosticKind::NativeMonitorWorkAreaFallback {
                reason: ImguiNativeMonitorFallback::Wayland,
            },
            ImguiDiagnosticKind::NativeMonitorWorkAreaFallback {
                reason: ImguiNativeMonitorFallback::AmbiguousDesktopScope,
            },
        );

        let unsorted = [
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousExclusiveInput { priority: 8 }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::ZeroSizedRenderTarget {
                target: ImguiRenderTargetKind::None,
            }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::MissingManualTextureViewTarget {
                texture_view: ManualTextureViewHandle(7),
            }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousPrimaryWindow { count: 9 }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary { order: 4 }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::MissingWindowTarget {
                window: high_window,
            }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousExclusiveInput { priority: -3 }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::ZeroSizedRenderTarget {
                target: ImguiRenderTargetKind::Window,
            }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::MissingManualTextureViewTarget {
                texture_view: ManualTextureViewHandle(2),
            }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousPrimaryWindow { count: 2 }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary { order: -5 }),
            ImguiDiagnostic::new(ImguiDiagnosticKind::MissingWindowTarget { window: low_window }),
        ];

        assert!(diagnostics.replace(ImguiDiagnosticOrigin::RenderRouting, 9, unsorted));
        assert!(!diagnostics.replace(
            ImguiDiagnosticOrigin::RenderRouting,
            8,
            [ImguiDiagnostic::new(
                ImguiDiagnosticKind::MissingPrimaryWindow,
            )],
        ));

        let kinds = diagnostics
            .entries_for(ImguiDiagnosticOrigin::RenderRouting)
            .map(|diagnostic| diagnostic.kind().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                ImguiDiagnosticKind::AmbiguousPrimaryWindow { count: 2 },
                ImguiDiagnosticKind::AmbiguousPrimaryWindow { count: 9 },
                ImguiDiagnosticKind::AmbiguousAutoPrimary { order: -5 },
                ImguiDiagnosticKind::AmbiguousAutoPrimary { order: 4 },
                ImguiDiagnosticKind::MissingWindowTarget { window: low_window },
                ImguiDiagnosticKind::MissingWindowTarget {
                    window: high_window,
                },
                ImguiDiagnosticKind::MissingManualTextureViewTarget {
                    texture_view: ManualTextureViewHandle(2),
                },
                ImguiDiagnosticKind::MissingManualTextureViewTarget {
                    texture_view: ManualTextureViewHandle(7),
                },
                ImguiDiagnosticKind::ZeroSizedRenderTarget {
                    target: ImguiRenderTargetKind::Window,
                },
                ImguiDiagnosticKind::ZeroSizedRenderTarget {
                    target: ImguiRenderTargetKind::None,
                },
                ImguiDiagnosticKind::AmbiguousExclusiveInput { priority: -3 },
                ImguiDiagnosticKind::AmbiguousExclusiveInput { priority: 8 },
            ],
        );
        assert_eq!(
            diagnostics.epoch(ImguiDiagnosticOrigin::RenderRouting),
            Some(9),
        );
    }

    #[test]
    fn native_monitor_and_viewport_diagnostic_batches_recover_independently() {
        let diagnostics = ImguiDiagnostics::default();
        assert!(diagnostics.replace(
            ImguiDiagnosticOrigin::NativeMonitor,
            1,
            [ImguiDiagnostic::new(
                ImguiDiagnosticKind::NativeMonitorPrimaryUnproven,
            )],
        ));
        assert!(diagnostics.replace(
            ImguiDiagnosticOrigin::NativeViewport,
            4,
            [ImguiDiagnostic::new(
                ImguiDiagnosticKind::MissingPrimaryWindow,
            )],
        ));

        assert!(diagnostics.replace(ImguiDiagnosticOrigin::NativeMonitor, 2, std::iter::empty(),));
        assert!(
            diagnostics
                .entries_for(ImguiDiagnosticOrigin::NativeMonitor)
                .next()
                .is_none()
        );
        assert_eq!(
            diagnostics
                .entries_for(ImguiDiagnosticOrigin::NativeViewport)
                .count(),
            1
        );
    }
}
