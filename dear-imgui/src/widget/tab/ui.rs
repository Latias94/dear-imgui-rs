use std::ptr;

use crate::sys;
use crate::ui::Ui;

use super::{TabBarFlags, TabBarOptions, TabBarToken, TabItemOptions, TabItemToken};

/// # Tab Widgets
impl Ui {
    /// Creates a tab bar and returns a tab bar token, allowing you to append
    /// Tab items afterwards. This passes no flags. To pass flags explicitly,
    /// use [tab_bar_with_flags](Self::tab_bar_with_flags).
    #[doc(alias = "BeginTabBar")]
    pub fn tab_bar(&self, id: impl AsRef<str>) -> Option<TabBarToken<'_>> {
        self.tab_bar_with_flags(id, TabBarFlags::NONE)
    }

    /// Creates a tab bar and returns a tab bar token, allowing you to append
    /// Tab items afterwards.
    #[doc(alias = "BeginTabBar")]
    pub fn tab_bar_with_flags(
        &self,
        id: impl AsRef<str>,
        flags: impl Into<TabBarOptions>,
    ) -> Option<TabBarToken<'_>> {
        let options = flags.into();
        options.validate("Ui::tab_bar_with_flags()");
        let id_ptr = self.scratch_txt(id);
        let should_render =
            self.run_with_bound_context(|| unsafe { sys::igBeginTabBar(id_ptr, options.raw()) });

        if should_render {
            Some(TabBarToken::new(self))
        } else {
            None
        }
    }

    /// Creates a new tab item and returns a token if its contents are visible.
    ///
    /// By default, this doesn't pass an opened bool nor any flags. See [tab_item_with_opened]
    /// and [tab_item_with_flags] for more.
    ///
    /// [tab_item_with_opened]: Self::tab_item_with_opened
    /// [tab_item_with_flags]: Self::tab_item_with_flags
    #[doc(alias = "BeginTabItem")]
    pub fn tab_item(&self, label: impl AsRef<str>) -> Option<TabItemToken<'_>> {
        self.tab_item_with_flags(label, None, TabItemOptions::new())
    }

    /// Creates a new tab item and returns a token if its contents are visible.
    ///
    /// By default, this doesn't pass any flags. See [tab_item_with_flags] for more.
    #[doc(alias = "BeginTabItem")]
    pub fn tab_item_with_opened(
        &self,
        label: impl AsRef<str>,
        opened: &mut bool,
    ) -> Option<TabItemToken<'_>> {
        self.tab_item_with_flags(label, Some(opened), TabItemOptions::new())
    }

    /// Creates a new tab item and returns a token if its contents are visible.
    #[doc(alias = "BeginTabItem")]
    pub fn tab_item_with_flags(
        &self,
        label: impl AsRef<str>,
        opened: Option<&mut bool>,
        flags: impl Into<TabItemOptions>,
    ) -> Option<TabItemToken<'_>> {
        let options = flags.into();
        options.validate_for_tab_item("Ui::tab_item_with_flags()");
        let label_ptr = self.scratch_txt(label);
        let opened_ptr = opened.map(|x| x as *mut bool).unwrap_or(ptr::null_mut());

        let should_render = self.run_with_bound_context(|| unsafe {
            sys::igBeginTabItem(label_ptr, opened_ptr, options.raw())
        });

        if should_render {
            Some(TabItemToken::new(self))
        } else {
            None
        }
    }

    /// Creates a button on the current tab bar (e.g. to append a `+` new-tab button).
    #[doc(alias = "TabItemButton")]
    pub fn tab_item_button(&self, label: impl AsRef<str>) -> bool {
        self.tab_item_button_with_flags(label, TabItemOptions::new())
    }

    /// Creates a button on the current tab bar with explicit flags.
    #[doc(alias = "TabItemButton")]
    pub fn tab_item_button_with_flags(
        &self,
        label: impl AsRef<str>,
        flags: impl Into<TabItemOptions>,
    ) -> bool {
        let options = flags.into();
        options.validate_for_tab_button("Ui::tab_item_button_with_flags()");
        let label_ptr = self.scratch_txt(label);
        self.run_with_bound_context(|| unsafe { sys::igTabItemButton(label_ptr, options.raw()) })
    }

    /// Notifies Dear ImGui that a tab (or docked window) has been closed.
    #[doc(alias = "SetTabItemClosed")]
    pub fn set_tab_item_closed(&self, tab_or_docked_window_label: impl AsRef<str>) {
        let label_ptr = self.scratch_txt(tab_or_docked_window_label);
        self.run_with_bound_context(|| unsafe { sys::igSetTabItemClosed(label_ptr) });
    }
}
