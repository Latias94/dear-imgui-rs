//! Main-world render and input routing for Dear ImGui Contexts.
//!
//! Route components are declarations placed on independent ECS entities. They do not own a Dear
//! ImGui Context or a Bevy camera. The resolver validates every declaration against the current
//! world and replaces [`ImguiResolvedRoutes`] atomically for the new epoch.
//!
//! Resolution runs in `PostUpdate` after Bevy's camera update set. Rendering uses that epoch
//! immediately; input in the next frame uses the same geometry that was actually presented.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use bevy_app::{App, PostUpdate};
use bevy_asset::{AssetId, Assets};
use bevy_camera::{
    Camera, CameraOutputMode, CameraUpdateSystems, ManualTextureViewHandle, NormalizedRenderTarget,
    RenderTarget, RenderTargetInfo, Viewport,
};
use bevy_core_pipeline::{Core2d, Core3d};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{InternedScheduleLabel, IntoScheduleConfigs, ScheduleLabel};
use bevy_image::Image;
use bevy_math::{Rect, UVec2, Vec2};
use bevy_render::{
    camera::{CameraRenderGraph, MissingRenderTargetInfoError, NormalizedRenderTargetExt},
    texture::ManualTextureViews,
};
use bevy_window::{PrimaryWindow, Window};
use dear_imgui_rs::ContextId;

use crate::context::ImguiContexts;

