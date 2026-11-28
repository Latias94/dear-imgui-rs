//! Atomic diagnostics from the main and render worlds.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use bevy_asset::AssetId;
use bevy_camera::ManualTextureViewHandle;
use bevy_ecs::prelude::{Entity, Resource};
use bevy_image::Image;
use dear_imgui_rs::ContextId;

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
    /// Validation of extracted draw commands before any renderer upload.
    RenderPreparation,
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
    /// A draw command requests a texture-owned native sampler unsupported by this renderer.
    TextureSamplerUnsupported,
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
        self.context_id
            .map(ContextId::get)
            .cmp(&other.context_id.map(ContextId::get))
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
    use bevy_asset::Assets;

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
