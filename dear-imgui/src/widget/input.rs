use crate::sys;
use crate::ui::Ui;
use crate::InputTextFlags;

/// # Input Widgets
impl Ui {
    /// Creates a single-line text input widget builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut text = String::new();
    /// if ui.input_text("Label", &mut text).build() {
    ///     println!("Text changed: {}", text);
    /// }
    /// ```
    #[doc(alias = "InputText", alias = "InputTextWithHint")]
    pub fn input_text<'p>(&self, label: impl AsRef<str>, buf: &'p mut String) -> InputText<'_, 'p> {
        InputText::new(self, label, buf)
    }

    /// Creates a multi-line text input widget builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut text = String::new();
    /// if ui.input_text_multiline("Label", &mut text, [200.0, 100.0]).build() {
    ///     println!("Text changed: {}", text);
    /// }
    /// ```
    #[doc(alias = "InputTextMultiline")]
    pub fn input_text_multiline<'p>(
        &self,
        label: impl AsRef<str>,
        buf: &'p mut String,
        size: impl Into<[f32; 2]>,
    ) -> InputTextMultiline<'_, 'p> {
        InputTextMultiline::new(self, label, buf, size)
    }

    /// Creates an integer input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputInt")]
    pub fn input_int(&self, label: impl AsRef<str>, value: &mut i32) -> bool {
        self.input_int_config(label).build(value)
    }

    /// Creates a float input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputFloat")]
    pub fn input_float(&self, label: impl AsRef<str>, value: &mut f32) -> bool {
        self.input_float_config(label).build(value)
    }

    /// Creates a double input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputDouble")]
    pub fn input_double(&self, label: impl AsRef<str>, value: &mut f64) -> bool {
        self.input_double_config(label).build(value)
    }

    /// Creates an integer input builder
    pub fn input_int_config(&self, label: impl AsRef<str>) -> InputInt<'_> {
        InputInt::new(self, label)
    }

    /// Creates a float input builder
    pub fn input_float_config(&self, label: impl AsRef<str>) -> InputFloat<'_> {
        InputFloat::new(self, label)
    }

    /// Creates a double input builder
    pub fn input_double_config(&self, label: impl AsRef<str>) -> InputDouble<'_> {
        InputDouble::new(self, label)
    }
}

/// Builder for a text input widget
#[derive(Debug)]
#[must_use]
pub struct InputText<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    buf: &'p mut String,
    flags: InputTextFlags,
    hint: Option<String>,
}

impl<'ui, 'p> InputText<'ui, 'p> {
    /// Creates a new text input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, buf: &'p mut String) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            buf,
            flags: InputTextFlags::NONE,
            hint: None,
        }
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets a hint text
    pub fn hint(mut self, hint: impl AsRef<str>) -> Self {
        self.hint = Some(hint.as_ref().to_string());
        self
    }

    /// Makes the input read-only
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.flags.set(InputTextFlags::READ_ONLY, read_only);
        self
    }

    /// Enables password mode
    pub fn password(mut self, password: bool) -> Self {
        self.flags.set(InputTextFlags::PASSWORD, password);
        self
    }

    /// Builds the text input widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let hint_ptr = self.ui.scratch_txt_opt(self.hint.as_ref());

        // Prepare buffer for ImGui
        let mut buffer = self.buf.clone().into_bytes();
        buffer.resize(256, 0); // Reserve space for input

        let result = unsafe {
            if hint_ptr.is_null() {
                sys::ImGui_InputText(
                    label_ptr,
                    buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                    buffer.len(),
                    self.flags.bits(),
                    None,
                    std::ptr::null_mut(),
                )
            } else {
                sys::ImGui_InputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                    buffer.len(),
                    self.flags.bits(),
                    None,
                    std::ptr::null_mut(),
                )
            }
        };

        // Update the string if changed
        if result {
            if let Some(null_pos) = buffer.iter().position(|&b| b == 0) {
                buffer.truncate(null_pos);
            }
            if let Ok(new_string) = String::from_utf8(buffer) {
                *self.buf = new_string;
            }
        }

        result
    }
}

/// Builder for multiline text input widget
#[derive(Debug)]
#[must_use]
pub struct InputTextMultiline<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    buf: &'p mut String,
    size: [f32; 2],
    flags: InputTextFlags,
}

