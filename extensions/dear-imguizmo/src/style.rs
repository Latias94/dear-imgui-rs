//! Style configuration for ImGuizmo
//!
//! This module provides comprehensive styling options for customizing the appearance
//! of gizmo elements, including colors, line thickness, and sizes.

use crate::types::colors;
use crate::types::{Color, ColorElement};

/// Style configuration for ImGuizmo
///
/// This structure contains all the visual customization options for gizmo rendering.
/// It follows the same pattern as the original ImGuizmo C++ library.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Thickness of lines for translation gizmo
    pub translation_line_thickness: f32,
    /// Size of arrow at the end of lines for translation gizmo
    pub translation_line_arrow_size: f32,
    /// Thickness of lines for rotation gizmo
    pub rotation_line_thickness: f32,
    /// Thickness of line surrounding the rotation gizmo
    pub rotation_outer_line_thickness: f32,
    /// Thickness of lines for scale gizmo
    pub scale_line_thickness: f32,
    /// Size of circle at the end of lines for scale gizmo
    pub scale_line_circle_size: f32,
    /// Thickness of hatched axis lines
    pub hatched_axis_line_thickness: f32,
    /// Size of circle at the center of the translate/scale gizmo
    pub center_circle_size: f32,
    /// Colors for different gizmo elements
    pub colors: [Color; ColorElement::COUNT],
}

impl Default for Style {
    fn default() -> Self {
        Self::new()
    }
}

impl Style {
    /// Create a new style with default values
    pub fn new() -> Self {
        let mut colors = [colors::WHITE; ColorElement::COUNT];

        // Set default colors for each element
        colors[ColorElement::DirectionX as usize] = colors::RED;
        colors[ColorElement::DirectionY as usize] = colors::GREEN;
        colors[ColorElement::DirectionZ as usize] = colors::BLUE;
        colors[ColorElement::PlaneX as usize] = [1.0, 0.0, 0.0, 0.5]; // Semi-transparent red
        colors[ColorElement::PlaneY as usize] = [0.0, 1.0, 0.0, 0.5]; // Semi-transparent green
        colors[ColorElement::PlaneZ as usize] = [0.0, 0.0, 1.0, 0.5]; // Semi-transparent blue
        colors[ColorElement::Selection as usize] = colors::YELLOW;
        colors[ColorElement::Inactive as usize] = colors::GRAY;
        colors[ColorElement::TranslationLine as usize] = [0.666, 0.666, 0.666, 0.666];
        colors[ColorElement::ScaleLine as usize] = [0.250, 0.250, 0.250, 1.0];
        colors[ColorElement::RotationUsingBorder as usize] = [1.0, 0.5, 0.0, 1.0]; // Orange
        colors[ColorElement::RotationUsingFill as usize] = [1.0, 0.5, 0.0, 0.5]; // Semi-transparent orange
        colors[ColorElement::HatchedAxisLines as usize] = [0.0, 0.0, 0.0, 0.5]; // Semi-transparent black
        colors[ColorElement::Text as usize] = colors::WHITE;
        colors[ColorElement::TextShadow as usize] = [0.0, 0.0, 0.0, 1.0]; // Black

        Self {
            translation_line_thickness: 3.0,
            translation_line_arrow_size: 6.0,
            rotation_line_thickness: 2.0,
            rotation_outer_line_thickness: 3.0,
            scale_line_thickness: 3.0,
            scale_line_circle_size: 6.0,
            hatched_axis_line_thickness: 6.0,
            center_circle_size: 6.0,
            colors,
        }
    }

    /// Create a dark theme style
    pub fn dark() -> Self {
        let mut style = Self::new();

        // Adjust colors for dark theme
        style.colors[ColorElement::DirectionX as usize] = [0.8, 0.2, 0.2, 1.0]; // Darker red
        style.colors[ColorElement::DirectionY as usize] = [0.2, 0.8, 0.2, 1.0]; // Darker green
        style.colors[ColorElement::DirectionZ as usize] = [0.2, 0.2, 0.8, 1.0]; // Darker blue
        style.colors[ColorElement::Selection as usize] = [1.0, 0.8, 0.0, 1.0]; // Darker yellow
        style.colors[ColorElement::Inactive as usize] = [0.3, 0.3, 0.3, 1.0]; // Darker gray
        style.colors[ColorElement::Text as usize] = [0.9, 0.9, 0.9, 1.0]; // Light gray text

        style
    }

    /// Create a light theme style
    pub fn light() -> Self {
        let mut style = Self::new();

        // Adjust colors for light theme
        style.colors[ColorElement::Text as usize] = [0.1, 0.1, 0.1, 1.0]; // Dark text
        style.colors[ColorElement::TextShadow as usize] = [1.0, 1.0, 1.0, 0.8]; // Light shadow
        style.colors[ColorElement::Inactive as usize] = [0.7, 0.7, 0.7, 1.0]; // Lighter gray

        style
    }

    /// Get the color for a specific element
    pub fn get_color(&self, element: ColorElement) -> Color {
        self.colors[element as usize]
    }

    /// Set the color for a specific element
    pub fn set_color(&mut self, element: ColorElement, color: Color) {
        self.colors[element as usize] = color;
    }

