use std::ffi::CStr;

use dear_imgui_rs::{Id, NativeWindowTarget};
use sdl3_sys::video::*;

use super::*;

/// When the SDL3 backend can apply a native window policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sdl3WindowPolicySupport {
    /// The owner can request updates after native creation; SDL may still reject a request.
    Dynamic,
    /// The policy must be selected before allocating the native window.
    CreationOnly,
    /// The backend cannot provide this policy on the active video driver.
    Unsupported,
}

/// Supported policy transitions for the active SDL video driver.
///
/// Support describes available requests, not a guarantee that a window manager accepts them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sdl3WindowCapabilities {
    pub native_viewports: bool,
    pub icon: Sdl3WindowPolicySupport,
    pub client_size_limits: Sdl3WindowPolicySupport,
    pub resizable: Sdl3WindowPolicySupport,
    pub focusable: Sdl3WindowPolicySupport,
    pub transparent: Sdl3WindowPolicySupport,
    pub native_owner: Sdl3WindowPolicySupport,
    /// SDL3 has no portable per-window theme selection API.
    pub theme: Sdl3WindowPolicySupport,
}

/// Native owner information, without retaining the owning window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sdl3NativeOwnerState {
    pub window_id: u32,
    /// Present only when the owner is registered by this exact backend runtime.
    pub instance_id: Option<NativeWindowInstanceId>,
}

/// Native state observed at the time of a window snapshot.
///
/// `None` preserves unavailable information; a capability does not turn an unknown state into
/// `false`. Visibility describes SDL's hidden flag, not final desktop pixel visibility.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sdl3WindowState {
    pub visible: Option<bool>,
    pub focused: Option<bool>,
    pub minimized: Option<bool>,
    pub maximized: Option<bool>,
    pub resizable: Option<bool>,
    pub focusable: Option<bool>,
    pub transparent: Option<bool>,
    pub always_on_top: Option<bool>,
    /// Outer `None` means unavailable; `Some(None)` means the native window has no owner.
    pub native_owner: Option<Option<Sdl3NativeOwnerState>>,
}

/// Validated RGBA8 icon data, limited to 512 pixels per dimension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sdl3WindowIcon {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Sdl3WindowIcon {
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, Sdl3BackendError> {
        if width == 0
            || height == 0
            || width > 512
            || height > 512
            || rgba.len() != width as usize * height as usize * 4
        {
            return Err(Sdl3BackendError::InvalidWindowConfiguration {
                reason: "icon must contain exactly width * height RGBA8 pixels with dimensions in 1..=512",
            });
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }
}

/// Bounded secondary-window configuration, applied while the new window is still hidden.
///
/// Size limits use SDL client coordinates, not framebuffer pixels. Geometry, initial visibility,
/// graphics API, decoration, taskbar policy and native ownership remain backend-controlled.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Sdl3WindowConfig {
    pub min_client_size: Option<[u32; 2]>,
    pub max_client_size: Option<[u32; 2]>,
    pub resizable: Option<bool>,
    pub focusable: Option<bool>,
    /// Requests a transparent backing buffer, not a guarantee of final compositor transparency.
    pub transparent: bool,
    pub icon: Option<Sdl3WindowIcon>,
}

/// Mutable native attributes for an existing exact registration.
///
/// SDL can reject a request after earlier attributes have applied. The returned error identifies
/// the failed attribute; query the window snapshot to observe the resulting native state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Sdl3WindowUpdate {
    pub min_client_size: Option<[u32; 2]>,
    pub max_client_size: Option<[u32; 2]>,
    pub resizable: Option<bool>,
    pub focusable: Option<bool>,
    pub icon: Option<Sdl3WindowIcon>,
}

impl From<Sdl3WindowUpdate> for Sdl3WindowConfig {
    fn from(update: Sdl3WindowUpdate) -> Self {
        Self {
            min_client_size: update.min_client_size,
            max_client_size: update.max_client_size,
            resizable: update.resizable,
            focusable: update.focusable,
            icon: update.icon,
            transparent: false,
        }
    }
}

