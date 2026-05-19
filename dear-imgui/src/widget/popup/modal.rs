use crate::sys;
use crate::ui::Ui;
use crate::window::{WindowFlags, validate_window_flags};

use super::ModalPopupToken;

/// Builder for a modal popup
#[derive(Debug)]
#[must_use]
pub struct ModalPopup<'ui> {
    pub(super) name: &'ui str,
    pub(super) opened: Option<&'ui mut bool>,
    pub(super) flags: WindowFlags,
    pub(super) ui: &'ui Ui,
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
        let name_ptr = self.ui.scratch_txt(self.name);
        let opened_ptr = self
            .opened
            .map(|o| o as *mut bool)
            .unwrap_or(std::ptr::null_mut());

        let render = unsafe { sys::igBeginPopupModal(name_ptr, opened_ptr, self.flags.bits()) };

        if render {
            Some(ModalPopupToken::new(self.ui))
        } else {
            None
        }
    }
}
