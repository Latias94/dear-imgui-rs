use std::cell::Cell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use dear_imgui_rs::{Context, sys};

use crate::core::Sdl3BackendError;
use crate::core::ffi;
use crate::runtime::{NativeRendererKind, RuntimeControl, with_current_runtime};

macro_rules! for_each_callback {
    ($macro:ident) => {
        $macro!(Platform_GetClipboardTextFn);
        $macro!(Platform_SetClipboardTextFn);
        $macro!(Platform_OpenInShellFn);
        $macro!(Platform_SetImeDataFn);
        $macro!(Platform_CreateWindow);
        $macro!(Platform_DestroyWindow);
        $macro!(Platform_ShowWindow);
        $macro!(Platform_SetWindowPos);
        $macro!(Platform_GetWindowPos);
        $macro!(Platform_SetWindowSize);
        $macro!(Platform_GetWindowSize);
        $macro!(Platform_GetWindowFramebufferScale);
        $macro!(Platform_SetWindowFocus);
        $macro!(Platform_GetWindowFocus);
        $macro!(Platform_GetWindowMinimized);
        $macro!(Platform_SetWindowTitle);
        $macro!(Platform_SetWindowAlpha);
        $macro!(Platform_UpdateWindow);
        $macro!(Platform_RenderWindow);
        $macro!(Platform_SwapBuffers);
        $macro!(Platform_GetWindowDpiScale);
        $macro!(Platform_OnChangedViewport);
        $macro!(Platform_GetWindowWorkAreaInsets);
        $macro!(Platform_CreateVkSurface);
    };
}

macro_rules! for_each_platform_window_callback {
    ($macro:ident) => {
        $macro!(Platform_CreateWindow);
        $macro!(Platform_DestroyWindow);
        $macro!(Platform_ShowWindow);
        $macro!(Platform_SetWindowPos);
        $macro!(Platform_GetWindowPos);
        $macro!(Platform_SetWindowSize);
        $macro!(Platform_GetWindowSize);
        $macro!(Platform_GetWindowFramebufferScale);
        $macro!(Platform_SetWindowFocus);
        $macro!(Platform_GetWindowFocus);
        $macro!(Platform_GetWindowMinimized);
        $macro!(Platform_SetWindowTitle);
        $macro!(Platform_SetWindowAlpha);
        $macro!(Platform_UpdateWindow);
        $macro!(Platform_RenderWindow);
        $macro!(Platform_SwapBuffers);
        $macro!(Platform_GetWindowDpiScale);
        $macro!(Platform_OnChangedViewport);
        $macro!(Platform_GetWindowWorkAreaInsets);
        $macro!(Platform_CreateVkSurface);
    };
}

// Platform services are composable hooks, not part of the viewport window ownership contract.
// Extensions such as Dear ImGui Test Engine temporarily replace the clipboard family together.
macro_rules! for_each_platform_service_callback {
    ($macro:ident) => {
        $macro!(Platform_GetClipboardTextFn);
        $macro!(Platform_SetClipboardTextFn);
        $macro!(Platform_OpenInShellFn);
        $macro!(Platform_SetImeDataFn);
    };
}

macro_rules! for_each_user_data {
    ($macro:ident) => {
        $macro!(Platform_ClipboardUserData);
        $macro!(Platform_OpenInShellUserData);
        $macro!(Platform_ImeUserData);
    };
}

macro_rules! for_each_renderer_non_aggregate_callback {
    ($macro:ident) => {
        $macro!(DrawCallback_ResetRenderState);
        $macro!(DrawCallback_SetSamplerLinear);
        $macro!(DrawCallback_SetSamplerNearest);
        $macro!(Renderer_CreateWindow);
        $macro!(Renderer_DestroyWindow);
        $macro!(Renderer_RenderWindow);
        $macro!(Renderer_SwapBuffers);
    };
}

macro_rules! for_each_renderer_callback {
    ($macro:ident) => {
        for_each_renderer_non_aggregate_callback!($macro);
        $macro!(Renderer_SetWindowSize);
    };
}

macro_rules! for_each_renderer_value {
    ($macro:ident) => {
        $macro!(Renderer_TextureMaxWidth);
        $macro!(Renderer_TextureMaxHeight);
        $macro!(Renderer_RenderState);
    };
}

pub(super) const SDL_PLATFORM_RESERVED_FLAGS: i32 = sys::ImGuiBackendFlags_HasMouseCursors as i32
    | sys::ImGuiBackendFlags_HasSetMousePos as i32
    | sys::ImGuiBackendFlags_HasGamepad as i32
    | sys::ImGuiBackendFlags_PlatformHasViewports as i32
    | sys::ImGuiBackendFlags_HasMouseHoveredViewport as i32
    | sys::ImGuiBackendFlags_HasParentViewport as i32;

pub(super) const SDL_RENDERER_RESERVED_FLAGS: i32 = sys::ImGuiBackendFlags_RendererHasVtxOffset
    as i32
    | sys::ImGuiBackendFlags_RendererHasTextures as i32
    | sys::ImGuiBackendFlags_RendererHasViewports as i32;

const SDL_PLATFORM_STABLE_FLAGS: i32 = sys::ImGuiBackendFlags_HasMouseCursors as i32
    | sys::ImGuiBackendFlags_HasSetMousePos as i32
    | sys::ImGuiBackendFlags_PlatformHasViewports as i32
    | sys::ImGuiBackendFlags_HasParentViewport as i32;

macro_rules! callback_eq {
    ($left:expr, $right:expr) => {
        match ($left, $right) {
            (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    };
}

struct PlatformCallbacks {
    raw: sys::ImGuiPlatformIO,
}

impl PlatformCallbacks {
    unsafe fn capture(raw: *const sys::ImGuiPlatformIO) -> Self {
        Self {
            raw: unsafe { std::ptr::read(raw) },
        }
    }

    fn snapshot(&self) -> Self {
        // The bindgen platform IO value has no Rust destructor. Copying it here
        // snapshots pointer-sized callback state without taking native ownership.
        unsafe { Self::capture(&self.raw) }
    }
}

#[derive(Clone, Copy)]
enum RendererSetWindowSizeCallback {
    Native(Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2_c)>),
    Pointer(unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2)),
}

impl RendererSetWindowSizeCallback {
    unsafe fn capture(platform_io: *mut sys::ImGuiPlatformIO) -> Self {
        if let Some(callback) =
            unsafe { sys::ImGuiPlatformIO_RendererSetWindowSizePointerParam(platform_io) }
        {
            Self::Pointer(callback)
        } else {
            Self::Native(unsafe { (*platform_io).Renderer_SetWindowSize })
        }
    }

    fn is_some(self) -> bool {
        match self {
            Self::Native(callback) => callback.is_some(),
            Self::Pointer(_) => true,
        }
    }

    fn same_callback(self, other: Self) -> bool {
        match (self, other) {
            (Self::Native(left), Self::Native(right)) => callback_eq!(left, right),
            (Self::Pointer(left), Self::Pointer(right)) => std::ptr::fn_addr_eq(left, right),
            _ => false,
        }
    }