pub(crate) fn install_route_resolution(app: &mut App) {
    app.init_resource::<ImguiResolvedRoutes>()
        .init_resource::<ImguiDiagnostics>()
        .add_systems(PostUpdate, resolve_imgui_routes.after(CameraUpdateSystems));
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiRenderRouteSource {
    /// The primary Context was assigned to the unique eligible primary-window camera.
    AutoPrimary,
    /// A user-owned route entity declared the association.
    Explicit,
}

/// One immutable main-world render route for an epoch.
#[derive(Clone, Debug)]
pub struct ImguiResolvedRenderRoute {
    context_id: ContextId,
    route_entity: Option<Entity>,
    camera: Entity,
    order: isize,
    camera_order: isize,
    camera_schedule: InternedScheduleLabel,
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

/// One immutable main-world input route for an epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImguiResolvedInputRoute {
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

    /// Return the user-owned declaration entity, or `None` for derived window input.
    #[must_use]
    pub const fn route_entity(&self) -> Option<Entity> {
        self.route_entity
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
pub struct ImguiResolvedRoutes {
    epoch: u64,
    render_routes: Vec<ImguiResolvedRenderRoute>,
    input_routes: Vec<ImguiResolvedInputRoute>,
}

impl ImguiResolvedRoutes {
    /// Return the latest resolver epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Borrow resolved render routes in stable camera/overlay order.
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
    #[must_use]
    pub fn input_route(&self, context_id: ContextId) -> Option<&ImguiResolvedInputRoute> {
        self.input_routes
            .iter()
            .find(|route| route.context_id == context_id)
    }

    fn replace(
        &mut self,
        render_routes: Vec<ImguiResolvedRenderRoute>,
        input_routes: Vec<ImguiResolvedInputRoute>,
    ) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.render_routes = render_routes;
        self.input_routes = input_routes;
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
    /// Render-world resource preparation.
    RenderPreparation,
    /// Render-world overlay execution.
    RenderExecution,
    /// Native platform viewport integration.
    Viewport,
    /// Bevy image and Dear ImGui texture integration.
    Texture,
    /// Context ownership and lifecycle.
    Context,
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
}

/// One diagnostic payload before or after it is published in a batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImguiDiagnostic {
    kind: ImguiDiagnosticKind,
    context_id: Option<ContextId>,
    route_entity: Option<Entity>,
    camera: Option<Entity>,
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

    fn stable_cmp(&self, other: &Self) -> Ordering {
        optional_context_key(self.context_id)
            .cmp(&optional_context_key(other.context_id))
            .then_with(|| self.route_entity.cmp(&other.route_entity))
            .then_with(|| self.camera.cmp(&other.camera))
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
    records: Vec<ImguiDiagnosticRecord>,
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
        state.rebuild_records();
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
        self.read_state().records.clone()
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
        self.read_state().records.is_empty()
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

impl ImguiDiagnosticsState {
    fn rebuild_records(&mut self) {
        self.records.clear();
        self.records
            .extend(self.batches.iter().flat_map(|(origin, batch)| {
                batch
                    .diagnostics
                    .iter()
                    .cloned()
                    .map(|diagnostic| ImguiDiagnosticRecord {
                        origin: *origin,
                        epoch: batch.epoch,
                        diagnostic,
                    })
            }));
    }
}

#[derive(Clone)]
struct CameraRecord {
    entity: Entity,
    camera: Camera,
    target: RenderTarget,
    schedule: Option<bevy_ecs::schedule::InternedScheduleLabel>,
}

#[derive(Clone)]
struct ValidatedCamera {
    entity: Entity,
    camera_order: isize,
    camera_schedule: InternedScheduleLabel,
    target: NormalizedRenderTarget,
    target_info: RenderTargetInfo,
    camera_viewport: Option<Viewport>,
}

impl ValidatedCamera {
    fn target_kind(&self) -> ImguiRenderTargetKind {
        render_target_kind(&self.target)
    }

    fn logical_window_region(&self) -> Option<(Entity, Rect)> {
        let NormalizedRenderTarget::Window(window) = &self.target else {
            return None;
        };
        let scale = self.target_info.scale_factor;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let (position, size) = self
            .camera_viewport
            .as_ref()
            .map_or((UVec2::ZERO, self.target_info.physical_size), |viewport| {
                (viewport.physical_position, viewport.physical_size)
            });
        let min = position.as_vec2() / scale;
        let max = min + size.as_vec2() / scale;
        Some((window.entity(), Rect { min, max }))
    }
}

struct ResolverWorld<'a> {
    registered_contexts: &'a HashSet<ContextId>,
    primary_context: Option<ContextId>,
    primary_windows: &'a [Entity],
    windows: &'a [(Entity, Window)],
    images: &'a Assets<Image>,
    manual_texture_views: &'a ManualTextureViews,
    cameras: &'a [CameraRecord],
}

/// Resolve all route declarations against the current main world.
///
/// This is an internal plugin system. Public consumers should inspect [`ImguiResolvedRoutes`] and
/// [`ImguiDiagnostics`] instead of scheduling it directly.
///
pub(crate) fn resolve_imgui_routes(
    contexts: Option<NonSend<ImguiContexts>>,
    mut resolved: ResMut<ImguiResolvedRoutes>,
    diagnostics: Res<ImguiDiagnostics>,
    primary_windows: Query<Entity, With<PrimaryWindow>>,
    windows: Query<(Entity, &Window)>,
    images: Option<Res<Assets<Image>>>,
    manual_texture_views: Option<Res<ManualTextureViews>>,
    cameras: Query<(Entity, &Camera, &RenderTarget, Option<&CameraRenderGraph>)>,
    render_routes: Query<(Entity, &ImguiRenderRoute)>,
    input_routes: Query<(Entity, &ImguiInputRoute)>,
) {
    let mut primary_windows = primary_windows.iter().collect::<Vec<_>>();
    primary_windows.sort();
    let windows = windows
        .iter()
        .map(|(entity, window)| (entity, window.clone()))
        .collect::<Vec<_>>();
    let empty_images = Assets::<Image>::default();
    let images = images.as_deref().unwrap_or(&empty_images);
    let empty_manual_texture_views = ManualTextureViews::default();
    let manual_texture_views = manual_texture_views
        .as_deref()
        .unwrap_or(&empty_manual_texture_views);
    let registered_contexts = contexts
        .as_deref()
        .map(|contexts| contexts.ids().collect::<HashSet<_>>())
        .unwrap_or_default();
    let mut cameras = cameras
        .iter()
        .map(|(entity, camera, target, schedule)| CameraRecord {
            entity,
            camera: camera.clone(),
            target: target.clone(),
            schedule: schedule.map(|schedule| schedule.0),
        })
        .collect::<Vec<_>>();
    cameras.sort_by_key(|camera| camera.entity);
    let mut render_declarations = render_routes
        .iter()
        .map(|(entity, route)| (entity, *route))
        .collect::<Vec<_>>();
    render_declarations.sort_by(|(left_entity, left), (right_entity, right)| {
        context_key(left.context_id)
            .cmp(&context_key(right.context_id))
            .then_with(|| left_entity.cmp(right_entity))
    });
    let mut input_declarations = input_routes
        .iter()
        .map(|(entity, route)| (entity, *route))
        .collect::<Vec<_>>();
    input_declarations.sort_by(|(left_entity, left), (right_entity, right)| {
        context_key(left.context_id)
            .cmp(&context_key(right.context_id))
            .then_with(|| left_entity.cmp(right_entity))
    });

    let route_world = ResolverWorld {
        registered_contexts: &registered_contexts,
        primary_context: contexts.as_deref().and_then(ImguiContexts::primary_id),
        primary_windows: &primary_windows,
        windows: &windows,
        images,
        manual_texture_views,
        cameras: &cameras,
    };
    let (mut resolved_render, render_diagnostics) =
        resolve_render_routes(&route_world, &render_declarations);
    resolved_render.sort_by(compare_render_routes);
    let (mut resolved_input, input_diagnostics) =
        resolve_input_routes(&route_world, &input_declarations, &resolved_render);
    resolved_input.sort_by(compare_input_routes);

    let epoch = resolved.replace(resolved_render, resolved_input);
    diagnostics.replace(
        ImguiDiagnosticOrigin::RenderRouting,
        epoch,
        render_diagnostics,
    );
    diagnostics.replace(
        ImguiDiagnosticOrigin::InputRouting,
        epoch,
        input_diagnostics,
    );
}

fn resolve_render_routes(
    world: &ResolverWorld<'_>,
    declarations: &[(Entity, ImguiRenderRoute)],
) -> (Vec<ImguiResolvedRenderRoute>, Vec<ImguiDiagnostic>) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    let explicit_primary = world.primary_context.is_some_and(|primary| {
        declarations
            .iter()
            .any(|(_, declaration)| declaration.context_id == primary)
    });

    let mut start = 0;
    while start < declarations.len() {
        let context_id = declarations[start].1.context_id;
        let mut end = start + 1;
        while end < declarations.len() && declarations[end].1.context_id == context_id {
            end += 1;
        }
        let group = &declarations[start..end];
        if group.len() > 1 {
            diagnostics.extend(group.iter().map(|(route_entity, declaration)| {
                ImguiDiagnostic::new(ImguiDiagnosticKind::DuplicateRenderRoute {
                    declarations: group.len(),
                })
                .with_context(context_id)
                .with_route(*route_entity)
                .with_camera(declaration.camera)
            }));
        } else if !world.registered_contexts.contains(&context_id) {
            let (route_entity, declaration) = group[0];
            diagnostics.push(
                ImguiDiagnostic::new(ImguiDiagnosticKind::UnknownContext)
                    .with_context(context_id)
                    .with_route(route_entity)
                    .with_camera(declaration.camera),
            );
        } else {
            let (route_entity, declaration) = group[0];
            match validate_camera(world, declaration.camera) {
                Ok(camera) => resolved.push(ImguiResolvedRenderRoute {
                    context_id,
                    route_entity: Some(route_entity),
                    camera: camera.entity,
                    order: declaration.order,
                    camera_order: camera.camera_order,
                    camera_schedule: camera.camera_schedule,
                    source: ImguiRenderRouteSource::Explicit,
                    target: camera.target,
                    target_info: camera.target_info,
                    camera_viewport: camera.camera_viewport,
                }),
                Err(kind) => diagnostics.push(
                    ImguiDiagnostic::new(kind)
                        .with_context(context_id)
                        .with_route(route_entity)
                        .with_camera(declaration.camera),
                ),
            }
        }
        start = end;
    }

    if !explicit_primary && let Some(primary_context) = world.primary_context {
        resolve_auto_primary(world, primary_context, &mut resolved, &mut diagnostics);
    }

    (resolved, diagnostics)
}

fn resolve_auto_primary(
    world: &ResolverWorld<'_>,
    primary_context: ContextId,
    resolved: &mut Vec<ImguiResolvedRenderRoute>,
    diagnostics: &mut Vec<ImguiDiagnostic>,
) {
    let primary_window = match world.primary_windows {
        [] => {
            diagnostics.push(
                ImguiDiagnostic::new(ImguiDiagnosticKind::MissingPrimaryWindow)
                    .with_context(primary_context),
            );
            return;
        }
        [primary_window] => *primary_window,
        primary_windows => {
            diagnostics.push(
                ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousPrimaryWindow {
                    count: primary_windows.len(),
                })
                .with_context(primary_context),
            );
            return;
        }
    };

    let mut candidates = Vec::new();
    for camera in world.cameras {
        let Some(normalized) = camera.target.normalize(Some(primary_window)) else {
            continue;
        };
        let NormalizedRenderTarget::Window(window) = &normalized else {
            continue;
        };
        if window.entity() != primary_window {
            continue;
        }
        match validate_camera(world, camera.entity) {
            Ok(camera) => candidates.push(camera),
            Err(kind) => diagnostics.push(
                ImguiDiagnostic::new(kind)
                    .with_context(primary_context)
                    .with_camera(camera.entity),
            ),
        }
    }

    let Some(highest_order) = candidates.iter().map(|camera| camera.camera_order).max() else {
        diagnostics.push(
            ImguiDiagnostic::new(ImguiDiagnosticKind::NoEligibleAutoPrimaryCamera)
                .with_context(primary_context),
        );
        return;
    };
    let mut highest = candidates
        .into_iter()
        .filter(|camera| camera.camera_order == highest_order);
    let winner = highest
        .next()
        .expect("a maximum camera order must have at least one candidate");
    if let Some(second) = highest.next() {
        diagnostics.push(
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary {
                order: highest_order,
            })
            .with_context(primary_context)
            .with_camera(winner.entity),
        );
        diagnostics.push(
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary {
                order: highest_order,
            })
            .with_context(primary_context)
            .with_camera(second.entity),
        );
        diagnostics.extend(highest.map(|camera| {
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary {
                order: highest_order,
            })
            .with_context(primary_context)
            .with_camera(camera.entity)
        }));
        return;
    }

    resolved.push(ImguiResolvedRenderRoute {
        context_id: primary_context,
        route_entity: None,
        camera: winner.entity,
        order: 0,
        camera_order: winner.camera_order,
        camera_schedule: winner.camera_schedule,
        source: ImguiRenderRouteSource::AutoPrimary,
        target: winner.target,
        target_info: winner.target_info,
        camera_viewport: winner.camera_viewport,
    });
}

