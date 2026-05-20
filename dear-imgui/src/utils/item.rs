use crate::sys;

impl crate::ui::Ui {
    /// Returns `true` if the last item open state was toggled
    #[doc(alias = "IsItemToggledOpen")]
    pub fn is_item_toggled_open(&self) -> bool {
        unsafe { sys::igIsItemToggledOpen() }
    }

    /// Returns the upper-left bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMin")]
    pub fn item_rect_min(&self) -> [f32; 2] {
        let rect = unsafe { sys::igGetItemRectMin() };
        [rect.x, rect.y]
    }

    /// Returns the lower-right bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMax")]
    pub fn item_rect_max(&self) -> [f32; 2] {
        let rect = unsafe { sys::igGetItemRectMax() };
        [rect.x, rect.y]
    }

    /// Allows the next item to be overlapped by a subsequent item.
    #[doc(alias = "SetNextItemAllowOverlap")]
    pub fn set_next_item_allow_overlap(&self) {
        unsafe { sys::igSetNextItemAllowOverlap() };
    }
}
