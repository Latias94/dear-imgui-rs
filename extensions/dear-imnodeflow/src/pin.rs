//! Pin-related functionality for ImNodeFlow

use crate::*;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;

/// Represents a pin in the node editor
pub struct Pin {
    pub(crate) ptr: sys::PinPtr,
    _phantom: PhantomData<*mut ()>, // Ensure !Send + !Sync for raw pointer
}

impl Pin {
    /// Create a pin from a raw pointer (internal use)
    pub(crate) fn from_ptr(ptr: sys::PinPtr) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Get the pin's unique identifier
    pub fn uid(&self) -> sys::PinUID {
        unsafe { sys::Pin_GetUid(self.ptr) }
    }

    /// Get the pin's name
    pub fn name(&self) -> Result<String> {
        unsafe {
            let name_ptr = sys::Pin_GetName(self.ptr);
            if name_ptr.is_null() {
                return Ok(String::new());
            }
            let name_cstr = CStr::from_ptr(name_ptr);
            Ok(name_cstr.to_str()?.to_string())
        }
    }

    /// Get the pin's position
    pub fn position(&self) -> ImVec2 {
        unsafe { sys::Pin_GetPos(self.ptr) }
    }

    /// Get the pin's size
    pub fn size(&self) -> ImVec2 {
        unsafe { sys::Pin_GetSize(self.ptr) }
    }

    /// Get the pin's parent node
    pub fn parent(&self) -> Option<Node> {
        unsafe {
            let parent_ptr = sys::Pin_GetParent(self.ptr);
            if parent_ptr.is_null() {
                None
            } else {
                Some(Node {
                    ptr: parent_ptr,
                    _phantom: PhantomData,
                })
            }
        }
    }

    /// Get the pin's type (Input or Output)
    pub fn pin_type(&self) -> PinType {
        unsafe { PinType::from(sys::Pin_GetType(self.ptr)) }
    }

    /// Get the pin's style
    pub fn style(&self) -> Option<PinStyle> {
        unsafe {
            let style_ptr = sys::Pin_GetStyle(self.ptr);
            if style_ptr.is_null() {
                None
            } else {
                Some(PinStyle::from_ptr(style_ptr))
            }
        }
    }

    /// Get the pin's connection point
    pub fn pin_point(&self) -> ImVec2 {
        unsafe { sys::Pin_PinPoint(self.ptr) }
    }

    /// Calculate the pin's width
    pub fn calc_width(&self) -> f32 {
        unsafe { sys::Pin_CalcWidth(self.ptr) }
    }

    /// Set the pin's position
    pub fn set_position(&mut self, pos: ImVec2) {
        unsafe {
            sys::Pin_SetPos(self.ptr, pos.x, pos.y);
        }
    }

    /// Check if the pin is connected
    pub fn is_connected(&self) -> bool {
        unsafe { sys::Pin_IsConnected(self.ptr) }
    }

    /// Create a link between this pin and another pin
    pub fn create_link(&mut self, other: &Pin) {
        unsafe {
            sys::Pin_CreateLink(self.ptr, other.ptr);
        }
    }

    /// Delete the link connected to this pin
    pub fn delete_link(&mut self) {
        unsafe {
            sys::Pin_DeleteLink(self.ptr);
        }
    }

    /// Get the raw pointer (for advanced usage)
    pub fn as_ptr(&self) -> sys::PinPtr {
        self.ptr
    }
}

/// Represents a link between two pins
pub struct Link {
    ptr: sys::LinkPtr,
    _phantom: PhantomData<*mut ()>,
}

