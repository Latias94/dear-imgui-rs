use crate::{MouseButton, sys};

use super::PopupContextFlags;
use super::flags::validate_popup_context_flags;

/// Single mouse button used by popup context helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PopupContextMouseButton {
    /// Open on left mouse release.
    Left,
    /// Open on right mouse release.
    #[default]
    Right,
    /// Open on middle mouse release.
    Middle,
}

impl PopupContextMouseButton {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Left => sys::ImGuiPopupFlags_MouseButtonLeft as i32,
            Self::Right => sys::ImGuiPopupFlags_MouseButtonRight as i32,
            Self::Middle => sys::ImGuiPopupFlags_MouseButtonMiddle as i32,
        }
    }
}

impl From<MouseButton> for PopupContextMouseButton {
    fn from(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => Self::Left,
            MouseButton::Right => Self::Right,
            MouseButton::Middle => Self::Middle,
            MouseButton::Extra1 | MouseButton::Extra2 => {
                panic!(
                    "Dear ImGui popup context helpers only support left, right, and middle buttons"
                )
            }
        }
    }
}

/// Complete popup options assembled from independent flags and optional
/// single mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PopupContextOptions {
    pub flags: PopupContextFlags,
    pub mouse_button: PopupContextMouseButton,
}

impl PopupContextOptions {
    pub const fn new() -> Self {
        Self {
            flags: PopupContextFlags::NONE,
            mouse_button: PopupContextMouseButton::Right,
        }
    }

    pub fn flags(mut self, flags: PopupContextFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn mouse_button(mut self, button: impl Into<PopupContextMouseButton>) -> Self {
        self.mouse_button = button.into();
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.raw() | self.mouse_button.raw()
    }

    #[inline]
    pub(super) fn validate(self, caller: &str) {
        validate_popup_context_flags(caller, self.flags);
        assert!(
            self.flags.bits() & (sys::ImGuiPopupFlags_MouseButtonMask_ as i32) == 0,
            "{caller} received non-independent PopupContextFlags mouse-button bits"
        );
    }
}

impl Default for PopupContextOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<PopupContextFlags> for PopupContextOptions {
    fn from(flags: PopupContextFlags) -> Self {
        Self::new().flags(flags)
    }
}

impl From<PopupContextMouseButton> for PopupContextOptions {
    fn from(button: PopupContextMouseButton) -> Self {
        Self::new().mouse_button(button)
    }
}

impl From<MouseButton> for PopupContextOptions {
    fn from(button: MouseButton) -> Self {
        Self::new().mouse_button(button)
    }
}