fn resolve_input_routes(
    world: &ResolverWorld<'_>,
    declarations: &[(Entity, ImguiInputRoute)],
    render_routes: &[ImguiResolvedRenderRoute],
) -> (Vec<ImguiResolvedInputRoute>, Vec<ImguiDiagnostic>) {
    let explicitly_declared = declarations
        .iter()
        .map(|(_, declaration)| declaration.context_id)
        .collect::<HashSet<_>>();
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();

    let mut start = 0;
    while start < declarations.len() {
        let context_id = declarations[start].1.context_id;
        let mut end = start + 1;
        while end < declarations.len() && declarations[end].1.context_id == context_id {
            end += 1;
        }
        let group = &declarations[start..end];
        if group.len() > 1 {
            diagnostics.extend(group.iter().map(|(route_entity, declaration)| {
                input_diagnostic(
                    ImguiDiagnosticKind::DuplicateInputRoute {
                        declarations: group.len(),
                    },
                    context_id,
                    *route_entity,
                    declaration.source,
                )
            }));
        } else if !world.registered_contexts.contains(&context_id) {
            let (route_entity, declaration) = group[0];
            diagnostics.push(input_diagnostic(
                ImguiDiagnosticKind::UnknownContext,
                context_id,
                route_entity,
                declaration.source,
            ));
        } else {
            let (route_entity, declaration) = group[0];
            if declaration.policy != ImguiInputPolicy::Disabled {
                match validate_input_source(world, declaration.source) {
                    Ok((host_window, logical_region)) => {
                        resolved.push(ImguiResolvedInputRoute {
                            context_id,
                            route_entity: Some(route_entity),
                            source: declaration.source,
                            policy: declaration.policy,
                            host_window,
                            logical_region,
                        });
                    }
                    Err(kind) => diagnostics.push(input_diagnostic(
                        kind,
                        context_id,
                        route_entity,
                        declaration.source,
                    )),
                }
            }
        }
        start = end;
    }

    for render_route in render_routes {
        if explicitly_declared.contains(&render_route.context_id) {
            continue;
        }
        let validated = ValidatedCamera {
            entity: render_route.camera,
            camera_order: render_route.camera_order,
            camera_schedule: render_route.camera_schedule,
            target: render_route.target.clone(),
            target_info: render_route.target_info.clone(),
            camera_viewport: render_route.camera_viewport.clone(),
        };
        let Some((host_window, logical_region)) = validated.logical_window_region() else {
            continue;
        };
        resolved.push(ImguiResolvedInputRoute {
            context_id: render_route.context_id,
            route_entity: None,
            source: ImguiInputSource::camera(render_route.camera),
            policy: ImguiInputPolicy::Exclusive { priority: 0 },
            host_window,
            logical_region,
        });
    }

    remove_ambiguous_exclusive_input(&mut resolved, &mut diagnostics);
    (resolved, diagnostics)
}