    unsafe fn install(self, platform_io: *mut sys::ImGuiPlatformIO) {
        match self {
            Self::Native(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(platform_io, None);
                (*platform_io).Renderer_SetWindowSize = callback;
            },
            Self::Pointer(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(
                    platform_io,
                    Some(callback),
                );
            },
        }
    }
}

pub(super) struct RendererSetWindowSizeInvocation {
    callback: RendererSetWindowSizeCallback,
    native_callbacks: PlatformCallbacks,
}

impl RendererSetWindowSizeInvocation {
    pub(super) fn invoke(
        &self,
        viewport: *mut sys::ImGuiViewport,
        size: *const sys::ImVec2,
    ) -> bool {
        match self.callback {
            RendererSetWindowSizeCallback::Native(_) => unsafe {
                sys::ImGuiPlatformIO_InvokeRendererSetWindowSize(
                    &self.native_callbacks.raw,
                    viewport,
                    size,
                )
            },
            RendererSetWindowSizeCallback::Pointer(callback) => {
                unsafe { callback(viewport, size) };
                true
            }
        }
    }
}

#[derive(Clone, Copy)]
struct BackendState {
    user_data: *mut c_void,
    name: *const std::ffi::c_char,
    flags: i32,
}

#[derive(Clone, Copy)]
struct RendererBackendState {
    user_data: *mut c_void,
    name: *const std::ffi::c_char,
    flags: i32,
}

impl RendererBackendState {
    unsafe fn capture(io: *const sys::ImGuiIO) -> Self {
        let io = unsafe { &*io };
        Self {
            user_data: io.BackendRendererUserData,
            name: io.BackendRendererName,
            flags: io.BackendFlags,
        }
    }

    unsafe fn restore(self, io: *mut sys::ImGuiIO) {
        let io = unsafe { &mut *io };
        io.BackendRendererUserData = self.user_data;
        io.BackendRendererName = self.name;
        io.BackendFlags = self.flags;
    }
}

impl BackendState {
    unsafe fn capture(io: *const sys::ImGuiIO) -> Self {
        let io = unsafe { &*io };
        Self {
            user_data: io.BackendPlatformUserData,
            name: io.BackendPlatformName,
            flags: io.BackendFlags,
        }
    }

    unsafe fn restore(self, io: *mut sys::ImGuiIO) {
        let io = unsafe { &mut *io };
        io.BackendPlatformUserData = self.user_data;
        io.BackendPlatformName = self.name;
        io.BackendFlags = self.flags;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MonitorState {
    size: i32,
    capacity: i32,
    data: *mut sys::ImGuiPlatformMonitor,
}

impl MonitorState {
    unsafe fn capture(platform_io: *const sys::ImGuiPlatformIO) -> Self {
        let monitors = unsafe { &(*platform_io).Monitors };
        Self {
            size: monitors.Size,
            capacity: monitors.Capacity,
            data: monitors.Data,
        }
    }

    fn is_empty(self) -> bool {
        self.size == 0 && self.capacity == 0 && self.data.is_null()
    }
}

#[derive(Clone, Copy)]
pub(super) struct ViewportPlatformState {
    user_data: *mut c_void,
    handle: *mut c_void,
    handle_raw: *mut c_void,
}

impl ViewportPlatformState {
    pub(super) unsafe fn capture(viewport: *const sys::ImGuiViewport) -> Self {
        let viewport = unsafe { &*viewport };
        Self {
            user_data: viewport.PlatformUserData,
            handle: viewport.PlatformHandle,
            handle_raw: viewport.PlatformHandleRaw,
        }
    }

    pub(super) unsafe fn restore(self, viewport: *mut sys::ImGuiViewport) {
        let viewport = unsafe { &mut *viewport };
        viewport.PlatformUserData = self.user_data;
        viewport.PlatformHandle = self.handle;
        viewport.PlatformHandleRaw = self.handle_raw;
    }

    unsafe fn clear(viewport: *mut sys::ImGuiViewport) {
        unsafe {
            Self {
                user_data: std::ptr::null_mut(),
                handle: std::ptr::null_mut(),
                handle_raw: std::ptr::null_mut(),
            }
            .restore(viewport);
        }
    }
}

fn restored_owned_pointer<T: Copy + Eq>(baseline: T, installed: T, current: T) -> T {
    if baseline == installed {
        current
    } else if installed == current {
        baseline
    } else {
        current
    }
}

pub(super) struct PlatformClaimBaseline {
    callbacks: PlatformCallbacks,
    renderer_set_window_size: RendererSetWindowSizeCallback,
    backend: BackendState,
    renderer_backend: RendererBackendState,
    main_viewport: ViewportPlatformState,
}

impl PlatformClaimBaseline {
    pub(super) fn snapshot(&self) -> Self {
        Self {
            callbacks: self.callbacks.snapshot(),
            renderer_set_window_size: self.renderer_set_window_size,
            backend: self.backend,
            renderer_backend: self.renderer_backend,
            main_viewport: self.main_viewport,
        }
    }
}

pub(super) struct PlatformCallbackOwnership {
    baseline: PlatformClaimBaseline,
    original: PlatformCallbacks,
    installed: PlatformCallbacks,
    installed_backend: BackendState,
    installed_main_viewport: ViewportPlatformState,
    owned_monitors: Cell<MonitorState>,
}

pub(super) struct PlatformShutdownRestore {
    callbacks: PlatformCallbacks,
    backend: BackendState,
    main_viewport: ViewportPlatformState,
    monitors_owned: bool,
    foreign_capabilities: bool,
}

impl PlatformShutdownRestore {
    pub(super) const fn main_viewport(&self) -> ViewportPlatformState {
        self.main_viewport
    }
}

pub(super) struct RendererCallbackOwnership {
    baseline: PlatformCallbacks,
    baseline_set_window_size: RendererSetWindowSizeCallback,
    original: PlatformCallbacks,
    original_set_window_size: RendererSetWindowSizeCallback,
    installed: PlatformCallbacks,
    installed_set_window_size: RendererSetWindowSizeCallback,
    baseline_backend: RendererBackendState,
    installed_backend: RendererBackendState,
}

pub(super) struct RendererShutdownRestore {
    callbacks: PlatformCallbacks,
    set_window_size: RendererSetWindowSizeCallback,
    backend: RendererBackendState,
    foreign_capabilities: bool,
}

pub(super) fn preflight_platform_claim(
    context: &Context,
    native_renderer: NativeRendererKind,
) -> Result<PlatformClaimBaseline, Sdl3BackendError> {
    context.binding().try_with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        if io.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        if !(*io).BackendPlatformUserData.is_null() {
            return Err(Sdl3BackendError::PlatformBackendOccupied);
        }
        if !(*io).BackendPlatformName.is_null() {
            return Err(Sdl3BackendError::PlatformStateOccupied {
                field: "BackendPlatformName",
            });
        }
        let platform_reserved_flags = (*io).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS;
        if platform_reserved_flags != 0 {
            return Err(Sdl3BackendError::PlatformCapabilityOccupied {
                flags: platform_reserved_flags,
            });
        }

        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        if platform_io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        let raw = &*platform_io;
        macro_rules! reject_platform_callback {
            ($field:ident) => {
                if raw.$field.is_some() {
                    return Err(Sdl3BackendError::PlatformCallbackOccupied {
                        callback: stringify!($field),
                    });
                }
            };
        }
        for_each_platform_window_callback!(reject_platform_callback);
        if !(*main_viewport).PlatformUserData.is_null() {
            return Err(Sdl3BackendError::ForeignPlatformUserData);
        }
        for (field, occupied) in [
            (
                "MainViewport.PlatformHandle",
                !(*main_viewport).PlatformHandle.is_null(),
            ),
            (
                "MainViewport.PlatformHandleRaw",
                !(*main_viewport).PlatformHandleRaw.is_null(),
            ),
        ] {
            if occupied {
                return Err(Sdl3BackendError::PlatformStateOccupied { field });
            }
        }
        if !MonitorState::capture(platform_io).is_empty() {
            return Err(Sdl3BackendError::PlatformStateOccupied {
                field: "PlatformIO.Monitors",
            });
        }

        if native_renderer != NativeRendererKind::None {
            for (field, occupied) in [
                (
                    "BackendRendererUserData",
                    !(*io).BackendRendererUserData.is_null(),
                ),
                ("BackendRendererName", !(*io).BackendRendererName.is_null()),
                (
                    "Renderer_TextureMaxWidth",
                    raw.Renderer_TextureMaxWidth != 0,
                ),
                (
                    "Renderer_TextureMaxHeight",
                    raw.Renderer_TextureMaxHeight != 0,
                ),
                ("Renderer_RenderState", !raw.Renderer_RenderState.is_null()),
            ] {
                if occupied {
                    return Err(Sdl3BackendError::RendererStateOccupied { field });
                }
            }
            macro_rules! reject_renderer_callback {
                ($field:ident) => {
                    if raw.$field.is_some() {
                        return Err(Sdl3BackendError::RendererCallbackOccupied {
                            callback: stringify!($field),
                        });
                    }
                };
            }
            for_each_renderer_callback!(reject_renderer_callback);
            let renderer_reserved_flags = (*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS;
            if renderer_reserved_flags != 0 {
                return Err(Sdl3BackendError::RendererCapabilityOccupied {
                    flags: renderer_reserved_flags,
                });
            }
        }

        Ok(PlatformClaimBaseline {
            callbacks: PlatformCallbacks::capture(platform_io),
            renderer_set_window_size: RendererSetWindowSizeCallback::capture(platform_io),
            backend: BackendState::capture(io),
            renderer_backend: RendererBackendState::capture(io),
            main_viewport: ViewportPlatformState::capture(main_viewport),
        })
    })?
}

impl RendererCallbackOwnership {
    pub(super) unsafe fn claim(
        control: &RuntimeControl,
        baseline: &PlatformClaimBaseline,
    ) -> Result<Option<Self>, Sdl3BackendError> {
        if control.native_renderer() == NativeRendererKind::None {
            return Ok(None);
        }
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() || io.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        let original = unsafe { PlatformCallbacks::capture(platform_io) };
        let original_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let installed_set_window_size = if original_set_window_size.is_some() {
            RendererSetWindowSizeCallback::Pointer(sdl3_renderer_set_window_size)
        } else {
            RendererSetWindowSizeCallback::Native(None)
        };
        unsafe {
            if original.raw.Renderer_CreateWindow.is_some() {
                (*platform_io).Renderer_CreateWindow = Some(sdl3_renderer_create_window);
            }
            if original.raw.Renderer_DestroyWindow.is_some() {
                (*platform_io).Renderer_DestroyWindow = Some(sdl3_renderer_destroy_window);
            }
            if original_set_window_size.is_some() {
                installed_set_window_size.install(platform_io);
            }
            if original.raw.Renderer_RenderWindow.is_some() {
                (*platform_io).Renderer_RenderWindow = Some(sdl3_renderer_render_window);
            }
            if original.raw.Renderer_SwapBuffers.is_some() {
                (*platform_io).Renderer_SwapBuffers = Some(sdl3_renderer_swap_buffers);
            }
        }

        Ok(Some(Self {
            baseline: baseline.callbacks.snapshot(),
            baseline_set_window_size: baseline.renderer_set_window_size,
            original,
            original_set_window_size,
            installed: unsafe { PlatformCallbacks::capture(platform_io) },
            installed_set_window_size,
            baseline_backend: baseline.renderer_backend,
            installed_backend: unsafe { RendererBackendState::capture(io) },
        }))
    }