    /// Set all axis colors at once
    pub fn set_axis_colors(&mut self, x: Color, y: Color, z: Color) {
        self.colors[ColorElement::DirectionX as usize] = x;
        self.colors[ColorElement::DirectionY as usize] = y;
        self.colors[ColorElement::DirectionZ as usize] = z;
    }

    /// Set all plane colors at once
    pub fn set_plane_colors(&mut self, x: Color, y: Color, z: Color) {
        self.colors[ColorElement::PlaneX as usize] = x;
        self.colors[ColorElement::PlaneY as usize] = y;
        self.colors[ColorElement::PlaneZ as usize] = z;
    }

    /// Scale all line thicknesses by a factor
    pub fn scale_line_thickness(&mut self, factor: f32) {
        self.translation_line_thickness *= factor;
        self.rotation_line_thickness *= factor;
        self.rotation_outer_line_thickness *= factor;
        self.scale_line_thickness *= factor;
        self.hatched_axis_line_thickness *= factor;
    }

    /// Scale all sizes by a factor
    pub fn scale_sizes(&mut self, factor: f32) {
        self.translation_line_arrow_size *= factor;
        self.scale_line_circle_size *= factor;
        self.center_circle_size *= factor;
    }

    /// Validate the style configuration
    pub fn validate(&self) -> Result<(), crate::GuizmoError> {
        // Check for negative values
        if self.translation_line_thickness < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "translation_line_thickness cannot be negative",
            ));
        }
        if self.translation_line_arrow_size < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "translation_line_arrow_size cannot be negative",
            ));
        }
        if self.rotation_line_thickness < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "rotation_line_thickness cannot be negative",
            ));
        }
        if self.rotation_outer_line_thickness < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "rotation_outer_line_thickness cannot be negative",
            ));
        }
        if self.scale_line_thickness < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "scale_line_thickness cannot be negative",
            ));
        }
        if self.scale_line_circle_size < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "scale_line_circle_size cannot be negative",
            ));
        }
        if self.hatched_axis_line_thickness < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "hatched_axis_line_thickness cannot be negative",
            ));
        }
        if self.center_circle_size < 0.0 {
            return Err(crate::GuizmoError::style_configuration(
                "center_circle_size cannot be negative",
            ));
        }

        // Validate colors (check for valid alpha values)
        for (i, color) in self.colors.iter().enumerate() {
            if color[3] < 0.0 || color[3] > 1.0 {
                return Err(crate::GuizmoError::style_configuration(format!(
                    "Invalid alpha value for color element {}: {}",
                    i, color[3]
                )));
            }
        }

        Ok(())
    }
}

/// Builder pattern for creating custom styles
pub struct StyleBuilder {
    style: Style,
}

impl StyleBuilder {
    /// Create a new style builder with default values
    pub fn new() -> Self {
        Self {
            style: Style::new(),
        }
    }

    /// Start with a dark theme
    pub fn dark() -> Self {
        Self {
            style: Style::dark(),
        }
    }

    /// Start with a light theme
    pub fn light() -> Self {
        Self {
            style: Style::light(),
        }
    }

    /// Set translation line thickness
    pub fn translation_line_thickness(mut self, thickness: f32) -> Self {
        self.style.translation_line_thickness = thickness;
        self
    }

    /// Set rotation line thickness
    pub fn rotation_line_thickness(mut self, thickness: f32) -> Self {
        self.style.rotation_line_thickness = thickness;
        self
    }

    /// Set scale line thickness
    pub fn scale_line_thickness(mut self, thickness: f32) -> Self {
        self.style.scale_line_thickness = thickness;
        self
    }

    /// Set axis colors
    pub fn axis_colors(mut self, x: Color, y: Color, z: Color) -> Self {
        self.style.set_axis_colors(x, y, z);
        self
    }

    /// Set a specific color
    pub fn color(mut self, element: ColorElement, color: Color) -> Self {
        self.style.set_color(element, color);
        self
    }

    /// Build the final style
    pub fn build(self) -> Result<Style, crate::GuizmoError> {
        self.style.validate()?;
        Ok(self.style)
    }
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_style() {
        let style = Style::new();
        assert_eq!(style.translation_line_thickness, 3.0);
        assert_eq!(style.get_color(ColorElement::DirectionX), colors::RED);
        assert_eq!(style.get_color(ColorElement::DirectionY), colors::GREEN);
        assert_eq!(style.get_color(ColorElement::DirectionZ), colors::BLUE);
    }

    #[test]
    fn test_style_builder() {
        let style = StyleBuilder::new()
            .translation_line_thickness(5.0)
            .axis_colors(colors::WHITE, colors::WHITE, colors::WHITE)
            .build()
            .unwrap();

        assert_eq!(style.translation_line_thickness, 5.0);
        assert_eq!(style.get_color(ColorElement::DirectionX), colors::WHITE);
    }

    #[test]
    fn test_style_validation() {
        let mut style = Style::new();
        style.translation_line_thickness = -1.0;
        assert!(style.validate().is_err());
    }

    #[test]
    fn test_color_operations() {
        let mut style = Style::new();
        let custom_color = [0.5, 0.5, 0.5, 1.0];

        style.set_color(ColorElement::Selection, custom_color);
        assert_eq!(style.get_color(ColorElement::Selection), custom_color);
    }
}
