use std::cell::RefCell;
use std::rc::{Rc, Weak};

use dear_imgui_rs::{ContextId, ContextLifecycle};

use super::runtime::{GlowViewportError, RuntimeControl};

struct RegisteredRuntime {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    context_id: ContextId,
    control: Weak<RuntimeControl>,
}

thread_local! {
    static RUNTIMES: RefCell<Vec<RegisteredRuntime>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn preflight_runtime(context: ContextId) -> Result<(), GlowViewportError> {
    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        if runtimes.iter().any(|entry| entry.context_id == context) {
            Err(GlowViewportError::RuntimeAlreadyAttached)
        } else {
            Ok(())
        }
    })
}

pub(super) fn register_runtime(control: &Rc<RuntimeControl>) {
    RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|entry| entry.control.strong_count() > 0);
        debug_assert!(
            !runtimes
                .iter()
                .any(|entry| entry.context_id == control.binding().id()),
            "Glow viewport runtime registered twice for one Context"
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
    // SAFETY: the pointer is used only as a registry key until ContextBinding validates it.
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
        ContextLifecycle::Dropping | ContextLifecycle::NativeDestroyed => None,
        _ => None,
    }
}