    pub(super) unsafe fn detect_replacements(&self, control: &RuntimeControl) -> bool {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() || io.is_null() {
            control.record_renderer_state_replaced("renderer callback table");
            return false;
        }

        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let current_backend = unsafe { RendererBackendState::capture(io) };
        let complete_foreign_takeover =
            self.is_complete_foreign_takeover(&current, current_set_window_size, current_backend);
        let capabilities_were_revoked =
            control.capabilities_were_revoked(SDL_RENDERER_RESERVED_FLAGS);
        let mut owned = true;

        macro_rules! detect_replacement {
            ($field:ident) => {
                if !callback_eq!(self.installed.raw.$field, current.raw.$field) {
                    control.record_renderer_callback_replaced(stringify!($field));
                    owned = false;
                }
            };
        }
        for_each_renderer_non_aggregate_callback!(detect_replacement);
        if !self
            .installed_set_window_size
            .same_callback(current_set_window_size)
        {
            control.record_renderer_callback_replaced("Renderer_SetWindowSize");
            owned = false;
        }
        macro_rules! detect_value_replacement {
            ($field:ident) => {
                if self.installed.raw.$field != current.raw.$field {
                    control.record_renderer_state_replaced(stringify!($field));
                    owned = false;
                }
            };
        }
        for_each_renderer_value!(detect_value_replacement);

        if self.baseline_backend.user_data != self.installed_backend.user_data
            && self.installed_backend.user_data != current_backend.user_data
        {
            control.record_renderer_state_replaced("BackendRendererUserData");
            owned = false;
        }
        if self.baseline_backend.name != self.installed_backend.name
            && self.installed_backend.name != current_backend.name
        {
            control.record_renderer_state_replaced("BackendRendererName");
            owned = false;
        }
        if !capabilities_were_revoked
            && current_backend.flags & SDL_RENDERER_RESERVED_FLAGS
                != self.installed_backend.flags & SDL_RENDERER_RESERVED_FLAGS
        {
            control.record_renderer_state_replaced("BackendFlags(renderer-owned bits)");
            owned = false;
        }

        if !owned && complete_foreign_takeover {
            control.preserve_complete_foreign_renderer_capabilities(current_backend.flags);
        }

        owned
    }

