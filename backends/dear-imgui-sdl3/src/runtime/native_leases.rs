use std::ffi::c_void;

use sdl3::video::Window;
use sdl3_sys::video::{SDL_GetWindowFromID, SDL_GetWindowParent, SDL_Window, SDL_WindowID};

use super::*;

/// One exact native SDL allocation, retained by its current registration and native children.
/// Retired native resources no longer contain ImGui sidecars or require a live Context.
pub(crate) struct NativeWindowAllocation {
    pointer: *mut SDL_Window,
    is_main: bool,
    retired: Cell<bool>,
    work_pins: Cell<usize>,
    retained_context: Cell<Option<*mut c_void>>,
    // Field order matters: destroy this native window in Drop, then release its native owner,
    // then the host's VideoSubsystem/native main-window keepalive.
    owner: RefCell<Option<Rc<NativeWindowAllocation>>>,
    _host_window: Window,
}

impl fmt::Debug for NativeWindowAllocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeWindowAllocation")
            .field("is_main", &self.is_main)
            .field("retired", &self.retired.get())
            .field("work_pins", &self.work_pins.get())
            .finish_non_exhaustive()
    }
}

impl NativeWindowAllocation {
    pub(super) fn new(pointer: *mut SDL_Window, host_window: Window, is_main: bool) -> Self {
        Self {
            pointer,
            is_main,
            retired: Cell::new(false),
            work_pins: Cell::new(0),
            retained_context: Cell::new(None),
            owner: RefCell::new(None),
            _host_window: host_window,
        }
    }

    pub(super) fn mark_retired(&self) {
        self.retired.set(true);
    }
}

impl Drop for NativeWindowAllocation {
    fn drop(&mut self) {
        if let Some(context) = self.retained_context.take() {
            // The current ImGui Context and allocator may already have been destroyed or changed.
            // Retirement released the C++ sidecar synchronously; this bridge touches SDL only.
            unsafe {
                crate::core::ffi::dear_imgui_sdl3_destroy_leased_window(self.pointer, context)
            };
        }
    }
}

struct NativeWindowPin {
    // Drop the target allocation before dropping the parent's pin. The parent's own resources
    // remain pinned while a retired child allocation releases its SDL window.
    allocation: Rc<NativeWindowAllocation>,
    owner: Option<Box<NativeWindowPin>>,
}

impl Drop for NativeWindowPin {
    fn drop(&mut self) {
        self.allocation
            .work_pins
            .set(self.allocation.work_pins.get().saturating_sub(1));
    }
}

impl NativeWindowPin {
    fn any_retired(&self) -> bool {
        self.allocation.retired.get()
            || self.owner.as_ref().is_some_and(|owner| owner.any_retired())
    }
}

/// Native lifetime capability retained by recorded or pending SDL3 renderer work.
///
/// The exact target and captured native owner chain remain alive until the last clone drops.
/// A retired registration cannot issue new work, but already submitted work can finish presentation
/// or discard before releasing this capability. Drop acquired frames and surfaces before the lease.
/// This capability is intentionally neither `Send` nor `Sync` and never borrows a live Context.
///
/// ```compile_fail
/// use dear_imgui_sdl3::Sdl3NativeWindowLease;
/// fn require_send<T: Send>() {}
/// require_send::<Sdl3NativeWindowLease>();
/// ```
///
/// ```compile_fail
/// use dear_imgui_sdl3::Sdl3NativeWindowLease;
/// fn require_sync<T: Sync>() {}
/// require_sync::<Sdl3NativeWindowLease>();
/// ```
#[doc(hidden)]
#[derive(Clone)]
pub struct Sdl3NativeWindowLease {
    snapshot: Sdl3WindowSnapshot,
    pin: Rc<NativeWindowPin>,
}

impl fmt::Debug for Sdl3NativeWindowLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3NativeWindowLease")
            .field("target", &self.target())
            .field("retired", &self.is_retired())
            .finish_non_exhaustive()
    }
}

impl Sdl3NativeWindowLease {
    pub fn target(&self) -> dear_imgui_rs::NativeWindowTarget {
        self.snapshot.target
    }
    pub fn snapshot(&self) -> Sdl3WindowSnapshot {
        self.snapshot
    }

