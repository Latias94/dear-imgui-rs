use crate::sys;
use crate::ui::Ui;

impl Ui {
    /// Creates a menu item.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item(&self, label: impl AsRef<str>) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igMenuItemEx(label_ptr, std::ptr::null(), std::ptr::null(), false, true) }
    }

    /// Creates a menu item with a shortcut.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_with_shortcut(
        &self,
        label: impl AsRef<str>,
        shortcut: impl AsRef<str>,
    ) -> bool {
        let (label_ptr, shortcut_ptr) = self.scratch_txt_two(label, shortcut);
        unsafe { sys::igMenuItemEx(label_ptr, std::ptr::null(), shortcut_ptr, false, true) }
    }

    /// Creates a menu item with explicit enabled/selected state.
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_enabled_selected(
        &self,
        label: impl AsRef<str>,
        shortcut: Option<impl AsRef<str>>,
        selected: bool,
        enabled: bool,
    ) -> bool {
        let label = label.as_ref();
        match shortcut {
            Some(shortcut) => {
                let (label_ptr, shortcut_ptr) = self.scratch_txt_two(label, shortcut.as_ref());
                unsafe { sys::igMenuItem_Bool(label_ptr, shortcut_ptr, selected, enabled) }
            }
            None => {
                let label_ptr = self.scratch_txt(label);
                unsafe { sys::igMenuItem_Bool(label_ptr, std::ptr::null(), selected, enabled) }
            }
        }
    }

    /// Creates a menu item with explicit enabled/selected state (no shortcut).
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_enabled_selected_no_shortcut(
        &self,
        label: impl AsRef<str>,
        selected: bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_enabled_selected(label, None::<&str>, selected, enabled)
    }

    /// Creates a menu item with explicit enabled/selected state and a shortcut.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_enabled_selected_with_shortcut(
        &self,
        label: impl AsRef<str>,
        shortcut: impl AsRef<str>,
        selected: bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_enabled_selected(label, Some(shortcut), selected, enabled)
    }

    /// Creates a toggleable menu item bound to `selected` (updated in place).
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_toggle(
        &self,
        label: impl AsRef<str>,
        shortcut: Option<impl AsRef<str>>,
        selected: &mut bool,
        enabled: bool,
    ) -> bool {
        let label = label.as_ref();
        match shortcut {
            Some(shortcut) => {
                let (label_ptr, shortcut_ptr) = self.scratch_txt_two(label, shortcut.as_ref());
                unsafe { sys::igMenuItem_BoolPtr(label_ptr, shortcut_ptr, selected, enabled) }
            }
            None => {
                let label_ptr = self.scratch_txt(label);
                unsafe { sys::igMenuItem_BoolPtr(label_ptr, std::ptr::null(), selected, enabled) }
            }
        }
    }

    /// Creates a toggleable menu item bound to `selected` (no shortcut).
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_toggle_no_shortcut(
        &self,
        label: impl AsRef<str>,
        selected: &mut bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_toggle(label, None::<&str>, selected, enabled)
    }

    /// Creates a toggleable menu item bound to `selected` with a shortcut.
    ///
    /// Returns true if the menu item is activated.
    #[doc(alias = "MenuItem")]
    pub fn menu_item_toggle_with_shortcut(
        &self,
        label: impl AsRef<str>,
        shortcut: impl AsRef<str>,
        selected: &mut bool,
        enabled: bool,
    ) -> bool {
        self.menu_item_toggle(label, Some(shortcut), selected, enabled)
    }
}