    fn is_complete_foreign_takeover(
        &self,
        current: &PlatformCallbacks,
        current_set_window_size: RendererSetWindowSizeCallback,
        current_backend: RendererBackendState,
    ) -> bool {
        let foreign_core_identity = !current_backend.user_data.is_null()
            && !current_backend.name.is_null()
            && current_backend.user_data != self.installed_backend.user_data
            && current_backend.name != self.installed_backend.name;
        if !foreign_core_identity {
            return false;
        }

        let mut all_owned_callbacks_replaced = true;
        macro_rules! require_owned_callback_replacement {
            ($field:ident) => {
                if self.original.raw.$field.is_some()
                    && callback_eq!(self.installed.raw.$field, current.raw.$field)
                {
                    all_owned_callbacks_replaced = false;
                }
            };
        }
        for_each_renderer_non_aggregate_callback!(require_owned_callback_replacement);
        if self.original_set_window_size.is_some()
            && self
                .installed_set_window_size
                .same_callback(current_set_window_size)
        {
            all_owned_callbacks_replaced = false;
        }
        all_owned_callbacks_replaced
    }

    pub(super) unsafe fn prepare_platform_shutdown(
        &self,
        control: &RuntimeControl,
    ) -> Result<RendererShutdownRestore, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() || io.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let current_backend = unsafe { RendererBackendState::capture(io) };
        let _ = unsafe { self.detect_replacements(control) };

        unsafe {
            macro_rules! restore_owned_callback {
                ($field:ident) => {
                    (*platform_io).$field = self.installed.raw.$field;
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_owned_callback);
            self.installed_set_window_size.install(platform_io);
            macro_rules! restore_owned_value {
                ($field:ident) => {
                    (*platform_io).$field = self.installed.raw.$field;
                };
            }
            for_each_renderer_value!(restore_owned_value);
            self.restore_owned_backend_fields(io, current_backend.flags);
        }

        Ok(RendererShutdownRestore {
            callbacks: current,
            set_window_size: current_set_window_size,
            backend: current_backend,
            foreign_capabilities: control.capabilities_are_foreign(SDL_RENDERER_RESERVED_FLAGS),
        })
    }

    pub(super) unsafe fn prepare_native_shutdown(
        &self,
        control: &RuntimeControl,
    ) -> Result<RendererShutdownRestore, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() || io.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_set_window_size =
            unsafe { RendererSetWindowSizeCallback::capture(platform_io) };
        let current_backend = unsafe { RendererBackendState::capture(io) };
        let _ = unsafe { self.detect_replacements(control) };

        unsafe {
            macro_rules! restore_original_callback {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_original_callback);
            self.original_set_window_size.install(platform_io);
            macro_rules! restore_original_value {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_value!(restore_original_value);
            self.restore_owned_backend_fields(io, current_backend.flags);
        }

        Ok(RendererShutdownRestore {
            callbacks: current,
            set_window_size: current_set_window_size,
            backend: current_backend,
            foreign_capabilities: control.capabilities_are_foreign(SDL_RENDERER_RESERVED_FLAGS),
        })
    }

    pub(super) unsafe fn switch_from_platform_to_native_shutdown(
        &self,
    ) -> Result<(), Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() || io.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        unsafe {
            macro_rules! restore_original_callback {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_original_callback);
            self.original_set_window_size.install(platform_io);
            macro_rules! restore_original_value {
                ($field:ident) => {
                    (*platform_io).$field = self.original.raw.$field;
                };
            }
            for_each_renderer_value!(restore_original_value);
            self.restore_owned_backend_fields(io, (*io).BackendFlags);
        }
        Ok(())
    }

    unsafe fn restore_owned_backend_fields(&self, io: *mut sys::ImGuiIO, current_flags: i32) {
        let io = unsafe { &mut *io };
        if self.baseline_backend.user_data != self.installed_backend.user_data {
            io.BackendRendererUserData = self.installed_backend.user_data;
        }
        if self.baseline_backend.name != self.installed_backend.name {
            io.BackendRendererName = self.installed_backend.name;
        }
        io.BackendFlags = (current_flags & !SDL_RENDERER_RESERVED_FLAGS)
            | (self.installed_backend.flags & SDL_RENDERER_RESERVED_FLAGS);
    }

    pub(super) unsafe fn restore_after_shutdown(
        &self,
        restore: RendererShutdownRestore,
    ) -> Result<(), Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        if platform_io.is_null() || io.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        unsafe {
            macro_rules! restore_callback {
                ($field:ident) => {
                    if callback_eq!(self.baseline.raw.$field, self.installed.raw.$field) {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    } else if callback_eq!(self.installed.raw.$field, restore.callbacks.raw.$field)
                    {
                        (*platform_io).$field = self.baseline.raw.$field;
                    } else {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    }
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_callback);
            let restored_set_window_size = if self
                .baseline_set_window_size
                .same_callback(self.installed_set_window_size)
            {
                restore.set_window_size
            } else if self
                .installed_set_window_size
                .same_callback(restore.set_window_size)
            {
                self.baseline_set_window_size
            } else {
                restore.set_window_size
            };
            restored_set_window_size.install(platform_io);
            macro_rules! restore_value {
                ($field:ident) => {
                    (*platform_io).$field = restored_owned_pointer(
                        self.baseline.raw.$field,
                        self.installed.raw.$field,
                        restore.callbacks.raw.$field,
                    );
                };
            }
            for_each_renderer_value!(restore_value);

            let current_flags = (*io).BackendFlags;
            let backend = RendererBackendState {
                user_data: restored_owned_pointer(
                    self.baseline_backend.user_data,
                    self.installed_backend.user_data,
                    restore.backend.user_data,
                ),
                name: restored_owned_pointer(
                    self.baseline_backend.name,
                    self.installed_backend.name,
                    restore.backend.name,
                ),
                flags: (current_flags & !SDL_RENDERER_RESERVED_FLAGS)
                    | if restore.foreign_capabilities {
                        restore.backend.flags & SDL_RENDERER_RESERVED_FLAGS
                    } else {
                        self.baseline_backend.flags & SDL_RENDERER_RESERVED_FLAGS
                    },
            };
            backend.restore(io);
        }
        Ok(())
    }

    pub(super) fn original_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Renderer_CreateWindow
    }

    pub(super) fn original_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Renderer_DestroyWindow
    }

    pub(super) fn original_render_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.original.raw.Renderer_RenderWindow
    }

    pub(super) fn original_set_window_size_invocation(&self) -> RendererSetWindowSizeInvocation {
        RendererSetWindowSizeInvocation {
            callback: self.original_set_window_size,
            native_callbacks: self.original.snapshot(),
        }
    }

    pub(super) fn original_swap_buffers(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.original.raw.Renderer_SwapBuffers
    }
}

impl PlatformCallbackOwnership {
    pub(super) unsafe fn claim(
        control: &Rc<RuntimeControl>,
        baseline: PlatformClaimBaseline,
    ) -> Result<Self, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        let original = unsafe { PlatformCallbacks::capture(platform_io) };
        register_runtime(control);
        unsafe {
            if original.raw.Platform_CreateWindow.is_some() {
                (*platform_io).Platform_CreateWindow = Some(sdl3_create_window);
            }
            if original.raw.Platform_DestroyWindow.is_some() {
                (*platform_io).Platform_DestroyWindow = Some(sdl3_destroy_window);
            }
            if original.raw.Platform_RenderWindow.is_some() {
                (*platform_io).Platform_RenderWindow = Some(sdl3_render_window);
            }
            if original.raw.Platform_SwapBuffers.is_some() {
                (*platform_io).Platform_SwapBuffers = Some(sdl3_swap_buffers);
            }
        }
        let installed = unsafe { PlatformCallbacks::capture(platform_io) };
        let installed_backend = unsafe { BackendState::capture(io) };
        let installed_main_viewport = unsafe { ViewportPlatformState::capture(main_viewport) };
        let owned_monitors = Cell::new(unsafe { MonitorState::capture(platform_io) });
        if !installed_main_viewport.user_data.is_null() {
            control.remember_owned_viewport(main_viewport, installed_main_viewport);
        }

