use std::borrow::Cow;

use crate::sys;
use crate::ui::Ui;
use crate::widget::ComboBoxFlags;

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
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    #[doc(alias = "BeginCombo")]
    pub fn begin_combo_with_flags(
        &self,
        label: impl AsRef<str>,
        preview_value: impl AsRef<str>,
        flags: ComboBoxFlags,
    ) -> Option<ComboBoxToken<'_>> {
        let label_ptr = self.scratch_txt(label);
        let preview_ptr = self.scratch_txt(preview_value);

        let should_render = unsafe { sys::ImGui_BeginCombo(label_ptr, preview_ptr, flags.bits()) };

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
        flags: ComboBoxFlags,
    ) -> Option<ComboBoxToken<'_>> {
        let label_ptr = self.scratch_txt(label);

        let should_render =
            unsafe { sys::ImGui_BeginCombo(label_ptr, std::ptr::null(), flags.bits()) };

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

    /// Sets the default focus for the next item
    pub fn set_item_default_focus(&self) {
        unsafe {
            sys::ImGui_SetItemDefaultFocus();
        }
    }
}

/// Builder for a combo box widget
#[derive(Clone, Debug)]
#[must_use]
pub struct ComboBox<'ui, Label, Preview = &'static str> {
    pub label: Label,
    pub preview_value: Option<Preview>,
    pub flags: ComboBoxFlags,
    pub ui: &'ui Ui,
}

impl<'ui, Label: AsRef<str>> ComboBox<'ui, Label> {
    /// Sets the preview value
    pub fn preview_value<P: AsRef<str>>(self, preview: P) -> ComboBox<'ui, Label, P> {
        ComboBox {
            label: self.label,
            preview_value: Some(preview),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Sets the flags
    pub fn flags(mut self, flags: ComboBoxFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Creates a combo box and starts appending to it.
    ///
    /// Returns `Some(ComboBoxToken)` if the combo box is open. After content has been
    /// rendered, the token must be ended by calling `.end()`.
    ///
    /// Returns `None` if the combo box is not open and no content should be rendered.
    #[must_use]
    pub fn begin(self) -> Option<ComboBoxToken<'ui>> {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let preview_ptr = self
            .preview_value
            .as_ref()
            .map(|p| self.ui.scratch_txt(p))
            .unwrap_or(std::ptr::null());

        let should_render =
            unsafe { sys::ImGui_BeginCombo(label_ptr, preview_ptr, self.flags.bits()) };

        if should_render {
            Some(ComboBoxToken::new(self.ui))
        } else {
            None
        }
    }
}

/// Tracks a combo box that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct ComboBoxToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> ComboBoxToken<'ui> {
    /// Creates a new combo box token
    fn new(ui: &'ui Ui) -> Self {
        ComboBoxToken { ui }
    }

    /// Ends the combo box
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for ComboBoxToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_EndCombo();
        }
    }
}
