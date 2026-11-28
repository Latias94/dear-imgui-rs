//! Miscellaneous widgets
//!
//! Small convenience widgets that don’t fit elsewhere (e.g. bullets, help
//! markers). See functions on `Ui` for details.
//!
//! Arrow buttons use [`crate::Direction`] directly. The `ArrowDirection` alias is intentionally
//! unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::ArrowDirection;
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::validation::assert_finite_vec2;
use crate::{ItemFlagStackToken, ItemFlags, Ui, sys};

impl Ui {
    /// Creates a bullet point
    #[doc(alias = "Bullet")]
    pub fn bullet(&self) {
        self.run_with_bound_context(|| unsafe {
            sys::igBullet();
        });
    }

    /// Creates a bullet point with text
    #[doc(alias = "BulletText")]
    pub fn bullet_text(&self, text: impl AsRef<str>) {
        self.with_scratch_txt(text, |text_ptr| unsafe {
            // Always treat the value as unformatted user text.
            const FMT: &[u8; 3] = b"%s\0";
            sys::igBulletText(FMT.as_ptr() as *const std::os::raw::c_char, text_ptr);
        });
    }

    /// Creates a small button
    #[doc(alias = "SmallButton")]
    pub fn small_button(&self, label: impl AsRef<str>) -> bool {
        self.with_scratch_txt(label, |label_ptr| unsafe { sys::igSmallButton(label_ptr) })
    }

    /// Creates an invisible button
    #[doc(alias = "InvisibleButton")]
    pub fn invisible_button(&self, str_id: impl AsRef<str>, size: impl Into<[f32; 2]>) -> bool {
        self.invisible_button_flags(str_id, size, ButtonFlags::NONE)
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
        flags: ButtonFlags,
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
        options: impl Into<InvisibleButtonOptions>,
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
        let size = size.into();
        assert_finite_vec2("Ui::invisible_button()", "size", size);
        let size_vec: sys::ImVec2 = size.into();
        self.with_scratch_txt(str_id, |id_ptr| unsafe {
            sys::igInvisibleButton(id_ptr, size_vec, flags)
        })
    }

    /// Creates an arrow button
    #[doc(alias = "ArrowButton")]
    pub fn arrow_button(&self, str_id: impl AsRef<str>, dir: crate::Direction) -> bool {
        validate_arrow_direction("Ui::arrow_button()", dir);
        self.with_scratch_txt(str_id, |id_ptr| unsafe {
            sys::igArrowButton(id_ptr, dir as i32)
        })
    }

    /// Enable/disable repeating behavior for subsequent buttons.
    ///
    /// Internally uses `PushItemFlag(ImGuiItemFlags_ButtonRepeat, repeat)`.
    #[doc(alias = "PushButtonRepeat")]
    pub fn push_button_repeat(&self, repeat: bool) -> ButtonRepeatToken<'_> {
        self.push_item_flag(ItemFlags::BUTTON_REPEAT, repeat)
    }

    /// Push a button repeat item flag, run `f`, then pop the flag.
    ///
    /// The flag is popped during unwinding if `f` panics.
    #[doc(alias = "PushButtonRepeat", alias = "PopButtonRepeat")]
    pub fn with_button_repeat<R>(&self, repeat: bool, f: impl FnOnce() -> R) -> R {
        let repeat = self.push_button_repeat(repeat);
        let result = f();
        drop(repeat);
        result
    }

    /// Begin a disabled scope for subsequent items.
    ///
    /// All following widgets will be disabled (grayed out and non-interactive)
    /// until the returned token is dropped.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled(&self) -> DisabledToken<'_> {
        self.begin_disabled_with_cond(true)
    }

    /// Begin a conditionally disabled scope for subsequent items.
    ///
    /// If `disabled` is false, this still needs to be paired with the returned
    /// token being dropped to correctly balance the internal stack.
    #[doc(alias = "BeginDisabled")]
    pub fn begin_disabled_with_cond(&self, disabled: bool) -> DisabledToken<'_> {
        let restores_alpha = self.run_with_bound_context(|| unsafe {
            let context = self.context_raw();
            let was_disabled = (*context).CurrentItemFlags & sys::ImGuiItemFlags_Disabled != 0;
            sys::igBeginDisabled(disabled);
            disabled && !was_disabled
        });
        DisabledToken::new(self, restores_alpha)
    }

    /// Runs `f` while subsequent items are disabled.
    ///
    /// The disabled scope is ended before a successful closure result is returned and during
    /// unwinding if `f` panics.
    #[doc(alias = "BeginDisabled", alias = "EndDisabled")]
    pub fn with_disabled<R>(&self, f: impl FnOnce() -> R) -> R {
        self.with_disabled_if(true, f)
    }

    /// Runs `f` inside a conditionally disabled scope.
    ///
    /// Dear ImGui requires a balanced `BeginDisabled`/`EndDisabled` pair even when `disabled` is
    /// false. This helper preserves that balance before returning or during unwinding.
    #[doc(alias = "BeginDisabled", alias = "EndDisabled")]
    pub fn with_disabled_if<R>(&self, disabled: bool, f: impl FnOnce() -> R) -> R {
        let token = self.begin_disabled_with_cond(disabled);
        let result = f();
        drop(token);
        result
    }

    /// Set the key owner for the last item, without flags.
    ///
    /// Returns `true` when ownership was requested for the item.
    #[doc(alias = "SetItemKeyOwner")]
    pub fn set_item_key_owner(&self, key: crate::input::Key) -> bool {
        let k: sys::ImGuiKey = key as sys::ImGuiKey;
        self.run_with_bound_context(|| unsafe { sys::igSetItemKeyOwner_Nil(k) })
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
        self.run_with_bound_context(|| unsafe { sys::igSetItemKeyOwner_InputFlags(k, flags.raw()) })
    }
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
        const ALLOW_OVERLAP = sys::ImGuiButtonFlags_AllowOverlap;
        /// Keep navigation/tabbing enabled for this invisible button.
        const ENABLE_NAV = sys::ImGuiButtonFlags_EnableNav;
    }
}

