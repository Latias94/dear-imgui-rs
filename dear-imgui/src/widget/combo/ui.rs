use std::borrow::Cow;

use crate::sys;
use crate::ui::Ui;

use super::{ComboBoxFlags, ComboBoxOptions, ComboBoxPreviewMode, ComboBoxToken};

/// # Combo Box Widgets
impl Ui {
    /// Creates a combo box and starts appending to it.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo(
        &self,
        label: impl AsRef<str>,
        preview_value: impl AsRef<str>,
    ) -> Option<ComboBoxToken<'_>> {
        self.begin_combo_with_flags(label, preview_value, ComboBoxFlags::NONE)
    }

    /// Creates a combo box with flags and starts appending to it.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo_with_flags(
        &self,
        label: impl AsRef<str>,
        preview_value: impl AsRef<str>,
        flags: impl Into<ComboBoxOptions>,
    ) -> Option<ComboBoxToken<'_>> {
        let options = flags.into();
        options.validate("Ui::begin_combo_with_flags()");
        let (label_ptr, preview_ptr) = self.scratch_txt_two(label, preview_value);

        let should_render = unsafe { sys::igBeginCombo(label_ptr, preview_ptr, options.raw()) };

        if should_render {
            Some(ComboBoxToken::new(self))
        } else {
            None
        }
    }

    /// Creates a combo box without preview value.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo_no_preview(&self, label: impl AsRef<str>) -> Option<ComboBoxToken<'_>> {
        self.begin_combo_no_preview_with_flags(label, ComboBoxFlags::NONE)
    }

    /// Creates a combo box without preview value and with flags.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo_no_preview_with_flags(
        &self,
        label: impl AsRef<str>,
        flags: impl Into<ComboBoxOptions>,
    ) -> Option<ComboBoxToken<'_>> {
        let mut options = flags.into();
        options.preview_mode = ComboBoxPreviewMode::NoPreview;
        options.validate("Ui::begin_combo_no_preview_with_flags()");
        let label_ptr = self.scratch_txt(label);

        let should_render =
            unsafe { sys::igBeginCombo(label_ptr, std::ptr::null(), options.raw()) };

        if should_render {
            Some(ComboBoxToken::new(self))
        } else {
            None
        }
    }

    /// Builds a simple combo box for choosing from a slice of values.
    #[doc(alias = "Combo")]
    pub fn combo<V, L>(
        &self,
        label: impl AsRef<str>,
        current_item: &mut usize,
        items: &[V],
        label_fn: L,
    ) -> bool
    where
        for<'b> L: Fn(&'b V) -> Cow<'b, str>,
    {
        let label_fn = &label_fn;
        let mut result = false;
        let preview_value = items.get(*current_item).map(label_fn);

        if let Some(combo_token) = self.begin_combo(
            label,
            preview_value.as_ref().map(|s| s.as_ref()).unwrap_or(""),
        ) {
            for (idx, item) in items.iter().enumerate() {
                let is_selected = idx == *current_item;
                if is_selected {
                    self.set_item_default_focus();
                }

                let clicked = self.selectable(label_fn(item).as_ref());

                if clicked {
                    *current_item = idx;
                    result = true;
                }
            }
            combo_token.end();
        }

        result
    }

    /// Builds a simple combo box using an `i32` index (ImGui-style).
    ///
    /// This is useful when you want to represent \"no selection\" with `-1`, matching Dear ImGui's
    /// `Combo()` API.
    #[doc(alias = "Combo")]
    pub fn combo_i32<V, L>(
        &self,
        label: impl AsRef<str>,
        current_item: &mut i32,
        items: &[V],
        label_fn: L,
    ) -> bool
    where
        for<'b> L: Fn(&'b V) -> Cow<'b, str>,
    {
        let label_fn = &label_fn;
        let mut result = false;

        let preview_value = if *current_item >= 0 {
            items.get(*current_item as usize).map(|v| label_fn(v))
        } else {
            None
        };

        if let Some(combo_token) = self.begin_combo(
            label,
            preview_value.as_ref().map(|s| s.as_ref()).unwrap_or(""),
        ) {
            for (idx, item) in items.iter().enumerate() {
                if idx > i32::MAX as usize {
                    break;
                }
                let idx_i32 = idx as i32;
                let is_selected = idx_i32 == *current_item;
                if is_selected {
                    self.set_item_default_focus();
                }

                let clicked = self.selectable(label_fn(item).as_ref());
                if clicked {
                    *current_item = idx_i32;
                    result = true;
                }
            }
            combo_token.end();
        }

        result
    }

    /// Builds a simple combo box for choosing from a slice of strings
    #[doc(alias = "Combo")]
    pub fn combo_simple_string(
        &self,
        label: impl AsRef<str>,
        current_item: &mut usize,
        items: &[impl AsRef<str>],
    ) -> bool {
        self.combo(label, current_item, items, |s| Cow::Borrowed(s.as_ref()))
    }

    /// Builds a simple combo box for choosing from a slice of strings using an `i32` index.
    #[doc(alias = "Combo")]
    pub fn combo_simple_string_i32(
        &self,
        label: impl AsRef<str>,
        current_item: &mut i32,
        items: &[impl AsRef<str>],
    ) -> bool {
        self.combo_i32(label, current_item, items, |s| Cow::Borrowed(s.as_ref()))
    }

    /// Sets the default focus for the next item
    pub fn set_item_default_focus(&self) {
        unsafe {
            sys::igSetItemDefaultFocus();
        }
    }
}
