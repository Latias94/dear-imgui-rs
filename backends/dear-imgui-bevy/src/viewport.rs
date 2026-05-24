//! Dear ImGui platform-viewport bridge for Bevy-owned windows.
//!
//! PlatformIO callbacks installed here only capture intent into an engine-owned queue. Bevy systems
//! drain that queue and mutate ECS-owned [`Window`] entities outside the C ABI callback boundary.

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
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::*;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::schedule::IntoScheduleConfigs;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::system::SystemParam;
use bevy_math::IVec2;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use bevy_window::WindowRef;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{
    ExitSystems, Monitor, PrimaryWindow, WindowCloseRequested, WindowMoved, WindowOccluded,
    WindowResized,
};
use bevy_window::{Window, WindowLevel, WindowPosition, WindowResolution};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::WINIT_WINDOWS;
use dear_imgui_rs as imgui;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_rs::sys;
use std::collections::{HashMap, HashSet};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::ffi::{CStr, c_void};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::ptr::NonNull;

/// Stable Dear ImGui viewport identifier used by the Bevy bridge.
pub type ImguiViewportId = imgui::Id;

/// Snapshot of Dear ImGui viewport state copied while a PlatformIO callback is running.
#[derive(Clone, Debug, PartialEq)]
pub struct ImguiViewportSnapshot {
    pub id: ImguiViewportId,
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub dpi_scale: f32,
    pub flags: imgui::ViewportFlags,
}

impl ImguiViewportSnapshot {
    #[must_use]
    pub fn from_viewport(viewport: &imgui::Viewport) -> Self {
        Self {
            id: viewport.id(),
            pos: viewport.pos(),
            size: viewport.size(),
            // SAFETY: `viewport` is a live Dear ImGui viewport reference for the duration of the
            // PlatformIO callback. We copy the scalar value and do not retain the raw pointer.
            dpi_scale: unsafe { (*viewport.as_raw()).DpiScale },
            flags: viewport.flags(),
        }
    }
}

/// Intent captured from Dear ImGui PlatformIO callbacks.
#[derive(Clone, Debug, PartialEq)]
pub enum ImguiViewportCommand {
    Create(ImguiViewportSnapshot),
    Destroy { id: ImguiViewportId },
    Show { id: ImguiViewportId },
    SetPos { id: ImguiViewportId, pos: [f32; 2] },
    SetSize { id: ImguiViewportId, size: [f32; 2] },
    SetFocus { id: ImguiViewportId },
    SetTitle { id: ImguiViewportId, title: String },
}

/// Last Bevy-observed platform state for a Dear ImGui viewport window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImguiViewportFeedback {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub framebuffer_scale: [f32; 2],
    pub dpi_scale: f32,
    pub focused: bool,
    pub minimized: bool,
}

/// Marker on Bevy `Window` entities created for Dear ImGui secondary platform viewports.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportWindow {
    pub viewport_id: ImguiViewportId,
}

/// Marker on Bevy camera entities created to render Dear ImGui secondary platform viewports.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ImguiViewportCamera {
    pub viewport_id: ImguiViewportId,
}

/// Backend-local queue and viewport-to-window map for Dear ImGui platform windows.
#[derive(Default)]
pub struct ImguiViewportBridge {
    inner: Box<ImguiViewportBridgeState>,
}

#[derive(Default)]
struct ImguiViewportBridgeState {
    commands: Vec<ImguiViewportCommand>,
    viewport_windows: HashMap<ImguiViewportId, Entity>,
    viewport_cameras: HashMap<ImguiViewportId, Entity>,
    viewport_feedback: HashMap<ImguiViewportId, ImguiViewportFeedback>,
    viewport_flags: HashMap<ImguiViewportId, imgui::ViewportFlags>,
    viewport_handles: HashMap<ImguiViewportId, Box<ImguiViewportPlatformHandle>>,
    focus_next_frame: HashSet<ImguiViewportId>,
    focus_ready: HashSet<ImguiViewportId>,
}

#[derive(Debug)]
struct ImguiViewportPlatformHandle {
    _viewport_id: ImguiViewportId,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
impl ImguiViewportBridgeState {
    fn queue(&mut self, command: ImguiViewportCommand) {
        self.commands.push(command);
    }

    fn platform_handle(&mut self, viewport_id: ImguiViewportId) -> *mut c_void {
        let handle = self.viewport_handles.entry(viewport_id).or_insert_with(|| {
            Box::new(ImguiViewportPlatformHandle {
                _viewport_id: viewport_id,
            })
        });
        (&mut **handle as *mut ImguiViewportPlatformHandle).cast::<c_void>()
    }

    fn remove_platform_handle(&mut self, viewport_id: ImguiViewportId) {
        self.viewport_handles.remove(&viewport_id);
    }

    fn set_viewport_flags(&mut self, viewport_id: ImguiViewportId, flags: imgui::ViewportFlags) {
        self.viewport_flags.insert(viewport_id, flags);
    }
}

impl ImguiViewportBridge {
    #[must_use]
    pub fn commands(&self) -> &[ImguiViewportCommand] {
        &self.inner.commands
    }

    #[must_use]
    pub fn viewport_window(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner.viewport_windows.get(&viewport_id).copied()
    }

