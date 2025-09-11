//! Core data types and enumerations for ImGuizmo
//!
//! This module defines the fundamental types used throughout the ImGuizmo library,
//! including operation modes, transformation types, and color definitions.

use bitflags::bitflags;

bitflags! {
    /// Gizmo operation types
    ///
    /// These flags can be combined using bitwise OR to create multi-operation gizmos.
    /// For example: `Operation::TRANSLATE | Operation::ROTATE`
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Operation: u32 {
        /// Translation along X axis
        const TRANSLATE_X = 1 << 0;
        /// Translation along Y axis
        const TRANSLATE_Y = 1 << 1;
        /// Translation along Z axis
        const TRANSLATE_Z = 1 << 2;
        /// Rotation around X axis
        const ROTATE_X = 1 << 3;
        /// Rotation around Y axis
        const ROTATE_Y = 1 << 4;
        /// Rotation around Z axis
        const ROTATE_Z = 1 << 5;
        /// Screen space rotation
        const ROTATE_SCREEN = 1 << 6;
        /// Scale along X axis
        const SCALE_X = 1 << 7;
        /// Scale along Y axis
        const SCALE_Y = 1 << 8;
        /// Scale along Z axis
        const SCALE_Z = 1 << 9;
        /// Bounds manipulation
        const BOUNDS = 1 << 10;
        /// Uniform scale along X axis
        const SCALE_XU = 1 << 11;
        /// Uniform scale along Y axis
        const SCALE_YU = 1 << 12;
        /// Uniform scale along Z axis
        const SCALE_ZU = 1 << 13;

        /// All translation operations
        const TRANSLATE = Self::TRANSLATE_X.bits() | Self::TRANSLATE_Y.bits() | Self::TRANSLATE_Z.bits();
        /// All rotation operations
        const ROTATE = Self::ROTATE_X.bits() | Self::ROTATE_Y.bits() | Self::ROTATE_Z.bits() | Self::ROTATE_SCREEN.bits();
        /// All scale operations
        const SCALE = Self::SCALE_X.bits() | Self::SCALE_Y.bits() | Self::SCALE_Z.bits();
        /// All uniform scale operations
        const SCALE_UNIFORM = Self::SCALE_XU.bits() | Self::SCALE_YU.bits() | Self::SCALE_ZU.bits();
        /// Universal gizmo (translate + rotate + uniform scale)
        const UNIVERSAL = Self::TRANSLATE.bits() | Self::ROTATE.bits() | Self::SCALE_UNIFORM.bits();
    }
}

/// Transformation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Mode {
    /// Local space transformation (relative to object)
    Local = 0,
    /// World space transformation (absolute coordinates)
    World = 1,
}

/// Color elements that can be customized in the gizmo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum ColorElement {
    /// X-axis direction color
    DirectionX = 0,
    /// Y-axis direction color
    DirectionY = 1,
    /// Z-axis direction color
    DirectionZ = 2,
    /// X-plane color
    PlaneX = 3,
    /// Y-plane color
    PlaneY = 4,
    /// Z-plane color
    PlaneZ = 5,
    /// Selection highlight color
    Selection = 6,
    /// Inactive element color
    Inactive = 7,
    /// Translation line color
    TranslationLine = 8,
    /// Scale line color
    ScaleLine = 9,
    /// Rotation border color when using
    RotationUsingBorder = 10,
    /// Rotation fill color when using
    RotationUsingFill = 11,
    /// Hatched axis lines color
    HatchedAxisLines = 12,
    /// Text color
    Text = 13,
    /// Text shadow color
    TextShadow = 14,
}

impl ColorElement {
    /// Total number of color elements
    pub const COUNT: usize = 15;
}

