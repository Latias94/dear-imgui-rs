//! List box widget for Dear ImGui
//!
//! List boxes provide a scrollable list of selectable items.

use dear_imgui_sys as sys;
use std::ffi::CString;
use crate::ui::Ui;

/// List box functionality for UI
impl<'frame> Ui<'frame> {
    /// Create a list box widget
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let items = ["Item 1", "Item 2", "Item 3"];
    /// let mut current = 0;
    /// 
    /// if ui.list_box("My List", &items, &mut current) {
    ///     println!("Selected item: {}", items[current as usize]);
    /// }
    /// # true });
    /// ```
    pub fn list_box(&mut self, label: impl AsRef<str>, items: &[&str], current_item: &mut i32) -> bool {
        self.list_box_with_height(label, items, current_item, -1)
    }
    
    /// Create a list box widget with custom height
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let items = ["Item 1", "Item 2", "Item 3", "Item 4", "Item 5"];
    /// let mut current = 0;
    /// 
    /// // Show only 3 items at a time
    /// if ui.list_box_with_height("My List", &items, &mut current, 3) {
    ///     println!("Selected item: {}", items[current as usize]);
    /// }
    /// # true });
    /// ```
    pub fn list_box_with_height(
        &mut self,
        label: impl AsRef<str>,
        items: &[&str],
        current_item: &mut i32,
        height_in_items: i32
    ) -> bool {
        let label_cstr = CString::new(label.as_ref()).unwrap_or_default();

        // Convert items to C strings
        let item_cstrs: Vec<CString> = items
            .iter()
            .map(|item| CString::new(*item).unwrap_or_default())
            .collect();

        let item_ptrs: Vec<*const i8> = item_cstrs
            .iter()
            .map(|cstr| cstr.as_ptr())
            .collect();

        unsafe {
            sys::ImGui_ListBox(
                label_cstr.as_ptr(),
                current_item,
                item_ptrs.as_ptr(),
                item_ptrs.len() as i32,
                height_in_items,
            )
        }
    }
    
    /// Create a list box with a callback for item rendering
    /// 
    /// This allows for more complex item rendering than simple strings.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let items = vec!["Red", "Green", "Blue"];
    /// let mut current = 0;
    /// 
    /// ui.list_box_callback("Colors", items.len(), &mut current, 3, |ui, index| {
    ///     let color = match index {
    ///         0 => [1.0, 0.0, 0.0, 1.0], // Red
    ///         1 => [0.0, 1.0, 0.0, 1.0], // Green
    ///         2 => [0.0, 0.0, 1.0, 1.0], // Blue
    ///         _ => [1.0, 1.0, 1.0, 1.0], // White
    ///     };
    ///     
    ///     ui.text_colored(color, items[index]);
    /// });
    /// # true });
    /// ```
    pub fn list_box_callback<F>(
        &mut self,
        label: impl AsRef<str>,
        items_count: usize,
        current_item: &mut i32,
        height_in_items: i32,
        mut callback: F,
    ) -> bool
    where
        F: FnMut(&mut Ui, usize),
    {
        let label_cstr = CString::new(label.as_ref()).unwrap_or_default();
        
        unsafe {
            let size = sys::ImVec2 {
                x: 0.0,
                y: height_in_items as f32 * sys::ImGui_GetTextLineHeightWithSpacing(),
            };
            let result = sys::ImGui_BeginListBox(
                label_cstr.as_ptr(),
                &size,
            );

            if result {
                for i in 0..items_count {
                    let is_selected = *current_item == i as i32;
                    let item_id = CString::new(format!("##item_{}", i)).unwrap_or_default();
                    let size = sys::ImVec2 { x: 0.0, y: 0.0 };

                    if sys::ImGui_Selectable(
                        item_id.as_ptr(),
                        is_selected,
                        0, // flags
                        &size,
                    ) {
                        *current_item = i as i32;
                    }

                    // Render custom content
                    callback(self, i);
                }

                sys::ImGui_EndListBox();
                true
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn test_list_box_creation() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let items = ["Item 1", "Item 2", "Item 3"];
            let mut current = 0;
            
            // Test basic list box
            ui.list_box("Test List", &items, &mut current);
            
            // Test list box with height
            ui.list_box_with_height("Test List 2", &items, &mut current, 2);
            
            true
        });
    }

    #[test]
    fn test_list_box_with_height() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();

        frame.window("Test").show(|ui| {
            let items = ["Item 1", "Item 2", "Item 3"];
            let mut current = 0;

            // Test list box with specific height
            ui.list_box_with_height("Height List", &items, &mut current, 3);

            true
        });
    }

    #[test]
    fn test_list_box_callback() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let mut current = 0;
            
            // Test callback list box
            ui.list_box_callback("Callback List", 3, &mut current, 3, |ui, index| {
                ui.text(format!("Custom Item {}", index));
            });
            
            true
        });
    }
}
