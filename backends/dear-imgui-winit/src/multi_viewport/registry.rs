use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use dear_imgui_rs::{ContextId, ContextLifecycle};
use winit::window::{Window, WindowId};

use super::WinitPlatformError;
use super::runtime::RuntimeControl;
use super::viewport_data::ViewportData;

struct RegisteredRuntime {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    context_id: ContextId,
    control: Weak<RuntimeControl>,
}

thread_local! {
    static RUNTIMES: RefCell<Vec<RegisteredRuntime>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct ViewportEntry {
    identity: ViewportIdentity,
    data: Box<ViewportData>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ViewportIdentity {
    address: usize,
    id: u32,
}

impl ViewportIdentity {
    unsafe fn capture(viewport: *mut dear_imgui_rs::sys::ImGuiViewport) -> Self {
        Self {
            address: viewport as usize,
            id: unsafe { (*viewport).ID },
        }
    }

    unsafe fn resolve(self) -> Option<*mut dear_imgui_rs::sys::ImGuiViewport> {
        let viewport = unsafe { dear_imgui_rs::sys::igFindViewportByID(self.id) };
        (!viewport.is_null() && viewport as usize == self.address).then_some(viewport)
    }
}

impl ViewportEntry {
    fn data_ptr(&self) -> *mut ViewportData {
        std::ptr::from_ref::<ViewportData>(&self.data).cast_mut()
    }

    fn matches_viewport(&self, viewport: *mut dear_imgui_rs::sys::ImGuiViewport) -> bool {
        viewport as usize == self.identity.address
    }

    unsafe fn resolve_viewport(&self) -> Option<*mut dear_imgui_rs::sys::ImGuiViewport> {
        unsafe { self.identity.resolve() }
    }

    unsafe fn native_ownership_loss(
        &self,
        viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    ) -> Option<&'static str> {
        debug_assert!(self.matches_viewport(viewport));
        let viewport = unsafe { &*viewport };
        if viewport.PlatformUserData != self.data_ptr().cast() {
            Some("PlatformUserData")
        } else if viewport.PlatformHandle != self.data.window_ptr().cast_mut().cast() {
            Some("PlatformHandle")
        } else if !viewport.PlatformHandleRaw.is_null() {
            Some("PlatformHandleRaw")
        } else {
            None
        }
    }

    unsafe fn native_fields_are_owned(&self) -> bool {
        let Some(viewport) = (unsafe { self.resolve_viewport() }) else {
            return false;
        };
        unsafe { self.native_ownership_loss(viewport).is_none() }
    }

    fn viewport_id(&self) -> u32 {
        self.identity.id
    }

    pub(super) fn detach_and_drop(self) {
        // Resolve through Dear ImGui's complete internal viewport list before touching native
        // storage. `PlatformIO.Viewports` intentionally omits hidden viewports, and a destroyed
        // viewport must never be reached through the address retained by this sidecar.
        if let Some(viewport) = unsafe { self.resolve_viewport() } {
            // SAFETY: `igFindViewportByID` returned this exact still-live viewport address.
            let viewport = unsafe { &mut *viewport };
            if viewport.PlatformUserData == self.data_ptr().cast() {
                viewport.PlatformUserData = std::ptr::null_mut();
            }
            if viewport.PlatformHandle == self.data.window_ptr().cast_mut().cast() {
                viewport.PlatformHandle = std::ptr::null_mut();
            }
        }
    }
}

pub(super) fn register_runtime(control: &Rc<RuntimeControl>) {
    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        debug_assert!(
            !runtimes
                .iter()
                .any(|entry| entry.context_id == control.binding().id())
        );
        runtimes.push(RegisteredRuntime {
            context_raw: control.context_raw(),
            context_id: control.binding().id(),
            control: Rc::downgrade(control),
        });
    });
}

pub(super) fn unregister_runtime(context: ContextId) {
    RUNTIMES.with(|runtimes| {
        runtimes
            .borrow_mut()
            .retain(|entry| entry.context_id != context);
    });
}

pub(super) fn runtime_for_context(
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
) -> Option<Rc<RuntimeControl>> {
    if context_raw.is_null() {
        return None;
    }

    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        runtimes
            .iter()
            .find(|entry| entry.context_raw == context_raw)
            .and_then(|entry| entry.control.upgrade())
    })
}

