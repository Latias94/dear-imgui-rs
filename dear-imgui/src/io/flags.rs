use crate::sys;
use bitflags::bitflags;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
impl Serialize for ConfigFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ConfigFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(ConfigFlags::from_bits_truncate(bits))
    }
}

#[cfg(feature = "serde")]
impl Serialize for BackendFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for BackendFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(BackendFlags::from_bits_truncate(bits))
    }
}

#[cfg(feature = "serde")]
impl Serialize for ViewportFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ViewportFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(ViewportFlags::from_bits_truncate(bits))
    }
}

bitflags! {
    /// Configuration flags
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ConfigFlags: i32 {
        /// Master keyboard navigation enable flag.
        const NAV_ENABLE_KEYBOARD = sys::ImGuiConfigFlags_NavEnableKeyboard as i32;
        /// Master gamepad navigation enable flag.
        const NAV_ENABLE_GAMEPAD = sys::ImGuiConfigFlags_NavEnableGamepad as i32;
        /// Instruction imgui-rs to clear mouse position/buttons in `frame()`.
        const NO_MOUSE = sys::ImGuiConfigFlags_NoMouse as i32;
        /// Instruction backend to not alter mouse cursor shape and visibility.
        const NO_MOUSE_CURSOR_CHANGE = sys::ImGuiConfigFlags_NoMouseCursorChange as i32;
        /// Disable keyboard inputs and interactions.
        const NO_KEYBOARD = sys::ImGuiConfigFlags_NoKeyboard as i32;
        /// Application is SRGB-aware.
        const IS_SRGB = sys::ImGuiConfigFlags_IsSRGB as i32;
        /// Application is using a touch screen instead of a mouse.
        const IS_TOUCH_SCREEN = sys::ImGuiConfigFlags_IsTouchScreen as i32;

        const DOCKING_ENABLE = sys::ImGuiConfigFlags_DockingEnable as i32;

        const VIEWPORTS_ENABLE = sys::ImGuiConfigFlags_ViewportsEnable as i32;
    }
}

bitflags! {
    /// Backend capabilities
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BackendFlags: i32 {
        /// Backend supports gamepad and currently has one connected
        const HAS_GAMEPAD = sys::ImGuiBackendFlags_HasGamepad as i32;
        /// Backend supports honoring `get_mouse_cursor` value to change the OS cursor shape
        const HAS_MOUSE_CURSORS = sys::ImGuiBackendFlags_HasMouseCursors as i32;
        /// Backend supports `io.want_set_mouse_pos` requests to reposition the OS mouse position.
        const HAS_SET_MOUSE_POS = sys::ImGuiBackendFlags_HasSetMousePos as i32;
        /// Backend can report which viewport the OS mouse is hovering via `add_mouse_viewport_event`
        const HAS_MOUSE_HOVERED_VIEWPORT =
            sys::ImGuiBackendFlags_HasMouseHoveredViewport as i32;
        /// Backend platform can honor viewport parent/child relationships.
        const HAS_PARENT_VIEWPORT = sys::ImGuiBackendFlags_HasParentViewport as i32;
        /// Backend renderer supports DrawCmd::vtx_offset.
        const RENDERER_HAS_VTX_OFFSET = sys::ImGuiBackendFlags_RendererHasVtxOffset as i32;
        /// Backend renderer supports ImTextureData requests to create/update/destroy textures.
        const RENDERER_HAS_TEXTURES = sys::ImGuiBackendFlags_RendererHasTextures as i32;

        #[cfg(feature = "multi-viewport")]
        /// Set if the platform backend supports viewports.
        const PLATFORM_HAS_VIEWPORTS = sys::ImGuiBackendFlags_PlatformHasViewports as i32;
        #[cfg(feature = "multi-viewport")]
        /// Set if the renderer backend supports viewports.
        const RENDERER_HAS_VIEWPORTS = sys::ImGuiBackendFlags_RendererHasViewports as i32;
    }
}

bitflags! {
    /// Viewport flags for multi-viewport support
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ViewportFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Represent a Platform Window
        const IS_PLATFORM_WINDOW = sys::ImGuiViewportFlags_IsPlatformWindow as i32;
        /// Represent a Platform Monitor (unused in our implementation)
        const IS_PLATFORM_MONITOR = sys::ImGuiViewportFlags_IsPlatformMonitor as i32;
        /// Platform Window: is created/managed by the application (rather than a dear imgui backend)
        const OWNED_BY_APP = sys::ImGuiViewportFlags_OwnedByApp as i32;
        /// Platform Window: Disable platform decorations: title bar, borders, etc.
        const NO_DECORATION = sys::ImGuiViewportFlags_NoDecoration as i32;
        /// Platform Window: Disable platform task bar icon (generally set on popups/tooltips, or all windows if ImGuiConfigFlags_ViewportsNoTaskBarIcon is set)
        const NO_TASK_BAR_ICON = sys::ImGuiViewportFlags_NoTaskBarIcon as i32;
        /// Platform Window: Don't take focus when created.
        const NO_FOCUS_ON_APPEARING = sys::ImGuiViewportFlags_NoFocusOnAppearing as i32;
        /// Platform Window: Don't take focus when clicked on.
        const NO_FOCUS_ON_CLICK = sys::ImGuiViewportFlags_NoFocusOnClick as i32;
        /// Platform Window: Make mouse pass through so we can drag this window while peaking behind it.
        const NO_INPUTS = sys::ImGuiViewportFlags_NoInputs as i32;
        /// Platform Window: Renderer doesn't need to clear the framebuffer ahead (because we will fill it entirely).
        const NO_RENDERER_CLEAR = sys::ImGuiViewportFlags_NoRendererClear as i32;
        /// Platform Window: Avoid merging this window into another host window. This can only be set via ImGuiWindowClass viewport flags override (because we need to now ahead if we are going to create a viewport in the first place!).
        const NO_AUTO_MERGE = sys::ImGuiViewportFlags_NoAutoMerge as i32;
        /// Platform Window: Display on top (for tooltips only).
        const TOP_MOST = sys::ImGuiViewportFlags_TopMost as i32;
        /// Viewport can host multiple imgui windows (secondary viewports are associated to a single window).
        const CAN_HOST_OTHER_WINDOWS = sys::ImGuiViewportFlags_CanHostOtherWindows as i32;
        /// Platform Window: Window is minimized, can skip render. When minimized we tend to avoid using the viewport pos/size for clipping rectangle computation.
        const IS_MINIMIZED = sys::ImGuiViewportFlags_IsMinimized as i32;
        /// Platform Window: Window is focused (last call to Platform_GetWindowFocus() returned true)
        const IS_FOCUSED = sys::ImGuiViewportFlags_IsFocused as i32;
    }
}

pub(crate) fn validate_config_flags(caller: &str, flags: ConfigFlags) {
    let unsupported = flags.bits() & !ConfigFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiConfigFlags bits: 0x{unsupported:X}"
    );
}

pub(crate) fn validate_backend_flags(caller: &str, flags: BackendFlags) {
    let unsupported = flags.bits() & !BackendFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiBackendFlags bits: 0x{unsupported:X}"
    );
}

pub(crate) fn validate_viewport_flags(caller: &str, flags: ViewportFlags) {
    let unsupported = flags.bits() & !ViewportFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiViewportFlags bits: 0x{unsupported:X}"
    );
}