fn validate_input_source(
    world: &ResolverWorld<'_>,
    source: ImguiInputSource,
) -> Result<(Entity, Rect), ImguiDiagnosticKind> {
    match source {
        ImguiInputSource::Camera(source) => {
            let camera = validate_camera(world, source.camera)?;
            camera.logical_window_region().ok_or(
                ImguiDiagnosticKind::InputCameraRequiresWindowTarget {
                    target: camera.target_kind(),
                },
            )
        }
        ImguiInputSource::Logical(source) => {
            let Some((_, window)) = world
                .windows
                .iter()
                .find(|(entity, _)| *entity == source.window)
            else {
                return Err(ImguiDiagnosticKind::MissingLogicalInputWindow {
                    window: source.window,
                });
            };
            if window.physical_width() == 0 || window.physical_height() == 0 {
                return Err(ImguiDiagnosticKind::MissingLogicalInputWindow {
                    window: source.window,
                });
            }
            if !valid_rect(source.region) {
                return Err(ImguiDiagnosticKind::InvalidLogicalInputRegion);
            }
            Ok((source.window, source.region))
        }
    }
}

fn remove_ambiguous_exclusive_input(
    routes: &mut Vec<ImguiResolvedInputRoute>,
    diagnostics: &mut Vec<ImguiDiagnostic>,
) {
    let mut ambiguous = HashSet::new();
    for left in 0..routes.len() {
        let ImguiInputPolicy::Exclusive {
            priority: left_priority,
        } = routes[left].policy
        else {
            continue;
        };
        for right in (left + 1)..routes.len() {
            let ImguiInputPolicy::Exclusive {
                priority: right_priority,
            } = routes[right].policy
            else {
                continue;
            };
            if routes[left].context_id != routes[right].context_id
                && routes[left].host_window == routes[right].host_window
                && left_priority == right_priority
                && rects_overlap(routes[left].logical_region, routes[right].logical_region)
            {
                ambiguous.insert(left);
                ambiguous.insert(right);
            }
        }
    }

    for index in ambiguous.iter().copied() {
        let route = routes[index];
        let priority = route
            .policy
            .priority()
            .expect("only exclusive routes are marked ambiguous");
        let mut diagnostic =
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousExclusiveInput { priority })
                .with_context(route.context_id);
        if let Some(route_entity) = route.route_entity {
            diagnostic = diagnostic.with_route(route_entity);
        }
        if let Some(camera) = route.source.as_camera() {
            diagnostic = diagnostic.with_camera(camera.camera);
        }
        diagnostics.push(diagnostic);
    }

    let mut index = 0;
    routes.retain(|_| {
        let keep = !ambiguous.contains(&index);
        index += 1;
        keep
    });
}

