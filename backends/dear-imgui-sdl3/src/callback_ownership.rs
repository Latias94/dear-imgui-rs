use std::cell::Cell;
use std::ffi::c_void;

use dear_imgui_rs::{Context, sys};

use crate::core::Sdl3BackendError;
use crate::core::ffi;
use crate::runtime::NativeRendererKind;

mod native_callbacks;
mod platform;
mod renderer;

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

pub(super) const SDL_PLATFORM_RESERVED_FLAGS: i32 = sys::ImGuiBackendFlags_HasMouseCursors
    | sys::ImGuiBackendFlags_HasSetMousePos
    | sys::ImGuiBackendFlags_HasGamepad
    | sys::ImGuiBackendFlags_PlatformHasViewports
    | sys::ImGuiBackendFlags_HasMouseHoveredViewport
    | sys::ImGuiBackendFlags_HasParentViewport;

pub(super) const SDL_RENDERER_RESERVED_FLAGS: i32 = sys::ImGuiBackendFlags_RendererHasVtxOffset
    | sys::ImGuiBackendFlags_RendererHasTextures
    | sys::ImGuiBackendFlags_RendererHasViewports;

const SDL_PLATFORM_STABLE_FLAGS: i32 = sys::ImGuiBackendFlags_HasMouseCursors
    | sys::ImGuiBackendFlags_HasSetMousePos
    | sys::ImGuiBackendFlags_PlatformHasViewports
    | sys::ImGuiBackendFlags_HasParentViewport;

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

#[derive(Clone, Copy, Eq, PartialEq)]
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

    pub(super) fn is_empty(self) -> bool {
        self.user_data.is_null() && self.handle.is_null() && self.handle_raw.is_null()
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
    main_viewport_renderer_user_data: *mut c_void,
}

impl PlatformClaimBaseline {
    pub(super) fn snapshot(&self) -> Self {
        Self {
            callbacks: self.callbacks.snapshot(),
            renderer_set_window_size: self.renderer_set_window_size,
            backend: self.backend,
            renderer_backend: self.renderer_backend,
            main_viewport: self.main_viewport,
            main_viewport_renderer_user_data: self.main_viewport_renderer_user_data,
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

pub(super) struct RendererCallbackOwnership {
    baseline: PlatformCallbacks,
    baseline_set_window_size: RendererSetWindowSizeCallback,
    original: PlatformCallbacks,
    original_set_window_size: RendererSetWindowSizeCallback,
    installed: PlatformCallbacks,
    installed_set_window_size: RendererSetWindowSizeCallback,
    baseline_backend: RendererBackendState,
    installed_backend: RendererBackendState,
    baseline_main_viewport_renderer_user_data: *mut c_void,
    installed_main_viewport_renderer_user_data: *mut c_void,
}

pub(super) struct RendererShutdownRestore {
    callbacks: PlatformCallbacks,
    set_window_size: RendererSetWindowSizeCallback,
    backend: RendererBackendState,
    main_viewport_renderer_user_data: *mut c_void,
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
                (
                    "MainViewport.RendererUserData",
                    !(*main_viewport).RendererUserData.is_null(),
                ),
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
            main_viewport_renderer_user_data: (*main_viewport).RendererUserData,
        })
    })?
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
        unsafe {
            baseline.main_viewport.restore(main_viewport);
            (*main_viewport).RendererUserData = baseline.main_viewport_renderer_user_data;
        }
    }
}

use callback_eq;
use for_each_callback;
use for_each_platform_service_callback;
use for_each_platform_window_callback;
use for_each_renderer_non_aggregate_callback;
use for_each_renderer_value;
use for_each_user_data;

#[cfg(all(test, feature = "sdlgpu3-renderer"))]
pub(super) use native_callbacks::finish_sdlgpu_renderer_create;
pub(super) use native_callbacks::validate_platform_viewport_state;
#[cfg(test)]
pub(super) use native_callbacks::{
    create_window_callback_for_test, destroy_window_callback_for_test,
    render_window_callback_for_test, swap_buffers_callback_for_test,
};
#[cfg(all(
    test,
    any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    )
))]
pub(super) use native_callbacks::{
    renderer_render_window_callback_for_test, renderer_set_window_size_callback_for_test,
};
