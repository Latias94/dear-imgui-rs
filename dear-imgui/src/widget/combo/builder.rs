use crate::sys;
use crate::ui::Ui;

use super::{ComboBoxFlags, ComboBoxHeight, ComboBoxOptions, ComboBoxPreviewMode, ComboBoxToken};

/// Builder for a combo box widget
#[derive(Clone, Debug)]
#[must_use]
pub struct ComboBox<'ui, Label, Preview = &'static str> {
    pub label: Label,
    pub preview_value: Option<Preview>,
    pub options: ComboBoxOptions,
    pub ui: &'ui Ui,
}

impl<'ui, Label: AsRef<str>> ComboBox<'ui, Label> {
    /// Sets the preview value
    pub fn preview_value<P: AsRef<str>>(self, preview: P) -> ComboBox<'ui, Label, P> {
        ComboBox {
            label: self.label,
            preview_value: Some(preview),
            options: self.options,
            ui: self.ui,
        }
    }

    /// Sets the flags
    pub fn flags(mut self, flags: ComboBoxFlags) -> Self {
        self.options.flags = flags;
        self
    }

    /// Sets the popup height policy.
    pub fn height(mut self, height: ComboBoxHeight) -> Self {
        self.options.height = Some(height);
        self
    }

    /// Sets the preview/arrow layout.
    pub fn preview_mode(mut self, mode: ComboBoxPreviewMode) -> Self {
        self.options.preview_mode = mode;
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
        self.options.validate("ComboBox::begin()");
        let (label_ptr, preview_ptr) = self
            .ui
            .scratch_txt_with_opt(self.label.as_ref(), self.preview_value.as_deref());

        let should_render = self.ui.run_with_bound_context(|| unsafe {
            sys::igBeginCombo(label_ptr, preview_ptr, self.options.raw())
        });

        if should_render {
            Some(ComboBoxToken::new(self.ui))
        } else {
            None
        }
    }
}
