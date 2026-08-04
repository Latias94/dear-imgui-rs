//! Reconciliation of Dear ImGui geometry requests with native viewport observations.
//!
//! Winit window queries return the native geometry that is true at the current frame boundary;
//! they are not delayed move or resize events. Every observation is therefore authoritative. The
//! latest requested position and size are retained only long enough to distinguish a successful
//! application from a rejected, delayed, or constrained request. If native geometry differs from
//! that latest request, Dear ImGui must immediately synchronize to the native value.

use super::{
    desktop::{DesktopCoordinateSpace, native_desktop_coordinate_space, positive_finite_or},
    protocol::ImguiViewportFeedback,
};

/// Latest unresolved geometry intent for one native viewport.
#[derive(Debug, Default)]
pub(super) struct ViewportGeometryReconciler {
    requested_position: Option<GeometryRequest>,
    requested_size: Option<GeometryRequest>,
}

impl ViewportGeometryReconciler {
    pub(super) fn record_position(&mut self, position: [f32; 2], dpi_scale: f32) {
        self.requested_position = Some(GeometryRequest::new(position, dpi_scale));
    }

    pub(super) fn record_size(&mut self, size: [f32; 2], dpi_scale: f32) {
        self.requested_size = Some(GeometryRequest::new(size, dpi_scale));
    }

    pub(super) fn clear_position(&mut self) {
        self.requested_position = None;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.requested_position.is_none() && self.requested_size.is_none()
    }

    pub(super) fn reconcile(
        mut self,
        previous: ImguiViewportFeedback,
        observed: ImguiViewportFeedback,
    ) -> ViewportGeometryReconciliation {
        ViewportGeometryReconciliation {
            request_move: reconcile_field(
                self.requested_position.take(),
                previous.pos,
                observed.pos,
                observed.dpi_scale,
            ),
            request_resize: reconcile_field(
                self.requested_size.take(),
                previous.size,
                observed.size,
                observed.dpi_scale,
            ),
        }
    }

    #[cfg(test)]
    pub(super) fn has_requested_position(&self) -> bool {
        self.requested_position.is_some()
    }

