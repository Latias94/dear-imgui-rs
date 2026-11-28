//! Popups and modals
//!
//! Popup windows (context menus, modals) with builders and token helpers to
//! ensure balanced begin/end calls.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::window::{WindowFlags, validate_window_flags};
use crate::{Id, MouseButton, Ui, sys};

/// # Popup Widgets
impl Ui {
    /// Instructs ImGui that a popup is open.
    ///
    /// You should **call this function once** while calling any of the following per-frame:
    ///
    /// - [`begin_popup`](Self::begin_popup)
    /// - [`popup`](Self::popup)
    /// - [`begin_modal_popup`](Self::begin_modal_popup)
    /// - [`modal_popup`](Self::modal_popup)
    ///
    /// The confusing aspect to popups is that ImGui holds control over the popup itself.
    ///
    /// Returns `true` when this request toggles the popup toward its open state. Existing callers
    /// that do not need to initialize popup-local state may ignore the result.
    #[doc(alias = "OpenPopup")]
    pub fn open_popup(&self, str_id: impl AsRef<str>) -> bool {
        self.with_scratch_txt(str_id, |str_id_ptr| unsafe {
            sys::igOpenPopup_Str(str_id_ptr, PopupOpenFlags::NONE.raw())
        })
    }

    /// Instructs ImGui that a popup is open with flags.
    ///
    /// Returns `true` when this request toggles the popup toward its open state.
    #[doc(alias = "OpenPopup")]
    pub fn open_popup_with_flags(&self, str_id: impl AsRef<str>, flags: PopupOpenFlags) -> bool {
        validate_popup_open_flags("Ui::open_popup_with_flags()", flags);
        self.with_scratch_txt(str_id, |str_id_ptr| unsafe {
            sys::igOpenPopup_Str(str_id_ptr, flags.raw())
        })
    }

    /// Opens a popup by an ID from the current ID stack.
    ///
    /// Returns `true` when the popup is toggled open.
    #[doc(alias = "OpenPopup")]
    pub fn open_popup_id(&self, id: Id) -> bool {
        self.open_popup_id_with_flags(id, PopupOpenFlags::NONE)
    }

    /// Opens a popup by an ID from the current ID stack, with flags.
    ///
    /// Returns `true` when the popup is toggled open.
    #[doc(alias = "OpenPopup")]
    pub fn open_popup_id_with_flags(&self, id: Id, flags: PopupOpenFlags) -> bool {
        assert!(
            id.raw() != 0,
            "Ui::open_popup_id_with_flags() id must be non-zero"
        );
        validate_popup_open_flags("Ui::open_popup_id_with_flags()", flags);
        self.run_with_bound_context(|| unsafe { sys::igOpenPopup_ID(id.raw(), flags.raw()) })
    }

    /// Opens a popup when the last item is clicked (typically right-click).
    ///
    /// If `str_id` is `None`, the popup is associated with the last item ID.
    /// Returns `true` only when the click opens the popup.
    #[doc(alias = "OpenPopupOnItemClick")]
    pub fn open_popup_on_item_click(&self, str_id: Option<&str>) -> bool {
        self.open_popup_on_item_click_with_flags(str_id, PopupContextOptions::new())
    }

    /// Opens a popup when the last item is clicked, with explicit flags.
    ///
    /// Returns `true` only when the configured click opens the popup.
    #[doc(alias = "OpenPopupOnItemClick")]
    pub fn open_popup_on_item_click_with_flags(
        &self,
        str_id: Option<&str>,
        flags: impl Into<PopupContextOptions>,
    ) -> bool {
        let options = flags.into();
        options.validate("Ui::open_popup_on_item_click_with_flags()");
        self.with_scratch_txt_opt(str_id, |str_id_ptr| unsafe {
            sys::igOpenPopupOnItemClick(str_id_ptr, options.raw())
        })
    }