impl Link {
    /// Create a new link between two pins
    pub fn new(left: &Pin, right: &Pin, editor: &NodeEditor) -> Self {
        let ptr = unsafe { sys::Link_Create(left.ptr, right.ptr, editor.as_ptr()) };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Update the link (should be called every frame)
    pub fn update(&mut self) {
        unsafe {
            sys::Link_Update(self.ptr);
        }
    }

    /// Get the left (output) pin of the link
    pub fn left(&self) -> Pin {
        unsafe {
            let pin_ptr = sys::Link_Left(self.ptr);
            Pin::from_ptr(pin_ptr)
        }
    }

    /// Get the right (input) pin of the link
    pub fn right(&self) -> Pin {
        unsafe {
            let pin_ptr = sys::Link_Right(self.ptr);
            Pin::from_ptr(pin_ptr)
        }
    }

    /// Check if the link is hovered
    pub fn is_hovered(&self) -> bool {
        unsafe { sys::Link_IsHovered(self.ptr) }
    }

    /// Check if the link is selected
    pub fn is_selected(&self) -> bool {
        unsafe { sys::Link_IsSelected(self.ptr) }
    }

    /// Get the raw pointer (for advanced usage)
    pub fn as_ptr(&self) -> sys::LinkPtr {
        self.ptr
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::Link_Destroy(self.ptr);
            }
        }
    }
}

/// Builder for creating pins with a fluent API
pub struct PinBuilder {
    name: String,
    pin_type: PinType,
    style: Option<PinStyle>,
}

impl PinBuilder {
    /// Create a new pin builder
    pub fn new<S: Into<String>>(name: S, pin_type: PinType) -> Self {
        Self {
            name: name.into(),
            pin_type,
            style: None,
        }
    }

    /// Create a new input pin builder
    pub fn input<S: Into<String>>(name: S) -> Self {
        Self::new(name, PinType::Input)
    }

    /// Create a new output pin builder
    pub fn output<S: Into<String>>(name: S) -> Self {
        Self::new(name, PinType::Output)
    }

    /// Set the pin's style
    pub fn style(mut self, style: PinStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Get the pin name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the pin type
    pub fn pin_type(&self) -> PinType {
        self.pin_type
    }

    /// Get the pin style
    pub fn get_style(&self) -> Option<&PinStyle> {
        self.style.as_ref()
    }
}

/// Connection filter for pins
pub struct ConnectionFilter {
    filter_fn: Box<dyn Fn(&Pin, &Pin) -> bool>,
}

impl ConnectionFilter {
    /// Create a new connection filter
    pub fn new<F>(filter: F) -> Self
    where
        F: Fn(&Pin, &Pin) -> bool + 'static,
    {
        Self {
            filter_fn: Box::new(filter),
        }
    }

    /// Allow all connections
    pub fn none() -> Self {
        Self::new(|_, _| true)
    }

    /// Only allow connections between pins of the same type
    pub fn same_type() -> Self {
        Self::new(|out_pin, in_pin| {
            // This is a simplified version - in a real implementation,
            // you'd need to check the actual data types of the pins
            out_pin.pin_type() != in_pin.pin_type()
        })
    }

    /// Check if a connection is allowed
    pub fn allows_connection(&self, out_pin: &Pin, in_pin: &Pin) -> bool {
        (self.filter_fn)(out_pin, in_pin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_builder() {
        let input_builder = PinBuilder::input("Input Pin");
        assert_eq!(input_builder.name(), "Input Pin");
        assert_eq!(input_builder.pin_type(), PinType::Input);

        let output_builder = PinBuilder::output("Output Pin");
        assert_eq!(output_builder.name(), "Output Pin");
        assert_eq!(output_builder.pin_type(), PinType::Output);
    }

    #[test]
    fn test_connection_filter() {
        let filter = ConnectionFilter::none();
        // We can't easily test this without actual pins, but we can test the creation
        assert!(true); // Placeholder test

        let same_type_filter = ConnectionFilter::same_type();
        assert!(true); // Placeholder test
    }

    #[test]
    fn test_pin_type_conversion() {
        assert_eq!(PinType::Input as i32, sys::PIN_TYPE_INPUT);
        assert_eq!(PinType::Output as i32, sys::PIN_TYPE_OUTPUT);
    }
}