    #[cfg(test)]
    pub(super) fn has_requested_size(&self) -> bool {
        self.requested_size.is_some()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ViewportGeometryReconciliation {
    pub(super) request_move: bool,
    pub(super) request_resize: bool,
}

#[derive(Clone, Copy, Debug)]
struct GeometryRequest {
    value: [f32; 2],
    dpi_scale: f32,
}

impl GeometryRequest {
    fn new(value: [f32; 2], dpi_scale: f32) -> Self {
        Self {
            value,
            dpi_scale: positive_finite_or(dpi_scale, 1.0),
        }
    }
}

fn reconcile_field(
    request: Option<GeometryRequest>,
    previous: [f32; 2],
    observed: [f32; 2],
    observed_dpi_scale: f32,
) -> bool {
    if let Some(request) = request {
        return !geometry_request_matches(request, observed, observed_dpi_scale);
    }
    !desktop_geometry_matches(previous, observed, observed_dpi_scale)
}

fn desktop_geometry_matches(left: [f32; 2], right: [f32; 2], dpi_scale: f32) -> bool {
    geometry_matches_in_space(native_desktop_coordinate_space(), left, right, dpi_scale)
}

fn geometry_request_matches(
    request: GeometryRequest,
    observed: [f32; 2],
    observed_dpi_scale: f32,
) -> bool {
    geometry_request_matches_in_space(
        native_desktop_coordinate_space(),
        request,
        observed,
        observed_dpi_scale,
    )
}

fn geometry_request_matches_in_space(
    coordinate_space: DesktopCoordinateSpace,
    request: GeometryRequest,
    observed: [f32; 2],
    observed_dpi_scale: f32,
) -> bool {
    let dpi_scale = request
        .dpi_scale
        .min(positive_finite_or(observed_dpi_scale, 1.0));
    geometry_matches_in_space(coordinate_space, request.value, observed, dpi_scale)
}

fn geometry_matches_in_space(
    coordinate_space: DesktopCoordinateSpace,
    left: [f32; 2],
    right: [f32; 2],
    dpi_scale: f32,
) -> bool {
    if !left.into_iter().chain(right).all(f32::is_finite) {
        return false;
    }
    const PHYSICAL_PIXEL_TOLERANCE: f32 = 0.51;
    let tolerance = match coordinate_space {
        DesktopCoordinateSpace::Physical => PHYSICAL_PIXEL_TOLERANCE,
        DesktopCoordinateSpace::Logical => {
            PHYSICAL_PIXEL_TOLERANCE / positive_finite_or(dpi_scale, 1.0)
        }
    };
    (left[0] - right[0]).abs() <= tolerance && (left[1] - right[1]).abs() <= tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feedback(pos: [f32; 2], size: [f32; 2]) -> ImguiViewportFeedback {
        ImguiViewportFeedback {
            pos,
            size,
            framebuffer_scale: [1.0, 1.0],
            dpi_scale: 1.0,
            focused: true,
            minimized: false,
        }
    }

    #[test]
    fn matching_latest_platform_request_is_acknowledged() {
        let mut geometry = ViewportGeometryReconciler::default();
        geometry.record_position([120.0, 240.0], 1.0);

        assert_eq!(
            geometry.reconcile(
                feedback([40.0, 80.0], [320.0, 180.0]),
                feedback([120.0, 240.0], [320.0, 180.0]),
            ),
            ViewportGeometryReconciliation {
                request_move: false,
                request_resize: false,
            }
        );
    }

    #[test]
    fn matching_and_constrained_size_requests_are_distinguished() {
        let mut matching = ViewportGeometryReconciler::default();
        matching.record_size([640.0, 360.0], 1.0);
        assert_eq!(
            matching.reconcile(
                feedback([40.0, 80.0], [320.0, 180.0]),
                feedback([40.0, 80.0], [640.0, 360.0]),
            ),
            ViewportGeometryReconciliation::default(),
            "the latest native size should acknowledge the pending request"
        );

        let mut constrained = ViewportGeometryReconciler::default();
        constrained.record_size([640.0, 360.0], 1.0);
        assert_eq!(
            constrained.reconcile(
                feedback([40.0, 80.0], [320.0, 180.0]),
                feedback([40.0, 80.0], [600.0, 340.0]),
            ),
            ViewportGeometryReconciliation {
                request_move: false,
                request_resize: true,
            },
            "a constrained native size must synchronize Dear ImGui to the actual client extent"
        );
    }

    #[test]
    fn unapplied_request_synchronizes_to_current_native_geometry() {
        let mut geometry = ViewportGeometryReconciler::default();
        geometry.record_position([140.0, 220.0], 1.0);

        assert_eq!(
            geometry.reconcile(
                feedback([100.0, 200.0], [320.0, 180.0]),
                feedback([100.0, 200.0], [320.0, 180.0]),
            ),
            ViewportGeometryReconciliation {
                request_move: true,
                request_resize: false,
            },
            "the queried native position is authoritative even while a newer request is delayed"
        );
    }

    #[test]
    fn clamp_to_a_previous_position_is_not_mistaken_for_a_stale_event() {
        let mut geometry = ViewportGeometryReconciler::default();
        geometry.record_position([180.0, 240.0], 1.0);

        assert_eq!(
            geometry.reconcile(
                feedback([100.0, 200.0], [320.0, 180.0]),
                feedback([100.0, 200.0], [320.0, 180.0]),
            ),
            ViewportGeometryReconciliation {
                request_move: true,
                request_resize: false,
            },
            "a clamp or user move back to an older coordinate is still the current native truth"
        );
    }

    #[test]
    fn repeated_requests_retain_only_the_latest_intent() {
        let mut older_observation = ViewportGeometryReconciler::default();
        for position in 0..9 {
            older_observation.record_position([position as f32, 0.0], 1.0);
        }
        assert_eq!(
            older_observation.reconcile(
                feedback([0.0, 0.0], [320.0, 180.0]),
                feedback([7.0, 0.0], [320.0, 180.0]),
            ),
            ViewportGeometryReconciliation {
                request_move: true,
                request_resize: false,
            },
            "an older request value must not acknowledge the latest intent"
        );

        let mut latest_observation = ViewportGeometryReconciler::default();
        for position in 0..9 {
            latest_observation.record_position([position as f32, 0.0], 1.0);
        }
        assert_eq!(
            latest_observation.reconcile(
                feedback([0.0, 0.0], [320.0, 180.0]),
                feedback([8.0, 0.0], [320.0, 180.0]),
            ),
            ViewportGeometryReconciliation {
                request_move: false,
                request_resize: false,
            }
        );
    }

    #[test]
    fn external_native_geometry_change_is_authoritative_without_a_window_event() {
        let geometry = ViewportGeometryReconciler::default();

        assert_eq!(
            geometry.reconcile(
                feedback([10.0, 20.0], [320.0, 180.0]),
                feedback([30.0, 40.0], [640.0, 360.0]),
            ),
            ViewportGeometryReconciliation {
                request_move: true,
                request_resize: true,
            }
        );
    }

    #[test]
    fn unchanged_native_geometry_does_not_request_platform_sync() {
        let geometry = ViewportGeometryReconciler::default();
        let feedback = feedback([10.0, 20.0], [320.0, 180.0]);

        assert_eq!(
            geometry.reconcile(feedback, feedback),
            ViewportGeometryReconciliation::default()
        );
    }

    #[test]
    fn physical_pixel_rounding_acknowledges_a_platform_request() {
        let mut geometry = ViewportGeometryReconciler::default();
        geometry.record_position([100.0, 200.0], 1.0);

        assert_eq!(
            geometry.reconcile(
                feedback([80.0, 180.0], [320.0, 180.0]),
                feedback([100.25, 199.75], [320.0, 180.0]),
            ),
            ViewportGeometryReconciliation::default()
        );
    }

    #[test]
    fn logical_coordinate_rounding_scales_with_dpi() {
        assert!(geometry_matches_in_space(
            DesktopCoordinateSpace::Logical,
            [100.0, 200.0],
            [100.25, 199.75],
            2.0,
        ));
        assert!(!geometry_matches_in_space(
            DesktopCoordinateSpace::Logical,
            [100.0, 200.0],
            [100.26, 200.0],
            2.0,
        ));
    }

    #[test]
    fn request_matching_uses_the_smaller_request_and_observation_dpi() {
        for (request_dpi, observed_dpi) in [(1.0, 2.0), (2.0, 1.0)] {
            assert!(geometry_request_matches_in_space(
                DesktopCoordinateSpace::Logical,
                GeometryRequest::new([100.0, 200.0], request_dpi),
                [100.4, 200.0],
                observed_dpi,
            ));
        }
    }
}
