use std::cell::Cell;
use std::ffi::c_void;

use dear_imgui_rs::{Context, sys};

use crate::core::Sdl3BackendError;
use crate::core::ffi;
use crate::runtime::NativeRendererKind;

mod native_callbacks;
mod platform;
mod renderer;

macro_rules! for_each_platform_window_callback {
    ($macro:ident) => {
        $macro!(Platform_CreateWindow);
        $macro!(Platform_DestroyWindow);
        $macro!(Platform_ShowWindow);
        $macro!(Platform_UpdateWindow);
        $macro!(Platform_SetWindowPos);
        $macro!(Platform_GetWindowPos);
        $macro!(Platform_SetWindowSize);
        $macro!(Platform_GetWindowSize);
        $macro!(Platform_GetWindowFramebufferScale);
        $macro!(Platform_SetWindowFocus);
        $macro!(Platform_GetWindowFocus);
        $macro!(Platform_GetWindowMinimized);
        $macro!(Platform_SetWindowTitle);
        $macro!(Platform_RenderWindow);
        $macro!(Platform_SwapBuffers);
        $macro!(Platform_SetWindowAlpha);
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

type PlatformSetVec2PointerCallback =
    unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2);
type PlatformGetVec2OutCallback = unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut sys::ImVec2);

#[derive(Clone, Copy)]
enum PlatformSetVec2Callback {
    Native(Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2_c)>),
    Pointer(PlatformSetVec2PointerCallback),
}