impl Sdl3WindowConfig {
    pub(super) fn validate(&self) -> Result<(), Sdl3BackendError> {
        for size in [self.min_client_size, self.max_client_size]
            .into_iter()
            .flatten()
        {
            if size
                .into_iter()
                .any(|value| value == 0 || value > i32::MAX as u32)
            {
                return Err(Sdl3BackendError::InvalidWindowConfiguration {
                    reason: "client size limits must be positive and fit SDL's signed coordinates",
                });
            }
        }
        if let (Some(min), Some(max)) = (self.min_client_size, self.max_client_size) {
            if min[0] > max[0] || min[1] > max[1] {
                return Err(Sdl3BackendError::InvalidWindowConfiguration {
                    reason: "minimum client size exceeds maximum client size",
                });
            }
        }
        Ok(())
    }
}

/// Owned inputs supplied before one secondary native window is allocated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sdl3WindowCreationRequest {
    pub target: NativeWindowTarget,
    pub desktop_position: [f32; 2],
    pub client_size: [f32; 2],
    pub parent_viewport_id: Option<Id>,
}

pub(super) type WindowConfigurator = dyn Fn(&Sdl3WindowCreationRequest, &mut Sdl3WindowConfig);

/// Why an exact native registration was retired from the backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sdl3WindowRetirementReason {
    /// The platform destroy callback released this registration.
    WindowCallback,
    BackendShutdown,
    ContextDestroyed,
    OwnershipLost,
}

/// Owned lifecycle observation; no event retains a window pointer or native resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sdl3WindowLifecycleEvent {
    Created(Sdl3WindowSnapshot),
    /// Registration retirement, rather than a claim about when the OS destroyed visible pixels.
    Retired {
        window: Sdl3WindowSnapshot,
        reason: Sdl3WindowRetirementReason,
    },
}

pub(crate) fn current_driver() -> Option<String> {
    let pointer = unsafe { SDL_GetCurrentVideoDriver() };
    (!pointer.is_null()).then(|| {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    })
}

impl RuntimeControl {
    pub(crate) fn record_window_configuration_failed(&self) {
        self.record_fault(RuntimeFault::WindowConfigurationFailed);
    }
    pub(super) fn window_capabilities_bound(&self) -> Sdl3WindowCapabilities {
        use Sdl3WindowPolicySupport::*;
        let driver = current_driver();
        let desktop = matches!(
            driver.as_deref(),
            Some("windows" | "cocoa" | "x11" | "wayland")
        );
        let platform_io = unsafe { sys::igGetIO_Nil() };
        let native_viewports = !platform_io.is_null()
            && unsafe {
                (*platform_io).BackendFlags & sys::ImGuiBackendFlags_PlatformHasViewports != 0
            };
        Sdl3WindowCapabilities {
            native_viewports,
            icon: if matches!(driver.as_deref(), Some("windows" | "x11")) {
                Dynamic
            } else {
                Unsupported
            },
            client_size_limits: if desktop { Dynamic } else { Unsupported },
            resizable: if desktop { Dynamic } else { Unsupported },
            focusable: if desktop { Dynamic } else { Unsupported },
            transparent: if desktop { CreationOnly } else { Unsupported },
            // The upstream SDL adapter deliberately skips parenting on Cocoa because it prevents
            // detached windows from moving to another monitor. Wayland has no native viewport route.
            native_owner: if native_viewports
                && matches!(driver.as_deref(), Some("windows" | "x11"))
            {
                Dynamic
            } else {
                Unsupported
            },
            theme: Unsupported,
        }
    }

    pub(crate) fn window_capabilities(&self) -> Result<Sdl3WindowCapabilities, Sdl3BackendError> {
        self.with_window_query(|| self.window_capabilities_bound())
    }

    pub(crate) fn set_secondary_window_config(
        &self,
        config: Sdl3WindowConfig,
    ) -> Result<(), Sdl3BackendError> {
        config.validate()?;
        self.with_window_query(|| self.secondary_window_config.replace(config))?;
        Ok(())
    }

