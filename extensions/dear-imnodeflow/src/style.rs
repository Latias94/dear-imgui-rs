//! Style-related functionality for ImNodeFlow

use crate::*;
use std::marker::PhantomData;

/// Style configuration for pins
pub struct PinStyle {
    ptr: sys::PinStylePtr,
    _phantom: PhantomData<*mut ()>,
}

impl PinStyle {
    /// Create a pin style from a raw pointer (internal use)
    pub(crate) fn from_ptr(ptr: sys::PinStylePtr) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a new custom pin style
    pub fn new(
        color: ImU32,
        socket_shape: i32,
        socket_radius: f32,
        socket_hovered_radius: f32,
        socket_connected_radius: f32,
        socket_thickness: f32,
    ) -> Self {
        let ptr = unsafe {
            sys::PinStyle_Create(
                color,
                socket_shape,
                socket_radius,
                socket_hovered_radius,
                socket_connected_radius,
                socket_thickness,
            )
        };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a cyan pin style
    pub fn cyan() -> Self {
        let ptr = unsafe { sys::PinStyle_Cyan() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a green pin style
    pub fn green() -> Self {
        let ptr = unsafe { sys::PinStyle_Green() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a blue pin style
    pub fn blue() -> Self {
        let ptr = unsafe { sys::PinStyle_Blue() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a brown pin style
    pub fn brown() -> Self {
        let ptr = unsafe { sys::PinStyle_Brown() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a red pin style
    pub fn red() -> Self {
        let ptr = unsafe { sys::PinStyle_Red() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a white pin style
    pub fn white() -> Self {
        let ptr = unsafe { sys::PinStyle_White() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Get the raw pointer (for advanced usage)
    pub fn as_ptr(&self) -> sys::PinStylePtr {
        self.ptr
    }
}

impl Drop for PinStyle {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::PinStyle_Destroy(self.ptr);
            }
        }
    }
}

impl Clone for PinStyle {
    fn clone(&self) -> Self {
        // Note: This creates a new style with the same properties
        // In a real implementation, you'd need to extract the properties
        // from the existing style and create a new one
        Self::cyan() // Placeholder - should copy actual properties
    }
}

/// Style configuration for nodes
pub struct NodeStyle {
    ptr: sys::NodeStylePtr,
    _phantom: PhantomData<*mut ()>,
}

impl NodeStyle {
    /// Create a node style from a raw pointer (internal use)
    pub(crate) fn from_ptr(ptr: sys::NodeStylePtr) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a new custom node style
    pub fn new(header_bg: ImU32, header_title_color: ImU32, radius: f32) -> Self {
        let ptr = unsafe { sys::NodeStyle_Create(header_bg, header_title_color, radius) };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a cyan node style
    pub fn cyan() -> Self {
        let ptr = unsafe { sys::NodeStyle_Cyan() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a green node style
    pub fn green() -> Self {
        let ptr = unsafe { sys::NodeStyle_Green() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a red node style
    pub fn red() -> Self {
        let ptr = unsafe { sys::NodeStyle_Red() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Create a brown node style
    pub fn brown() -> Self {
        let ptr = unsafe { sys::NodeStyle_Brown() };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Get the raw pointer (for advanced usage)
    pub fn as_ptr(&self) -> sys::NodeStylePtr {
        self.ptr
    }
}

impl Drop for NodeStyle {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::NodeStyle_Destroy(self.ptr);
            }
        }
    }
}

impl Clone for NodeStyle {
    fn clone(&self) -> Self {
        // Note: This creates a new style with the same properties
        // In a real implementation, you'd need to extract the properties
        // from the existing style and create a new one
        Self::cyan() // Placeholder - should copy actual properties
    }
}

/// Builder for creating pin styles with a fluent API
pub struct PinStyleBuilder {
    color: ImU32,
    socket_shape: i32,
    socket_radius: f32,
    socket_hovered_radius: f32,
    socket_connected_radius: f32,
    socket_thickness: f32,
}

impl PinStyleBuilder {
    /// Create a new pin style builder with default values
    pub fn new() -> Self {
        Self {
            color: 0xFF_FF_FF_FF, // White
            socket_shape: 0,      // Circle
            socket_radius: 4.0,
            socket_hovered_radius: 4.67,
            socket_connected_radius: 3.7,
            socket_thickness: 1.0,
        }
    }

    /// Set the pin color
    pub fn color(mut self, color: ImU32) -> Self {
        self.color = color;
        self
    }

    /// Set the socket shape
    pub fn socket_shape(mut self, shape: i32) -> Self {
        self.socket_shape = shape;
        self
    }

    /// Set the socket radius
    pub fn socket_radius(mut self, radius: f32) -> Self {
        self.socket_radius = radius;
        self
    }

    /// Set the socket hovered radius
    pub fn socket_hovered_radius(mut self, radius: f32) -> Self {
        self.socket_hovered_radius = radius;
        self
    }

    /// Set the socket connected radius
    pub fn socket_connected_radius(mut self, radius: f32) -> Self {
        self.socket_connected_radius = radius;
        self
    }

    /// Set the socket thickness
    pub fn socket_thickness(mut self, thickness: f32) -> Self {
        self.socket_thickness = thickness;
        self
    }

    /// Build the pin style
    pub fn build(self) -> PinStyle {
        PinStyle::new(
            self.color,
            self.socket_shape,
            self.socket_radius,
            self.socket_hovered_radius,
            self.socket_connected_radius,
            self.socket_thickness,
        )
    }
}

impl Default for PinStyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating node styles with a fluent API
pub struct NodeStyleBuilder {
    header_bg: ImU32,
    header_title_color: ImU32,
    radius: f32,
}

impl NodeStyleBuilder {
    /// Create a new node style builder with default values
    pub fn new() -> Self {
        Self {
            header_bg: 0xFF_4A_4A_4A,          // Dark gray
            header_title_color: 0xFF_FF_FF_FF, // White
            radius: 6.5,
        }
    }

    /// Set the header background color
    pub fn header_bg(mut self, color: ImU32) -> Self {
        self.header_bg = color;
        self
    }

    /// Set the header title color
    pub fn header_title_color(mut self, color: ImU32) -> Self {
        self.header_title_color = color;
        self
    }

    /// Set the corner radius
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Build the node style
    pub fn build(self) -> NodeStyle {
        NodeStyle::new(self.header_bg, self.header_title_color, self.radius)
    }
}

impl Default for NodeStyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Color utilities for creating ImU32 colors
pub mod colors {
    use super::ImU32;

    /// Create an ImU32 color from RGBA components (0-255)
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> ImU32 {
        ((a as ImU32) << 24) | ((b as ImU32) << 16) | ((g as ImU32) << 8) | (r as ImU32)
    }

    /// Create an ImU32 color from RGB components (0-255) with full alpha
    pub fn rgb(r: u8, g: u8, b: u8) -> ImU32 {
        rgba(r, g, b, 255)
    }

    /// Create an ImU32 color from RGBA components (0.0-1.0)
    pub fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> ImU32 {
        rgba(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        )
    }

    /// Create an ImU32 color from RGB components (0.0-1.0) with full alpha
    pub fn rgb_f(r: f32, g: f32, b: f32) -> ImU32 {
        rgba_f(r, g, b, 1.0)
    }

    // Common colors
    pub const WHITE: ImU32 = 0xFF_FF_FF_FF;
    pub const BLACK: ImU32 = 0xFF_00_00_00;
    pub const RED: ImU32 = 0xFF_00_00_FF;
    pub const GREEN: ImU32 = 0xFF_00_FF_00;
    pub const BLUE: ImU32 = 0xFF_FF_00_00;
    pub const CYAN: ImU32 = 0xFF_FF_FF_00;
    pub const MAGENTA: ImU32 = 0xFF_FF_00_FF;
    pub const YELLOW: ImU32 = 0xFF_00_FF_FF;
    pub const TRANSPARENT: ImU32 = 0x00_00_00_00;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_style_creation() {
        let style = PinStyle::cyan();
        assert!(!style.as_ptr().is_null());

        let custom_style = PinStyleBuilder::new()
            .color(colors::RED)
            .socket_radius(5.0)
            .build();
        assert!(!custom_style.as_ptr().is_null());
    }

    #[test]
    fn test_node_style_creation() {
        let style = NodeStyle::green();
        assert!(!style.as_ptr().is_null());

        let custom_style = NodeStyleBuilder::new()
            .header_bg(colors::BLUE)
            .header_title_color(colors::WHITE)
            .radius(10.0)
            .build();
        assert!(!custom_style.as_ptr().is_null());
    }

    #[test]
    fn test_color_utilities() {
        assert_eq!(colors::rgb(255, 0, 0), colors::RED);
        assert_eq!(colors::rgba(255, 255, 255, 255), colors::WHITE);
        assert_eq!(colors::rgb_f(1.0, 0.0, 0.0), colors::RED);
        assert_eq!(colors::rgba_f(1.0, 1.0, 1.0, 1.0), colors::WHITE);
    }
}