pub(super) fn with_current_runtime<R>(
    callback: impl FnOnce(&Rc<RuntimeControl>) -> R,
) -> Option<R> {
    // SAFETY: this reads the current Context pointer without dereferencing it.
    let context_raw = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
    let control = runtime_for_context(context_raw)?;
    if !control.is_callback_accessible() {
        return None;
    }

    match control.binding().lifecycle() {
        ContextLifecycle::Alive => control
            .binding()
            .try_with_bound_context(|| callback(&control))
            .ok(),
        ContextLifecycle::Dropping if control.teardown_callbacks_active() => {
            // The platform attachment opened this narrow callback window from a
            // ContextTeardown capability and verified that this raw Context is current.
            Some(callback(&control))
        }
        ContextLifecycle::Dropping | ContextLifecycle::NativeDestroyed => None,
        _ => None,
    }
}

pub(super) fn insert_viewport_data(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    data: ViewportData,
) -> Result<*mut ViewportData, WinitPlatformError> {
    if viewport.is_null() {
        return Err(WinitPlatformError::ForeignPlatformUserData);
    }

    let mut viewports = control.viewports.borrow_mut();
    if viewports
        .iter()
        .any(|entry| entry.matches_viewport(viewport))
    {
        return Err(WinitPlatformError::ForeignPlatformUserData);
    }

    let mut data = Box::new(data);
    let data_ptr = std::ptr::from_mut::<ViewportData>(&mut data);
    viewports.push(ViewportEntry {
        identity: unsafe { ViewportIdentity::capture(viewport) },
        data,
    });
    Ok(data_ptr)
}

pub(super) fn owns_viewport_data(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    data: *mut ViewportData,
) -> bool {
    if viewport.is_null() || data.is_null() {
        return false;
    }
    control.viewports.borrow().iter().any(|entry| {
        entry.matches_viewport(viewport)
            && std::ptr::eq(entry.data_ptr(), data)
            && unsafe { entry.native_fields_are_owned() }
    })
}

pub(super) fn with_viewport_data<R>(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    callback: impl FnOnce(&ViewportData) -> R,
) -> Option<R> {
    if viewport.is_null() {
        return None;
    }
    let viewports = control.viewports.borrow();
    let entry = viewports.iter().find(|entry| {
        entry.matches_viewport(viewport)
            // SAFETY: `viewport` is live for the callback or Context-bound event routing call.
            && unsafe { entry.native_fields_are_owned() }
    })?;
    Some(callback(&entry.data))
}

/// Resolves a native Winit window through the authoritative runtime registry.
///
/// Dear ImGui's public `PlatformIO.Viewports` vector is a per-frame visible snapshot and may omit
/// hidden or transitional viewports. Event routing must therefore resolve by the Winit window ID
/// first, then validate the retained native ownership before exposing the viewport pointer.
pub(super) fn with_viewport_data_for_window<R>(
    control: &RuntimeControl,
    window_id: WindowId,
    callback: impl FnOnce(*mut dear_imgui_rs::sys::ImGuiViewport, &ViewportData) -> R,
) -> Option<R> {
    let viewports = control.viewports.borrow();
    let entry = viewports
        .iter()
        .find(|entry| !entry.data.is_main() && entry.data.window().id() == window_id)?;
    let viewport = unsafe { entry.resolve_viewport()? };
    if unsafe { entry.native_ownership_loss(viewport).is_some() } {
        return None;
    }
    Some(callback(viewport, &entry.data))
}