    pub(crate) fn set_secondary_window_configurator(
        &self,
        callback: Option<Rc<WindowConfigurator>>,
    ) -> Result<(), Sdl3BackendError> {
        self.with_window_query(|| self.secondary_window_configurator.replace(callback))?;
        Ok(())
    }

    pub(crate) fn drain_window_lifecycle_events(&self) -> Vec<Sdl3WindowLifecycleEvent> {
        self.window_lifecycle_events
            .borrow_mut()
            .drain(..)
            .collect()
    }

    pub(crate) fn update_window(
        &self,
        instance_id: NativeWindowInstanceId,
        update: Sdl3WindowUpdate,
    ) -> Result<Option<()>, Sdl3BackendError> {
        let config = Sdl3WindowConfig::from(update);
        config.validate()?;
        self.with_window_query(|| {
            let capabilities = self.window_capabilities_bound();
            if let Some(attribute) = unsupported_attribute(&config, capabilities) {
                return Err(Sdl3BackendError::UnsupportedWindowPolicy { attribute });
            }
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
            let Some((_, pointer)) = self.window_registration_bound(address) else {
                return Ok(None);
            };
            unsafe { apply_mutable_window_config(pointer, &config) }?;
            if let Some(viewport) = self
                .viewport_key(address as *mut sys::ImGuiViewport)
                .map(|_| address as *mut sys::ImGuiViewport)
            {
                // SDL size constraints can resize the native client area. Let the existing ImGui
                // geometry reconciliation observe that change instead of inventing UI coordinates.
                unsafe { (*viewport).PlatformRequestResize = true };
            }
            Ok(Some(()))
        })?
    }

    pub(crate) unsafe fn configure_secondary_window(
        &self,
        viewport: *mut sys::ImGuiViewport,
        instance_id: NativeWindowInstanceId,
    ) -> Option<Sdl3WindowConfig> {
        let mut config = self.secondary_window_config.borrow().clone();
        let callback = self.secondary_window_configurator.borrow().clone();
        if let Some(callback) = callback {
            let viewport = unsafe { &*viewport };
            let parent_viewport_id = if viewport.ParentViewport.is_null() {
                None
            } else {
                Some(Id::from(unsafe { (*viewport.ParentViewport).ID }))
            };
            let request = Sdl3WindowCreationRequest {
                target: NativeWindowTarget {
                    instance_id,
                    viewport_id: Id::from(viewport.ID),
                    is_main: false,
                },
                desktop_position: [viewport.Pos.x, viewport.Pos.y],
                client_size: [viewport.Size.x, viewport.Size.y],
                parent_viewport_id,
            };
            if catch_unwind(AssertUnwindSafe(|| callback(&request, &mut config))).is_err() {
                self.record_fault(RuntimeFault::WindowConfigurationPanicked);
                return None;
            }
        }
        if let Err(Sdl3BackendError::InvalidWindowConfiguration { reason }) = config.validate() {
            self.record_fault(RuntimeFault::InvalidWindowConfiguration(reason));
            return None;
        }
        let capabilities = self.window_capabilities_bound();
        if let Some(attribute) = unsupported_attribute(&config, capabilities) {
            self.record_fault(RuntimeFault::UnsupportedWindowPolicy(attribute));
            return None;
        }
        Some(config)
    }

