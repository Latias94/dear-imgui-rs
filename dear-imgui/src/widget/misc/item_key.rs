use crate::Ui;
use crate::sys;

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