/// Rectangle definition for viewport and UI elements
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// X coordinate of the top-left corner
    pub x: f32,
    /// Y coordinate of the top-left corner
    pub y: f32,
    /// Width of the rectangle
    pub width: f32,
    /// Height of the rectangle
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Get the right edge coordinate
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Get the bottom edge coordinate
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Get the center point
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    /// Check if a point is inside the rectangle
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Convert to viewport array format [x, y, width, height]
    pub fn as_viewport(&self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// 2D vector for screen coordinates and sizes
pub type Vec2 = glam::Vec2;

/// 3D vector for world coordinates
pub type Vec3 = glam::Vec3;

/// 4D vector for colors and homogeneous coordinates
pub type Vec4 = glam::Vec4;

/// 4x4 transformation matrix
pub type Mat4 = glam::Mat4;

/// Color represented as RGBA floats (0.0 to 1.0)
pub type Color = [f32; 4];

/// Extension trait for Color operations
pub trait ColorExt {
    /// Convert color to u32 RGBA format (0xAABBGGRR)
    fn as_u32(&self) -> u32;
}

impl ColorExt for Color {
    fn as_u32(&self) -> u32 {
        let r = (self[0].clamp(0.0, 1.0) * 255.0) as u32;
        let g = (self[1].clamp(0.0, 1.0) * 255.0) as u32;
        let b = (self[2].clamp(0.0, 1.0) * 255.0) as u32;
        let a = (self[3].clamp(0.0, 1.0) * 255.0) as u32;
        (a << 24) | (b << 16) | (g << 8) | r
    }
}

/// Default colors for gizmo elements
pub mod colors {
    use super::Color;

    /// Red color for X-axis
    pub const RED: Color = [1.0, 0.0, 0.0, 1.0];
    /// Green color for Y-axis
    pub const GREEN: Color = [0.0, 1.0, 0.0, 1.0];
    /// Blue color for Z-axis
    pub const BLUE: Color = [0.0, 0.0, 1.0, 1.0];
    /// White color
    pub const WHITE: Color = [1.0, 1.0, 1.0, 1.0];
    /// Gray color for inactive elements
    pub const GRAY: Color = [0.5, 0.5, 0.5, 1.0];
    /// Yellow color for selection
    pub const YELLOW: Color = [1.0, 1.0, 0.0, 1.0];
    /// Transparent color
    pub const TRANSPARENT: Color = [0.0, 0.0, 0.0, 0.0];
}

/// Constants used throughout the library
pub mod constants {
    /// PI constant
    pub const PI: f32 = std::f32::consts::PI;
    /// Degrees to radians conversion factor
    pub const DEG_TO_RAD: f32 = PI / 180.0;
    /// Radians to degrees conversion factor
    pub const RAD_TO_DEG: f32 = 180.0 / PI;
    /// Screen rotate size factor
    pub const SCREEN_ROTATE_SIZE: f32 = 0.06;
    /// Rotation display factor for better visibility
    pub const ROTATION_DISPLAY_FACTOR: f32 = 1.2;
    /// Minimum distance for interaction
    pub const MIN_INTERACTION_DISTANCE: f32 = 0.001;
    /// Default gizmo size
    pub const DEFAULT_GIZMO_SIZE: f32 = 1.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_flags() {
        let translate = Operation::TRANSLATE;
        assert!(translate.contains(Operation::TRANSLATE_X));
        assert!(translate.contains(Operation::TRANSLATE_Y));
        assert!(translate.contains(Operation::TRANSLATE_Z));
        assert!(!translate.contains(Operation::ROTATE_X));

        let combined = Operation::TRANSLATE | Operation::ROTATE;
        assert!(combined.contains(Operation::TRANSLATE_X));
        assert!(combined.contains(Operation::ROTATE_Y));
    }

    #[test]
    fn test_rect_operations() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(rect.right(), 110.0);
        assert_eq!(rect.bottom(), 70.0);
        assert_eq!(rect.center(), (60.0, 45.0));
        assert!(rect.contains(50.0, 40.0));
        assert!(!rect.contains(5.0, 40.0));
    }

    #[test]
    fn test_color_element_count() {
        assert_eq!(ColorElement::COUNT, 15);
    }
}