fn validate_camera(
    world: &ResolverWorld<'_>,
    camera_entity: Entity,
) -> Result<ValidatedCamera, ImguiDiagnosticKind> {
    let camera = world
        .cameras
        .iter()
        .find(|camera| camera.entity == camera_entity)
        .ok_or(ImguiDiagnosticKind::MissingCamera)?;
    if !camera.camera.is_active {
        return Err(ImguiDiagnosticKind::InactiveCamera);
    }
    if !matches!(camera.camera.output_mode, CameraOutputMode::Write { .. }) {
        return Err(ImguiDiagnosticKind::CameraDoesNotWrite);
    }
    let Some(camera_schedule) = camera
        .schedule
        .filter(|schedule| *schedule == Core2d.intern() || *schedule == Core3d.intern())
    else {
        return Err(ImguiDiagnosticKind::UnsupportedCameraSchedule);
    };

    let primary_window = match world.primary_windows {
        [primary_window] => Some(*primary_window),
        _ => None,
    };
    let Some(target) = camera.target.normalize(primary_window) else {
        return Err(ImguiDiagnosticKind::UnresolvedPrimaryWindowTarget {
            candidates: world.primary_windows.len(),
        });
    };
    if matches!(target, NormalizedRenderTarget::None { .. }) {
        return Err(ImguiDiagnosticKind::UnsupportedRenderTargetNone);
    }
    let target_info = target
        .get_render_target_info(
            world
                .windows
                .iter()
                .map(|(entity, window)| (*entity, window)),
            world.images,
            world.manual_texture_views,
        )
        .map_err(missing_target_diagnostic)?;
    let target_kind = render_target_kind(&target);
    if target_info.physical_size.x == 0 || target_info.physical_size.y == 0 {
        return Err(ImguiDiagnosticKind::ZeroSizedRenderTarget {
            target: target_kind,
        });
    }
    if !target_info.scale_factor.is_finite() || target_info.scale_factor <= 0.0 {
        return Err(ImguiDiagnosticKind::InvalidRenderTargetScaleFactor {
            target: target_kind,
        });
    }
    let mut camera_viewport = camera.camera.viewport.clone();
    if let Some(viewport) = &mut camera_viewport {
        viewport.clamp_to_size(target_info.physical_size);
    }
    if camera_viewport
        .as_ref()
        .is_some_and(|viewport| viewport.physical_size.x == 0 || viewport.physical_size.y == 0)
    {
        return Err(ImguiDiagnosticKind::ZeroSizedRenderTarget {
            target: target_kind,
        });
    }

    Ok(ValidatedCamera {
        entity: camera.entity,
        camera_order: camera.camera.order,
        camera_schedule,
        target,
        target_info,
        camera_viewport,
    })
}