impl<'ui, 'p> InputTextMultiline<'ui, 'p> {
    /// Creates a new multiline text input builder
    pub fn new(
        ui: &'ui Ui,
        label: impl AsRef<str>,
        buf: &'p mut String,
        size: impl Into<[f32; 2]>,
    ) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            buf,
            size: size.into(),
            flags: InputTextFlags::NONE,
        }
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Makes the input read-only
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.flags.set(InputTextFlags::READ_ONLY, read_only);
        self
    }

    /// Builds the multiline text input widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);

        // Prepare buffer for ImGui
        let mut buffer = self.buf.clone().into_bytes();
        buffer.resize(1024, 0); // Reserve more space for multiline

        let size_vec: sys::ImVec2 = self.size.into();
        let result = unsafe {
            sys::ImGui_InputTextMultiline(
                label_ptr,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
                &size_vec,
                self.flags.bits(),
                None,
                std::ptr::null_mut(),
            )
        };

        // Update the string if changed
        if result {
            if let Some(null_pos) = buffer.iter().position(|&b| b == 0) {
                buffer.truncate(null_pos);
            }
            if let Ok(new_string) = String::from_utf8(buffer) {
                *self.buf = new_string;
            }
        }

        result
    }
}

/// Builder for integer input widget
#[derive(Debug)]
#[must_use]
pub struct InputInt<'ui> {
    ui: &'ui Ui,
    label: String,
    step: i32,
    step_fast: i32,
    flags: InputTextFlags,
}

impl<'ui> InputInt<'ui> {
    /// Creates a new integer input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            step: 1,
            step_fast: 100,
            flags: InputTextFlags::NONE,
        }
    }

    /// Sets the step value
    pub fn step(mut self, step: i32) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: i32) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the integer input widget
    pub fn build(self, value: &mut i32) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        unsafe {
            sys::ImGui_InputInt(
                label_ptr,
                value as *mut i32,
                self.step,
                self.step_fast,
                self.flags.bits(),
            )
        }
    }
}

/// Builder for float input widget
#[derive(Debug)]
#[must_use]
pub struct InputFloat<'ui> {
    ui: &'ui Ui,
    label: String,
    step: f32,
    step_fast: f32,
    format: Option<String>,
    flags: InputTextFlags,
}

impl<'ui> InputFloat<'ui> {
    /// Creates a new float input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            step: 0.0,
            step_fast: 0.0,
            format: None,
            flags: InputTextFlags::NONE,
        }
    }

    /// Sets the step value
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: f32) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the display format
    pub fn format(mut self, format: impl AsRef<str>) -> Self {
        self.format = Some(format.as_ref().to_string());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the float input widget
    pub fn build(self, value: &mut f32) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let format_ptr = self.ui.scratch_txt_opt(self.format.as_ref());
        let format_ptr = if format_ptr.is_null() {
            self.ui.scratch_txt("%.3f")
        } else {
            format_ptr
        };

        unsafe {
            sys::ImGui_InputFloat(
                label_ptr,
                value as *mut f32,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.bits(),
            )
        }
    }
}

/// Builder for double input widget
#[derive(Debug)]
#[must_use]
pub struct InputDouble<'ui> {
    ui: &'ui Ui,
    label: String,
    step: f64,
    step_fast: f64,
    format: Option<String>,
    flags: InputTextFlags,
}

impl<'ui> InputDouble<'ui> {
    /// Creates a new double input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            step: 0.0,
            step_fast: 0.0,
            format: None,
            flags: InputTextFlags::NONE,
        }
    }

    /// Sets the step value
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: f64) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the display format
    pub fn format(mut self, format: impl AsRef<str>) -> Self {
        self.format = Some(format.as_ref().to_string());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the double input widget
    pub fn build(self, value: &mut f64) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let format_ptr = self.ui.scratch_txt_opt(self.format.as_ref());
        let format_ptr = if format_ptr.is_null() {
            self.ui.scratch_txt("%.6f")
        } else {
            format_ptr
        };

        unsafe {
            sys::ImGui_InputDouble(
                label_ptr,
                value as *mut f64,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.bits(),
            )
        }
    }
}