impl PlatformSetVec2Callback {
    fn same_callback(self, other: Self) -> bool {
        match (self, other) {
            (Self::Native(left), Self::Native(right)) => callback_eq!(left, right),
            (Self::Pointer(left), Self::Pointer(right)) => std::ptr::fn_addr_eq(left, right),
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
enum PlatformGetVec2Callback {
    Native(Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> sys::ImVec2_c>),
    Out(PlatformGetVec2OutCallback),
}

impl PlatformGetVec2Callback {
    fn same_callback(self, other: Self) -> bool {
        match (self, other) {
            (Self::Native(left), Self::Native(right)) => callback_eq!(left, right),
            (Self::Out(left), Self::Out(right)) => std::ptr::fn_addr_eq(left, right),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlatformCallbackSlot {
    CreateWindow,
    DestroyWindow,
    ShowWindow,
    UpdateWindow,
    SetWindowPos,
    GetWindowPos,
    SetWindowSize,
    GetWindowSize,
    GetWindowFramebufferScale,
    SetWindowFocus,
    GetWindowFocus,
    GetWindowMinimized,
    SetWindowTitle,
    RenderWindow,
    SwapBuffers,
    SetWindowAlpha,
    CreateVkSurface,
}

impl PlatformCallbackSlot {
    pub(super) const ALL: [Self; 17] = [
        Self::CreateWindow,
        Self::DestroyWindow,
        Self::ShowWindow,
        Self::UpdateWindow,
        Self::SetWindowPos,
        Self::GetWindowPos,
        Self::SetWindowSize,
        Self::GetWindowSize,
        Self::GetWindowFramebufferScale,
        Self::SetWindowFocus,
        Self::GetWindowFocus,
        Self::GetWindowMinimized,
        Self::SetWindowTitle,
        Self::RenderWindow,
        Self::SwapBuffers,
        Self::SetWindowAlpha,
        Self::CreateVkSurface,
    ];

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::CreateWindow => "Platform_CreateWindow",
            Self::DestroyWindow => "Platform_DestroyWindow",
            Self::ShowWindow => "Platform_ShowWindow",
            Self::UpdateWindow => "Platform_UpdateWindow",
            Self::SetWindowPos => "Platform_SetWindowPos",
            Self::GetWindowPos => "Platform_GetWindowPos",
            Self::SetWindowSize => "Platform_SetWindowSize",
            Self::GetWindowSize => "Platform_GetWindowSize",
            Self::GetWindowFramebufferScale => "Platform_GetWindowFramebufferScale",
            Self::SetWindowFocus => "Platform_SetWindowFocus",
            Self::GetWindowFocus => "Platform_GetWindowFocus",
            Self::GetWindowMinimized => "Platform_GetWindowMinimized",
            Self::SetWindowTitle => "Platform_SetWindowTitle",
            Self::RenderWindow => "Platform_RenderWindow",
            Self::SwapBuffers => "Platform_SwapBuffers",
            Self::SetWindowAlpha => "Platform_SetWindowAlpha",
            Self::CreateVkSurface => "Platform_CreateVkSurface",
        }
    }
}

pub(super) struct PlatformCallbacks {
    raw: sys::ImGuiPlatformIO,
    set_window_pos: PlatformSetVec2Callback,
    get_window_pos: PlatformGetVec2Callback,
    set_window_size: PlatformSetVec2Callback,
    get_window_size: PlatformGetVec2Callback,
    get_window_framebuffer_scale: PlatformGetVec2Callback,
}

impl PlatformCallbacks {
    unsafe fn capture(raw: *const sys::ImGuiPlatformIO) -> Self {
        let platform_io = raw.cast_mut();
        let raw = unsafe { std::ptr::read(raw) };
        Self {
            set_window_pos: unsafe {
                sys::ImGuiPlatformIO_PlatformSetWindowPosPointerParam(platform_io)
            }
            .map_or(
                PlatformSetVec2Callback::Native(raw.Platform_SetWindowPos),
                PlatformSetVec2Callback::Pointer,
            ),
            get_window_pos: unsafe {
                sys::ImGuiPlatformIO_PlatformGetWindowPosOutParam(platform_io)
            }
            .map_or(
                PlatformGetVec2Callback::Native(raw.Platform_GetWindowPos),
                PlatformGetVec2Callback::Out,
            ),
            set_window_size: unsafe {
                sys::ImGuiPlatformIO_PlatformSetWindowSizePointerParam(platform_io)
            }
            .map_or(
                PlatformSetVec2Callback::Native(raw.Platform_SetWindowSize),
                PlatformSetVec2Callback::Pointer,
            ),
            get_window_size: unsafe {
                sys::ImGuiPlatformIO_PlatformGetWindowSizeOutParam(platform_io)
            }
            .map_or(
                PlatformGetVec2Callback::Native(raw.Platform_GetWindowSize),
                PlatformGetVec2Callback::Out,
            ),
            get_window_framebuffer_scale: unsafe {
                sys::ImGuiPlatformIO_PlatformGetWindowFramebufferScaleOutParam(platform_io)
            }
            .map_or(
                PlatformGetVec2Callback::Native(raw.Platform_GetWindowFramebufferScale),
                PlatformGetVec2Callback::Out,
            ),
            raw,
        }
    }

    pub(super) fn snapshot(&self) -> Self {
        // The bindgen platform IO value has no Rust destructor. Copying it here
        // snapshots pointer-sized callback state without taking native ownership.
        Self {
            raw: unsafe { std::ptr::read(&self.raw) },
            set_window_pos: self.set_window_pos,
            get_window_pos: self.get_window_pos,
            set_window_size: self.set_window_size,
            get_window_size: self.get_window_size,
            get_window_framebuffer_scale: self.get_window_framebuffer_scale,
        }
    }

    fn same_window_callback(&self, other: &Self, slot: PlatformCallbackSlot) -> bool {
        match slot {
            PlatformCallbackSlot::CreateWindow => {
                callback_eq!(
                    self.raw.Platform_CreateWindow,
                    other.raw.Platform_CreateWindow
                )
            }
            PlatformCallbackSlot::DestroyWindow => callback_eq!(
                self.raw.Platform_DestroyWindow,
                other.raw.Platform_DestroyWindow
            ),
            PlatformCallbackSlot::ShowWindow => {
                callback_eq!(self.raw.Platform_ShowWindow, other.raw.Platform_ShowWindow)
            }
            PlatformCallbackSlot::UpdateWindow => callback_eq!(
                self.raw.Platform_UpdateWindow,
                other.raw.Platform_UpdateWindow
            ),
            PlatformCallbackSlot::SetWindowPos => {
                self.set_window_pos.same_callback(other.set_window_pos)
            }
            PlatformCallbackSlot::GetWindowPos => {
                self.get_window_pos.same_callback(other.get_window_pos)
            }
            PlatformCallbackSlot::SetWindowSize => {
                self.set_window_size.same_callback(other.set_window_size)
            }
            PlatformCallbackSlot::GetWindowSize => {
                self.get_window_size.same_callback(other.get_window_size)
            }
            PlatformCallbackSlot::GetWindowFramebufferScale => self
                .get_window_framebuffer_scale
                .same_callback(other.get_window_framebuffer_scale),
            PlatformCallbackSlot::SetWindowFocus => callback_eq!(
                self.raw.Platform_SetWindowFocus,
                other.raw.Platform_SetWindowFocus
            ),
            PlatformCallbackSlot::GetWindowFocus => callback_eq!(
                self.raw.Platform_GetWindowFocus,
                other.raw.Platform_GetWindowFocus
            ),
            PlatformCallbackSlot::GetWindowMinimized => callback_eq!(
                self.raw.Platform_GetWindowMinimized,
                other.raw.Platform_GetWindowMinimized
            ),
            PlatformCallbackSlot::SetWindowTitle => callback_eq!(
                self.raw.Platform_SetWindowTitle,
                other.raw.Platform_SetWindowTitle
            ),
            PlatformCallbackSlot::RenderWindow => callback_eq!(
                self.raw.Platform_RenderWindow,
                other.raw.Platform_RenderWindow
            ),
            PlatformCallbackSlot::SwapBuffers => callback_eq!(
                self.raw.Platform_SwapBuffers,
                other.raw.Platform_SwapBuffers
            ),
            PlatformCallbackSlot::SetWindowAlpha => callback_eq!(
                self.raw.Platform_SetWindowAlpha,
                other.raw.Platform_SetWindowAlpha
            ),
            PlatformCallbackSlot::CreateVkSurface => callback_eq!(
                self.raw.Platform_CreateVkSurface,
                other.raw.Platform_CreateVkSurface
            ),
        }
    }

    pub(super) fn window_callback_is_some(&self, slot: PlatformCallbackSlot) -> bool {
        match slot {
            PlatformCallbackSlot::CreateWindow => self.raw.Platform_CreateWindow.is_some(),
            PlatformCallbackSlot::DestroyWindow => self.raw.Platform_DestroyWindow.is_some(),
            PlatformCallbackSlot::ShowWindow => self.raw.Platform_ShowWindow.is_some(),
            PlatformCallbackSlot::UpdateWindow => self.raw.Platform_UpdateWindow.is_some(),
            PlatformCallbackSlot::SetWindowPos => match self.set_window_pos {
                PlatformSetVec2Callback::Native(callback) => callback.is_some(),
                PlatformSetVec2Callback::Pointer(_) => true,
            },
            PlatformCallbackSlot::GetWindowPos => match self.get_window_pos {
                PlatformGetVec2Callback::Native(callback) => callback.is_some(),
                PlatformGetVec2Callback::Out(_) => true,
            },
            PlatformCallbackSlot::SetWindowSize => match self.set_window_size {
                PlatformSetVec2Callback::Native(callback) => callback.is_some(),
                PlatformSetVec2Callback::Pointer(_) => true,
            },
            PlatformCallbackSlot::GetWindowSize => match self.get_window_size {
                PlatformGetVec2Callback::Native(callback) => callback.is_some(),
                PlatformGetVec2Callback::Out(_) => true,
            },
            PlatformCallbackSlot::GetWindowFramebufferScale => {
                match self.get_window_framebuffer_scale {
                    PlatformGetVec2Callback::Native(callback) => callback.is_some(),
                    PlatformGetVec2Callback::Out(_) => true,
                }
            }
            PlatformCallbackSlot::SetWindowFocus => self.raw.Platform_SetWindowFocus.is_some(),
            PlatformCallbackSlot::GetWindowFocus => self.raw.Platform_GetWindowFocus.is_some(),
            PlatformCallbackSlot::GetWindowMinimized => {
                self.raw.Platform_GetWindowMinimized.is_some()
            }
            PlatformCallbackSlot::SetWindowTitle => self.raw.Platform_SetWindowTitle.is_some(),
            PlatformCallbackSlot::RenderWindow => self.raw.Platform_RenderWindow.is_some(),
            PlatformCallbackSlot::SwapBuffers => self.raw.Platform_SwapBuffers.is_some(),
            PlatformCallbackSlot::SetWindowAlpha => self.raw.Platform_SetWindowAlpha.is_some(),
            PlatformCallbackSlot::CreateVkSurface => self.raw.Platform_CreateVkSurface.is_some(),
        }
    }

    unsafe fn install_window_callbacks(&self, platform_io: *mut sys::ImGuiPlatformIO) {
        unsafe {
            (*platform_io).Platform_CreateWindow = self.raw.Platform_CreateWindow;
            (*platform_io).Platform_DestroyWindow = self.raw.Platform_DestroyWindow;
            (*platform_io).Platform_ShowWindow = self.raw.Platform_ShowWindow;
            (*platform_io).Platform_UpdateWindow = self.raw.Platform_UpdateWindow;
            self.install_aggregate_callbacks(platform_io);
            (*platform_io).Platform_SetWindowFocus = self.raw.Platform_SetWindowFocus;
            (*platform_io).Platform_GetWindowFocus = self.raw.Platform_GetWindowFocus;
            (*platform_io).Platform_GetWindowMinimized = self.raw.Platform_GetWindowMinimized;
            (*platform_io).Platform_SetWindowTitle = self.raw.Platform_SetWindowTitle;
            (*platform_io).Platform_RenderWindow = self.raw.Platform_RenderWindow;
            (*platform_io).Platform_SwapBuffers = self.raw.Platform_SwapBuffers;
            (*platform_io).Platform_SetWindowAlpha = self.raw.Platform_SetWindowAlpha;
            (*platform_io).Platform_CreateVkSurface = self.raw.Platform_CreateVkSurface;
        }
    }

    unsafe fn install_window_callback(
        &self,
        platform_io: *mut sys::ImGuiPlatformIO,
        slot: PlatformCallbackSlot,
    ) {
        unsafe {
            match slot {
                PlatformCallbackSlot::CreateWindow => {
                    (*platform_io).Platform_CreateWindow = self.raw.Platform_CreateWindow;
                }
                PlatformCallbackSlot::DestroyWindow => {
                    (*platform_io).Platform_DestroyWindow = self.raw.Platform_DestroyWindow;
                }
                PlatformCallbackSlot::ShowWindow => {
                    (*platform_io).Platform_ShowWindow = self.raw.Platform_ShowWindow;
                }
                PlatformCallbackSlot::UpdateWindow => {
                    (*platform_io).Platform_UpdateWindow = self.raw.Platform_UpdateWindow;
                }
                PlatformCallbackSlot::SetWindowPos => self.set_window_pos.install_pos(platform_io),
                PlatformCallbackSlot::GetWindowPos => self.get_window_pos.install_pos(platform_io),
                PlatformCallbackSlot::SetWindowSize => {
                    self.set_window_size.install_size(platform_io);
                }
                PlatformCallbackSlot::GetWindowSize => {
                    self.get_window_size.install_size(platform_io);
                }
                PlatformCallbackSlot::GetWindowFramebufferScale => self
                    .get_window_framebuffer_scale
                    .install_framebuffer_scale(platform_io),
                PlatformCallbackSlot::SetWindowFocus => {
                    (*platform_io).Platform_SetWindowFocus = self.raw.Platform_SetWindowFocus;
                }
                PlatformCallbackSlot::GetWindowFocus => {
                    (*platform_io).Platform_GetWindowFocus = self.raw.Platform_GetWindowFocus;
                }
                PlatformCallbackSlot::GetWindowMinimized => {
                    (*platform_io).Platform_GetWindowMinimized =
                        self.raw.Platform_GetWindowMinimized;
                }
                PlatformCallbackSlot::SetWindowTitle => {
                    (*platform_io).Platform_SetWindowTitle = self.raw.Platform_SetWindowTitle;
                }
                PlatformCallbackSlot::RenderWindow => {
                    (*platform_io).Platform_RenderWindow = self.raw.Platform_RenderWindow;
                }
                PlatformCallbackSlot::SwapBuffers => {
                    (*platform_io).Platform_SwapBuffers = self.raw.Platform_SwapBuffers;
                }
                PlatformCallbackSlot::SetWindowAlpha => {
                    (*platform_io).Platform_SetWindowAlpha = self.raw.Platform_SetWindowAlpha;
                }
                PlatformCallbackSlot::CreateVkSurface => {
                    (*platform_io).Platform_CreateVkSurface = self.raw.Platform_CreateVkSurface;
                }
            }
        }
    }

    unsafe fn install_aggregate_callbacks(&self, platform_io: *mut sys::ImGuiPlatformIO) {
        unsafe {
            self.set_window_pos.install_pos(platform_io);
            self.get_window_pos.install_pos(platform_io);
            self.set_window_size.install_size(platform_io);
            self.get_window_size.install_size(platform_io);
            self.get_window_framebuffer_scale
                .install_framebuffer_scale(platform_io);
        }
    }

    pub(super) unsafe fn invoke_set_window_pos(
        &self,
        viewport: *mut sys::ImGuiViewport,
        pos: *const sys::ImVec2,
    ) -> bool {
        match self.set_window_pos {
            PlatformSetVec2Callback::Native(Some(_)) => unsafe {
                sys::ImGuiPlatformIO_InvokePlatformSetWindowPos(&self.raw, viewport, pos)
            },
            PlatformSetVec2Callback::Native(None) => false,
            PlatformSetVec2Callback::Pointer(callback) => {
                unsafe { callback(viewport, pos) };
                true
            }
        }
    }

    pub(super) unsafe fn invoke_get_window_pos(
        &self,
        viewport: *mut sys::ImGuiViewport,
        out_pos: *mut sys::ImVec2,
    ) -> bool {
        match self.get_window_pos {
            PlatformGetVec2Callback::Native(Some(_)) => unsafe {
                sys::ImGuiPlatformIO_InvokePlatformGetWindowPos(&self.raw, viewport, out_pos)
            },
            PlatformGetVec2Callback::Native(None) => false,
            PlatformGetVec2Callback::Out(callback) => {
                unsafe { callback(viewport, out_pos) };
                true
            }
        }
    }

    pub(super) unsafe fn invoke_set_window_size(
        &self,
        viewport: *mut sys::ImGuiViewport,
        size: *const sys::ImVec2,
    ) -> bool {
        match self.set_window_size {
            PlatformSetVec2Callback::Native(Some(_)) => unsafe {
                sys::ImGuiPlatformIO_InvokePlatformSetWindowSize(&self.raw, viewport, size)
            },
            PlatformSetVec2Callback::Native(None) => false,
            PlatformSetVec2Callback::Pointer(callback) => {
                unsafe { callback(viewport, size) };
                true
            }
        }
    }

    pub(super) unsafe fn invoke_get_window_size(
        &self,
        viewport: *mut sys::ImGuiViewport,
        out_size: *mut sys::ImVec2,
    ) -> bool {
        match self.get_window_size {
            PlatformGetVec2Callback::Native(Some(_)) => unsafe {
                sys::ImGuiPlatformIO_InvokePlatformGetWindowSize(&self.raw, viewport, out_size)
            },
            PlatformGetVec2Callback::Native(None) => false,
            PlatformGetVec2Callback::Out(callback) => {
                unsafe { callback(viewport, out_size) };
                true
            }
        }
    }

    pub(super) unsafe fn invoke_get_window_framebuffer_scale(
        &self,
        viewport: *mut sys::ImGuiViewport,
        out_scale: *mut sys::ImVec2,
    ) -> bool {
        match self.get_window_framebuffer_scale {
            PlatformGetVec2Callback::Native(Some(_)) => unsafe {
                sys::ImGuiPlatformIO_InvokePlatformGetWindowFramebufferScale(
                    &self.raw, viewport, out_scale,
                )
            },
            PlatformGetVec2Callback::Native(None) => false,
            PlatformGetVec2Callback::Out(callback) => {
                unsafe { callback(viewport, out_scale) };
                true
            }
        }
    }

    pub(super) fn create_window(&self) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.raw.Platform_CreateWindow
    }

    pub(super) fn destroy_window(&self) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.raw.Platform_DestroyWindow
    }

    pub(super) fn show_window(&self) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.raw.Platform_ShowWindow
    }

    pub(super) fn update_window(&self) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.raw.Platform_UpdateWindow
    }