    pub(super) fn capture_window_state(&self, pointer: *mut SDL_Window) -> Sdl3WindowState {
        let flags = unsafe { SDL_GetWindowFlags(pointer) };
        let has = |flag| flags & flag != 0;
        let native_owner = if self.window_capabilities_bound().native_owner
            == Sdl3WindowPolicySupport::Unsupported
        {
            None
        } else {
            let parent = unsafe { SDL_GetWindowParent(pointer) };
            Some(if parent.is_null() {
                None
            } else {
                let window_id = unsafe { SDL_GetWindowID(parent) }.0;
                let candidate =
                    self.owned_viewports
                        .borrow()
                        .iter()
                        .find_map(|(address, lease)| {
                            (lease.window_address == parent as usize
                                && lease.last_snapshot.is_some())
                            .then_some((*address, lease.clone()))
                        });
                let instance_id = candidate.and_then(|(address, lease)| {
                    let viewport = address as *mut sys::ImGuiViewport;
                    let key = self.viewport_key(viewport)?;
                    (lease.key.same_identity(key)
                        && unsafe { ViewportPlatformState::capture(viewport) } == lease.state
                        && lease.state.sdl_window_id() == Some(window_id))
                    .then_some(lease.instance_id)
                });
                Some(Sdl3NativeOwnerState {
                    window_id,
                    instance_id,
                })
            })
        };
        Sdl3WindowState {
            visible: Some(!has(SDL_WINDOW_HIDDEN)),
            focused: Some(has(SDL_WINDOW_INPUT_FOCUS)),
            minimized: Some(has(SDL_WINDOW_MINIMIZED)),
            maximized: Some(has(SDL_WINDOW_MAXIMIZED)),
            resizable: Some(has(SDL_WINDOW_RESIZABLE)),
            focusable: Some(!has(SDL_WINDOW_NOT_FOCUSABLE)),
            transparent: Some(has(SDL_WINDOW_TRANSPARENT)),
            always_on_top: Some(has(SDL_WINDOW_ALWAYS_ON_TOP)),
            native_owner,
        }
    }

    pub(crate) unsafe fn apply_secondary_window_config(
        &self,
        viewport: *mut sys::ImGuiViewport,
        config: &Sdl3WindowConfig,
    ) -> bool {
        // Synthetic callback tests do not own SDL windows. Defaults require no native mutations.
        let Some(state) = (unsafe { self.capture_viewport_platform_state(viewport) }) else {
            return false;
        };
        let Some(window_id) = state.sdl_window_id() else {
            return false;
        };
        let window = unsafe { SDL_GetWindowFromID(SDL_WindowID(window_id)) };
        if window.is_null() {
            #[cfg(test)]
            return *config == Sdl3WindowConfig::default();
            #[cfg(not(test))]
            return false;
        }
        let flags = unsafe { SDL_GetWindowFlags(window) };
        let mut width = 0;
        let mut height = 0;
        if flags & SDL_WINDOW_HIDDEN == 0
            || !unsafe { SDL_GetWindowSize(window, &mut width, &mut height) }
        {
            return false;
        }
        if config
            .min_client_size
            .is_some_and(|min| width < min[0] as i32 || height < min[1] as i32)
            || config
                .max_client_size
                .is_some_and(|max| width > max[0] as i32 || height > max[1] as i32)
        {
            self.record_fault(RuntimeFault::InvalidWindowConfiguration(
                "creation size must remain inside the requested client size limits",
            ));
            return false;
        }
        let applied = unsafe { apply_mutable_window_config(window, config) };
        let mut after_width = 0;
        let mut after_height = 0;
        applied.is_ok()
            && unsafe { SDL_GetWindowSize(window, &mut after_width, &mut after_height) }
            && (width, height) == (after_width, after_height)
            && unsafe { SDL_GetWindowFlags(window) } & SDL_WINDOW_HIDDEN != 0
            && (!config.transparent
                || unsafe { SDL_GetWindowFlags(window) } & SDL_WINDOW_TRANSPARENT != 0)
    }

    pub(crate) fn publish_owned_window(&self, viewport: *mut sys::ImGuiViewport) {
        // A host window may have a foreign SDL owner. Keep ordinary routing/observations usable;
        // issuing a native work lease separately requires the complete owner chain to be owned.
        self.refresh_native_window_owner(viewport);
        let address = viewport as usize;
        let Some((snapshot, _)) = self.window_registration_bound(address) else {
            return;
        };
        let mut registry = self.owned_viewports.borrow_mut();
        if let Some(lease) = registry.get_mut(&address) {
            if lease.last_snapshot.is_none() {
                lease.last_snapshot = Some(snapshot);
                self.callback_window_targets
                    .publish(snapshot.window_id, snapshot.target);
                self.window_lifecycle_events
                    .borrow_mut()
                    .push(Sdl3WindowLifecycleEvent::Created(snapshot));
            }
        }
    }