    /// Construct a popup that can have any kind of content.
    ///
    /// This should be called *per frame*, whereas [`open_popup`](Self::open_popup) should be called *once*
    /// to signal that this popup is active.
    #[doc(alias = "BeginPopup")]
    pub fn begin_popup(&self, str_id: impl AsRef<str>) -> Option<PopupToken<'_>> {
        self.begin_popup_with_flags(str_id, WindowFlags::empty())
    }

    /// Construct a popup with window flags.
    #[doc(alias = "BeginPopup")]
    pub fn begin_popup_with_flags(
        &self,
        str_id: impl AsRef<str>,
        flags: WindowFlags,
    ) -> Option<PopupToken<'_>> {
        validate_window_flags("Ui::begin_popup_with_flags()", flags);
        let render = self.with_scratch_txt(str_id, |str_id_ptr| unsafe {
            sys::igBeginPopup(str_id_ptr, flags.bits())
        });

        if render {
            Some(PopupToken::new(self))
        } else {
            None
        }
    }

    /// Construct a popup that can have any kind of content.
    ///
    /// This should be called *per frame*, whereas [`open_popup`](Self::open_popup) should be called *once*
    /// to signal that this popup is active.
    #[doc(alias = "BeginPopup")]
    pub fn popup<F>(&self, str_id: impl AsRef<str>, f: F)
    where
        F: FnOnce(),
    {
        if let Some(_token) = self.begin_popup(str_id) {
            f();
        }
    }

    /// Creates a modal popup.
    ///
    /// Modal popups block interaction with the rest of the application until closed.
    #[doc(alias = "BeginPopupModal")]
    pub fn begin_modal_popup(&self, name: impl AsRef<str>) -> Option<ModalPopupToken<'_>> {
        let render = self.with_scratch_txt(name, |name_ptr| unsafe {
            sys::igBeginPopupModal(name_ptr, std::ptr::null_mut(), WindowFlags::empty().bits())
        });

        if render {
            Some(ModalPopupToken::new(self))
        } else {
            None
        }
    }

    /// Creates a modal popup with an opened-state tracking variable.
    ///
    /// Passing `opened` enables the title-bar close button (X). When clicked, ImGui will set
    /// `*opened = false` and close the popup.
    ///
    /// Notes:
    /// - You still need to call [`open_popup`](Self::open_popup) once to open the modal.
    /// - To pass window flags, use [`begin_modal_popup_config`](Self::begin_modal_popup_config).
    #[doc(alias = "BeginPopupModal")]
    pub fn begin_modal_popup_with_opened(
        &self,
        name: impl AsRef<str>,
        opened: &mut bool,
    ) -> Option<ModalPopupToken<'_>> {
        let opened_ptr = opened as *mut bool;
        let render = self.with_scratch_txt(name, |name_ptr| unsafe {
            sys::igBeginPopupModal(name_ptr, opened_ptr, WindowFlags::empty().bits())
        });

        if render {
            Some(ModalPopupToken::new(self))
        } else {
            None
        }
    }

    /// Creates a modal popup builder.
    pub fn begin_modal_popup_config<'a>(&'a self, name: &'a str) -> ModalPopup<'a> {
        ModalPopup {
            name,
            opened: None,
            flags: WindowFlags::empty(),
            ui: self,
        }
    }

    /// Creates a modal popup and runs a closure to construct the contents.
    ///
    /// Returns the result of the closure if the popup is open.
    pub fn modal_popup<F, R>(&self, name: impl AsRef<str>, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        let token = self.begin_modal_popup(name)?;
        let result = f();
        drop(token);
        Some(result)
    }

    /// Creates a modal popup with an opened-state tracking variable and runs a closure to
    /// construct the contents.
    ///
    /// Returns the result of the closure if the popup is open.
    pub fn modal_popup_with_opened<F, R>(
        &self,
        name: impl AsRef<str>,
        opened: &mut bool,
        f: F,
    ) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        let token = self.begin_modal_popup_with_opened(name, opened)?;
        let result = f();
        drop(token);
        Some(result)
    }

    /// Closes the current popup.
    #[doc(alias = "CloseCurrentPopup")]
    pub fn close_current_popup(&self) {
        self.run_with_bound_context(|| unsafe {
            sys::igCloseCurrentPopup();
        });
    }

    /// Returns true if the popup is open.
    #[doc(alias = "IsPopupOpen")]
    pub fn is_popup_open(&self, str_id: impl AsRef<str>) -> bool {
        self.with_scratch_txt(str_id, |str_id_ptr| unsafe {
            sys::igIsPopupOpen_Str(str_id_ptr, PopupQueryFlags::NONE.raw())
        })
    }

    /// Returns true if the popup is open with flags.
    #[doc(alias = "IsPopupOpen")]
    pub fn is_popup_open_with_flags(
        &self,
        str_id: impl AsRef<str>,
        flags: PopupQueryFlags,
    ) -> bool {
        validate_popup_query_flags("Ui::is_popup_open_with_flags()", flags);
        self.with_scratch_txt(str_id, |str_id_ptr| unsafe {
            sys::igIsPopupOpen_Str(str_id_ptr, flags.raw())
        })
    }

    /// Begin a popup context menu for the last item.
    #[doc(alias = "BeginPopupContextItem")]
    pub fn begin_popup_context_item(&self) -> Option<PopupToken<'_>> {
        self.begin_popup_context_item_with_flags(None, PopupContextOptions::new())
    }

    /// Begin a popup context menu for the last item with a custom label.
    #[doc(alias = "BeginPopupContextItem")]
    pub fn begin_popup_context_item_with_label(
        &self,
        str_id: Option<&str>,
    ) -> Option<PopupToken<'_>> {
        self.begin_popup_context_item_with_flags(str_id, PopupContextOptions::new())
    }

    /// Begin a popup context menu for the last item with explicit popup flags.
    #[doc(alias = "BeginPopupContextItem")]
    pub fn begin_popup_context_item_with_flags(
        &self,
        str_id: Option<&str>,
        flags: impl Into<PopupContextOptions>,
    ) -> Option<PopupToken<'_>> {
        let options = flags.into();
        options.validate("Ui::begin_popup_context_item_with_flags()");
        let render = self.with_scratch_txt_opt(str_id, |str_id_ptr| unsafe {
            sys::igBeginPopupContextItem(str_id_ptr, options.raw())
        });

        render.then(|| PopupToken::new(self))
    }

    /// Begin a popup context menu for the current window.
    #[doc(alias = "BeginPopupContextWindow")]
    pub fn begin_popup_context_window(&self) -> Option<PopupToken<'_>> {
        self.begin_popup_context_window_with_flags(None, PopupContextOptions::new())
    }

    /// Begin a popup context menu for the current window with a custom label.
    #[doc(alias = "BeginPopupContextWindow")]
    pub fn begin_popup_context_window_with_label(
        &self,
        str_id: Option<&str>,
    ) -> Option<PopupToken<'_>> {
        self.begin_popup_context_window_with_flags(str_id, PopupContextOptions::new())
    }

    /// Begin a popup context menu for the current window with explicit popup flags.
    #[doc(alias = "BeginPopupContextWindow")]
    pub fn begin_popup_context_window_with_flags(
        &self,
        str_id: Option<&str>,
        flags: impl Into<PopupContextOptions>,
    ) -> Option<PopupToken<'_>> {
        let options = flags.into();
        options.validate("Ui::begin_popup_context_window_with_flags()");
        let render = self.with_scratch_txt_opt(str_id, |str_id_ptr| unsafe {
            sys::igBeginPopupContextWindow(str_id_ptr, options.raw())
        });

        render.then(|| PopupToken::new(self))
    }

    /// Begin a popup context menu for empty space (void).
    #[doc(alias = "BeginPopupContextVoid")]
    pub fn begin_popup_context_void(&self) -> Option<PopupToken<'_>> {
        self.begin_popup_context_void_with_flags(None, PopupContextOptions::new())
    }

    /// Begin a popup context menu for empty space with a custom label.
    #[doc(alias = "BeginPopupContextVoid")]
    pub fn begin_popup_context_void_with_label(
        &self,
        str_id: Option<&str>,
    ) -> Option<PopupToken<'_>> {
        self.begin_popup_context_void_with_flags(str_id, PopupContextOptions::new())
    }

    /// Begin a popup context menu for empty space (void) with explicit popup flags.
    #[doc(alias = "BeginPopupContextVoid")]
    pub fn begin_popup_context_void_with_flags(
        &self,
        str_id: Option<&str>,
        flags: impl Into<PopupContextOptions>,
    ) -> Option<PopupToken<'_>> {
        let options = flags.into();
        options.validate("Ui::begin_popup_context_void_with_flags()");
        let render = self.with_scratch_txt_opt(str_id, |str_id_ptr| unsafe {
            sys::igBeginPopupContextVoid(str_id_ptr, options.raw())
        });

        render.then(|| PopupToken::new(self))
    }
}

