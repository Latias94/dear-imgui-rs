use super::validation::assert_finite_vec2;
use crate::Ui;
use crate::sys;

fn validate_invisible_button_flags(caller: &str, flags: ButtonFlags) {
    let unsupported = flags.bits() & !ButtonFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiButtonFlags bits: 0x{unsupported:X}"
    );
}

fn validate_invisible_button_options(caller: &str, options: InvisibleButtonOptions) {
    validate_invisible_button_flags(caller, options.flags);
    let unsupported_buttons =
        options.mouse_buttons.bits() & !InvisibleButtonMouseButtons::all().bits();
    assert!(
        unsupported_buttons == 0,
        "{caller} received unsupported ImGuiButtonFlags mouse-button bits: 0x{unsupported_buttons:X}"
    );
    assert!(
        !options.mouse_buttons.is_empty(),
        "{caller} requires at least one invisible-button mouse button"
    );
}

fn validate_arrow_direction(caller: &str, dir: crate::Direction) {
    assert!(
        matches!(
            dir,
            crate::Direction::Left
                | crate::Direction::Right
                | crate::Direction::Up
                | crate::Direction::Down
        ),
        "{caller} direction must be Left, Right, Up, or Down"
    );
}

bitflags::bitflags! {
    /// Independent flags for invisible buttons.
    ///
    /// Mouse-button selection is represented separately by
    /// [`InvisibleButtonMouseButtons`] / [`InvisibleButtonOptions`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ButtonFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Allow interaction overlap with following items.
        const ALLOW_OVERLAP = sys::ImGuiButtonFlags_AllowOverlap as i32;
        /// Keep navigation/tabbing enabled for this invisible button.
        const ENABLE_NAV = sys::ImGuiButtonFlags_EnableNav as i32;
    }
}

bitflags::bitflags! {
    /// Mouse buttons accepted by invisible buttons.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct InvisibleButtonMouseButtons: i32 {
        /// React on left mouse button.
        const LEFT = sys::ImGuiButtonFlags_MouseButtonLeft as i32;
        /// React on right mouse button.
        const RIGHT = sys::ImGuiButtonFlags_MouseButtonRight as i32;
        /// React on middle mouse button.
        const MIDDLE = sys::ImGuiButtonFlags_MouseButtonMiddle as i32;
    }
}

impl Default for InvisibleButtonMouseButtons {
    fn default() -> Self {
        Self::LEFT
    }
}

impl From<crate::MouseButton> for InvisibleButtonMouseButtons {
    fn from(button: crate::MouseButton) -> Self {
        match button {
            crate::MouseButton::Left => Self::LEFT,
            crate::MouseButton::Right => Self::RIGHT,
            crate::MouseButton::Middle => Self::MIDDLE,
            crate::MouseButton::Extra1 | crate::MouseButton::Extra2 => {
                panic!("Dear ImGui invisible buttons only support left, right, and middle buttons")
            }
        }
    }
}

/// Complete options accepted by `InvisibleButton()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InvisibleButtonOptions {
    pub flags: ButtonFlags,
    pub mouse_buttons: InvisibleButtonMouseButtons,
}

impl InvisibleButtonOptions {
    pub const fn new() -> Self {
        Self {
            flags: ButtonFlags::NONE,
            mouse_buttons: InvisibleButtonMouseButtons::LEFT,
        }
    }

    pub fn flags(mut self, flags: ButtonFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn mouse_buttons(mut self, buttons: InvisibleButtonMouseButtons) -> Self {
        self.mouse_buttons = buttons;
        self
    }

    pub fn mouse_button(mut self, button: crate::MouseButton) -> Self {
        self.mouse_buttons = button.into();
        self
    }

    /// Returns the raw `ImGuiButtonFlags` bits assembled for `InvisibleButton()`.
    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits() | self.mouse_buttons.bits()
    }
}

impl Default for InvisibleButtonOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ButtonFlags> for InvisibleButtonOptions {
    fn from(flags: ButtonFlags) -> Self {
        Self::new().flags(flags)
    }
}

impl From<InvisibleButtonMouseButtons> for InvisibleButtonOptions {
    fn from(buttons: InvisibleButtonMouseButtons) -> Self {
        Self::new().mouse_buttons(buttons)
    }
}

impl From<crate::MouseButton> for InvisibleButtonOptions {
    fn from(button: crate::MouseButton) -> Self {
        Self::new().mouse_button(button)
    }
}

/// Direction for arrow buttons (alias for Direction)
pub use crate::Direction as ArrowDirection;

impl Ui {
    /// Creates an invisible button
    #[doc(alias = "InvisibleButton")]
    pub fn invisible_button(&self, str_id: impl AsRef<str>, size: impl Into<[f32; 2]>) -> bool {
        self.invisible_button_flags(str_id, size, crate::widget::ButtonFlags::NONE)
    }

    /// Creates an invisible button with independent flags.
    ///
    /// Use [`Self::invisible_button_options`] to choose a mouse button other
    /// than the default left button.
    #[doc(alias = "InvisibleButton")]
    pub fn invisible_button_flags(
        &self,
        str_id: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        flags: crate::widget::ButtonFlags,
    ) -> bool {
        validate_invisible_button_flags("Ui::invisible_button_flags()", flags);
        self.invisible_button_raw(str_id, size, flags.bits())
    }

    /// Creates an invisible button with complete options.
    #[doc(alias = "InvisibleButton")]
    pub fn invisible_button_options(
        &self,
        str_id: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        options: impl Into<crate::widget::InvisibleButtonOptions>,
    ) -> bool {
        let options = options.into();
        validate_invisible_button_options("Ui::invisible_button_options()", options);
        self.invisible_button_raw(str_id, size, options.raw())
    }

    fn invisible_button_raw(
        &self,
        str_id: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        flags: i32,
    ) -> bool {
        let id_ptr = self.scratch_txt(str_id);
        let size = size.into();
        assert_finite_vec2("Ui::invisible_button()", "size", size);
        let size_vec: sys::ImVec2 = size.into();
        self.run_with_bound_context(|| unsafe { sys::igInvisibleButton(id_ptr, size_vec, flags) })
    }

    /// Creates an arrow button
    #[doc(alias = "ArrowButton")]
    pub fn arrow_button(&self, str_id: impl AsRef<str>, dir: crate::Direction) -> bool {
        validate_arrow_direction("Ui::arrow_button()", dir);
        let id_ptr = self.scratch_txt(str_id);
        self.run_with_bound_context(|| unsafe { sys::igArrowButton(id_ptr, dir as i32) })
    }
}
