use crate::sys;
use crate::ui::Ui;
use crate::window::WindowFlags;

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
    #[doc(alias = "OpenPopup")]
    pub fn open_popup(&self, str_id: impl AsRef<str>) {
        let str_id_ptr = self.scratch_txt(str_id);
        unsafe {
            sys::ImGui_OpenPopup(str_id_ptr, sys::ImGuiPopupFlags_None);
        }
    }

    /// Instructs ImGui that a popup is open with flags.
    #[doc(alias = "OpenPopup")]
    pub fn open_popup_with_flags(&self, str_id: impl AsRef<str>, flags: PopupFlags) {
        let str_id_ptr = self.scratch_txt(str_id);
        unsafe {
            sys::ImGui_OpenPopup(str_id_ptr, flags.bits());
        }
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
        let str_id_ptr = self.scratch_txt(str_id);
        let render = unsafe { sys::ImGui_BeginPopup(str_id_ptr, flags.bits()) };

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
        let name_ptr = self.scratch_txt(name);
        let render = unsafe {
            sys::ImGui_BeginPopupModal(name_ptr, std::ptr::null_mut(), WindowFlags::empty().bits())
        };

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
        self.begin_modal_popup(name).map(|_token| f())
    }

    /// Closes the current popup.
    #[doc(alias = "CloseCurrentPopup")]
    pub fn close_current_popup(&self) {
        unsafe {
            sys::ImGui_CloseCurrentPopup();
        }
    }

    /// Returns true if the popup is open.
    #[doc(alias = "IsPopupOpen")]
    pub fn is_popup_open(&self, str_id: impl AsRef<str>) -> bool {
        let str_id_ptr = self.scratch_txt(str_id);
        unsafe { sys::ImGui_IsPopupOpen(str_id_ptr, sys::ImGuiPopupFlags_None) }
    }

    /// Returns true if the popup is open with flags.
    #[doc(alias = "IsPopupOpen")]
    pub fn is_popup_open_with_flags(&self, str_id: impl AsRef<str>, flags: PopupFlags) -> bool {
        let str_id_ptr = self.scratch_txt(str_id);
        unsafe { sys::ImGui_IsPopupOpen(str_id_ptr, flags.bits()) }
    }

    /// Begin a popup context menu for the last item.
    /// This is typically used with right-click context menus.
    #[doc(alias = "BeginPopupContextItem")]
    pub fn begin_popup_context_item(&self) -> Option<PopupToken<'_>> {
        self.begin_popup_context_item_with_label(None)
    }

    /// Begin a popup context menu for the last item with a custom label.
    #[doc(alias = "BeginPopupContextItem")]
    pub fn begin_popup_context_item_with_label(
        &self,
        str_id: Option<&str>,
    ) -> Option<PopupToken<'_>> {
        let str_id_ptr = str_id
            .map(|s| self.scratch_txt(s))
            .unwrap_or(std::ptr::null());

        let render = unsafe {
            sys::ImGui_BeginPopupContextItem(str_id_ptr, sys::ImGuiPopupFlags_MouseButtonRight)
        };

        if render {
            Some(PopupToken::new(self))
        } else {
            None
        }
    }

    /// Begin a popup context menu for the current window.
    #[doc(alias = "BeginPopupContextWindow")]
    pub fn begin_popup_context_window(&self) -> Option<PopupToken<'_>> {
        self.begin_popup_context_window_with_label(None)
    }

    /// Begin a popup context menu for the current window with a custom label.
    #[doc(alias = "BeginPopupContextWindow")]
    pub fn begin_popup_context_window_with_label(
        &self,
        str_id: Option<&str>,
    ) -> Option<PopupToken<'_>> {
        let str_id_ptr = str_id
            .map(|s| self.scratch_txt(s))
            .unwrap_or(std::ptr::null());

        let render = unsafe {
            sys::ImGui_BeginPopupContextWindow(str_id_ptr, sys::ImGuiPopupFlags_MouseButtonRight)
        };

        if render {
            Some(PopupToken::new(self))
        } else {
            None
        }
    }

    /// Begin a popup context menu for empty space (void).
    #[doc(alias = "BeginPopupContextVoid")]
    pub fn begin_popup_context_void(&self) -> Option<PopupToken<'_>> {
        self.begin_popup_context_void_with_label(None)
    }

    /// Begin a popup context menu for empty space with a custom label.
    #[doc(alias = "BeginPopupContextVoid")]
    pub fn begin_popup_context_void_with_label(
        &self,
        str_id: Option<&str>,
    ) -> Option<PopupToken<'_>> {
        let str_id_ptr = str_id
            .map(|s| self.scratch_txt(s))
            .unwrap_or(std::ptr::null());

        let render = unsafe {
            sys::ImGui_BeginPopupContextVoid(str_id_ptr, sys::ImGuiPopupFlags_MouseButtonRight)
        };

        if render {
            Some(PopupToken::new(self))
        } else {
            None
        }
    }
}