        Ok(Self {
            baseline,
            original,
            installed,
            installed_backend,
            installed_main_viewport,
            owned_monitors,
        })
    }

    pub(super) unsafe fn prepare_shutdown(
        &self,
        control: &RuntimeControl,
    ) -> Result<PlatformShutdownRestore, Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        let current_backend = unsafe { BackendState::capture(io) };
        let current_main_viewport = unsafe { ViewportPlatformState::capture(main_viewport) };
        let current_monitors = unsafe { MonitorState::capture(platform_io) };
        let complete_foreign_takeover = self.is_complete_foreign_takeover(
            &current.raw,
            current_backend,
            current_main_viewport,
            current_monitors,
        );
        let capabilities_were_revoked =
            control.capabilities_were_revoked(SDL_PLATFORM_RESERVED_FLAGS);
        let monitors_owned = current_monitors == self.owned_monitors.get();

        macro_rules! detect_window_replacement {
            ($field:ident) => {
                if !callback_eq!(self.installed.raw.$field, current.raw.$field) {
                    control.record_callback_replaced(stringify!($field));
                }
            };
        }
        for_each_platform_window_callback!(detect_window_replacement);

        if self.baseline.backend.user_data != self.installed_backend.user_data
            && self.installed_backend.user_data != current_backend.user_data
        {
            control.record_platform_state_replaced("BackendPlatformUserData");
        }
        if self.baseline.backend.name != self.installed_backend.name
            && self.installed_backend.name != current_backend.name
        {
            control.record_platform_state_replaced("BackendPlatformName");
        }
        if !capabilities_were_revoked
            && current_backend.flags & SDL_PLATFORM_STABLE_FLAGS
                != self.installed_backend.flags & SDL_PLATFORM_STABLE_FLAGS
        {
            control.record_platform_state_replaced("BackendFlags(platform-owned bits)");
        }
        if !monitors_owned {
            control.record_platform_state_replaced("PlatformIO.Monitors");
        }
        if self.baseline.main_viewport.user_data != self.installed_main_viewport.user_data
            && self.installed_main_viewport.user_data != current_main_viewport.user_data
        {
            control.record_foreign_platform_user_data();
        }
        if self.baseline.main_viewport.handle != self.installed_main_viewport.handle
            && self.installed_main_viewport.handle != current_main_viewport.handle
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandle");
        }
        if self.baseline.main_viewport.handle_raw != self.installed_main_viewport.handle_raw
            && self.installed_main_viewport.handle_raw != current_main_viewport.handle_raw
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandleRaw");
        }
        if complete_foreign_takeover {
            control.preserve_complete_foreign_platform_capabilities(current_backend.flags);
        }

        unsafe {
            macro_rules! restore_owned_service_callback {
                ($field:ident) => {
                    if !callback_eq!(
                        self.baseline.callbacks.raw.$field,
                        self.installed.raw.$field
                    ) {
                        (*platform_io).$field = self.installed.raw.$field;
                    }
                };
            }
            for_each_platform_service_callback!(restore_owned_service_callback);
            macro_rules! restore_owned_window_callback {
                ($field:ident) => {
                    (*platform_io).$field = self.installed.raw.$field;
                };
            }
            for_each_platform_window_callback!(restore_owned_window_callback);

            macro_rules! restore_owned_user_data {
                ($field:ident) => {
                    if self.baseline.callbacks.raw.$field != self.installed.raw.$field {
                        (*platform_io).$field = self.installed.raw.$field;
                    }
                };
            }
            for_each_user_data!(restore_owned_user_data);

            if self.baseline.backend.user_data != self.installed_backend.user_data {
                (*io).BackendPlatformUserData = self.installed_backend.user_data;
            }
            if self.baseline.backend.name != self.installed_backend.name {
                (*io).BackendPlatformName = self.installed_backend.name;
            }
            (*io).BackendFlags = (current_backend.flags & !SDL_PLATFORM_STABLE_FLAGS)
                | (self.installed_backend.flags & SDL_PLATFORM_STABLE_FLAGS);
            if self.baseline.main_viewport.user_data != self.installed_main_viewport.user_data {
                (*main_viewport).PlatformUserData = self.installed_main_viewport.user_data;
            }
            if self.baseline.main_viewport.handle != self.installed_main_viewport.handle {
                (*main_viewport).PlatformHandle = self.installed_main_viewport.handle;
            }
            if self.baseline.main_viewport.handle_raw != self.installed_main_viewport.handle_raw {
                (*main_viewport).PlatformHandleRaw = self.installed_main_viewport.handle_raw;
            }
        }

        Ok(PlatformShutdownRestore {
            callbacks: current,
            backend: current_backend,
            main_viewport: current_main_viewport,
            monitors_owned,
            foreign_capabilities: control.capabilities_are_foreign(SDL_PLATFORM_RESERVED_FLAGS),
        })
    }

    pub(super) unsafe fn restore_after_shutdown(
        &self,
        restore: PlatformShutdownRestore,
    ) -> Result<(), Sdl3BackendError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            return Err(Sdl3BackendError::PlatformStateUnavailable);
        }

        unsafe {
            macro_rules! restore_callback {
                ($field:ident) => {
                    if callback_eq!(
                        self.baseline.callbacks.raw.$field,
                        self.installed.raw.$field
                    ) {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    } else if callback_eq!(self.installed.raw.$field, restore.callbacks.raw.$field)
                    {
                        (*platform_io).$field = self.baseline.callbacks.raw.$field;
                    } else {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    }
                };
            }
            for_each_callback!(restore_callback);

            macro_rules! restore_user_data {
                ($field:ident) => {
                    if self.baseline.callbacks.raw.$field == self.installed.raw.$field {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    } else if self.installed.raw.$field == restore.callbacks.raw.$field {
                        (*platform_io).$field = self.baseline.callbacks.raw.$field;
                    } else {
                        (*platform_io).$field = restore.callbacks.raw.$field;
                    }
                };
            }
            for_each_user_data!(restore_user_data);

            let current_flags = (*io).BackendFlags;
            let backend = BackendState {
                user_data: restored_owned_pointer(
                    self.baseline.backend.user_data,
                    self.installed_backend.user_data,
                    restore.backend.user_data,
                ),
                name: restored_owned_pointer(
                    self.baseline.backend.name,
                    self.installed_backend.name,
                    restore.backend.name,
                ),
                flags: (current_flags & !SDL_PLATFORM_RESERVED_FLAGS)
                    | if restore.foreign_capabilities {
                        restore.backend.flags & SDL_PLATFORM_RESERVED_FLAGS
                    } else {
                        self.baseline.backend.flags & SDL_PLATFORM_RESERVED_FLAGS
                    },
            };
            backend.restore(io);

            if restore.monitors_owned {
                ffi::dear_imgui_sdl3_backend_clear_platform_monitors();
            } else {
                (*platform_io).Monitors = restore.callbacks.raw.Monitors;
            }
        }

        let main_viewport_restore = ViewportPlatformState {
            user_data: restored_owned_pointer(
                self.baseline.main_viewport.user_data,
                self.installed_main_viewport.user_data,
                restore.main_viewport.user_data,
            ),
            handle: restored_owned_pointer(
                self.baseline.main_viewport.handle,
                self.installed_main_viewport.handle,
                restore.main_viewport.handle,
            ),
            handle_raw: restored_owned_pointer(
                self.baseline.main_viewport.handle_raw,
                self.installed_main_viewport.handle_raw,
                restore.main_viewport.handle_raw,
            ),
        };
        unsafe { main_viewport_restore.restore(main_viewport) };
        Ok(())
    }

    pub(super) fn original_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Platform_CreateWindow
    }

    pub(super) fn original_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.original.raw.Platform_DestroyWindow
    }

    pub(super) fn original_render_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.original.raw.Platform_RenderWindow
    }

    pub(super) fn original_swap_buffers(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.original.raw.Platform_SwapBuffers
    }

    pub(super) unsafe fn detect_replacements(&self, control: &RuntimeControl) -> bool {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let io = unsafe { sys::igGetIO_Nil() };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if platform_io.is_null() || io.is_null() || main_viewport.is_null() {
            control.record_platform_state_replaced("platform callback table");
            return false;
        }
        let current = unsafe { &*platform_io };
        // Snapshot every compared value before recording a fault. Recording revokes this
        // runtime's capability bits, which must not be mistaken for a second external drift in
        // the same validation pass.
        let current_backend = unsafe { BackendState::capture(io) };
        let current_main_viewport = unsafe { ViewportPlatformState::capture(main_viewport) };
        let current_monitors = unsafe { MonitorState::capture(platform_io) };
        let complete_foreign_takeover = self.is_complete_foreign_takeover(
            current,
            current_backend,
            current_main_viewport,
            current_monitors,
        );
        let capabilities_were_revoked =
            control.capabilities_were_revoked(SDL_PLATFORM_RESERVED_FLAGS);
        let mut owned = true;
        macro_rules! detect_window_replacement {
            ($field:ident) => {
                if !callback_eq!(self.installed.raw.$field, current.$field) {
                    control.record_callback_replaced(stringify!($field));
                    owned = false;
                }
            };
        }
        for_each_platform_window_callback!(detect_window_replacement);

        if self.baseline.backend.user_data != self.installed_backend.user_data
            && self.installed_backend.user_data != current_backend.user_data
        {
            control.record_platform_state_replaced("BackendPlatformUserData");
            owned = false;
        }
        if self.baseline.backend.name != self.installed_backend.name
            && self.installed_backend.name != current_backend.name
        {
            control.record_platform_state_replaced("BackendPlatformName");
            owned = false;
        }
        if !capabilities_were_revoked
            && current_backend.flags & SDL_PLATFORM_STABLE_FLAGS
                != self.installed_backend.flags & SDL_PLATFORM_STABLE_FLAGS
        {
            control.record_platform_state_replaced("BackendFlags(platform-owned bits)");
            owned = false;
        }
        if self.baseline.main_viewport.user_data != self.installed_main_viewport.user_data
            && self.installed_main_viewport.user_data != current_main_viewport.user_data
        {
            if current_main_viewport.user_data.is_null() {
                control.record_platform_state_replaced("MainViewport.PlatformUserData");
            } else {
                control.record_foreign_platform_user_data();
            }
            owned = false;
        }
        if self.baseline.main_viewport.handle != self.installed_main_viewport.handle
            && self.installed_main_viewport.handle != current_main_viewport.handle
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandle");
            owned = false;
        }
        if self.baseline.main_viewport.handle_raw != self.installed_main_viewport.handle_raw
            && self.installed_main_viewport.handle_raw != current_main_viewport.handle_raw
        {
            control.record_platform_state_replaced("MainViewport.PlatformHandleRaw");
            owned = false;
        }
        if current_monitors != self.owned_monitors.get() {
            control.record_platform_state_replaced("PlatformIO.Monitors");
            owned = false;
        }
        if !owned && complete_foreign_takeover {
            control.preserve_complete_foreign_platform_capabilities(current_backend.flags);
        }
        owned
    }

    fn is_complete_foreign_takeover(
        &self,
        current: &sys::ImGuiPlatformIO,
        current_backend: BackendState,
        current_main_viewport: ViewportPlatformState,
        current_monitors: MonitorState,
    ) -> bool {
        let foreign_core_identity = !current_backend.user_data.is_null()
            && !current_backend.name.is_null()
            && current_backend.user_data != self.installed_backend.user_data
            && current_backend.name != self.installed_backend.name;
        let foreign_main_viewport_identity = !current_main_viewport.user_data.is_null()
            && !current_main_viewport.handle.is_null()
            && !current_main_viewport.handle_raw.is_null()
            && current_main_viewport.user_data != self.installed_main_viewport.user_data
            && current_main_viewport.handle != self.installed_main_viewport.handle
            && current_main_viewport.handle_raw != self.installed_main_viewport.handle_raw;
        if !foreign_core_identity || !foreign_main_viewport_identity {
            return false;
        }

        let mut all_owned_callbacks_replaced = true;
        macro_rules! require_owned_callback_replacement {
            ($field:ident) => {
                if !callback_eq!(
                    self.baseline.callbacks.raw.$field,
                    self.installed.raw.$field
                ) && (current.$field.is_none()
                    || callback_eq!(self.installed.raw.$field, current.$field))
                {
                    all_owned_callbacks_replaced = false;
                }
            };
        }
        for_each_platform_window_callback!(require_owned_callback_replacement);

        let owned_monitors = self.owned_monitors.get();
        let monitors_transferred = owned_monitors.is_empty()
            || (!current_monitors.is_empty() && current_monitors != owned_monitors);
        all_owned_callbacks_replaced && monitors_transferred
    }

    pub(super) unsafe fn refresh_owned_monitors(&self) {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if !platform_io.is_null() {
            self.owned_monitors
                .set(unsafe { MonitorState::capture(platform_io) });
        }
    }
}

