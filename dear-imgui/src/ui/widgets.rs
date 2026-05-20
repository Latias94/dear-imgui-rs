use super::*;

impl Ui {
    /// Display text
    #[doc(alias = "TextUnformatted")]
    pub fn text<T: AsRef<str>>(&self, text: T) {
        let s = text.as_ref();
        unsafe {
            let start = s.as_ptr();
            let end = start.add(s.len());
            crate::sys::igTextUnformatted(
                start as *const std::os::raw::c_char,
                end as *const std::os::raw::c_char,
            );
        }
    }

    /// Convenience: draw an image with background and tint (ImGui 1.92+)
    ///
    /// Equivalent to using `image_config(...).build_with_bg(bg, tint)` but in one call.
    #[doc(alias = "ImageWithBg")]
    pub fn image_with_bg<'tex>(
        &self,
        texture: impl Into<TextureRef<'tex>>,
        size: [f32; 2],
        bg_color: [f32; 4],
        tint_color: [f32; 4],
    ) {
        crate::widget::image::Image::new(self, texture, size).build_with_bg(bg_color, tint_color)
    }

    // Drag widgets

    /// Creates a drag float slider
    #[doc(alias = "DragFloat")]
    pub fn drag_float(&self, label: impl AsRef<str>, value: &mut f32) -> bool {
        crate::widget::drag::Drag::new(label).build(self, value)
    }

    /// Creates a drag float slider with configuration
    #[doc(alias = "DragFloat")]
    pub fn drag_float_config<L: AsRef<str>>(&self, label: L) -> crate::widget::drag::Drag<f32, L> {
        crate::widget::drag::Drag::new(label)
    }

    /// Creates a drag int slider
    #[doc(alias = "DragInt")]
    pub fn drag_int(&self, label: impl AsRef<str>, value: &mut i32) -> bool {
        crate::widget::drag::Drag::new(label).build(self, value)
    }

    /// Creates a drag int slider with configuration
    #[doc(alias = "DragInt")]
    pub fn drag_int_config<L: AsRef<str>>(&self, label: L) -> crate::widget::drag::Drag<i32, L> {
        crate::widget::drag::Drag::new(label)
    }

    /// Creates a drag float range slider
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2(&self, label: impl AsRef<str>, min: &mut f32, max: &mut f32) -> bool {
        crate::widget::drag::DragRange::<f32, _>::new(label).build(self, min, max)
    }

    /// Creates a drag float range slider with configuration
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2_config<L: AsRef<str>>(
        &self,
        label: L,
    ) -> crate::widget::drag::DragRange<f32, L> {
        crate::widget::drag::DragRange::new(label)
    }

    /// Creates a drag int range slider
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2(&self, label: impl AsRef<str>, min: &mut i32, max: &mut i32) -> bool {
        crate::widget::drag::DragRange::<i32, _>::new(label).build(self, min, max)
    }

    /// Creates a drag int range slider with configuration
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2_config<L: AsRef<str>>(
        &self,
        label: L,
    ) -> crate::widget::drag::DragRange<i32, L> {
        crate::widget::drag::DragRange::new(label)
    }

    /// Set next item to be open by default.
    ///
    /// This is useful for tree nodes, collapsing headers, etc.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open(&self, is_open: bool) {
        unsafe {
            sys::igSetNextItemOpen(is_open, 0); // 0 = ImGuiCond_Always
        }
    }

    /// Set next item to be open by default with condition.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open_with_cond(&self, is_open: bool, cond: crate::Condition) {
        unsafe { sys::igSetNextItemOpen(is_open, cond as sys::ImGuiCond) }
    }

    /// Set next item width.
    ///
    /// Set to 0.0 for default width, >0.0 for explicit width, <0.0 for relative width.
    #[doc(alias = "SetNextItemWidth")]
    pub fn set_next_item_width(&self, item_width: f32) {
        Self::assert_finite_f32("Ui::set_next_item_width()", "item_width", item_width);
        unsafe {
            sys::igSetNextItemWidth(item_width);
        }
    }

    /// Display a text label with a boolean value (for quick debug UIs).
    #[doc(alias = "Value")]
    pub fn value_bool(&self, prefix: impl AsRef<str>, v: bool) {
        unsafe { sys::igValue_Bool(self.scratch_txt(prefix), v) }
    }
}
