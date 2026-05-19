use crate::sys;
use crate::ui::Ui;
use crate::window::{WindowFlags, validate_window_flags};

use super::context::PopupContextOptions;
use super::flags::{validate_popup_flags, validate_popup_query_flags};
use super::{ModalPopup, ModalPopupToken, PopupFlags, PopupToken};

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
        unsafe { sys::igOpenPopup_Str(str_id_ptr, PopupFlags::NONE.bits()) }
    }

    /// Instructs ImGui that a popup is open with flags.
    #[doc(alias = "OpenPopup")]
    pub fn open_popup_with_flags(&self, str_id: impl AsRef<str>, flags: PopupFlags) {
        validate_popup_flags("Ui::open_popup_with_flags()", flags);
        let str_id_ptr = self.scratch_txt(str_id);
        unsafe { sys::igOpenPopup_Str(str_id_ptr, flags.bits()) }
    }

    /// Opens a popup when the last item is clicked (typically right-click).
    ///
    /// If `str_id` is `None`, the popup is associated with the last item ID.
    #[doc(alias = "OpenPopupOnItemClick")]
    pub fn open_popup_on_item_click(&self, str_id: Option<&str>) {
        self.open_popup_on_item_click_with_flags(str_id, PopupContextOptions::new());
    }

    /// Opens a popup when the last item is clicked, with explicit flags.
    #[doc(alias = "OpenPopupOnItemClick")]
    pub fn open_popup_on_item_click_with_flags(
        &self,
        str_id: Option<&str>,
        flags: impl Into<PopupContextOptions>,
    ) {
        let options = flags.into();
        options.validate("Ui::open_popup_on_item_click_with_flags()");
        let str_id_ptr = str_id
            .map(|s| self.scratch_txt(s))
            .unwrap_or(std::ptr::null());
        unsafe { sys::igOpenPopupOnItemClick(str_id_ptr, options.raw()) }
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
        let str_id_ptr = self.scratch_txt(str_id);
        let render = unsafe { sys::igBeginPopup(str_id_ptr, flags.bits()) };

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
            sys::igBeginPopupModal(name_ptr, std::ptr::null_mut(), WindowFlags::empty().bits())
        };

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
        let name_ptr = self.scratch_txt(name);
        let opened_ptr = opened as *mut bool;
        let render =
            unsafe { sys::igBeginPopupModal(name_ptr, opened_ptr, WindowFlags::empty().bits()) };

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
        self.begin_modal_popup_with_opened(name, opened)
            .map(|_token| f())
    }

    /// Closes the current popup.
    #[doc(alias = "CloseCurrentPopup")]
    pub fn close_current_popup(&self) {
        unsafe {
            sys::igCloseCurrentPopup();
        }
    }

    /// Returns true if the popup is open.
    #[doc(alias = "IsPopupOpen")]
    pub fn is_popup_open(&self, str_id: impl AsRef<str>) -> bool {
        let str_id_ptr = self.scratch_txt(str_id);
        unsafe { sys::igIsPopupOpen_Str(str_id_ptr, PopupFlags::NONE.bits()) }
    }

    /// Returns true if the popup is open with flags.
    #[doc(alias = "IsPopupOpen")]
    pub fn is_popup_open_with_flags(&self, str_id: impl AsRef<str>, flags: PopupFlags) -> bool {
        validate_popup_query_flags("Ui::is_popup_open_with_flags()", flags);
        let str_id_ptr = self.scratch_txt(str_id);
        unsafe { sys::igIsPopupOpen_Str(str_id_ptr, flags.bits()) }
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
        let str_id_ptr = str_id
            .map(|s| self.scratch_txt(s))
            .unwrap_or(std::ptr::null());

        let render = unsafe { sys::igBeginPopupContextItem(str_id_ptr, options.raw()) };

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
        let str_id_ptr = str_id
            .map(|s| self.scratch_txt(s))
            .unwrap_or(std::ptr::null());

        let render = unsafe { sys::igBeginPopupContextWindow(str_id_ptr, options.raw()) };

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
        let str_id_ptr = str_id
            .map(|s| self.scratch_txt(s))
            .unwrap_or(std::ptr::null());

        let render = unsafe { sys::igBeginPopupContextVoid(str_id_ptr, options.raw()) };

        render.then(|| PopupToken::new(self))
    }
}
