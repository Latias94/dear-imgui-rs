use bitflags::bitflags;

use crate::{Ui, sys};

bitflags! {
    /// Flags that can be applied to subsequently submitted items.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemFlags: i32 {
        /// No item flags.
        const NONE = sys::ImGuiItemFlags_None as i32;
        /// Disable keyboard tabbing while retaining directional navigation.
        const NO_TAB_STOP = sys::ImGuiItemFlags_NoTabStop as i32;
        /// Disable keyboard and gamepad navigation.
        const NO_NAV = sys::ImGuiItemFlags_NoNav as i32;
        /// Prevent the item from receiving default navigation focus.
        const NO_NAV_DEFAULT_FOCUS = sys::ImGuiItemFlags_NoNavDefaultFocus as i32;
        /// Enable repeat behavior for button-like items.
        const BUTTON_REPEAT = sys::ImGuiItemFlags_ButtonRepeat as i32;
        /// Automatically close a parent popup after activating a menu item or selectable.
        const AUTO_CLOSE_POPUPS = sys::ImGuiItemFlags_AutoClosePopups as i32;
        /// Allow duplicate item IDs without a debug conflict warning.
        const ALLOW_DUPLICATE_ID = sys::ImGuiItemFlags_AllowDuplicateId as i32;
    }
}

bitflags! {
    /// Flags recorded for the last submitted item.
    ///
    /// This includes the public flags accepted by [`Ui::push_item_flag`] plus
    /// read-only state such as [`Self::DISABLED`]. Unknown internal bits are
    /// retained when returned by [`Ui::item_flags`].
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemStateFlags: i32 {
        /// No item flags.
        const NONE = sys::ImGuiItemFlags_None as i32;
        /// Keyboard tabbing was disabled while retaining directional navigation.
        const NO_TAB_STOP = sys::ImGuiItemFlags_NoTabStop as i32;
        /// Keyboard and gamepad navigation was disabled.
        const NO_NAV = sys::ImGuiItemFlags_NoNav as i32;
        /// The item could not receive default navigation focus.
        const NO_NAV_DEFAULT_FOCUS = sys::ImGuiItemFlags_NoNavDefaultFocus as i32;
        /// Repeat behavior was enabled for the item.
        const BUTTON_REPEAT = sys::ImGuiItemFlags_ButtonRepeat as i32;
        /// The item could automatically close its parent popup.
        const AUTO_CLOSE_POPUPS = sys::ImGuiItemFlags_AutoClosePopups as i32;
        /// Duplicate IDs were allowed for the item.
        const ALLOW_DUPLICATE_ID = sys::ImGuiItemFlags_AllowDuplicateId as i32;
        /// The last item was disabled.
        const DISABLED = sys::ImGuiItemFlags_Disabled as i32;
    }
}

impl Default for ItemFlags {
    fn default() -> Self {
        Self::NONE
    }
}

impl Default for ItemStateFlags {
    fn default() -> Self {
        Self::NONE
    }
}

impl From<ItemFlags> for ItemStateFlags {
    fn from(flags: ItemFlags) -> Self {
        Self::from_bits_retain(flags.bits())
    }
}

create_token!(
    /// Tracks item flags pushed with [`Ui::push_item_flag`].
    pub struct ItemFlagStackToken<'ui>;

    /// Pops item flags pushed with [`Ui::push_item_flag`].
    #[doc(alias = "PopItemFlag")]
    drop { unsafe { sys::igPopItemFlag() } }
);

impl ItemFlagStackToken<'_> {
    /// Pops the item flag scope.
    pub fn pop(self) {
        self.end()
    }
}

impl Ui {
    /// Returns the flags recorded for the last submitted item.
    ///
    /// Unknown bits introduced by newer Dear ImGui versions are retained.
    #[doc(alias = "GetItemFlags")]
    pub fn item_flags(&self) -> ItemStateFlags {
        self.run_with_bound_context(|| unsafe {
            ItemStateFlags::from_bits_retain(sys::igGetItemFlags())
        })
    }

    /// Enables or disables flags for subsequently submitted items.
    ///
    /// The returned token restores the previous item flags when dropped.
    #[doc(alias = "PushItemFlag")]
    pub fn push_item_flag(&self, flags: ItemFlags, enabled: bool) -> ItemFlagStackToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushItemFlag(flags.bits(), enabled) });
        ItemFlagStackToken::new(self)
    }

    /// Runs `f` with the requested item flags enabled or disabled.
    ///
    /// The previous flags are restored even if `f` panics.
    #[doc(alias = "PushItemFlag", alias = "PopItemFlag")]
    pub fn with_item_flag<R>(&self, flags: ItemFlags, enabled: bool, f: impl FnOnce() -> R) -> R {
        let _flags = self.push_item_flag(flags, enabled);
        f()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.font_atlas().build();
        ctx
    }

    #[test]
    fn item_flag_scope_is_typed_and_restores_previous_flags() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("item_flags").build(|| {
            ui.with_item_flag(
                ItemFlags::NO_NAV | ItemFlags::ALLOW_DUPLICATE_ID,
                true,
                || {
                    ui.button("scoped");
                    let flags = ui.item_flags();
                    assert!(flags.contains(ItemStateFlags::NO_NAV));
                    assert!(flags.contains(ItemStateFlags::ALLOW_DUPLICATE_ID));
                },
            );

            ui.button("restored");
            let flags = ui.item_flags();
            assert!(!flags.contains(ItemStateFlags::NO_NAV));
            assert!(!flags.contains(ItemStateFlags::ALLOW_DUPLICATE_ID));
        });
    }

    #[test]
    fn item_flag_scope_can_clear_defaults_and_reports_disabled_items() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("item_flag_values").build(|| {
            ui.button("default");
            assert!(ui.item_flags().contains(ItemStateFlags::AUTO_CLOSE_POPUPS));

            {
                let _flags = ui.push_item_flag(ItemFlags::AUTO_CLOSE_POPUPS, false);
                ui.button("without_auto_close");
                assert!(!ui.item_flags().contains(ItemStateFlags::AUTO_CLOSE_POPUPS));
            }

            ui.button("restored_default");
            assert!(ui.item_flags().contains(ItemStateFlags::AUTO_CLOSE_POPUPS));

            {
                let _disabled = ui.begin_disabled();
                ui.button("disabled");
                assert!(ui.item_flags().contains(ItemStateFlags::DISABLED));
            }
        });
    }
}
