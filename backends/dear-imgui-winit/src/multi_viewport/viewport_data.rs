use std::cell::Cell;
use std::ffi::c_void;
use std::sync::Arc;

use winit::window::Window;

use super::WinitPlatformError;
use super::native_cursor_hittest::NativeCursorHitTest;
use super::registry::{insert_viewport_data, owns_viewport_data};
use super::runtime::RuntimeControl;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct GeometryRefresh {
    pub(super) position: bool,
    pub(super) size: bool,
}

impl GeometryRefresh {
    pub(super) const fn is_empty(self) -> bool {
        !self.position && !self.size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClientGeometryReconciliationAction {
    Wait,
    ApplyTarget,
    PublishNative,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ClientGeometryReconciliationDecision {
    pub(super) action: ClientGeometryReconciliationAction,
    pub(super) position: [f32; 2],
    pub(super) size: [f32; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClientGeometryReconciliationPhase {
    AwaitingResponse,
    WatchingLateExtents,
    AwaitingCorrection,
}

#[derive(Clone, Copy, Debug)]
struct ClientGeometryReconciliation {
    pub(super) position: [f32; 2],
    pub(super) size: [f32; 2],
    decoration_offset: Option<[f32; 2]>,
    starting_geometry_epoch: u64,
    phase: ClientGeometryReconciliationPhase,
}

impl ClientGeometryReconciliation {
    fn new(
        position: [f32; 2],
        size: [f32; 2],
        decoration_offset: Option<[f32; 2]>,
        starting_geometry_epoch: u64,
    ) -> Self {
        Self {
            position,
            size,
            decoration_offset,
            starting_geometry_epoch,
            phase: ClientGeometryReconciliationPhase::AwaitingResponse,
        }
    }

    fn observe(
        mut self,
        geometry_epoch: u64,
        visible: bool,
        actual_position: [f32; 2],
        actual_size: [f32; 2],
        decoration_offset: Option<[f32; 2]>,
    ) -> (Option<Self>, ClientGeometryReconciliationAction) {
        let observed_post_request_geometry = geometry_epoch > self.starting_geometry_epoch;
        if !visible || !observed_post_request_geometry {
            return (Some(self), ClientGeometryReconciliationAction::Wait);
        }

        if client_geometry_matches(self.position, self.size, actual_position, actual_size) {
            self.starting_geometry_epoch = geometry_epoch;
            match self.phase {
                ClientGeometryReconciliationPhase::AwaitingResponse => {
                    self.phase = ClientGeometryReconciliationPhase::WatchingLateExtents;
                    // Publish the first stable-looking geometry immediately so an event loop using
                    // ControlFlow::Wait never depends on a synthetic timer. Keep a short-lived watch
                    // for one late X11 frame-extents change, which may arrive after the first
                    // ConfigureNotify.
                    return (
                        Some(self),
                        ClientGeometryReconciliationAction::PublishNative,
                    );
                }
                ClientGeometryReconciliationPhase::WatchingLateExtents
                | ClientGeometryReconciliationPhase::AwaitingCorrection => {
                    return (None, ClientGeometryReconciliationAction::PublishNative);
                }
            }
        }

        if self.phase != ClientGeometryReconciliationPhase::AwaitingCorrection
            && frame_extent_change_explains_geometry(
                self.position,
                self.size,
                actual_position,
                actual_size,
                self.decoration_offset,
                decoration_offset,
            )
        {
            self.phase = ClientGeometryReconciliationPhase::AwaitingCorrection;
            self.decoration_offset = decoration_offset;
            self.starting_geometry_epoch = geometry_epoch;
            return (Some(self), ClientGeometryReconciliationAction::ApplyTarget);
        }

        // A second divergent native event is authoritative. This bounds X11 frame-extents
        // correction without fighting a real title-bar move or resize initiated by the user.
        (None, ClientGeometryReconciliationAction::PublishNative)
    }

    fn retarget(
        mut self,
        position: [f32; 2],
        size: [f32; 2],
        decoration_offset: Option<[f32; 2]>,
        starting_geometry_epoch: u64,
    ) -> Self {
        self.position = position;
        self.size = size;
        self.decoration_offset = decoration_offset;
        self.starting_geometry_epoch = starting_geometry_epoch;
        self.phase = ClientGeometryReconciliationPhase::AwaitingResponse;
        self
    }
}

fn client_geometry_matches(
    expected_position: [f32; 2],
    expected_size: [f32; 2],
    actual_position: [f32; 2],
    actual_size: [f32; 2],
) -> bool {
    vec2_approximately_equals(expected_position, actual_position)
        && vec2_approximately_equals(expected_size, actual_size)
}

fn frame_extent_change_explains_geometry(
    target_position: [f32; 2],
    target_size: [f32; 2],
    actual_position: [f32; 2],
    actual_size: [f32; 2],
    previous_offset: Option<[f32; 2]>,
    current_offset: Option<[f32; 2]>,
) -> bool {
    let (Some(previous_offset), Some(current_offset)) = (previous_offset, current_offset) else {
        return false;
    };
    if vec2_approximately_equals(previous_offset, current_offset)
        || !vec2_approximately_equals(target_size, actual_size)
    {
        return false;
    }
    let expected_position = [
        target_position[0] + current_offset[0] - previous_offset[0],
        target_position[1] + current_offset[1] - previous_offset[1],
    ];
    vec2_approximately_equals(expected_position, actual_position)
}

fn vec2_approximately_equals(expected: [f32; 2], actual: [f32; 2]) -> bool {
    expected
        .into_iter()
        .zip(actual)
        .all(|(expected, actual)| (expected - actual).abs() <= 1.0)
}

#[cfg(test)]
mod client_geometry_reconciliation_tests {
    use super::*;

    #[test]
    fn waits_for_a_post_request_native_geometry_event() {
        let pending =
            ClientGeometryReconciliation::new([120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]), 4);

        let (pending, action) =
            pending.observe(4, true, [131.0, 285.0], [640.0, 480.0], Some([0.0, 0.0]));

        assert_eq!(action, ClientGeometryReconciliationAction::Wait);
        let pending = pending.unwrap();
        assert_eq!(pending.position, [120.0, 240.0]);
        assert_eq!(pending.size, [640.0, 480.0]);
    }

    #[test]
    fn visibility_wait_preserves_the_transaction_for_the_visible_observation() {
        let pending =
            ClientGeometryReconciliation::new([120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]), 7);

        let (pending, action) =
            pending.observe(8, false, [120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]));
        assert_eq!(action, ClientGeometryReconciliationAction::Wait);

        let (pending, action) = pending
            .expect("an invisible window must retain its reconciliation")
            .observe(8, true, [120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]));
        assert!(pending.is_some());
        assert_eq!(action, ClientGeometryReconciliationAction::PublishNative);
    }

    #[test]
    fn changed_frame_extents_are_corrected_then_native_geometry_wins() {
        let pending =
            ClientGeometryReconciliation::new([120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]), 7);

        let (pending, action) =
            pending.observe(8, true, [131.0, 285.0], [640.0, 480.0], Some([11.0, 45.0]));
        assert_eq!(action, ClientGeometryReconciliationAction::ApplyTarget);
        let (pending, action) =
            pending
                .unwrap()
                .observe(9, true, [132.0, 286.0], [640.0, 480.0], Some([11.0, 45.0]));
        assert!(pending.is_none());
        assert_eq!(action, ClientGeometryReconciliationAction::PublishNative);
    }