    /// Returns true when the target or any captured native owner has retired from routing.
    pub fn is_retired(&self) -> bool {
        self.pin.any_retired()
    }

    /// Returns the retained SDL window for native surface creation.
    ///
    /// Use the pointer only while retaining this lease, on SDL's owning thread. Do not transfer it
    /// to an owning SDL wrapper, destroy it, or change native ownership outside the platform owner.
    pub fn as_ptr(&self) -> *mut SDL_Window {
        self.pin.allocation.pointer
    }
}

impl RuntimeControl {
    pub(crate) fn record_native_window_owner_unavailable(&self) {
        self.record_fault(RuntimeFault::NativeWindowLeaseUnavailable);
    }
    pub(crate) fn native_allocation_for_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<Rc<NativeWindowAllocation>> {
        self.owned_viewports
            .borrow()
            .get(&(viewport as usize))?
            .native_allocation
            .clone()
    }

    fn native_allocation_for_pointer(
        &self,
        pointer: *mut SDL_Window,
    ) -> Option<Rc<NativeWindowAllocation>> {
        let address = self
            .owned_viewports
            .borrow()
            .iter()
            .find_map(|(address, lease)| {
                (lease.window_address == pointer as usize && lease.last_snapshot.is_some())
                    .then_some(*address)
            })?;
        let (_, actual) = self.window_registration_bound(address)?;
        (actual == pointer)
            .then(|| self.native_allocation_for_viewport(address as *mut sys::ImGuiViewport))
            .flatten()
    }

    /// Refresh persistent native ownership without constructing another routing registry.
    /// SDL destroys native children recursively, so a registered child must retain its owner even
    /// when it has no outstanding renderer work of its own.
    pub(crate) fn refresh_native_window_owner(&self, viewport: *mut sys::ImGuiViewport) -> bool {
        let Some(allocation) = self.native_allocation_for_viewport(viewport) else {
            return true;
        };
        let parent = unsafe { SDL_GetWindowParent(allocation.pointer) };
        let owner = if parent.is_null() {
            None
        } else {
            let Some(owner) = self.native_allocation_for_pointer(parent) else {
                return false;
            };
            let mut visited = HashSet::new();
            let mut ancestor = Some(Rc::clone(&owner));
            while let Some(current) = ancestor {
                if Rc::ptr_eq(&current, &allocation) || !visited.insert(current.pointer as usize) {
                    return false;
                }
                ancestor = current.owner.borrow().clone();
            }
            Some(owner)
        };
        *allocation.owner.borrow_mut() = owner;
        true
    }

    pub(crate) fn acquire_native_window_lease(
        &self,
        instance_id: NativeWindowInstanceId,
    ) -> Result<Option<Sdl3NativeWindowLease>, Sdl3BackendError> {
        self.with_window_query(|| {
            let address = self
                .owned_viewports
                .borrow()
                .iter()
                .find_map(|(address, lease)| {
                    (lease.instance_id == instance_id && lease.last_snapshot.is_some())
                        .then_some(*address)
                });
            let Some(address) = address else {
                return Ok(None);
            };
            let Some((snapshot, pointer)) = self.window_registration_bound(address) else {
                return Ok(None);
            };
            if !self.refresh_native_window_owner(address as *mut sys::ImGuiViewport) {
                return Err(Sdl3BackendError::NativeWindowOwnerChainUnavailable);
            }
            let mut allocations = Vec::new();
            let mut seen = HashSet::new();
            let mut current = pointer;
            while !current.is_null() {
                if allocations.len() >= 256 || !seen.insert(current as usize) {
                    return Err(Sdl3BackendError::NativeWindowOwnerChainUnavailable);
                }
                let allocation = self
                    .native_allocation_for_pointer(current)
                    .ok_or(Sdl3BackendError::NativeWindowOwnerChainUnavailable)?;
                if allocation.retired.get() || allocation.pointer.is_null() {
                    return Ok(None);
                }
                if !allocation.is_main {
                    let callback = self
                        .original_platform_callbacks()
                        .and_then(|callbacks| callbacks.destroy_window());
                    if !unsafe {
                        crate::core::ffi::dear_imgui_sdl3_is_native_destroy_callback(callback)
                    } {
                        return Err(Sdl3BackendError::NativeWindowLeaseUnavailable);
                    }
                }
                let parent = unsafe { SDL_GetWindowParent(current) };
                let retained_parent = allocation
                    .owner
                    .borrow()
                    .as_ref()
                    .map(|parent| parent.pointer)
                    .unwrap_or(std::ptr::null_mut());
                if parent != retained_parent {
                    return Err(Sdl3BackendError::NativeWindowOwnerChainUnavailable);
                }
                allocations.push(allocation);
                current = parent;
            }
            let mut owner = None;
            for allocation in allocations.into_iter().rev() {
                let next = allocation
                    .work_pins
                    .get()
                    .checked_add(1)
                    .ok_or(Sdl3BackendError::NativeWindowLeaseUnavailable)?;
                allocation.work_pins.set(next);
                owner = Some(Box::new(NativeWindowPin { allocation, owner }));
            }
            let Some(pin) = owner else {
                return Err(Sdl3BackendError::NativeWindowLeaseUnavailable);
            };
            Ok(Some(Sdl3NativeWindowLease {
                snapshot,
                pin: Rc::from(pin),
            }))
        })?
    }

