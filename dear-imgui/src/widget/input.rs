use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Input widgets
///
/// This module contains all input-related UI components like text inputs, sliders, checkboxes, etc.
use std::ffi::CString;

/// # Widgets: Input
impl<'frame> Ui<'frame> {
    /// Display a checkbox
    ///
    /// Returns `true` if the checkbox state changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut enabled = false;
    /// # frame.window("Example").show(|ui| {
    /// if ui.checkbox("Enable feature", &mut enabled) {
    ///     println!("Checkbox toggled: {}", enabled);
    /// }
    /// # });
    /// ```
    pub fn checkbox(&mut self, label: impl AsRef<str>, value: &mut bool) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe { sys::ImGui_Checkbox(c_label.as_ptr(), value as *mut bool) }
    }

    /// Display a float slider
    ///
    /// Returns `true` if the value was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut value = 0.5f32;
    /// # frame.window("Example").show(|ui| {
    /// if ui.slider_float("Volume", &mut value, 0.0, 1.0) {
    ///     println!("Volume changed to: {}", value);
    /// }
    /// # });
    /// ```
    pub fn slider_float(
        &mut self,
        label: impl AsRef<str>,
        value: &mut f32,
        min: f32,
        max: f32,
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_SliderFloat(
                c_label.as_ptr(),
                value as *mut f32,
                min,
                max,
                std::ptr::null(), // Use default format
                0,                // Default flags
            )
        }
    }

    /// Display a float slider with custom format
    ///
    /// Returns `true` if the value was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut value = 0.5f32;
    /// # frame.window("Example").show(|ui| {
    /// if ui.slider_float_with_format("Percentage", &mut value, 0.0, 1.0, "%.1f%%") {
    ///     println!("Percentage changed to: {:.1}%", value * 100.0);
    /// }
    /// # });
    /// ```
    pub fn slider_float_with_format(
        &mut self,
        label: impl AsRef<str>,
        value: &mut f32,
        min: f32,
        max: f32,
        format: &str,
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let c_format = CString::new(format).unwrap_or_default();
        unsafe {
            sys::ImGui_SliderFloat(
                c_label.as_ptr(),
                value as *mut f32,
                min,
                max,
                c_format.as_ptr(),
                0, // Default flags
            )
        }
    }

    /// Display an integer slider
    ///
    /// Returns `true` if the value was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut value = 50i32;
    /// # frame.window("Example").show(|ui| {
    /// if ui.slider_int("Count", &mut value, 0, 100) {
    ///     println!("Count changed to: {}", value);
    /// }
    /// # });
    /// ```
    pub fn slider_int(
        &mut self,
        label: impl AsRef<str>,
        value: &mut i32,
        min: i32,
        max: i32,
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_SliderInt(
                c_label.as_ptr(),
                value as *mut i32,
                min,
                max,
                std::ptr::null(), // Use default format
                0,                // Default flags
            )
        }
    }

    /// Display a text input field
    ///
    /// Returns `true` if the text was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut text = String::from("Hello");
    /// # frame.window("Example").show(|ui| {
    /// if ui.input_text("Name", &mut text) {
    ///     println!("Name changed to: {}", text);
    /// }
    /// # });
    /// ```
    pub fn input_text(&mut self, label: impl AsRef<str>, text: &mut String) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();

        // Create a buffer with extra space for editing
        let mut buffer = text.clone().into_bytes();
        buffer.resize(buffer.len() + 256, 0); // Add extra space

        let changed = unsafe {
            sys::ImGui_InputText(
                c_label.as_ptr(),
                buffer.as_mut_ptr() as *mut i8,
                buffer.len(),
                0,                    // Default flags
                None,                 // No callback
                std::ptr::null_mut(), // No user data
            )
        };

        if changed {
            // Find the null terminator and update the string
            if let Some(null_pos) = buffer.iter().position(|&b| b == 0) {
                buffer.truncate(null_pos);
                if let Ok(new_text) = String::from_utf8(buffer) {
                    *text = new_text;
                }
            }
        }

        changed
    }

    /// Display a float input field
    ///
    /// Returns `true` if the value was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut value = 3.14f32;
    /// # frame.window("Example").show(|ui| {
    /// if ui.input_float("Pi", &mut value) {
    ///     println!("Pi changed to: {}", value);
    /// }
    /// # });
    /// ```
    pub fn input_float(&mut self, label: impl AsRef<str>, value: &mut f32) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_InputFloat(
                c_label.as_ptr(),
                value as *mut f32,
                0.0,              // Default step
                0.0,              // Default step_fast
                std::ptr::null(), // Use default format
                0,                // Default flags
            )
        }
    }

    /// Display an integer input field
    ///
    /// Returns `true` if the value was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut value = 42i32;
    /// # frame.window("Example").show(|ui| {
    /// if ui.input_int("Answer", &mut value) {
    ///     println!("Answer changed to: {}", value);
    /// }
    /// # });
    /// ```
    pub fn input_int(&mut self, label: impl AsRef<str>, value: &mut i32) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_InputInt(
                c_label.as_ptr(),
                value as *mut i32,
                1,   // Default step
                100, // Default step_fast
                0,   // Default flags
            )
        }
    }

    /// Display a combo box (dropdown)
    ///
    /// Returns `true` if the selection changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut current_item = 0;
    /// # frame.window("Example").show(|ui| {
    /// let items = ["Option 1", "Option 2", "Option 3"];
    /// if ui.combo("Choose", &mut current_item, &items) {
    ///     println!("Selected: {}", items[current_item]);
    /// }
    /// # });
    /// ```
    pub fn combo(
        &mut self,
        label: impl AsRef<str>,
        current_item: &mut i32,
        items: &[&str],
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();

        // Convert items to C strings
        let c_items: Vec<CString> = items
            .iter()
            .map(|&item| CString::new(item).unwrap_or_default())
            .collect();
        let c_item_ptrs: Vec<*const i8> = c_items.iter().map(|c_str| c_str.as_ptr()).collect();

        unsafe {
            sys::ImGui_Combo(
                c_label.as_ptr(),
                current_item as *mut i32,
                c_item_ptrs.as_ptr(),
                items.len() as i32,
                -1, // Default popup_max_height_in_items
            )
        }
    }

    /// Display a listbox
    ///
    /// Returns `true` if the selection changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut current_item = 0;
    /// # frame.window("Example").show(|ui| {
    /// let items = ["Item 1", "Item 2", "Item 3"];
    /// if ui.listbox("List", &mut current_item, &items, 4) {
    ///     println!("Selected: {}", items[current_item]);
    /// }
    /// # });
    /// ```
    pub fn listbox(
        &mut self,
        label: impl AsRef<str>,
        current_item: &mut i32,
        items: &[&str],
        height_in_items: i32,
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();

        // Convert items to C strings
        let c_items: Vec<CString> = items
            .iter()
            .map(|&item| CString::new(item).unwrap_or_default())
            .collect();
        let c_item_ptrs: Vec<*const i8> = c_items.iter().map(|c_str| c_str.as_ptr()).collect();

        unsafe {
            sys::ImGui_ListBox(
                c_label.as_ptr(),
                current_item as *mut i32,
                c_item_ptrs.as_ptr(),
                items.len() as i32,
                height_in_items,
            )
        }
    }

    /// Display a selectable item
    ///
    /// Returns `true` if the item was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut selected = false;
    /// # frame.window("Example").show(|ui| {
    /// if ui.selectable("Selectable item", &mut selected) {
    ///     println!("Item clicked, selected: {}", selected);
    /// }
    /// # });
    /// ```
    pub fn selectable(&mut self, label: impl AsRef<str>, selected: &mut bool) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_Selectable1(
                c_label.as_ptr(),
                selected as *mut bool,
                0,                                           // Default flags
                &sys::ImVec2 { x: 0.0, y: 0.0 } as *const _, // Default size
            )
        }
    }

    /// Display a radio button (boolean version)
    ///
    /// Returns `true` if the radio button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.radio_button_bool("Option A", true) {
    ///     println!("Option A selected");
    /// }
    /// if ui.radio_button_bool("Option B", false) {
    ///     println!("Option B selected");
    /// }
    /// # });
    /// ```
    pub fn radio_button_bool(&mut self, label: impl AsRef<str>, active: bool) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe { sys::ImGui_RadioButton(c_label.as_ptr(), active) }
    }

    /// Display a radio button for choosing between values
    ///
    /// Returns `true` if the radio button was clicked and the value changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut selected_option = 0;
    /// # frame.window("Example").show(|ui| {
    /// if ui.radio_button("Option 1", &mut selected_option, 0) {
    ///     println!("Selected option 1");
    /// }
    /// if ui.radio_button("Option 2", &mut selected_option, 1) {
    ///     println!("Selected option 2");
    /// }
    /// if ui.radio_button("Option 3", &mut selected_option, 2) {
    ///     println!("Selected option 3");
    /// }
    /// # });
    /// ```
    pub fn radio_button<T>(
        &mut self,
        label: impl AsRef<str>,
        value: &mut T,
        button_value: T,
    ) -> bool
    where
        T: Copy + PartialEq,
    {
        let pressed = self.radio_button_bool(label, *value == button_value);
        if pressed {
            *value = button_value;
        }
        pressed
    }

    /// Display a multi-line text input field
    ///
    /// Returns `true` if the text was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut text = String::from("Hello\nWorld");
    /// # frame.window("Example").show(|ui| {
    /// if ui.input_text_multiline("Description", &mut text, Vec2::new(300.0, 100.0)) {
    ///     println!("Text changed to: {}", text);
    /// }
    /// # });
    /// ```
    pub fn input_text_multiline(
        &mut self,
        label: impl AsRef<str>,
        text: &mut String,
        size: crate::types::Vec2,
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();

        // Create a buffer with extra space for editing
        let mut buffer = text.clone().into_bytes();
        buffer.resize(buffer.len() + 1024, 0); // Add extra space for multiline

        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };

        let changed = unsafe {
            sys::ImGui_InputTextMultiline(
                c_label.as_ptr(),
                buffer.as_mut_ptr() as *mut i8,
                buffer.len(),
                &size_vec as *const _,
                0,                    // Default flags
                None,                 // No callback
                std::ptr::null_mut(), // No user data
            )
        };

        if changed {
            // Find the null terminator and update the string
            if let Some(null_pos) = buffer.iter().position(|&b| b == 0) {
                buffer.truncate(null_pos);
                if let Ok(new_text) = String::from_utf8(buffer) {
                    *text = new_text;
                }
            }
        }

        changed
    }

    /// Display a float drag input
    ///
    /// Returns `true` if the value was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut value = 1.0f32;
    /// # frame.window("Example").show(|ui| {
    /// if ui.drag_float("Speed", &mut value, 0.1, 0.0, 10.0) {
    ///     println!("Speed changed to: {}", value);
    /// }
    /// # });
    /// ```
    pub fn drag_float(
        &mut self,
        label: impl AsRef<str>,
        value: &mut f32,
        speed: f32,
        min: f32,
        max: f32,
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_DragFloat(
                c_label.as_ptr(),
                value as *mut f32,
                speed,
                min,
                max,
                std::ptr::null(), // Use default format
                0,                // Default flags
            )
        }
    }

    /// Display an integer drag input
    ///
    /// Returns `true` if the value was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut value = 10i32;
    /// # frame.window("Example").show(|ui| {
    /// if ui.drag_int("Count", &mut value, 1.0, 0, 100) {
    ///     println!("Count changed to: {}", value);
    /// }
    /// # });
    /// ```
    pub fn drag_int(
        &mut self,
        label: impl AsRef<str>,
        value: &mut i32,
        speed: f32,
        min: i32,
        max: i32,
    ) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_DragInt(
                c_label.as_ptr(),
                value as *mut i32,
                speed,
                min,
                max,
                std::ptr::null(), // Use default format
                0,                // Default flags
            )
        }
    }
}