    pub(super) fn set_window_focus(&self) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.raw.Platform_SetWindowFocus
    }

    pub(super) fn get_window_focus(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool> {
        self.raw.Platform_GetWindowFocus
    }

    pub(super) fn get_window_minimized(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport) -> bool> {
        self.raw.Platform_GetWindowMinimized
    }

    pub(super) fn set_window_title(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const std::ffi::c_char)> {
        self.raw.Platform_SetWindowTitle
    }

    pub(super) fn render_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.raw.Platform_RenderWindow
    }

    pub(super) fn swap_buffers(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.raw.Platform_SwapBuffers
    }

    pub(super) fn set_window_alpha(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, f32)> {
        self.raw.Platform_SetWindowAlpha
    }

    pub(super) fn create_vk_surface(
        &self,
    ) -> Option<
        unsafe extern "C" fn(
            *mut sys::ImGuiViewport,
            sys::ImU64,
            *const c_void,
            *mut sys::ImU64,
        ) -> std::os::raw::c_int,
    > {
        self.raw.Platform_CreateVkSurface
    }
}

impl PlatformSetVec2Callback {
    unsafe fn install_pos(self, platform_io: *mut sys::ImGuiPlatformIO) {
        match self {
            Self::Native(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(platform_io, None);
                (*platform_io).Platform_SetWindowPos = callback;
            },
            Self::Pointer(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_SetWindowPos_PointerParam(
                    platform_io,
                    Some(callback),
                );
            },
        }
    }