fn missing_target_diagnostic(error: MissingRenderTargetInfoError) -> ImguiDiagnosticKind {
    match error {
        MissingRenderTargetInfoError::Window { window } => {
            ImguiDiagnosticKind::MissingWindowTarget { window }
        }
        MissingRenderTargetInfoError::Image { image } => {
            ImguiDiagnosticKind::MissingImageTarget { image }
        }
        MissingRenderTargetInfoError::TextureView { texture_view } => {
            ImguiDiagnosticKind::MissingManualTextureViewTarget { texture_view }
        }
    }
}

fn input_diagnostic(
    kind: ImguiDiagnosticKind,
    context_id: ContextId,
    route_entity: Entity,
    source: ImguiInputSource,
) -> ImguiDiagnostic {
    let mut diagnostic = ImguiDiagnostic::new(kind)
        .with_context(context_id)
        .with_route(route_entity);
    if let Some(source) = source.as_camera() {
        diagnostic = diagnostic.with_camera(source.camera);
    }
    diagnostic
}

fn render_target_kind(target: &NormalizedRenderTarget) -> ImguiRenderTargetKind {
    match target {
        NormalizedRenderTarget::Window(_) => ImguiRenderTargetKind::Window,
        NormalizedRenderTarget::Image(_) => ImguiRenderTargetKind::Image,
        NormalizedRenderTarget::TextureView(_) => ImguiRenderTargetKind::ManualTextureView,
        NormalizedRenderTarget::None { .. } => ImguiRenderTargetKind::None,
    }
}

fn compare_render_routes(
    left: &ImguiResolvedRenderRoute,
    right: &ImguiResolvedRenderRoute,
) -> Ordering {
    left.camera
        .cmp(&right.camera)
        .then_with(|| left.order.cmp(&right.order))
        .then_with(|| context_key(left.context_id).cmp(&context_key(right.context_id)))
        .then_with(|| left.route_entity.cmp(&right.route_entity))
}

fn compare_input_routes(
    left: &ImguiResolvedInputRoute,
    right: &ImguiResolvedInputRoute,
) -> Ordering {
    left.host_window
        .cmp(&right.host_window)
        .then_with(|| context_key(left.context_id).cmp(&context_key(right.context_id)))
        .then_with(|| left.route_entity.cmp(&right.route_entity))
}

fn context_key(context_id: ContextId) -> u64 {
    context_id.get().get()
}

fn optional_context_key(context_id: Option<ContextId>) -> Option<u64> {
    context_id.map(context_key)
}

fn valid_rect(rect: Rect) -> bool {
    finite_vec2(rect.min)
        && finite_vec2(rect.max)
        && rect.max.x > rect.min.x
        && rect.max.y > rect.min.y
}

fn finite_vec2(value: Vec2) -> bool {
    value.x.is_finite() && value.y.is_finite()
}

fn rects_overlap(left: Rect, right: Rect) -> bool {
    left.min.x < right.max.x
        && left.max.x > right.min.x
        && left.min.y < right.max.y
        && left.max.y > right.min.y
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
}