bitflags::bitflags! {
    /// Flags for popup functions
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None;
        /// For BeginPopupContext*(): open on Left Mouse release. Guaranteed to always be == 0 (same as ImGuiMouseButton_Left)
        const MOUSE_BUTTON_LEFT = sys::ImGuiPopupFlags_MouseButtonLeft;
        /// For BeginPopupContext*(): open on Right Mouse release. Guaranteed to always be == 1 (same as ImGuiMouseButton_Right)
        const MOUSE_BUTTON_RIGHT = sys::ImGuiPopupFlags_MouseButtonRight;
        /// For BeginPopupContext*(): open on Middle Mouse release. Guaranteed to always be == 2 (same as ImGuiMouseButton_Middle)
        const MOUSE_BUTTON_MIDDLE = sys::ImGuiPopupFlags_MouseButtonMiddle;
        /// For OpenPopup*(), BeginPopupContext*(): don't open if there's already a popup at the same level of the popup stack
        const NO_OPEN_OVER_EXISTING_POPUP = sys::ImGuiPopupFlags_NoOpenOverExistingPopup;
        /// For BeginPopupContext*(): don't return true when hovering items, only when hovering empty space
        const NO_OPEN_OVER_ITEMS = sys::ImGuiPopupFlags_NoOpenOverItems;
        /// For IsPopupOpen(): ignore the ImGuiID parameter and test for any popup
        const ANY_POPUP_ID = sys::ImGuiPopupFlags_AnyPopupId;
        /// For IsPopupOpen(): search/test at any level of the popup stack (default test in the current level)
        const ANY_POPUP_LEVEL = sys::ImGuiPopupFlags_AnyPopupLevel;
        /// For IsPopupOpen(): test for any popup
        const ANY_POPUP = Self::ANY_POPUP_ID.bits() | Self::ANY_POPUP_LEVEL.bits();
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
        let name_ptr = self.ui.scratch_txt(self.name);
        let opened_ptr = self
            .opened
            .map(|o| o as *mut bool)
            .unwrap_or(std::ptr::null_mut());

        let render = unsafe { sys::ImGui_BeginPopupModal(name_ptr, opened_ptr, self.flags.bits()) };

        if render {
            Some(ModalPopupToken::new(self.ui))
        } else {
            None
        }
    }
}

/// Tracks a popup that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct PopupToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> PopupToken<'ui> {
    /// Creates a new popup token
    fn new(ui: &'ui Ui) -> Self {
        PopupToken { ui }
    }

    /// Ends the popup
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for PopupToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_EndPopup();
        }
    }
}

/// Tracks a modal popup that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct ModalPopupToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> ModalPopupToken<'ui> {
    /// Creates a new modal popup token
    fn new(ui: &'ui Ui) -> Self {
        ModalPopupToken { ui }
    }

    /// Ends the modal popup
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for ModalPopupToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_EndPopup();
        }
    }
}