pub(super) unsafe fn restore_baseline_after_failed_initialization(baseline: PlatformClaimBaseline) {
    let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
    let io = unsafe { sys::igGetIO_Nil() };
    let main_viewport = unsafe { sys::igGetMainViewport() };
    if !platform_io.is_null() {
        unsafe {
            macro_rules! restore_callback {
                ($field:ident) => {
                    (*platform_io).$field = baseline.callbacks.raw.$field;
                };
            }
            for_each_callback!(restore_callback);
            for_each_renderer_non_aggregate_callback!(restore_callback);
            baseline.renderer_set_window_size.install(platform_io);
            macro_rules! restore_renderer_value {
                ($field:ident) => {
                    (*platform_io).$field = baseline.callbacks.raw.$field;
                };
            }
            for_each_renderer_value!(restore_renderer_value);
            macro_rules! restore_user_data {
                ($field:ident) => {
                    (*platform_io).$field = baseline.callbacks.raw.$field;
                };
            }
            for_each_user_data!(restore_user_data);
            if MonitorState::capture(platform_io) != MonitorState::capture(&baseline.callbacks.raw)
            {
                ffi::dear_imgui_sdl3_backend_clear_platform_monitors();
                (*platform_io).Monitors = baseline.callbacks.raw.Monitors;
            }
        }
    }
    if !io.is_null() {
        unsafe {
            baseline.backend.restore(io);
            baseline.renderer_backend.restore(io);
        }
    }
    if !main_viewport.is_null() {
        unsafe { baseline.main_viewport.restore(main_viewport) };
    }
}