    unsafe fn install_size(self, platform_io: *mut sys::ImGuiPlatformIO) {
        match self {
            Self::Native(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(platform_io, None);
                (*platform_io).Platform_SetWindowSize = callback;
            },
            Self::Pointer(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_SetWindowSize_PointerParam(
                    platform_io,
                    Some(callback),
                );
            },
        }
    }
}

impl PlatformGetVec2Callback {
    unsafe fn install_pos(self, platform_io: *mut sys::ImGuiPlatformIO) {
        match self {
            Self::Native(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(platform_io, None);
                (*platform_io).Platform_GetWindowPos = callback;
            },
            Self::Out(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowPos_OutParam(
                    platform_io,
                    Some(callback),
                );
            },
        }
    }

    unsafe fn install_size(self, platform_io: *mut sys::ImGuiPlatformIO) {
        match self {
            Self::Native(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(platform_io, None);
                (*platform_io).Platform_GetWindowSize = callback;
            },
            Self::Out(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowSize_OutParam(
                    platform_io,
                    Some(callback),
                );
            },
        }
    }

    unsafe fn install_framebuffer_scale(self, platform_io: *mut sys::ImGuiPlatformIO) {
        match self {
            Self::Native(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                    platform_io,
                    None,
                );
                (*platform_io).Platform_GetWindowFramebufferScale = callback;
            },
            Self::Out(callback) => unsafe {
                sys::ImGuiPlatformIO_Set_Platform_GetWindowFramebufferScale_OutParam(
                    platform_io,
                    Some(callback),
                );
            },
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

    pub(super) unsafe fn clear(viewport: *mut sys::ImGuiViewport) {
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
            macro_rules! restore_service_callback {
                ($field:ident) => {
                    (*platform_io).$field = baseline.callbacks.raw.$field;
                };
            }
            for_each_platform_service_callback!(restore_service_callback);
            baseline.callbacks.install_window_callbacks(platform_io);
            macro_rules! restore_renderer_callback {
                ($field:ident) => {
                    (*platform_io).$field = baseline.callbacks.raw.$field;
                };
            }
            for_each_renderer_non_aggregate_callback!(restore_renderer_callback);
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
use for_each_platform_service_callback;
use for_each_renderer_non_aggregate_callback;
use for_each_renderer_value;
use for_each_user_data;

#[cfg(all(test, feature = "sdlgpu3-renderer"))]
pub(super) use native_callbacks::finish_sdlgpu_renderer_create;
#[cfg(feature = "multi-viewport")]
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

#[cfg(test)]
mod inventory_tests {
    #[test]
    fn platform_window_callback_inventory_matches_imgui_impl_sdl3() {
        let mut callbacks = Vec::new();
        macro_rules! collect_callback {
            ($field:ident) => {
                callbacks.push(stringify!($field));
            };
        }

        for_each_platform_window_callback!(collect_callback);

        assert_eq!(
            callbacks,
            [
                "Platform_CreateWindow",
                "Platform_DestroyWindow",
                "Platform_ShowWindow",
                "Platform_UpdateWindow",
                "Platform_SetWindowPos",
                "Platform_GetWindowPos",
                "Platform_SetWindowSize",
                "Platform_GetWindowSize",
                "Platform_GetWindowFramebufferScale",
                "Platform_SetWindowFocus",
                "Platform_GetWindowFocus",
                "Platform_GetWindowMinimized",
                "Platform_SetWindowTitle",
                "Platform_RenderWindow",
                "Platform_SwapBuffers",
                "Platform_SetWindowAlpha",
                "Platform_CreateVkSurface",
            ]
        );
    }
}

impl PlatformCallbackOwnership {
    pub(super) fn original_callbacks(&self) -> PlatformCallbacks {
        self.original.snapshot()
    }

    pub(super) unsafe fn owns_live_slot(&self, slot: PlatformCallbackSlot) -> bool {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return false;
        }
        let current = unsafe { PlatformCallbacks::capture(platform_io) };
        self.installed.same_window_callback(&current, slot)
    }

    #[cfg(test)]
    pub(super) fn wraps_slot(&self, slot: PlatformCallbackSlot) -> bool {
        self.original.window_callback_is_some(slot)
            && !self.original.same_window_callback(&self.installed, slot)
    }
}
