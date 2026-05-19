//! Miscellaneous widgets
//!
//! Small convenience widgets that don’t fit elsewhere (e.g. bullets, help
//! markers). See functions on `Ui` for details.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::Ui;
use crate::create_token;
use crate::sys;

fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

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
    /// Creates a bullet point
    #[doc(alias = "Bullet")]
    pub fn bullet(&self) {
        unsafe {
            sys::igBullet();
        }
    }

    /// Creates a bullet point with text
    #[doc(alias = "BulletText")]
    pub fn bullet_text(&self, text: impl AsRef<str>) {
        let text_ptr = self.scratch_txt(text);
        unsafe {
            // Always treat the value as unformatted user text.
            const FMT: &[u8; 3] = b"%s\0";
            sys::igBulletText(FMT.as_ptr() as *const std::os::raw::c_char, text_ptr);
        }
    }
}

impl Ui {
    /// Creates a small button
    #[doc(alias = "SmallButton")]
    pub fn small_button(&self, label: impl AsRef<str>) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igSmallButton(label_ptr) }
    }

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
        unsafe { sys::igInvisibleButton(id_ptr, size_vec, flags) }
    }

    /// Creates an arrow button
    #[doc(alias = "ArrowButton")]
    pub fn arrow_button(&self, str_id: impl AsRef<str>, dir: crate::Direction) -> bool {
        validate_arrow_direction("Ui::arrow_button()", dir);
        let id_ptr = self.scratch_txt(str_id);
        unsafe { sys::igArrowButton(id_ptr, dir as i32) }
    }
}

// ============================================================================
// Disabled scope (RAII)
// ============================================================================

/// Tracks a disabled scope begun with [`Ui::begin_disabled`] and ended on drop.
#[must_use]
pub struct DisabledToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> DisabledToken<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        DisabledToken { _ui: ui }
    }

    /// Ends the disabled scope explicitly.
    pub fn end(self) {
        // Drop will call EndDisabled
    }
}

impl<'ui> Drop for DisabledToken<'ui> {
    fn drop(&mut self) {
        unsafe { sys::igEndDisabled() }
    }
}

impl Ui {
    /// Begin a disabled scope for subsequent items.
    ///
    /// All following widgets will be disabled (grayed out and non-interactive)
    /// until the returned token is dropped.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled(&self) -> DisabledToken<'_> {
        unsafe { sys::igBeginDisabled(true) }
        DisabledToken::new(self)
    }

    /// Begin a conditionally disabled scope for subsequent items.
    ///
    /// If `disabled` is false, this still needs to be paired with the returned
    /// token being dropped to correctly balance the internal stack.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled_with_cond(&self, disabled: bool) -> DisabledToken<'_> {
        unsafe { sys::igBeginDisabled(disabled) }
        DisabledToken::new(self)
    }
}

// ============================================================================
// Button repeat (convenience over item flag)
// ============================================================================

create_token!(
    /// Tracks a button repeat item flag pushed with [`Ui::push_button_repeat_token`].
    pub struct ButtonRepeatToken<'ui>;

    /// Pops the button repeat item flag.
    #[doc(alias = "PopButtonRepeat")]
    drop { unsafe { sys::igPopItemFlag() } }
);

impl ButtonRepeatToken<'_> {
    /// Pops the button repeat item flag.
    pub fn pop(self) {
        self.end()
    }
}

impl Ui {
    /// Enable/disable repeating behavior for subsequent buttons.
    ///
    /// Internally uses `PushItemFlag(ImGuiItemFlags_ButtonRepeat, repeat)`.
    ///
    /// Prefer [`Self::push_button_repeat_token`] or [`Self::with_button_repeat`]
    /// for scoped usage that remains balanced if a panic unwinds through the
    /// scope. This manual API is kept for compatibility with existing
    /// push/pop-style code.
    #[doc(alias = "PushButtonRepeat")]
    pub fn push_button_repeat(&self, repeat: bool) {
        unsafe { sys::igPushItemFlag(sys::ImGuiItemFlags_ButtonRepeat as i32, repeat) }
    }

    /// Push a button repeat item flag and return an RAII token that pops it on drop.
    #[doc(alias = "PushButtonRepeat")]
    pub fn push_button_repeat_token(&self, repeat: bool) -> ButtonRepeatToken<'_> {
        self.push_button_repeat(repeat);
        ButtonRepeatToken::new(self)
    }

    /// Push a button repeat item flag, run `f`, then pop the flag.
    ///
    /// The flag is popped during unwinding if `f` panics.
    #[doc(alias = "PushButtonRepeat", alias = "PopButtonRepeat")]
    pub fn with_button_repeat<R>(&self, repeat: bool, f: impl FnOnce() -> R) -> R {
        let _repeat = self.push_button_repeat_token(repeat);
        f()
    }

    /// Pop the button repeat item flag.
    #[doc(alias = "PopButtonRepeat")]
    pub fn pop_button_repeat(&self) {
        unsafe { sys::igPopItemFlag() }
    }
}

// ============================================================================
// Item key ownership
// ============================================================================

impl Ui {
    /// Set the key owner for the last item, without flags.
    ///
    /// Returns `true` when ownership was requested for the item.
    #[doc(alias = "SetItemKeyOwner")]
    pub fn set_item_key_owner(&self, key: crate::input::Key) -> bool {
        let k: sys::ImGuiKey = key as sys::ImGuiKey;
        unsafe { sys::igSetItemKeyOwner_Nil(k) }
    }

    /// Set the key owner for the last item with input flags.
    ///
    /// Returns `true` when ownership was requested for the item.
    #[doc(alias = "SetItemKeyOwner")]
    pub fn set_item_key_owner_with_flags(
        &self,
        key: crate::input::Key,
        flags: crate::input::ItemKeyOwnerFlags,
    ) -> bool {
        let k: sys::ImGuiKey = key as sys::ImGuiKey;
        unsafe { sys::igSetItemKeyOwner_InputFlags(k, flags.raw()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas_mut().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx
    }

    #[test]
    fn with_button_repeat_pops_after_panic() {
        let mut ctx = setup_context();
        let ui = ctx.frame();
        let raw_ctx = unsafe { sys::igGetCurrentContext() };
        assert!(!raw_ctx.is_null());
        let initial_stack_size = unsafe { (*raw_ctx).ItemFlagsStack.Size };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.with_button_repeat(true, || {
                assert_eq!(
                    unsafe { (*raw_ctx).ItemFlagsStack.Size },
                    initial_stack_size + 1
                );
                panic!("forced panic while button repeat is pushed");
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            unsafe { (*raw_ctx).ItemFlagsStack.Size },
            initial_stack_size
        );
    }
}