    pub(crate) unsafe fn defer_native_window_destruction(
        &self,
        viewport: *mut sys::ImGuiViewport,
        allocation: Option<&Rc<NativeWindowAllocation>>,
    ) -> bool {
        let Some(allocation) = allocation else {
            return false;
        };
        if allocation.is_main || Rc::strong_count(allocation) == 1 {
            return false;
        }
        let callback = self
            .original_platform_callbacks()
            .and_then(|callbacks| callbacks.destroy_window());
        if !unsafe { crate::core::ffi::dear_imgui_sdl3_is_native_destroy_callback(callback) } {
            self.record_fault(RuntimeFault::NativeWindowLeaseUnavailable);
            unsafe { self.clear_viewport_platform_state(viewport) };
            return true;
        }
        let mut context = std::ptr::null_mut();
        if unsafe {
            crate::core::ffi::dear_imgui_sdl3_detach_leased_window(
                viewport,
                allocation.pointer,
                &mut context,
            )
        } {
            allocation.retained_context.set(Some(context));
        } else {
            // A native lease was issued only after checking the exact upstream destructor. A
            // foreign sidecar cannot be destroyed to repair contract drift while work is live.
            self.record_fault(RuntimeFault::NativeWindowLeaseUnavailable);
            unsafe { self.clear_viewport_platform_state(viewport) };
        }
        true
    }

    pub(crate) unsafe fn native_window_owner_change_blocked(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> bool {
        let Some(allocation) = self.native_allocation_for_viewport(viewport) else {
            return false;
        };
        if allocation.work_pins.get() == 0 {
            return false;
        }
        let actual = unsafe { SDL_GetWindowParent(allocation.pointer) };
        let parent_viewport = unsafe { (*viewport).ParentViewport };
        let requested = if parent_viewport.is_null() {
            Some(std::ptr::null_mut())
        } else {
            let parent = self.live_viewport(parent_viewport);
            parent
                .and_then(|parent| unsafe { self.capture_viewport_platform_state(parent) })
                .and_then(|state| state.sdl_window_id())
                .map(|id| unsafe { SDL_GetWindowFromID(SDL_WindowID(id)) })
        };
        if requested == Some(actual) {
            return false;
        }
        self.record_fault(RuntimeFault::NativeWindowOwnerLeased);
        true
    }
}

impl Sdl3ViewportRendererAdapter {
    /// Pins the exact native allocation and its validated native owner chain for renderer work.
    pub fn acquire_native_window_lease(
        &self,
        instance_id: NativeWindowInstanceId,
    ) -> Result<Option<Sdl3NativeWindowLease>, Sdl3BackendError> {
        self.control.acquire_native_window_lease(instance_id)
    }
}