fn register_runtime(control: &Rc<RuntimeControl>) {
    crate::runtime::register_runtime(control);
}

unsafe extern "C" fn sdl3_create_window(viewport: *mut sys::ImGuiViewport) {
    run_callback("Platform_CreateWindow", (), |control| unsafe {
        if viewport.is_null() {
            return;
        }
        if !(*viewport).PlatformUserData.is_null() {
            control.record_foreign_platform_user_data();
            (*viewport).PlatformRequestClose = true;
            return;
        }
        let Some(callback) = control.original_create_window() else {
            return;
        };
        let Some(transaction) = NativeTransaction::begin(control, NativePhase::Create, viewport)
        else {
            control.mark_viewport_failed(viewport);
            return;
        };
        callback(viewport);
        let native_faults = transaction.finish();
        let state = ViewportPlatformState::capture(viewport);
        if !state.user_data.is_null() {
            control.remember_owned_viewport(viewport, state);
        }
        if state.user_data.is_null() || state.handle.is_null() || native_faults != 0 {
            control.record_viewport_creation_failed();
            control.mark_viewport_failed(viewport);
        }
    });
}

unsafe extern "C" fn sdl3_destroy_window(viewport: *mut sys::ImGuiViewport) {
    let invoked = run_callback("Platform_DestroyWindow", false, |control| unsafe {
        if viewport.is_null() {
            return false;
        }
        control.forget_failed_viewport(viewport);
        let Some(callback) = control.original_destroy_window() else {
            return false;
        };
        let actual = ViewportPlatformState::capture(viewport);
        let Some(expected) = control.take_owned_viewport(viewport) else {
            record_viewport_replacements(control, None, actual);
            control.defer_platform_viewport_restore(viewport, actual);
            ViewportPlatformState::clear(viewport);
            return true;
        };

        if viewport_platform_state_eq(actual, expected) {
            callback(viewport);
            ViewportPlatformState::clear(viewport);
            return true;
        }

        record_viewport_replacements(control, Some(expected), actual);
        expected.restore(viewport);
        callback(viewport);
        ViewportPlatformState::clear(viewport);
        control.defer_platform_viewport_restore(viewport, actual);
        true
    });
    if invoked && !viewport.is_null() {
        unsafe { ViewportPlatformState::clear(viewport) };
    }
}

unsafe extern "C" fn sdl3_render_window(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Platform_RenderWindow", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !validate_platform_viewport_state(control, viewport) {
            control.mark_viewport_failed(viewport);
            return;
        }
        let Some(callback) = control.original_render_window() else {
            return;
        };
        let Some(transaction) = NativeTransaction::begin(control, NativePhase::Render, viewport)
        else {
            control.mark_viewport_failed(viewport);
            return;
        };
        callback(viewport, render_argument);
        if transaction.finish() != 0 {
            control.mark_viewport_failed(viewport);
        }
    });
}

unsafe extern "C" fn sdl3_swap_buffers(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Platform_SwapBuffers", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !validate_platform_viewport_state(control, viewport) {
            control.mark_viewport_failed(viewport);
            return;
        }
        let Some(callback) = control.original_swap_buffers() else {
            return;
        };
        let Some(transaction) = NativeTransaction::begin(control, NativePhase::Swap, viewport)
        else {
            control.mark_viewport_failed(viewport);
            return;
        };
        callback(viewport, render_argument);
        if transaction.finish() != 0 {
            control.mark_viewport_failed(viewport);
        }
    });
}

unsafe extern "C" fn sdl3_renderer_create_window(viewport: *mut sys::ImGuiViewport) {
    run_callback("Renderer_CreateWindow", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        if !(*viewport).RendererUserData.is_null() {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            control.mark_viewport_failed(viewport);
            return;
        }
        let Some(callback) = control.original_renderer_create_window() else {
            return;
        };
        #[cfg(not(feature = "sdlgpu3-renderer"))]
        {
            callback(viewport);
            control.remember_owned_renderer_viewport(viewport, (*viewport).RendererUserData);
        }

        #[cfg(feature = "sdlgpu3-renderer")]
        {
            if control.native_renderer() != NativeRendererKind::SdlGpu3 {
                callback(viewport);
                control.remember_owned_renderer_viewport(viewport, (*viewport).RendererUserData);
                return;
            }
            let Some(transaction) =
                NativeTransaction::begin(control, NativePhase::SdlGpuCreate, viewport)
            else {
                control.mark_viewport_failed(viewport);
                return;
            };
            callback(viewport);
            let native_faults = transaction.finish();
            finish_sdlgpu_renderer_create(control, viewport, native_faults);
        }
    });
}

#[cfg(feature = "sdlgpu3-renderer")]
pub(super) unsafe fn finish_sdlgpu_renderer_create(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
    native_faults: u64,
) {
    if native_faults != 0 {
        // Upstream assigns its sentinel even when SDL rejected claim/configuration. Clearing it
        // prevents DestroyWindow from releasing an unclaimed window or releasing a
        // configure-failure claim that the native transaction already rolled back.
        unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };
        control.mark_viewport_failed(viewport);
        return;
    }
    let renderer_user_data = unsafe { (*viewport).RendererUserData };
    if renderer_user_data.is_null() {
        control.record_renderer_state_replaced("Viewport.RendererUserData(create)");
        control.mark_viewport_failed(viewport);
        return;
    }
    control.remember_owned_renderer_viewport(viewport, renderer_user_data);
}

unsafe extern "C" fn sdl3_renderer_destroy_window(viewport: *mut sys::ImGuiViewport) {
    let invoked = run_callback("Renderer_DestroyWindow", false, |control| unsafe {
        if viewport.is_null() {
            return false;
        }
        if !control.validate_renderer_ownership_bound() {
            control.defer_renderer_viewport_restore(viewport, (*viewport).RendererUserData);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        }
        let Some(callback) = control.original_renderer_destroy_window() else {
            control.defer_renderer_viewport_restore(viewport, (*viewport).RendererUserData);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        };
        let Some(expected_platform) = control.owned_viewport(viewport) else {
            record_viewport_replacements(control, None, ViewportPlatformState::capture(viewport));
            control.defer_renderer_viewport_restore(viewport, (*viewport).RendererUserData);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        };
        let actual_platform = ViewportPlatformState::capture(viewport);
        let expected_renderer = control.owned_renderer_viewport(viewport);
        let actual_renderer = (*viewport).RendererUserData;
        if expected_renderer.is_none() && actual_renderer.is_null() {
            return true;
        }
        let Some(expected_renderer) = expected_renderer else {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
            (*viewport).RendererUserData = std::ptr::null_mut();
            return true;
        };

        if !viewport_platform_state_eq(actual_platform, expected_platform) {
            record_viewport_replacements(control, Some(expected_platform), actual_platform);
            control.defer_platform_viewport_restore(viewport, actual_platform);
        }
        if actual_renderer != expected_renderer {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            control.defer_renderer_viewport_restore(viewport, actual_renderer);
        }

        expected_platform.restore(viewport);
        (*viewport).RendererUserData = expected_renderer;
        callback(viewport);
        (*viewport).RendererUserData = std::ptr::null_mut();
        control.forget_owned_renderer_viewport(viewport);
        true
    });
    if invoked && !viewport.is_null() {
        unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };
    }
}