fn assert_supported_bits(caller: &str, ty: &str, bits: i32, supported: i32) {
    let unsupported = bits & !supported;
    assert!(
        unsupported == 0,
        "{caller} received unsupported {ty} bits: 0x{unsupported:X}"
    );
}

fn validate_popup_open_flags(caller: &str, flags: PopupOpenFlags) {
    assert_supported_bits(
        caller,
        "PopupOpenFlags",
        flags.bits(),
        PopupOpenFlags::all().bits(),
    );
}

fn validate_popup_context_flags(caller: &str, flags: PopupContextFlags) {
    assert_supported_bits(
        caller,
        "PopupContextFlags",
        flags.bits(),
        PopupContextFlags::all().bits(),
    );
}

fn validate_popup_query_flags(caller: &str, flags: PopupQueryFlags) {
    assert_supported_bits(
        caller,
        "PopupQueryFlags",
        flags.bits(),
        PopupQueryFlags::all().bits(),
    );
    assert!(
        !flags.contains(PopupQueryFlags::ANY_POPUP_LEVEL)
            || flags.contains(PopupQueryFlags::ANY_POPUP_ID),
        "{caller} requires ANY_POPUP_ID when using ANY_POPUP_LEVEL with a string id"
    );
}

macro_rules! impl_popup_flag_raw {
    ($ty:ident) => {
        impl $ty {
            #[inline]
            pub(crate) fn raw(self) -> i32 {
                self.bits()
            }
        }
    };
}

