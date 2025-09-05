use crate::sys;
use crate::Ui;

impl Ui {
    /// Creates a button with the given label
    #[doc(alias = "Button")]
    pub fn button(&self, label: impl AsRef<str>) -> bool {
        self.button_config(label).build()
    }

    /// Creates a button with the given label and size
    #[doc(alias = "Button")]
    pub fn button_with_size(&self, label: impl AsRef<str>, size: impl Into<[f32; 2]>) -> bool {
        self.button_config(label).size(size).build()
    }

    /// Creates a button builder
    pub fn button_config(&self, label: impl AsRef<str>) -> Button<'_> {
        Button::new(self, label)
    }
}

impl Ui {
    /// Creates a checkbox
    #[doc(alias = "Checkbox")]
    pub fn checkbox(&self, label: impl AsRef<str>, value: &mut bool) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::ImGui_Checkbox(label_ptr, value) }
    }

    /// Creates a radio button
    #[doc(alias = "RadioButton")]
    pub fn radio_button(&self, label: impl AsRef<str>, active: bool) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::ImGui_RadioButton(label_ptr, active) }
    }

    /// Creates a radio button with integer value
    #[doc(alias = "RadioButton")]
    pub fn radio_button_int(&self, label: impl AsRef<str>, v: &mut i32, v_button: i32) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::ImGui_RadioButton1(label_ptr, v, v_button) }
    }
}

/// Builder for button widget
pub struct Button<'ui> {
    ui: &'ui Ui,
    label: String,
    size: Option<[f32; 2]>,
}

impl<'ui> Button<'ui> {
    /// Creates a new button builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            size: None,
        }
    }

    /// Sets the size of the button
    pub fn size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Builds the button
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let size = self.size.unwrap_or([0.0, 0.0]);
        let size_vec: sys::ImVec2 = size.into();
        unsafe { sys::ImGui_Button(label_ptr, &size_vec) }
    }
}
