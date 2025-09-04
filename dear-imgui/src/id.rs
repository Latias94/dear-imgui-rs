//! ID management system for Dear ImGui
//!
//! This module provides functionality for managing Dear ImGui's ID stack,
//! which is used to generate unique identifiers for widgets and avoid conflicts.

use dear_imgui_sys as sys;
use std::ffi::CString;
use crate::ui::Ui;

// Create ID stack token using our new token system
crate::create_token!(
    /// Tracks an ID pushed to the ID stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct IdToken<'ui>;

    /// Pops a change from the ID stack.
    drop { sys::ImGui_PopID() }
);

/// ID management functionality for UI
impl<'frame> Ui<'frame> {
    /// Push a string ID onto the ID stack
    /// 
    /// This creates a new ID scope using the provided string. All widgets created
    /// within this scope will have their IDs prefixed with this string.
    /// 
    /// # Arguments
    /// 
    /// * `id` - The string to use as an ID component
    /// 
    /// # Returns
    /// 
    /// An `IdToken` that automatically pops the ID when dropped
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// {
    ///     let _id = ui.push_id("section1");
    ///     ui.button("Button"); // ID will be "section1/Button"
    ///     
    ///     {
    ///         let _id2 = ui.push_id("subsection");
    ///         ui.button("Button"); // ID will be "section1/subsection/Button"
    ///     } // _id2 automatically popped here
    /// } // _id automatically popped here
    /// 
    /// ui.button("Button"); // ID will be just "Button"
    /// # true });
    /// ```
    pub fn push_id(&mut self, id: impl AsRef<str>) -> IdToken<'_> {
        let id = id.as_ref();
        let c_id = CString::new(id).unwrap_or_default();
        unsafe {
            sys::ImGui_PushID(c_id.as_ptr());
        }
        IdToken::new()
    }
    
    /// Push an integer ID onto the ID stack
    /// 
    /// This creates a new ID scope using the provided integer. Useful when
    /// iterating over collections where you need unique IDs for each item.
    /// 
    /// # Arguments
    /// 
    /// * `id` - The integer to use as an ID component
    /// 
    /// # Returns
    /// 
    /// An `IdToken` that automatically pops the ID when dropped
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let items = vec!["Item 1", "Item 2", "Item 3"];
    /// 
    /// for (index, item) in items.iter().enumerate() {
    ///     let _id = ui.push_id(index as i32);
    ///     ui.button(item); // Each button gets a unique ID based on index
    /// }
    /// # true });
    /// ```
    pub fn push_id_int(&mut self, id: i32) -> IdToken<'_> {
        unsafe {
            sys::ImGui_PushID3(id);
        }
        IdToken::new()
    }
    
    /// Push a pointer ID onto the ID stack
    /// 
    /// This creates a new ID scope using the provided pointer address.
    /// Useful when you have objects and want to use their memory address
    /// as a unique identifier.
    /// 
    /// # Arguments
    /// 
    /// * `ptr` - The pointer to use as an ID component
    /// 
    /// # Returns
    /// 
    /// An `IdToken` that automatically pops the ID when dropped
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let objects = vec![String::from("Object 1"), String::from("Object 2")];
    /// 
    /// for obj in &objects {
    ///     let _id = ui.push_id_ptr(obj.as_ptr() as *const std::ffi::c_void);
    ///     ui.button(&obj); // Each button gets a unique ID based on object address
    /// }
    /// # true });
    /// ```
    pub fn push_id_ptr(&mut self, ptr: *const std::ffi::c_void) -> IdToken<'_> {
        unsafe {
            sys::ImGui_PushID2(ptr);
        }
        IdToken::new()
    }
    
    /// Get the current ID
    /// 
    /// Returns the ID that would be assigned to the next widget.
    /// This is useful for debugging ID conflicts or for advanced use cases
    /// where you need to know the current ID.
    /// 
    /// # Returns
    /// 
    /// The current ID as a 32-bit unsigned integer
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let base_id = ui.get_id();
    /// 
    /// {
    ///     let _id = ui.push_id("section");
    ///     let section_id = ui.get_id();
    ///     assert_ne!(base_id, section_id);
    /// }
    /// 
    /// let back_to_base = ui.get_id();
    /// assert_eq!(base_id, back_to_base);
    /// # true });
    /// ```
    pub fn get_id(&self) -> u32 {
        unsafe {
            sys::ImGui_GetID(std::ptr::null())
        }
    }
    
    /// Get an ID for a specific string
    /// 
    /// Returns the ID that would be generated for the given string
    /// in the current ID scope. This doesn't modify the ID stack.
    /// 
    /// # Arguments
    /// 
    /// * `id` - The string to generate an ID for
    /// 
    /// # Returns
    /// 
    /// The generated ID as a 32-bit unsigned integer
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let id1 = ui.get_id_str("button1");
    /// let id2 = ui.get_id_str("button2");
    /// assert_ne!(id1, id2);
    /// 
    /// // Same string always generates same ID in same scope
    /// let id1_again = ui.get_id_str("button1");
    /// assert_eq!(id1, id1_again);
    /// # true });
    /// ```
    pub fn get_id_str(&self, id: impl AsRef<str>) -> u32 {
        let id = id.as_ref();
        let c_id = CString::new(id).unwrap_or_default();
        unsafe {
            sys::ImGui_GetID(c_id.as_ptr())
        }
    }
    
    /// Get an ID for a specific pointer
    /// 
    /// Returns the ID that would be generated for the given pointer
    /// in the current ID scope. This doesn't modify the ID stack.
    /// 
    /// # Arguments
    /// 
    /// * `ptr` - The pointer to generate an ID for
    /// 
    /// # Returns
    /// 
    /// The generated ID as a 32-bit unsigned integer
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let obj1 = String::from("Object 1");
    /// let obj2 = String::from("Object 2");
    /// 
    /// let id1 = ui.get_id_ptr(obj1.as_ptr() as *const std::ffi::c_void);
    /// let id2 = ui.get_id_ptr(obj2.as_ptr() as *const std::ffi::c_void);
    /// assert_ne!(id1, id2);
    /// # true });
    /// ```
    pub fn get_id_ptr(&self, ptr: *const std::ffi::c_void) -> u32 {
        unsafe {
            sys::ImGui_GetID2(ptr)
        }
    }
    
    /// Create a scoped ID context
    /// 
    /// This is a convenience method that creates an ID scope and executes
    /// the provided closure within that scope. The ID is automatically
    /// popped when the closure completes.
    /// 
    /// # Arguments
    /// 
    /// * `id` - The ID to push
    /// * `f` - The closure to execute within the ID scope
    /// 
    /// # Returns
    /// 
    /// The return value of the closure
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// ui.with_id("section1", |ui| {
    ///     ui.button("Button 1");
    ///     ui.button("Button 2");
    ///     
    ///     ui.with_id("subsection", |ui| {
    ///         ui.button("Nested Button");
    ///     });
    /// });
    /// # true });
    /// ```
    pub fn with_id<R>(&mut self, id: impl AsRef<str>, f: impl FnOnce(&mut Self) -> R) -> R {
        let id = id.as_ref();
        let c_id = CString::new(id).unwrap_or_default();
        unsafe {
            sys::ImGui_PushID(c_id.as_ptr());
        }
        let result = f(self);
        unsafe {
            sys::ImGui_PopID();
        }
        result
    }
    
    /// Create a scoped ID context with an integer ID
    /// 
    /// Similar to `with_id` but uses an integer ID.
    /// 
    /// # Arguments
    /// 
    /// * `id` - The integer ID to push
    /// * `f` - The closure to execute within the ID scope
    /// 
    /// # Returns
    /// 
    /// The return value of the closure
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// for i in 0..3 {
    ///     ui.with_id_int(i, |ui| {
    ///         ui.button(format!("Button {}", i));
    ///     });
    /// }
    /// # true });
    /// ```
    pub fn with_id_int<R>(&mut self, id: i32, f: impl FnOnce(&mut Self) -> R) -> R {
        unsafe {
            sys::ImGui_PushID3(id);
        }
        let result = f(self);
        unsafe {
            sys::ImGui_PopID();
        }
        result
    }
    
    /// Create a scoped ID context with a pointer ID
    /// 
    /// Similar to `with_id` but uses a pointer ID.
    /// 
    /// # Arguments
    /// 
    /// * `ptr` - The pointer ID to push
    /// * `f` - The closure to execute within the ID scope
    /// 
    /// # Returns
    /// 
    /// The return value of the closure
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let objects = vec![String::from("A"), String::from("B")];
    /// 
    /// for obj in &objects {
    ///     ui.with_id_ptr(obj.as_ptr() as *const std::ffi::c_void, |ui| {
    ///         ui.button(&obj);
    ///     });
    /// }
    /// # true });
    /// ```
    pub fn with_id_ptr<R>(&mut self, ptr: *const std::ffi::c_void, f: impl FnOnce(&mut Self) -> R) -> R {
        unsafe {
            sys::ImGui_PushID2(ptr);
        }
        let result = f(self);
        unsafe {
            sys::ImGui_PopID();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::Context;

    #[test]
    fn test_id_stack_string() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();

        frame.window("Test").show(|ui| {
            let base_id = ui.get_id();

            // Test with_id scoped method
            ui.with_id("section1", |ui| {
                let section_id = ui.get_id();
                assert_ne!(base_id, section_id);

                ui.with_id("subsection", |ui| {
                    let subsection_id = ui.get_id();
                    assert_ne!(section_id, subsection_id);
                });

                // Back to section level
                let back_to_section = ui.get_id();
                assert_eq!(section_id, back_to_section);
            });

            // Back to base level
            let back_to_base = ui.get_id();
            assert_eq!(base_id, back_to_base);

            true
        });
    }

    #[test]
    fn test_id_stack_int() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();

        frame.window("Test").show(|ui| {
            let base_id = ui.get_id();

            ui.with_id_int(42, |ui| {
                let int_id = ui.get_id();
                assert_ne!(base_id, int_id);
            });

            let back_to_base = ui.get_id();
            assert_eq!(base_id, back_to_base);

            true
        });
    }

    #[test]
    fn test_id_generation() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let id1 = ui.get_id_str("button1");
            let id2 = ui.get_id_str("button2");
            assert_ne!(id1, id2);
            
            // Same string should generate same ID
            let id1_again = ui.get_id_str("button1");
            assert_eq!(id1, id1_again);
            
            true
        });
    }

    #[test]
    fn test_scoped_id() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let base_id = ui.get_id();
            
            let result = ui.with_id("scoped", |ui| {
                let scoped_id = ui.get_id();
                assert_ne!(base_id, scoped_id);
                42 // Return value
            });
            
            assert_eq!(result, 42);
            
            // Should be back to base ID
            let back_to_base = ui.get_id();
            assert_eq!(base_id, back_to_base);
            
            true
        });
    }

    #[test]
    fn test_scoped_id_int() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let base_id = ui.get_id();
            
            ui.with_id_int(123, |ui| {
                let scoped_id = ui.get_id();
                assert_ne!(base_id, scoped_id);
            });
            
            // Should be back to base ID
            let back_to_base = ui.get_id();
            assert_eq!(base_id, back_to_base);
            
            true
        });
    }
}