    #[must_use]
    pub fn viewport_camera(&self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner.viewport_cameras.get(&viewport_id).copied()
    }

    #[must_use]
    pub fn viewport_feedback(&self, viewport_id: ImguiViewportId) -> Option<ImguiViewportFeedback> {
        self.inner.viewport_feedback.get(&viewport_id).copied()
    }

    pub fn queue(&mut self, command: ImguiViewportCommand) {
        self.inner.commands.push(command);
    }

    #[must_use]
    pub fn drain_commands(&mut self) -> Vec<ImguiViewportCommand> {
        self.inner.commands.drain(..).collect()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn clear_viewport_state(&mut self) {
        self.inner.viewport_windows.clear();
        self.inner.viewport_cameras.clear();
        self.inner.viewport_feedback.clear();
        self.inner.viewport_flags.clear();
        self.inner.viewport_handles.clear();
        self.inner.commands.clear();
        self.inner.focus_next_frame.clear();
        self.inner.focus_ready.clear();
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn stable_ptr(&mut self) -> *mut ImguiViewportBridgeState {
        (&mut *self.inner) as *mut ImguiViewportBridgeState
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn set_viewport_window(&mut self, viewport_id: ImguiViewportId, entity: Entity) {
        self.inner.viewport_windows.insert(viewport_id, entity);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_viewport_window(&mut self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner.viewport_windows.remove(&viewport_id)
    }

    #[cfg(all(
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    fn set_viewport_camera(&mut self, viewport_id: ImguiViewportId, entity: Entity) {
        self.inner.viewport_cameras.insert(viewport_id, entity);
    }

    #[cfg(all(
        feature = "render",
        feature = "multi-viewport",
        not(target_arch = "wasm32")
    ))]
    fn remove_viewport_camera(&mut self, viewport_id: ImguiViewportId) -> Option<Entity> {
        self.inner.viewport_cameras.remove(&viewport_id)
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_viewport_feedback(&mut self, viewport_id: ImguiViewportId) {
        self.inner.viewport_feedback.remove(&viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_viewport_flags(&mut self, viewport_id: ImguiViewportId) {
        self.inner.viewport_flags.remove(&viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn remove_platform_handle(&mut self, viewport_id: ImguiViewportId) {
        self.inner.remove_platform_handle(viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn set_viewport_feedback(
        &mut self,
        viewport_id: ImguiViewportId,
        feedback: ImguiViewportFeedback,
    ) {
        self.inner.viewport_feedback.insert(viewport_id, feedback);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn show_should_focus(&self, viewport_id: ImguiViewportId) -> bool {
        !self
            .inner
            .viewport_flags
            .get(&viewport_id)
            .is_some_and(|flags| flags.contains(imgui::ViewportFlags::NO_FOCUS_ON_APPEARING))
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn request_focus_next_frame(&mut self, viewport_id: ImguiViewportId) {
        self.inner.focus_next_frame.insert(viewport_id);
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    fn clear_focus_request(&mut self, viewport_id: ImguiViewportId) {
        self.inner.focus_next_frame.remove(&viewport_id);
        self.inner.focus_ready.remove(&viewport_id);
    }
}

impl Drop for ImguiViewportBridge {
    fn drop(&mut self) {
        self.inner.commands.clear();
        self.inner.viewport_windows.clear();
        self.inner.viewport_cameras.clear();
        self.inner.viewport_feedback.clear();
        self.inner.viewport_flags.clear();
        self.inner.viewport_handles.clear();
        self.inner.focus_next_frame.clear();
        self.inner.focus_ready.clear();
    }
}

pub(crate) fn install_viewport_bridge(_app: &mut App) {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        let app = _app;
        let multi_viewport_requested = app
            .world()
            .get_resource::<crate::ImguiBackendConfig>()
            .is_some_and(|config| config.multi_viewport);
        if !multi_viewport_requested {
            return;
        }
        if app.world().get_non_send::<ImguiViewportBridge>().is_none() {
            app.insert_non_send(ImguiViewportBridge::default());
        }
        app.add_message::<WindowMoved>();
        app.add_message::<WindowResized>();
        app.add_message::<WindowCloseRequested>();
        app.add_message::<WindowOccluded>();
        app.add_systems(
            PreUpdate,
            sync_os_viewport_window_events.before(crate::input::ImguiInputSystems),
        );
        attach_bridge_to_imgui_context(app.world_mut());
        app.add_systems(
            crate::ImguiEndFrame,
            apply_viewport_commands_system.after(crate::context::end_primary_frame_system),
        );
        app.add_systems(
            Last,
            cleanup_secondary_viewports_on_primary_close.before(ExitSystems),
        );
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn attach_bridge_to_imgui_context(world: &mut World) {
    let bridge_ptr = {
        let Some(mut bridge) = world.get_non_send_mut::<ImguiViewportBridge>() else {
            return;
        };
        bridge.stable_ptr().cast()
    };

    let Some(mut imgui_context) = world.get_non_send_mut::<crate::ImguiContext>() else {
        return;
    };
    let context = imgui_context.context_mut();
    context.io_mut().set_backend_platform_user_data(bridge_ptr);
    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_platform_create_window(Some(platform_create_window));
        platform_io.set_platform_destroy_window(Some(platform_destroy_window));
        platform_io.set_platform_show_window(Some(platform_show_window));
        platform_io.set_platform_set_window_pos(Some(platform_set_window_pos));
        platform_io.set_platform_set_window_size(Some(platform_set_window_size));
        platform_io.set_platform_set_window_focus(Some(platform_set_window_focus));
        platform_io.set_platform_set_window_title(Some(platform_set_window_title));
        platform_io.set_platform_get_window_pos(Some(platform_get_window_pos));
        platform_io.set_platform_get_window_size(Some(platform_get_window_size));
        platform_io
            .set_platform_get_window_framebuffer_scale(Some(platform_get_window_framebuffer_scale));
        platform_io.set_platform_get_window_dpi_scale(Some(platform_get_window_dpi_scale));
        platform_io.set_platform_get_window_focus(Some(platform_get_window_focus));
        platform_io.set_platform_get_window_minimized(Some(platform_get_window_minimized));
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe fn current_bridge_mut() -> Option<&'static mut ImguiViewportBridgeState> {
    let io = unsafe { sys::igGetIO_Nil() };
    if io.is_null() {
        return None;
    }
    let ptr = unsafe { (*io).BackendPlatformUserData };
    NonNull::new(ptr.cast::<ImguiViewportBridgeState>()).map(|mut ptr| unsafe { ptr.as_mut() })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_create_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_mut() }) else {
        return;
    };
    let Some(bridge) = (unsafe { current_bridge_mut() }) else {
        return;
    };
    let handle = bridge.platform_handle(viewport.id());
    bridge.set_viewport_flags(viewport.id(), viewport.flags());
    viewport.set_platform_user_data(handle);
    viewport.set_platform_handle(handle);
    bridge.queue(ImguiViewportCommand::Create(
        ImguiViewportSnapshot::from_viewport(viewport),
    ));
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_destroy_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_mut() }) else {
        return;
    };
    let viewport_id = viewport.id();
    let owned_by_app = viewport
        .flags()
        .contains(imgui::ViewportFlags::OWNED_BY_APP);
    if let Some(bridge) = unsafe { current_bridge_mut() } {
        if !owned_by_app {
            bridge.queue(ImguiViewportCommand::Destroy { id: viewport_id });
        }
        bridge.viewport_flags.remove(&viewport_id);
        bridge.focus_next_frame.remove(&viewport_id);
        bridge.focus_ready.remove(&viewport_id);
        bridge.remove_platform_handle(viewport_id);
    }
    viewport.set_platform_user_data(std::ptr::null_mut());
    viewport.set_platform_handle(std::ptr::null_mut());
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_show_window(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let Some(bridge) = (unsafe { current_bridge_mut() }) else {
        return;
    };
    bridge.set_viewport_flags(viewport.id(), viewport.flags());
    bridge.queue(ImguiViewportCommand::Show { id: viewport.id() });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_pos(viewport: *mut imgui::Viewport, pos: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let Some(bridge) = (unsafe { current_bridge_mut() }) else {
        return;
    };
    bridge.queue(ImguiViewportCommand::SetPos {
        id: viewport.id(),
        pos: [pos.x, pos.y],
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_size(viewport: *mut imgui::Viewport, size: sys::ImVec2) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let Some(bridge) = (unsafe { current_bridge_mut() }) else {
        return;
    };
    bridge.queue(ImguiViewportCommand::SetSize {
        id: viewport.id(),
        size: [size.x, size.y],
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_focus(viewport: *mut imgui::Viewport) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let Some(bridge) = (unsafe { current_bridge_mut() }) else {
        return;
    };
    bridge.queue(ImguiViewportCommand::SetFocus { id: viewport.id() });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_set_window_title(
    viewport: *mut imgui::Viewport,
    title: *const std::ffi::c_char,
) {
    let Some(viewport) = (unsafe { viewport.as_ref() }) else {
        return;
    };
    let Some(bridge) = (unsafe { current_bridge_mut() }) else {
        return;
    };
    let title = if title.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(title) }
            .to_string_lossy()
            .into_owned()
    };
    bridge.queue(ImguiViewportCommand::SetTitle {
        id: viewport.id(),
        title,
    });
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_pos(
    viewport: *mut imgui::Viewport,
    out_pos: *mut sys::ImVec2,
) {
    let pos = feedback_for_viewport(viewport)
        .map(|feedback| feedback.pos)
        .unwrap_or([0.0, 0.0]);
    if let Some(out_pos) = unsafe { out_pos.as_mut() } {
        *out_pos = sys::ImVec2 {
            x: pos[0],
            y: pos[1],
        };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_size(
    viewport: *mut imgui::Viewport,
    out_size: *mut sys::ImVec2,
) {
    let size = feedback_for_viewport(viewport)
        .map(|feedback| feedback.size)
        .unwrap_or([0.0, 0.0]);
    if let Some(out_size) = unsafe { out_size.as_mut() } {
        *out_size = sys::ImVec2 {
            x: size[0],
            y: size[1],
        };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_framebuffer_scale(
    viewport: *mut imgui::Viewport,
    out_scale: *mut sys::ImVec2,
) {
    let scale = feedback_for_viewport(viewport)
        .map(|feedback| feedback.framebuffer_scale)
        .unwrap_or([1.0, 1.0]);
    if let Some(out_scale) = unsafe { out_scale.as_mut() } {
        *out_scale = sys::ImVec2 {
            x: scale[0],
            y: scale[1],
        };
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_dpi_scale(viewport: *mut imgui::Viewport) -> f32 {
    feedback_for_viewport(viewport)
        .map(|feedback| feedback.dpi_scale)
        .unwrap_or(1.0)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_focus(viewport: *mut imgui::Viewport) -> bool {
    feedback_for_viewport(viewport).is_some_and(|feedback| feedback.focused)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
unsafe extern "C" fn platform_get_window_minimized(viewport: *mut imgui::Viewport) -> bool {
    feedback_for_viewport(viewport).is_some_and(|feedback| feedback.minimized)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn feedback_for_viewport(viewport: *mut imgui::Viewport) -> Option<ImguiViewportFeedback> {
    let viewport = unsafe { viewport.as_ref() }?;
    let bridge = unsafe { current_bridge_mut() }?;
    bridge.viewport_feedback.get(&viewport.id()).copied()
}

#[derive(SystemParam)]
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
struct OsViewportWindowEvents<'w, 's> {
    moved: MessageReader<'w, 's, WindowMoved>,
    resized: MessageReader<'w, 's, WindowResized>,
    close_requests: MessageReader<'w, 's, WindowCloseRequested>,
    occluded: MessageReader<'w, 's, WindowOccluded>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn sync_os_viewport_window_events(
    mut events: OsViewportWindowEvents,
    windows: Query<&Window>,
    viewport_windows: Query<(Entity, &ImguiViewportWindow)>,
    mut imgui_context: NonSendMut<crate::ImguiContext>,
    mut bridge: NonSendMut<ImguiViewportBridge>,
) {
    let window_to_viewport = viewport_windows.iter().collect::<HashMap<_, _>>();
    let mut moved_viewports = HashSet::new();
    let mut resized_viewports = HashSet::new();
    let mut closed_viewports = HashSet::new();

    for event in events.moved.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied() {
            moved_viewports.insert(viewport_window.viewport_id);
            if let Ok(window) = windows.get(event.window) {
                let previous = bridge.viewport_feedback(viewport_window.viewport_id);
                bridge.set_viewport_feedback(
                    viewport_window.viewport_id,
                    feedback_from_window_for_entity(event.window, window, previous, None),
                );
            }
        }
    }

    for event in events.resized.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied() {
            resized_viewports.insert(viewport_window.viewport_id);
            if let Ok(window) = windows.get(event.window) {
                let previous = bridge.viewport_feedback(viewport_window.viewport_id);
                bridge.set_viewport_feedback(
                    viewport_window.viewport_id,
                    feedback_from_window_for_entity(event.window, window, previous, None),
                );
            }
        }
    }

    for event in events.close_requests.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied() {
            closed_viewports.insert(viewport_window.viewport_id);
        }
    }

    for event in events.occluded.read() {
        if let Some(viewport_window) = window_to_viewport.get(&event.window).copied()
            && let Ok(window) = windows.get(event.window)
        {
            let previous = bridge.viewport_feedback(viewport_window.viewport_id);
            bridge.set_viewport_feedback(
                viewport_window.viewport_id,
                feedback_from_window_for_entity(
                    event.window,
                    window,
                    previous,
                    Some(event.occluded),
                ),
            );
        }
    }

    mark_platform_viewport_requests(
        imgui_context.context_mut(),
        moved_viewports.iter().copied(),
        resized_viewports.iter().copied(),
        closed_viewports.iter().copied(),
    );
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn mark_platform_viewport_requests(
    context: &mut imgui::Context,
    moved_viewports: impl IntoIterator<Item = ImguiViewportId>,
    resized_viewports: impl IntoIterator<Item = ImguiViewportId>,
    closed_viewports: impl IntoIterator<Item = ImguiViewportId>,
) {
    let moved = moved_viewports.into_iter().collect::<HashSet<_>>();
    let resized = resized_viewports.into_iter().collect::<HashSet<_>>();
    let closed = closed_viewports.into_iter().collect::<HashSet<_>>();
    if moved.is_empty() && resized.is_empty() && closed.is_empty() {
        return;
    }

    for viewport in context.platform_io_mut().viewports_iter_mut() {
        let id = viewport.id();
        if moved.contains(&id) {
            viewport.set_platform_request_move(true);
        }
        if resized.contains(&id) {
            viewport.set_platform_request_resize(true);
        }
        if closed.contains(&id) {
            viewport.set_platform_request_close(true);
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_viewport_commands_system(
    mut ecs_commands: Commands,
    mut bridge: NonSendMut<ImguiViewportBridge>,
    mut windows: Query<&mut Window>,
    viewport_cameras: Query<Entity, With<ImguiViewportCamera>>,
) {
    let queued = bridge.drain_commands();
    let mut feedback_candidates = HashSet::new();
    let mut pending_windows: HashMap<ImguiViewportId, Window> = HashMap::new();
    #[cfg(feature = "render")]
    let mut pending_cameras = HashSet::new();
    #[cfg(feature = "render")]
    let mut scheduled_camera_despawns = HashSet::new();
    for command in queued {
        match command {
            ImguiViewportCommand::Create(snapshot) => {
                bridge
                    .inner
                    .viewport_flags
                    .insert(snapshot.id, snapshot.flags);
                let entity = if let Some(entity) = bridge.viewport_window(snapshot.id) {
                    entity
                } else {
                    let entity = ecs_commands
                        .spawn((
                            window_from_snapshot(&snapshot),
                            ImguiViewportWindow {
                                viewport_id: snapshot.id,
                            },
                        ))
                        .id();
                    bridge.set_viewport_window(snapshot.id, entity);
                    entity
                };
                #[cfg(feature = "render")]
                ensure_viewport_camera(
                    &mut ecs_commands,
                    &mut bridge,
                    snapshot.id,
                    entity,
                    &mut pending_cameras,
                );
                if let Ok(mut window) = windows.get_mut(entity) {
                    apply_snapshot_to_window(&snapshot, entity, &mut window);
                } else {
                    pending_windows.insert(snapshot.id, window_from_snapshot(&snapshot));
                }
                feedback_candidates.insert(snapshot.id);
            }
            ImguiViewportCommand::Destroy { id } => {
                pending_windows.remove(&id);
                if let Some(entity) = bridge.remove_viewport_window(id) {
                    ecs_commands.entity(entity).despawn();
                }
                bridge.remove_viewport_feedback(id);
                bridge.remove_viewport_flags(id);
                bridge.remove_platform_handle(id);
                bridge.clear_focus_request(id);
                #[cfg(feature = "render")]
                {
                    pending_cameras.remove(&id);
                    if let Some(entity) = bridge.remove_viewport_camera(id) {
                        scheduled_camera_despawns.insert(entity);
                        ecs_commands.entity(entity).despawn();
                    }
                }
            }
            ImguiViewportCommand::Show { id } => {
                let should_focus = bridge.show_should_focus(id);
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.visible = true;
                    if should_focus {
                        window.focused = false;
                    }
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        window.visible = true;
                        if should_focus {
                            window.focused = false;
                        }
                    });
                }
                if should_focus {
                    bridge.request_focus_next_frame(id);
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetPos { id, pos } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.position = WindowPosition::At(physical_pos_for_window(pos, window));
                } else {
                    if let Some(entity) = bridge.viewport_window(id)
                        && let Ok(mut window) = windows.get_mut(entity)
                    {
                        window.position = WindowPosition::At(physical_outer_pos_for_client_pos(
                            entity, pos, &window,
                        ));
                    }
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetSize { id, size } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.resolution.set(size[0].max(1.0), size[1].max(1.0));
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        window.resolution.set(size[0].max(1.0), size[1].max(1.0));
                    });
                }
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetFocus { id } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.focused = false;
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        window.focused = false;
                    });
                }
                bridge.request_focus_next_frame(id);
                feedback_candidates.insert(id);
            }
            ImguiViewportCommand::SetTitle { id, title } => {
                if let Some(window) = pending_windows.get_mut(&id) {
                    window.title = title;
                } else {
                    with_window_mut(&mut windows, &bridge, id, |window| {
                        window.title = title;
                    });
                }
                feedback_candidates.insert(id);
            }
        }
    }

    let pending_viewport_ids = pending_windows.keys().copied().collect::<HashSet<_>>();
    for (viewport_id, window) in pending_windows {
        if let Some(entity) = bridge.viewport_window(viewport_id) {
            let previous = bridge.viewport_feedback(viewport_id);
            bridge.set_viewport_feedback(
                viewport_id,
                feedback_from_window_for_entity(entity, &window, previous, None),
            );
            ecs_commands.entity(entity).insert(window);
        }
    }

    for viewport_id in feedback_candidates {
        if pending_viewport_ids.contains(&viewport_id) {
            continue;
        }
        if let Some(entity) = bridge.viewport_window(viewport_id)
            && let Ok(window) = windows.get(entity)
        {
            let previous = bridge.viewport_feedback(viewport_id);
            bridge.set_viewport_feedback(
                viewport_id,
                feedback_from_window_for_entity(entity, window, previous, None),
            );
        }
    }

    apply_pending_viewport_focus_requests(&mut windows, &mut bridge);

    #[cfg(feature = "render")]
    cleanup_orphaned_viewport_cameras(
        &mut ecs_commands,
        &mut bridge,
        viewport_cameras.iter(),
        &scheduled_camera_despawns,
    );
    #[cfg(not(feature = "render"))]
    cleanup_orphaned_viewport_cameras(&mut ecs_commands, &mut bridge, viewport_cameras.iter());
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_pending_viewport_focus_requests(
    windows: &mut Query<&mut Window>,
    bridge: &mut ImguiViewportBridge,
) {
    let ready = std::mem::take(&mut bridge.inner.focus_ready);
    for viewport_id in ready {
        if let Some(entity) = bridge.viewport_window(viewport_id)
            && let Ok(mut window) = windows.get_mut(entity)
        {
            window.focused = true;
        }
    }
    bridge
        .inner
        .focus_ready
        .extend(bridge.inner.focus_next_frame.drain());
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn cleanup_secondary_viewports_on_primary_close(
    mut ecs_commands: Commands,
    mut close_requests: MessageReader<WindowCloseRequested>,
    primary_windows: Query<Entity, With<PrimaryWindow>>,
    viewport_windows: Query<Entity, With<ImguiViewportWindow>>,
    viewport_cameras: Query<Entity, With<ImguiViewportCamera>>,
    mut bridge: NonSendMut<ImguiViewportBridge>,
) {
    let Ok(primary_window) = primary_windows.single() else {
        return;
    };
    if !close_requests
        .read()
        .any(|event| event.window == primary_window)
    {
        return;
    }

    for entity in viewport_windows.iter().chain(viewport_cameras.iter()) {
        ecs_commands.entity(entity).despawn();
    }
    bridge.clear_viewport_state();
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn prepare_platform_viewports_for_frame(
    context: &mut imgui::Context,
    bridge: &mut ImguiViewportBridge,
    primary_window: Entity,
    window: &Window,
    monitors: &[sys::ImGuiPlatformMonitor],
    viewport_windows: impl Iterator<Item = (Entity, ImguiViewportId, ImguiViewportFeedback)>,
    enable_viewports: bool,
) {
    let mut live_feedback = HashSet::new();
    let main_viewport_id = {
        let main_viewport = context.main_viewport();
        main_viewport.id()
    };
    bridge.set_viewport_window(main_viewport_id, primary_window);
    bridge.set_viewport_feedback(
        main_viewport_id,
        feedback_from_window_for_entity(
            primary_window,
            window,
            bridge.viewport_feedback(main_viewport_id),
            None,
        ),
    );
    live_feedback.insert(main_viewport_id);

    for (entity, viewport_id, feedback) in viewport_windows {
        bridge.set_viewport_window(viewport_id, entity);
        bridge.set_viewport_feedback(viewport_id, feedback);
        live_feedback.insert(viewport_id);
    }

    bridge
        .inner
        .viewport_feedback
        .retain(|viewport_id, _| live_feedback.contains(viewport_id));
    bridge
        .inner
        .viewport_windows
        .retain(|viewport_id, _| live_feedback.contains(viewport_id));
    bridge
        .inner
        .viewport_cameras
        .retain(|viewport_id, _| live_feedback.contains(viewport_id));
    bridge
        .inner
        .viewport_flags
        .retain(|viewport_id, _| live_feedback.contains(viewport_id));
    bridge
        .inner
        .focus_next_frame
        .retain(|viewport_id| live_feedback.contains(viewport_id));
    bridge
        .inner
        .focus_ready
        .retain(|viewport_id| live_feedback.contains(viewport_id));
    bridge
        .inner
        .viewport_handles
        .retain(|viewport_id, _| live_feedback.contains(viewport_id));
    let main_viewport = context.main_viewport();
    let main_viewport_handle = bridge.inner.platform_handle(main_viewport_id);
    main_viewport.set_platform_handle(main_viewport_handle);
    main_viewport.set_platform_user_data(main_viewport_handle);

    let fallback_monitor = monitor_from_window(window);
    let monitors = if monitors.is_empty() {
        std::slice::from_ref(&fallback_monitor)
    } else {
        monitors
    };
    context.platform_io_mut().set_monitors(monitors);

    let io = context.io_mut();
    let mut backend_flags = io.backend_flags();
    backend_flags.remove(
        imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
            | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
    );
    if enable_viewports {
        backend_flags |= imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
            | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
    }
    io.set_backend_flags(backend_flags);

    let mut config_flags = io.config_flags();
    if enable_viewports {
        config_flags.insert(imgui::ConfigFlags::VIEWPORTS_ENABLE);
    } else {
        config_flags.remove(imgui::ConfigFlags::VIEWPORTS_ENABLE);
    }
    io.set_config_flags(config_flags);
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn cleanup_orphaned_viewport_cameras(
    ecs_commands: &mut Commands,
    bridge: &mut ImguiViewportBridge,
    viewport_cameras: impl Iterator<Item = Entity>,
    scheduled_camera_despawns: &HashSet<Entity>,
) {
    let live_cameras = viewport_cameras.collect::<HashSet<_>>();
    let mapped_cameras = bridge
        .inner
        .viewport_cameras
        .values()
        .copied()
        .collect::<HashSet<_>>();
    let orphaned_cameras = live_cameras
        .into_iter()
        .filter(|camera| {
            !mapped_cameras.contains(camera) && !scheduled_camera_despawns.contains(camera)
        })
        .collect::<Vec<_>>();
    for camera in orphaned_cameras {
        ecs_commands.entity(camera).despawn();
    }
}

#[cfg(all(
    not(feature = "render"),
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn cleanup_orphaned_viewport_cameras(
    _ecs_commands: &mut Commands,
    _bridge: &mut ImguiViewportBridge,
    _viewport_cameras: impl Iterator<Item = Entity>,
) {
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn ensure_viewport_camera(
    ecs_commands: &mut Commands,
    bridge: &mut ImguiViewportBridge,
    viewport_id: ImguiViewportId,
    window_entity: Entity,
    pending_cameras: &mut HashSet<ImguiViewportId>,
) {
    if bridge.viewport_camera(viewport_id).is_some() || !pending_cameras.insert(viewport_id) {
        return;
    }

    let camera = ecs_commands
        .spawn((
            Camera2d,
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window_entity)),
            RenderLayers::none(),
            ImguiViewportCamera { viewport_id },
        ))
        .id();
    bridge.set_viewport_camera(viewport_id, camera);
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn monitor_from_window(window: &Window) -> sys::ImGuiPlatformMonitor {
    let mut monitor = sys::ImGuiPlatformMonitor::default();
    let pos = match window.position {
        WindowPosition::At(pos) => logical_pos(pos, window),
        WindowPosition::Automatic | WindowPosition::Centered(_) => [0.0, 0.0],
    };
    let size = [window.width().max(1.0), window.height().max(1.0)];
    monitor.MainPos = sys::ImVec2 {
        x: pos[0],
        y: pos[1],
    };
    monitor.MainSize = sys::ImVec2 {
        x: size[0],
        y: size[1],
    };
    monitor.WorkPos = monitor.MainPos;
    monitor.WorkSize = monitor.MainSize;
    monitor.DpiScale = positive_finite_or(window.scale_factor(), 1.0);
    monitor
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub fn platform_monitors_from_bevy_monitors(
    monitors: impl IntoIterator<Item = (Monitor, bool)>,
) -> Vec<sys::ImGuiPlatformMonitor> {
    let mut monitors = monitors.into_iter().collect::<Vec<_>>();
    monitors.sort_by_key(|(monitor, is_primary)| {
        (
            !*is_primary,
            monitor.physical_position.x,
            monitor.physical_position.y,
        )
    });
    monitors
        .into_iter()
        .map(|(monitor, _)| platform_monitor_from_bevy_monitor(&monitor))
        .collect()
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn platform_monitor_from_bevy_monitor(monitor: &Monitor) -> sys::ImGuiPlatformMonitor {
    let scale = positive_finite_or(monitor.scale_factor as f32, 1.0);
    let pos = monitor.physical_position.as_vec2() / scale;
    let size = bevy_math::Vec2::new(
        monitor.physical_width as f32,
        monitor.physical_height as f32,
    ) / scale;
    let mut platform_monitor = sys::ImGuiPlatformMonitor::default();
    platform_monitor.MainPos = sys::ImVec2 { x: pos.x, y: pos.y };
    platform_monitor.MainSize = sys::ImVec2 {
        x: size.x,
        y: size.y,
    };
    platform_monitor.WorkPos = platform_monitor.MainPos;
    platform_monitor.WorkSize = platform_monitor.MainSize;
    platform_monitor.DpiScale = scale;
    platform_monitor
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn viewport_feedback_from_window(
    entity: Entity,
    window: &Window,
    previous: Option<ImguiViewportFeedback>,
) -> ImguiViewportFeedback {
    feedback_from_window_for_entity(entity, window, previous, None)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn feedback_from_window_for_entity(
    entity: Entity,
    window: &Window,
    previous: Option<ImguiViewportFeedback>,
    minimized: Option<bool>,
) -> ImguiViewportFeedback {
    let pos = window_client_origin_logical(entity, &window.position, window.scale_factor())
        .or_else(|| previous.map(|feedback| feedback.pos))
        .unwrap_or([0.0, 0.0]);
    let scale_factor = window_client_scale_factor(entity, window);
    ImguiViewportFeedback {
        pos,
        size: [window.width().max(0.0), window.height().max(0.0)],
        framebuffer_scale: [scale_factor, scale_factor],
        dpi_scale: scale_factor,
        focused: window.focused,
        minimized: minimized
            .or_else(|| previous.map(|feedback| feedback.minimized))
            .unwrap_or(false),
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn window_client_origin_logical(
    entity: Entity,
    position: &WindowPosition,
    scale_factor: f32,
) -> Option<[f32; 2]> {
    if let Some(pos) = winit_window_client_origin_logical(entity) {
        return Some(pos);
    }
    match *position {
        WindowPosition::At(pos) => Some(logical_pos_with_scale(pos, scale_factor)),
        WindowPosition::Automatic | WindowPosition::Centered(_) => None,
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn window_client_scale_factor(entity: Entity, window: &Window) -> f32 {
    winit_window_scale_factor(entity)
        .unwrap_or_else(|| positive_finite_or(window.scale_factor(), 1.0))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_client_origin_logical(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = window.scale_factor();
        if let Ok(pos_phys) = window.inner_position() {
            let pos_logical = pos_phys.to_logical::<f64>(scale);
            Some([pos_logical.x as f32, pos_logical.y as f32])
        } else if let Ok(pos_phys) = window.outer_position() {
            let pos_logical = pos_phys.to_logical::<f64>(scale);
            Some([pos_logical.x as f32, pos_logical.y as f32])
        } else {
            None
        }
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_decoration_offset_logical(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = window.scale_factor();
        let inner = window.inner_position().ok()?.to_logical::<f64>(scale);
        let outer = window.outer_position().ok()?.to_logical::<f64>(scale);
        Some([(inner.x - outer.x) as f32, (inner.y - outer.y) as f32])
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_scale_factor(entity: Entity) -> Option<f32> {
    WINIT_WINDOWS.with_borrow(|windows| {
        windows
            .get_window(entity)
            .map(|window| positive_finite_or(window.scale_factor() as f32, 1.0))
    })
}

fn positive_finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn with_window_mut(
    windows: &mut Query<&mut Window>,
    bridge: &ImguiViewportBridge,
    id: ImguiViewportId,
    f: impl FnOnce(&mut Window),
) -> Option<()> {
    let entity = bridge.viewport_window(id)?;
    let Ok(mut window) = windows.get_mut(entity) else {
        return None;
    };
    f(&mut window);
    Some(())
}

#[must_use]
pub fn window_from_snapshot(snapshot: &ImguiViewportSnapshot) -> Window {
    let scale_factor = positive_finite_or(snapshot.dpi_scale, 1.0);
    let mut window = Window {
        title: format!("Dear ImGui Viewport {}", snapshot.id.raw()),
        position: WindowPosition::At(physical_pos(snapshot.pos, scale_factor)),
        resolution: WindowResolution::new(
            physical_extent(snapshot.size[0]),
            physical_extent(snapshot.size[1]),
        ),
        decorations: !snapshot.flags.contains(imgui::ViewportFlags::NO_DECORATION),
        skip_taskbar: snapshot
            .flags
            .contains(imgui::ViewportFlags::NO_TASK_BAR_ICON),
        window_level: if snapshot.flags.contains(imgui::ViewportFlags::TOP_MOST) {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        },
        visible: false,
        focused: false,
        ..Default::default()
    };
    window.resolution.set_scale_factor(scale_factor);
    window
        .resolution
        .set(snapshot.size[0].max(1.0), snapshot.size[1].max(1.0));
    window
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_snapshot_to_window(snapshot: &ImguiViewportSnapshot, entity: Entity, window: &mut Window) {
    let next = window_from_snapshot(snapshot);
    window.position = WindowPosition::At(physical_outer_pos_for_client_pos(
        entity,
        snapshot.pos,
        &next,
    ));
    window.resolution = next.resolution;
    window.decorations = next.decorations;
    window.skip_taskbar = next.skip_taskbar;
    window.window_level = next.window_level;
    window.focused = false;
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn physical_pos_for_window(pos: [f32; 2], window: &Window) -> IVec2 {
    physical_pos(pos, positive_finite_or(window.scale_factor(), 1.0))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn physical_outer_pos_for_client_pos(entity: Entity, pos: [f32; 2], window: &Window) -> IVec2 {
    let pos = if let Some(offset) = winit_window_decoration_offset_logical(entity) {
        [pos[0] - offset[0], pos[1] - offset[1]]
    } else {
        pos
    };
    physical_pos(pos, window_client_scale_factor(entity, window))
}

fn physical_pos(pos: [f32; 2], scale_factor: f32) -> IVec2 {
    IVec2::new(
        (pos[0] * scale_factor).round() as i32,
        (pos[1] * scale_factor).round() as i32,
    )
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn logical_pos(pos: IVec2, window: &Window) -> [f32; 2] {
    logical_pos_with_scale(pos, window.scale_factor())
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn logical_pos_with_scale(pos: IVec2, scale_factor: f32) -> [f32; 2] {
    let scale_factor = positive_finite_or(scale_factor, 1.0);
    [pos.x as f32 / scale_factor, pos.y as f32 / scale_factor]
}

fn physical_extent(value: f32) -> u32 {
    value.round().max(1.0) as u32
}

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn feedback() -> ImguiViewportFeedback {
        ImguiViewportFeedback {
            pos: [0.0, 0.0],
            size: [64.0, 64.0],
            framebuffer_scale: [1.0, 1.0],
            dpi_scale: 1.0,
            focused: false,
            minimized: false,
        }
    }

    #[test]
    fn prepare_platform_viewports_prunes_handles_for_missing_viewports() {
        let mut context = imgui::Context::create();
        let mut bridge = ImguiViewportBridge::default();
        let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");
        let secondary_window = Entity::from_raw_u32(2).expect("test entity index should be valid");
        let stale_viewport = imgui::Id::from(0x500);
        let live_viewport = imgui::Id::from(0x501);

        bridge.inner.platform_handle(stale_viewport);
        bridge.inner.platform_handle(live_viewport);

        prepare_platform_viewports_for_frame(
            &mut context,
            &mut bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::once((secondary_window, live_viewport, feedback())),
            true,
        );

        let main_viewport_id = context.main_viewport().id();
        assert!(
            bridge
                .inner
                .viewport_handles
                .contains_key(&main_viewport_id)
        );
        assert!(bridge.inner.viewport_handles.contains_key(&live_viewport));
        assert!(
            !bridge.inner.viewport_handles.contains_key(&stale_viewport),
            "platform handles must not outlive viewports that disappeared from the Bevy mapping"
        );

        let main_viewport = context.main_viewport();
        main_viewport.set_platform_handle(std::ptr::null_mut());
        main_viewport.set_platform_user_data(std::ptr::null_mut());
    }
}