unsafe extern "C" fn sdl3_renderer_render_window(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Renderer_RenderWindow", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
            || !validate_renderer_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        #[cfg(feature = "sdlgpu3-renderer")]
        if control.native_renderer() == NativeRendererKind::SdlGpu3 {
            let faults = ffi::dear_imgui_sdl3_backend_sdlgpu3_render_viewport(viewport);
            control.record_native_faults(faults);
            if faults != 0 {
                control.mark_viewport_failed(viewport);
            }
            return;
        }
        if let Some(callback) = control.original_renderer_render_window() {
            callback(viewport, render_argument);
        }
    });
}

unsafe extern "C" fn sdl3_renderer_set_window_size(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    run_callback("Renderer_SetWindowSize", (), |control| unsafe {
        if viewport.is_null() || size.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
            || !validate_renderer_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        control.invoke_original_renderer_set_window_size(viewport, size);
    });
}

unsafe extern "C" fn sdl3_renderer_swap_buffers(
    viewport: *mut sys::ImGuiViewport,
    render_argument: *mut c_void,
) {
    run_callback("Renderer_SwapBuffers", (), |control| unsafe {
        if viewport.is_null() || control.viewport_failed(viewport) {
            return;
        }
        if !control.validate_renderer_ownership_bound()
            || !validate_platform_viewport_state(control, viewport)
            || !validate_renderer_viewport_state(control, viewport)
        {
            control.mark_viewport_failed(viewport);
            return;
        }
        if let Some(callback) = control.original_renderer_swap_buffers() {
            callback(viewport, render_argument);
        }
    });
}

#[repr(u32)]
#[derive(Clone, Copy)]
enum NativePhase {
    Create = 1,
    Render = 2,
    Swap = 3,
    #[cfg(feature = "sdlgpu3-renderer")]
    SdlGpuCreate = 4,
}

struct NativeTransaction<'a> {
    control: &'a RuntimeControl,
    active: bool,
}

impl<'a> NativeTransaction<'a> {
    unsafe fn begin(
        control: &'a RuntimeControl,
        phase: NativePhase,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<Self> {
        let (swap_interval_policy, explicit_swap_interval) = control.native_gl_swap_interval();
        let faults = unsafe {
            ffi::dear_imgui_sdl3_native_begin(
                phase as u32,
                u32::from(control.expects_opengl()),
                swap_interval_policy,
                explicit_swap_interval,
                viewport,
            )
        };
        control.record_native_faults(faults);
        (faults == 0).then_some(Self {
            control,
            active: true,
        })
    }

    unsafe fn finish(mut self) -> u64 {
        let faults = unsafe { ffi::dear_imgui_sdl3_native_end() };
        self.active = false;
        self.control.record_native_faults(faults);
        faults
    }
}

impl Drop for NativeTransaction<'_> {
    fn drop(&mut self) {
        if self.active {
            let faults = unsafe { ffi::dear_imgui_sdl3_native_end() };
            self.control.record_native_faults(faults);
        }
    }
}

pub(super) unsafe fn validate_platform_viewport_state(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    let actual = unsafe { ViewportPlatformState::capture(viewport) };
    let expected = control.owned_viewport(viewport);
    if expected.is_some_and(|expected| viewport_platform_state_eq(actual, expected)) {
        return true;
    }
    record_viewport_replacements(control, expected, actual);
    false
}

unsafe fn validate_renderer_viewport_state(
    control: &RuntimeControl,
    viewport: *mut sys::ImGuiViewport,
) -> bool {
    let actual = unsafe { (*viewport).RendererUserData };
    match control.owned_renderer_viewport(viewport) {
        Some(expected) if expected == actual => true,
        None if actual.is_null() => true,
        _ => {
            control.record_renderer_state_replaced("Viewport.RendererUserData");
            false
        }
    }
}

fn viewport_platform_state_eq(left: ViewportPlatformState, right: ViewportPlatformState) -> bool {
    left.user_data == right.user_data
        && left.handle == right.handle
        && left.handle_raw == right.handle_raw
}

fn record_viewport_replacements(
    control: &RuntimeControl,
    expected: Option<ViewportPlatformState>,
    actual: ViewportPlatformState,
) {
    if expected.map_or(!actual.user_data.is_null(), |expected| {
        expected.user_data != actual.user_data
    }) {
        if actual.user_data.is_null() {
            control.record_platform_state_replaced("Viewport.PlatformUserData");
        } else {
            control.record_foreign_platform_user_data();
        }
    }
    if expected.map_or(!actual.handle.is_null(), |expected| {
        expected.handle != actual.handle
    }) {
        control.record_platform_state_replaced("Viewport.PlatformHandle");
    }
    if expected.map_or(!actual.handle_raw.is_null(), |expected| {
        expected.handle_raw != actual.handle_raw
    }) {
        control.record_platform_state_replaced("Viewport.PlatformHandleRaw");
    }
}

fn run_callback<R: Copy>(
    name: &'static str,
    fallback: R,
    callback: impl FnOnce(&RuntimeControl) -> R,
) -> R {
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_current_runtime(|control| {
            if control.validate_platform_ownership_bound() || control.callback_teardown_active() {
                callback(control)
            } else {
                fallback
            }
        })
        .unwrap_or(fallback)
    }));
    match result {
        Ok(result) => result,
        Err(_) => {
            let _ = with_current_runtime(|control| control.record_callback_panicked(name));
            fallback
        }
    }
}

#[cfg(test)]
pub(super) unsafe fn create_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_create_window(viewport) }
}

#[cfg(test)]
pub(super) unsafe fn destroy_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_destroy_window(viewport) }
}

#[cfg(test)]
pub(super) unsafe fn render_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_render_window(viewport, std::ptr::null_mut()) }
}

#[cfg(test)]
pub(super) unsafe fn swap_buffers_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_swap_buffers(viewport, std::ptr::null_mut()) }
}

#[cfg(all(
    test,
    any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    )
))]
pub(super) unsafe fn renderer_render_window_callback_for_test(viewport: *mut sys::ImGuiViewport) {
    unsafe { sdl3_renderer_render_window(viewport, std::ptr::null_mut()) }
}

#[cfg(all(
    test,
    any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    )
))]
pub(super) unsafe fn renderer_set_window_size_callback_for_test(
    viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    unsafe { sdl3_renderer_set_window_size(viewport, size) }
}
