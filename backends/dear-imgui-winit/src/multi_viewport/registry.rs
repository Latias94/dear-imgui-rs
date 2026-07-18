use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use dear_imgui_rs::{ContextId, ContextLifecycle};
use winit::window::Window;

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
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    data: Box<ViewportData>,
}

impl ViewportEntry {
    fn data_ptr(&self) -> *mut ViewportData {
        std::ptr::from_ref::<ViewportData>(&self.data).cast_mut()
    }

    pub(super) fn detach_and_drop(self) {
        if self.viewport.is_null() {
            return;
        }

        // SAFETY: this path is called only while the Context is alive or from its platform-window
        // teardown phase. The entry was created from this live viewport and remains registered
        // until this operation takes its Box exactly once.
        unsafe {
            let viewport = &mut *self.viewport;
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
    if viewports.iter().any(|entry| entry.viewport == viewport) {
        return Err(WinitPlatformError::ForeignPlatformUserData);
    }

    let mut data = Box::new(data);
    let data_ptr = std::ptr::from_mut::<ViewportData>(&mut data);
    viewports.push(ViewportEntry { viewport, data });
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
    control
        .viewports
        .borrow()
        .iter()
        .any(|entry| entry.viewport == viewport && std::ptr::eq(entry.data_ptr(), data))
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
        entry.viewport == viewport
            // SAFETY: `viewport` is live for the callback or Context-bound event routing call.
            && unsafe { (*viewport).PlatformUserData == entry.data_ptr().cast() }
    })?;
    Some(callback(&entry.data))
}

pub(super) fn window_for_viewport(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Option<Arc<Window>> {
    with_viewport_data(control, viewport, |data| Arc::clone(data.window()))
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
            entry.viewport == viewport
                // SAFETY: the callback supplies a live viewport for the current Context.
                && unsafe { (*viewport).PlatformUserData == entry.data_ptr().cast() }
        }) else {
            return false;
        };
        viewports.remove(index)
    };
    entry.detach_and_drop();
    true
}