bitflags::bitflags! {
    /// Mouse buttons accepted by invisible buttons.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct InvisibleButtonMouseButtons: i32 {
        /// React on left mouse button.
        const LEFT = sys::ImGuiButtonFlags_MouseButtonLeft;
        /// React on right mouse button.
        const RIGHT = sys::ImGuiButtonFlags_MouseButtonRight;
        /// React on middle mouse button.
        const MIDDLE = sys::ImGuiButtonFlags_MouseButtonMiddle;
    }
}

impl Default for InvisibleButtonMouseButtons {
    fn default() -> Self {
        Self::LEFT
    }
}

impl TryFrom<crate::MouseButton> for InvisibleButtonMouseButtons {
    type Error = crate::MouseButton;

    /// Returns the unsupported button on failure.
    fn try_from(button: crate::MouseButton) -> Result<Self, Self::Error> {
        match button {
            crate::MouseButton::Left => Ok(Self::LEFT),
            crate::MouseButton::Right => Ok(Self::RIGHT),
            crate::MouseButton::Middle => Ok(Self::MIDDLE),
            crate::MouseButton::Extra1 | crate::MouseButton::Extra2 => Err(button),
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

/// Tracks a button repeat item flag pushed with [`Ui::push_button_repeat`].
pub type ButtonRepeatToken<'ui> = ItemFlagStackToken<'ui>;

/// Tracks a disabled scope begun with [`Ui::begin_disabled`] and ended on drop.
///
/// Disabled scopes share Dear ImGui's item-flag stack with [`Ui::push_item_flag`]. A scope that
/// transitions into the disabled state also shares `Style.Alpha` restoration order with
/// [`crate::StyleVar::Alpha`] tokens. Tokens on either affected stack must finish in reverse
/// creation order and in their originating window `Begin` scope. Prefer [`Ui::with_disabled`] or
/// [`Ui::with_disabled_if`] when a closure expresses the intended lifetime.
#[must_use]
#[doc(alias = "EndDisabled")]
pub struct DisabledToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> DisabledToken<'ui> {
    fn new(ui: &'ui Ui, restores_alpha: bool) -> Self {
        Self {
            scope: ui.begin_native_scope(
                NativeScopePop::EndDisabled { restores_alpha },
                "DisabledToken",
            ),
        }
    }

    /// Ends the disabled scope explicitly.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later item-flag or disabled token is active, an Alpha style token
    /// depends on this scope's saved state, or this token is outside its originating window
    /// `Begin` scope.
    pub fn end(self) {
        // Drop will call EndDisabled
    }
}

impl<'ui> Drop for DisabledToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

#[cfg(test)]
mod tests;
