use super::focus::{FocusedFlags, validate_focused_flags};
use super::hover_flags::{WindowHoveredFlags, validate_window_hovered_flags};
use crate::sys;

impl crate::ui::Ui {
    /// Returns `true` if the current window is hovered (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered(&self) -> bool {
        unsafe { sys::igIsWindowHovered(WindowHoveredFlags::NONE.bits()) }
    }

    /// Returns `true` if the current window is hovered based on the given flags
    #[doc(alias = "IsWindowHovered")]
    pub fn is_window_hovered_with_flags(&self, flags: WindowHoveredFlags) -> bool {
        validate_window_hovered_flags("Ui::is_window_hovered_with_flags()", flags);
        unsafe { sys::igIsWindowHovered(flags.bits()) }
    }

    /// Returns `true` if the current window is focused (and typically: not blocked by a popup/modal)
    #[doc(alias = "IsWindowFocused")]
    pub fn is_window_focused(&self) -> bool {
        self.is_window_focused_with_flags(FocusedFlags::NONE)
    }

    /// Returns `true` if the current window is focused based on the given flags
    #[doc(alias = "IsWindowFocused")]
    pub fn is_window_focused_with_flags(&self, flags: FocusedFlags) -> bool {
        validate_focused_flags("Ui::is_window_focused_with_flags()", flags);
        unsafe { sys::igIsWindowFocused(flags.bits()) }
    }

    /// Returns `true` if the current window is appearing this frame.
    #[doc(alias = "IsWindowAppearing")]
    pub fn is_window_appearing(&self) -> bool {
        unsafe { sys::igIsWindowAppearing() }
    }

    /// Returns `true` if the current window is collapsed.
    #[doc(alias = "IsWindowCollapsed")]
    pub fn is_window_collapsed(&self) -> bool {
        unsafe { sys::igIsWindowCollapsed() }
    }
}