bitflags::bitflags! {
    /// Independent flags accepted by `OpenPopup*()` call sites.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupOpenFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None;
        /// Do not reopen the same popup if already open.
        const NO_REOPEN = sys::ImGuiPopupFlags_NoReopen;
        /// Don't open if there's already a popup at the same level of the popup stack.
        const NO_OPEN_OVER_EXISTING_POPUP = sys::ImGuiPopupFlags_NoOpenOverExistingPopup;
    }
}

bitflags::bitflags! {
    /// Independent flags accepted by `OpenPopupOnItemClick()` and
    /// `BeginPopupContext*()` call sites.
    ///
    /// Mouse button selection is a single-choice setting represented by
    /// [`PopupContextMouseButton`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupContextFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None;
        /// Do not reopen the same popup if already open.
        const NO_REOPEN = sys::ImGuiPopupFlags_NoReopen;
        /// Don't open if there's already a popup at the same level of the popup stack.
        const NO_OPEN_OVER_EXISTING_POPUP = sys::ImGuiPopupFlags_NoOpenOverExistingPopup;
        /// For window context helpers: don't return true when hovering items, only when hovering empty space.
        const NO_OPEN_OVER_ITEMS = sys::ImGuiPopupFlags_NoOpenOverItems;
    }
}

bitflags::bitflags! {
    /// Independent flags accepted by `IsPopupOpen()` string-id queries.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupQueryFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None;
        /// Ignore the string id and test for any popup at the current popup stack level.
        const ANY_POPUP_ID = sys::ImGuiPopupFlags_AnyPopupId;
        /// Search/test at any level of the popup stack.
        ///
        /// With a string-id query this must be combined with [`ANY_POPUP_ID`](Self::ANY_POPUP_ID),
        /// matching Dear ImGui's string-id assertion.
        const ANY_POPUP_LEVEL = sys::ImGuiPopupFlags_AnyPopupLevel;
        /// Test for any popup at any popup stack level.
        const ANY_POPUP = sys::ImGuiPopupFlags_AnyPopup;
    }
}

impl_popup_flag_raw!(PopupOpenFlags);
impl_popup_flag_raw!(PopupContextFlags);
impl_popup_flag_raw!(PopupQueryFlags);

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
            Self::Left => sys::ImGuiPopupFlags_MouseButtonLeft,
            Self::Right => sys::ImGuiPopupFlags_MouseButtonRight,
            Self::Middle => sys::ImGuiPopupFlags_MouseButtonMiddle,
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
    fn validate(self, caller: &str) {
        validate_popup_context_flags(caller, self.flags);
        assert!(
            self.flags.bits() & sys::ImGuiPopupFlags_MouseButtonMask_ == 0,
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

/// Builder for a modal popup
#[derive(Debug)]
#[must_use]
pub struct ModalPopup<'ui> {
    name: &'ui str,
    opened: Option<&'ui mut bool>,
    flags: WindowFlags,
    ui: &'ui Ui,
}

impl<'ui> ModalPopup<'ui> {
    /// Sets the opened state tracking variable
    pub fn opened(mut self, opened: &'ui mut bool) -> Self {
        self.opened = Some(opened);
        self
    }

    /// Sets the window flags
    pub fn flags(mut self, flags: WindowFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Begins the modal popup
    pub fn begin(self) -> Option<ModalPopupToken<'ui>> {
        validate_window_flags("ModalPopup::begin()", self.flags);
        let opened_ptr = self
            .opened
            .map(|o| o as *mut bool)
            .unwrap_or(std::ptr::null_mut());

        let render = self.ui.with_scratch_txt(self.name, |name_ptr| unsafe {
            sys::igBeginPopupModal(name_ptr, opened_ptr, self.flags.bits())
        });

        if render {
            Some(ModalPopupToken::new(self.ui))
        } else {
            None
        }
    }
}

/// Tracks a popup that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested window-like scope and in its originating window
/// `Begin` scope. Prefer [`crate::Ui::popup`] for ordinary use.
#[must_use]
#[doc(alias = "EndPopup")]
pub struct PopupToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> PopupToken<'ui> {
    /// Creates a new popup token
    fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndPopup, "PopupToken"),
        }
    }

    /// Ends the popup
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is outside its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for PopupToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

/// Tracks a modal popup that can be ended by calling `.end()` or by dropping.
///
/// The token must finish after every nested window-like scope and in its originating window
/// `Begin` scope. Prefer [`crate::Ui::modal_popup`] for ordinary use.
#[must_use]
pub struct ModalPopupToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> ModalPopupToken<'ui> {
    /// Creates a new modal popup token
    fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndPopup, "ModalPopupToken"),
        }
    }

    /// Ends the modal popup
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested window-like scope is active or this token is outside its
    /// originating window `Begin` scope.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for ModalPopupToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}