pub(super) fn window_for_viewport(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Option<Arc<Window>> {
    with_viewport_data(control, viewport, |data| Arc::clone(data.window()))
}

pub(super) fn secondary_viewport_windows(control: &RuntimeControl) -> Vec<Arc<Window>> {
    control
        .viewports
        .borrow()
        .iter()
        .filter(|entry| !entry.data.is_main())
        .map(|entry| Arc::clone(entry.data.window()))
        .collect()
}

pub(super) fn request_geometry_refresh_for_window(
    control: &RuntimeControl,
    window_id: WindowId,
    position: bool,
    size: bool,
) {
    if let Some(entry) = control
        .viewports
        .borrow()
        .iter()
        .find(|entry| entry.data.window().id() == window_id)
    {
        entry.data.request_geometry_refresh(position, size);
    }
}

pub(super) fn apply_pending_geometry_refresh(control: &RuntimeControl) {
    for entry in control.viewports.borrow().iter() {
        let refresh = entry.data.take_geometry_refresh();
        if refresh.is_empty() {
            continue;
        }
        let Some(viewport) = (unsafe { entry.resolve_viewport() }) else {
            continue;
        };
        if unsafe { entry.native_ownership_loss(viewport).is_some() } {
            continue;
        }
        unsafe {
            (*viewport).PlatformRequestMove |= refresh.position;
            (*viewport).PlatformRequestResize |= refresh.size;
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) fn viewport_id_for_native_window(
    control: &RuntimeControl,
    native_window: usize,
) -> Option<u32> {
    control.viewports.borrow().iter().find_map(|entry| {
        (entry.data.native_window_id() == native_window).then_some(entry.viewport_id())
    })
}

pub(super) fn remove_viewport_data(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> bool {
    if viewport.is_null() {
        return false;
    }

    let entry = {
        let mut viewports = control.viewports.borrow_mut();
        let Some(index) = viewports.iter().position(|entry| {
            entry.matches_viewport(viewport)
                // SAFETY: the callback supplies a live viewport for the current Context.
                && unsafe { entry.native_fields_are_owned() }
        }) else {
            return false;
        };
        viewports.remove(index)
    };
    entry.detach_and_drop();
    true
}

fn discard_destroyed_viewport_data(control: &RuntimeControl) {
    control.viewports.borrow_mut().retain(|entry| {
        // SAFETY: callers hold the current live Context. The resolver never dereferences the
        // retained address and only returns a pointer Dear ImGui still owns under this identity.
        unsafe { entry.resolve_viewport().is_some() }
    });
}

pub(super) unsafe fn preflight_viewport_ownership(
    control: &RuntimeControl,
    platform_io: *const dear_imgui_rs::sys::ImGuiPlatformIO,
) -> Result<(), WinitPlatformError> {
    if platform_io.is_null() {
        return Err(WinitPlatformError::ContextMismatch);
    }
    let native_viewports = unsafe { &(*platform_io).Viewports };
    let count = usize::try_from(native_viewports.Size)
        .map_err(|_| WinitPlatformError::ForeignPlatformUserData)?;
    if native_viewports.Capacity < native_viewports.Size
        || count > 0 && native_viewports.Data.is_null()
    {
        return Err(WinitPlatformError::ForeignPlatformUserData);
    }
    let native_viewports = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(native_viewports.Data, count) }
    };
    discard_destroyed_viewport_data(control);
    let entries = control.viewports.borrow();

    for entry in entries.iter() {
        let Some(viewport) = (unsafe { entry.resolve_viewport() }) else {
            continue;
        };
        if let Some(field) = unsafe { entry.native_ownership_loss(viewport) } {
            return Err(WinitPlatformError::ViewportOwnershipLost {
                viewport_id: entry.viewport_id(),
                field,
            });
        }
    }
    for &viewport in native_viewports {
        if viewport.is_null() {
            return Err(WinitPlatformError::ForeignPlatformUserData);
        }
        let viewport = unsafe { &*viewport };
        let viewport_ptr = std::ptr::from_ref(viewport).cast_mut();
        if (!viewport.PlatformUserData.is_null()
            || !viewport.PlatformHandle.is_null()
            || !viewport.PlatformHandleRaw.is_null())
            && !entries.iter().any(|entry| {
                entry.matches_viewport(viewport_ptr) && unsafe { entry.native_fields_are_owned() }
            })
        {
            let field = if !viewport.PlatformUserData.is_null() {
                "PlatformUserData"
            } else if !viewport.PlatformHandle.is_null() {
                "PlatformHandle"
            } else {
                "PlatformHandleRaw"
            };
            return Err(WinitPlatformError::ViewportOwnershipLost {
                viewport_id: viewport.ID,
                field,
            });
        }
    }
    Ok(())
}