    #[test]
    fn a_matching_response_to_the_correction_completes_the_transaction() {
        let pending =
            ClientGeometryReconciliation::new([120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]), 7);
        let (pending, action) =
            pending.observe(8, true, [131.0, 285.0], [640.0, 480.0], Some([11.0, 45.0]));
        assert_eq!(action, ClientGeometryReconciliationAction::ApplyTarget);

        let (pending, action) =
            pending
                .unwrap()
                .observe(9, true, [120.0, 240.0], [640.0, 480.0], Some([11.0, 45.0]));
        assert!(pending.is_none());
        assert_eq!(action, ClientGeometryReconciliationAction::PublishNative);
    }

    #[test]
    fn matching_native_geometry_is_published_immediately_then_watch_is_retired() {
        let pending =
            ClientGeometryReconciliation::new([120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]), 7);
        let (pending, action) =
            pending.observe(8, true, [120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]));
        assert_eq!(action, ClientGeometryReconciliationAction::PublishNative);

        let (pending, action) =
            pending
                .unwrap()
                .observe(9, true, [120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]));
        assert!(pending.is_none());
        assert_eq!(action, ClientGeometryReconciliationAction::PublishNative);
    }

    #[test]
    fn a_native_move_with_unchanged_frame_extents_is_not_overridden() {
        let pending = ClientGeometryReconciliation::new(
            [120.0, 240.0],
            [640.0, 480.0],
            Some([11.0, 45.0]),
            7,
        );
        let (pending, action) =
            pending.observe(8, true, [170.0, 240.0], [640.0, 480.0], Some([11.0, 45.0]));

        assert!(pending.is_none());
        assert_eq!(action, ClientGeometryReconciliationAction::PublishNative);
    }

    #[test]
    fn a_user_move_in_the_same_batch_as_frame_extents_wins() {
        let pending =
            ClientGeometryReconciliation::new([120.0, 240.0], [640.0, 480.0], Some([0.0, 0.0]), 7);

        let (pending, action) =
            pending.observe(8, true, [181.0, 285.0], [640.0, 480.0], Some([11.0, 45.0]));

        assert!(pending.is_none());
        assert_eq!(action, ClientGeometryReconciliationAction::PublishNative);
    }

    #[test]
    fn resizing_updates_the_target_of_an_in_flight_reconciliation() {
        let pending = ClientGeometryReconciliation::new(
            [120.0, 240.0],
            [640.0, 480.0],
            Some([11.0, 45.0]),
            7,
        );
        let updated = pending.retarget(pending.position, [800.0, 600.0], Some([11.0, 45.0]), 8);

        assert_eq!(updated.position, [120.0, 240.0]);
        assert_eq!(updated.size, [800.0, 600.0]);
        assert_eq!(updated.starting_geometry_epoch, 8);
        assert_eq!(
            updated.phase,
            ClientGeometryReconciliationPhase::AwaitingResponse
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ViewportWindowPolicy {
    pub(super) decorations: bool,
    pub(super) top_most: bool,
    pub(super) skip_taskbar: bool,
    pub(super) cursor_hittest: bool,
    pub(super) no_focus_on_appearing: bool,
    pub(super) no_focus_on_click: bool,
}

impl Default for ViewportWindowPolicy {
    fn default() -> Self {
        Self {
            decorations: true,
            top_most: false,
            skip_taskbar: false,
            cursor_hittest: true,
            no_focus_on_appearing: false,
            no_focus_on_click: false,
        }
    }
}

impl ViewportWindowPolicy {
    pub(super) fn from_flags(flags: dear_imgui_rs::sys::ImGuiViewportFlags) -> Self {
        Self {
            decorations: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoDecoration == 0,
            top_most: flags & dear_imgui_rs::sys::ImGuiViewportFlags_TopMost != 0,
            skip_taskbar: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoTaskBarIcon != 0,
            cursor_hittest: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoInputs == 0,
            no_focus_on_appearing: flags
                & dear_imgui_rs::sys::ImGuiViewportFlags_NoFocusOnAppearing
                != 0,
            no_focus_on_click: flags & dear_imgui_rs::sys::ImGuiViewportFlags_NoFocusOnClick != 0,
        }
    }
}

/// Runtime-owned sidecar stored in `ImGuiViewport::PlatformUserData`.
#[repr(C)]
pub(super) struct ViewportData {
    // Keep the native subclass ahead of the Window so it is removed before the final Arc can
    // destroy the HWND.
    cursor_hittest: NativeCursorHitTest,
    window: Arc<Window>,
    main: bool,
    pub(super) window_policy: Cell<ViewportWindowPolicy>,
    pub(super) last_log_fb_scale: Cell<f32>,
    pending_geometry_refresh: Cell<GeometryRefresh>,
    pending_client_geometry_reconciliation: Cell<Option<ClientGeometryReconciliation>>,
    geometry_event_epoch: Cell<u64>,
}

impl ViewportData {
    pub(super) fn new(window: Arc<Window>, main: bool) -> Result<Self, WinitPlatformError> {
        let cursor_hittest = NativeCursorHitTest::install(&window)?;
        Ok(Self {
            cursor_hittest,
            window,
            main,
            window_policy: Cell::new(ViewportWindowPolicy::default()),
            last_log_fb_scale: Cell::new(0.0),
            pending_geometry_refresh: Cell::new(GeometryRefresh::default()),
            pending_client_geometry_reconciliation: Cell::new(None),
            geometry_event_epoch: Cell::new(0),
        })
    }

    pub(super) fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub(super) fn set_cursor_hittest(&self, enabled: bool) -> Result<(), WinitPlatformError> {
        self.cursor_hittest.set_enabled(&self.window, enabled)
    }

    pub(super) fn set_no_focus_on_click(&self, enabled: bool) -> Result<(), WinitPlatformError> {
        self.cursor_hittest
            .set_no_focus_on_click(&self.window, enabled)
    }

    #[cfg(target_os = "windows")]
    pub(super) fn native_window_id(&self) -> usize {
        self.cursor_hittest.native_window_id()
    }

    pub(super) fn window_ptr(&self) -> *const Window {
        Arc::as_ptr(&self.window)
    }

    pub(super) fn is_main(&self) -> bool {
        self.main
    }

    pub(super) fn request_geometry_refresh(&self, position: bool, size: bool) {
        let current = self.pending_geometry_refresh.get();
        self.pending_geometry_refresh.set(GeometryRefresh {
            position: current.position || position,
            size: current.size || size,
        });
    }

    pub(super) fn take_geometry_refresh(&self) -> GeometryRefresh {
        self.pending_geometry_refresh.take()
    }

    pub(super) fn note_geometry_event(&self) {
        if self.pending_client_geometry_reconciliation.get().is_none() {
            return;
        }
        self.geometry_event_epoch
            .set(self.geometry_event_epoch.get().wrapping_add(1));
    }

    pub(super) fn cancel_client_geometry_reconciliation(&self) {
        self.pending_client_geometry_reconciliation.set(None);
    }

    pub(super) fn request_client_geometry_reconciliation(
        &self,
        position: [f32; 2],
        size: [f32; 2],
        decoration_offset: Option<[f32; 2]>,
    ) {
        self.pending_client_geometry_reconciliation
            .set(Some(ClientGeometryReconciliation::new(
                position,
                size,
                decoration_offset,
                self.geometry_event_epoch.get(),
            )));
    }

    pub(super) fn update_reconciling_client_size(&self, size: [f32; 2]) {
        let Some(pending) = self.pending_client_geometry_reconciliation.get() else {
            return;
        };
        self.pending_client_geometry_reconciliation
            .set(Some(pending.retarget(
                pending.position,
                size,
                pending.decoration_offset,
                self.geometry_event_epoch.get(),
            )));
    }

    pub(super) fn update_reconciling_client_position(&self, position: [f32; 2]) {
        let Some(pending) = self.pending_client_geometry_reconciliation.get() else {
            return;
        };
        self.pending_client_geometry_reconciliation
            .set(Some(pending.retarget(
                position,
                pending.size,
                pending.decoration_offset,
                self.geometry_event_epoch.get(),
            )));
    }

    pub(super) fn has_client_geometry_reconciliation(&self) -> bool {
        self.pending_client_geometry_reconciliation.get().is_some()
    }

    pub(super) fn observe_client_geometry_reconciliation(
        &self,
        visible: bool,
        actual_position: [f32; 2],
        actual_size: [f32; 2],
        decoration_offset: Option<[f32; 2]>,
    ) -> Option<ClientGeometryReconciliationDecision> {
        let pending = self.pending_client_geometry_reconciliation.get()?;
        let decision = ClientGeometryReconciliationDecision {
            action: ClientGeometryReconciliationAction::Wait,
            position: pending.position,
            size: pending.size,
        };
        let (pending, action) = pending.observe(
            self.geometry_event_epoch.get(),
            visible,
            actual_position,
            actual_size,
            decoration_offset,
        );
        self.pending_client_geometry_reconciliation.set(pending);
        Some(ClientGeometryReconciliationDecision { action, ..decision })
    }
}

pub(super) fn preflight_main_viewport(
    context: &dear_imgui_rs::Context,
) -> Result<(), WinitPlatformError> {
    let binding = context.binding();
    binding.with_bound_context(|| unsafe {
        let viewport = dear_imgui_rs::sys::igGetMainViewport();
        if viewport.is_null() {
            Err(WinitPlatformError::ContextMismatch)
        } else if (*viewport).PlatformUserData.is_null()
            && (*viewport).PlatformHandle.is_null()
            && (*viewport).PlatformHandleRaw.is_null()
        {
            Ok(())
        } else {
            Err(WinitPlatformError::ForeignPlatformUserData)
        }
    })
}

pub(super) fn init_main_viewport(
    control: &RuntimeControl,
    main_window: Arc<Window>,
) -> Result<(), WinitPlatformError> {
    control.binding().try_with_bound_context(|| unsafe {
        let viewport = dear_imgui_rs::sys::igGetMainViewport();
        if viewport.is_null()
            || !(*viewport).PlatformUserData.is_null()
            || !(*viewport).PlatformHandle.is_null()
            || !(*viewport).PlatformHandleRaw.is_null()
        {
            return Err(WinitPlatformError::ForeignPlatformUserData);
        }

        let data = insert_viewport_data(
            control,
            viewport,
            ViewportData::new(Arc::clone(&main_window), true)?,
        )?;
        (*viewport).PlatformUserData = data.cast::<c_void>();
        (*viewport).PlatformHandle = Arc::as_ptr(&main_window).cast_mut().cast();
        Ok(())
    })?
}

pub(super) fn viewport_data_is_owned(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    if viewport.is_null() {
        return false;
    }
    // SAFETY: callers invoke this only for a live viewport in the current Context.
    let data = unsafe { (*viewport).PlatformUserData.cast::<ViewportData>() };
    owns_viewport_data(control, viewport, data)
}