    pub(super) fn retire_window_lease(
        &self,
        mut lease: OwnedViewportLease,
        reason: Sdl3WindowRetirementReason,
    ) {
        if let Some(allocation) = &lease.native_allocation {
            allocation.mark_retired();
        }
        if let Some(mut window) = lease.last_snapshot.take() {
            window.target.viewport_id = Id::from(lease.key.id);
            self.callback_window_targets
                .retire(window.window_id, window.target);
            self.window_lifecycle_events
                .borrow_mut()
                .push(Sdl3WindowLifecycleEvent::Retired { window, reason });
        }
    }

    pub(super) fn retire_all_windows(&self, reason: Sdl3WindowRetirementReason) {
        let mut leases: Vec<_> = self
            .owned_viewports
            .borrow_mut()
            .drain()
            .map(|(_, lease)| lease)
            .collect();
        leases.sort_unstable_by_key(|lease| lease.instance_id.get());
        for lease in leases {
            self.retire_window_lease(lease, reason);
        }
    }
}

fn unsupported_attribute(
    config: &Sdl3WindowConfig,
    capabilities: Sdl3WindowCapabilities,
) -> Option<&'static str> {
    [
        (config.transparent, capabilities.transparent, "transparent"),
        (config.icon.is_some(), capabilities.icon, "icon"),
        (
            config.min_client_size.is_some() || config.max_client_size.is_some(),
            capabilities.client_size_limits,
            "client size limits",
        ),
        (
            config.resizable.is_some(),
            capabilities.resizable,
            "resizable",
        ),
        (
            config.focusable.is_some(),
            capabilities.focusable,
            "focusable",
        ),
    ]
    .into_iter()
    .find_map(|(requested, support, attribute)| {
        (requested && support == Sdl3WindowPolicySupport::Unsupported).then_some(attribute)
    })
}

unsafe fn apply_mutable_window_config(
    window: *mut SDL_Window,
    config: &Sdl3WindowConfig,
) -> Result<(), Sdl3BackendError> {
    let check = |success, attribute| {
        if success {
            Ok(())
        } else {
            Err(Sdl3BackendError::WindowPolicyFailed {
                attribute,
                message: sdl3::get_error().to_string(),
            })
        }
    };
    if let Some([width, height]) = config.min_client_size {
        check(
            unsafe { SDL_SetWindowMinimumSize(window, width as i32, height as i32) },
            "minimum client size",
        )?;
    }
    if let Some([width, height]) = config.max_client_size {
        check(
            unsafe { SDL_SetWindowMaximumSize(window, width as i32, height as i32) },
            "maximum client size",
        )?;
    }
    if let Some(resizable) = config.resizable {
        check(
            unsafe { SDL_SetWindowResizable(window, resizable) },
            "resizable",
        )?;
    }
    if let Some(focusable) = config.focusable {
        check(
            unsafe { SDL_SetWindowFocusable(window, focusable) },
            "focusable",
        )?;
    }
    if let Some(icon) = &config.icon {
        // SDL_SetWindowIcon copies the surface; the temporary borrowed pixel storage remains live
        // through the call and the surface is always released before returning.
        let mut pixels = icon.rgba.clone();
        let surface = unsafe {
            sdl3_sys::surface::SDL_CreateSurfaceFrom(
                icon.width as i32,
                icon.height as i32,
                sdl3_sys::pixels::SDL_PIXELFORMAT_RGBA32,
                pixels.as_mut_ptr().cast(),
                icon.width as i32 * 4,
            )
        };
        if surface.is_null() {
            return check(false, "icon surface");
        }
        let result = check(unsafe { SDL_SetWindowIcon(window, surface) }, "icon");
        unsafe { sdl3_sys::surface::SDL_DestroySurface(surface) };
        result?;
    }
    Ok(())
}
