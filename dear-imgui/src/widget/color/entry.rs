use super::button::ColorButton;
use super::edit::{ColorEdit3, ColorEdit4};
use super::flags::ColorEditOptions;
use super::picker::{ColorPicker3, ColorPicker4};
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

/// # Color Edit Widgets
impl Ui {
    /// Initializes default color editing/picking options.
    ///
    /// This configures the defaults used by the various `Color*` widgets (unless
    /// overridden per-call via flags). Users can still change many options via
    /// the right-click context menu unless `_NO_OPTIONS` is passed.
    #[doc(alias = "SetColorEditOptions")]
    pub fn set_color_edit_options(&self, options: impl Into<ColorEditOptions>) {
        let options = options.into();
        options.validate("Ui::set_color_edit_options()");
        unsafe { sys::igSetColorEditOptions(options.bits() as i32) }
    }

    /// Creates a color edit widget for 3 components (RGB)
    #[doc(alias = "ColorEdit3")]
    pub fn color_edit3(&self, label: impl AsRef<str>, color: &mut [f32; 3]) -> bool {
        self.color_edit3_config(label.as_ref(), color).build()
    }

    /// Creates a color edit widget for 4 components (RGBA)
    #[doc(alias = "ColorEdit4")]
    pub fn color_edit4(&self, label: impl AsRef<str>, color: &mut [f32; 4]) -> bool {
        self.color_edit4_config(label.as_ref(), color).build()
    }

    /// Creates a color picker widget for 3 components (RGB)
    #[doc(alias = "ColorPicker3")]
    pub fn color_picker3(&self, label: impl AsRef<str>, color: &mut [f32; 3]) -> bool {
        self.color_picker3_config(label.as_ref(), color).build()
    }

    /// Creates a color picker widget for 4 components (RGBA)
    #[doc(alias = "ColorPicker4")]
    pub fn color_picker4(&self, label: impl AsRef<str>, color: &mut [f32; 4]) -> bool {
        self.color_picker4_config(label.as_ref(), color).build()
    }

    /// Creates a color button widget
    #[doc(alias = "ColorButton")]
    pub fn color_button(&self, desc_id: impl AsRef<str>, color: [f32; 4]) -> bool {
        self.color_button_config(desc_id.as_ref(), color).build()
    }

    /// Creates a color edit builder for 3 components
    pub fn color_edit3_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 3],
    ) -> ColorEdit3<'ui, 'p> {
        ColorEdit3::new(self, label, color)
    }

    /// Creates a color edit builder for 4 components
    pub fn color_edit4_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 4],
    ) -> ColorEdit4<'ui, 'p> {
        ColorEdit4::new(self, label, color)
    }

    /// Creates a color picker builder for 3 components
    pub fn color_picker3_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 3],
    ) -> ColorPicker3<'ui, 'p> {
        ColorPicker3::new(self, label, color)
    }

    /// Creates a color picker builder for 4 components
    pub fn color_picker4_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 4],
    ) -> ColorPicker4<'ui, 'p> {
        ColorPicker4::new(self, label, color)
    }

    /// Creates a color button builder
    pub fn color_button_config<'ui>(
        &'ui self,
        desc_id: impl Into<Cow<'ui, str>>,
        color: [f32; 4],
    ) -> ColorButton<'ui> {
        ColorButton::new(self, desc_id, color)
    }
}
