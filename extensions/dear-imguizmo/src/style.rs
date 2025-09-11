//! ImGuizmo styling and appearance configuration

use crate::{sys, Color, ColorType, GuizmoUi};

/// ImGuizmo style configuration
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
    pub colors: [Color; 15], // COLOR::COUNT
}

impl Default for Style {
    fn default() -> Self {
        Self {
            translation_line_thickness: 3.0,
            translation_line_arrow_size: 6.0,
            rotation_line_thickness: 2.0,
            rotation_outer_line_thickness: 3.0,
            scale_line_thickness: 3.0,
            scale_line_circle_size: 6.0,
            hatched_axis_line_thickness: 6.0,
            center_circle_size: 6.0,
            colors: [
                [0.666, 0.000, 0.000, 1.000], // DIRECTION_X
                [0.000, 0.666, 0.000, 1.000], // DIRECTION_Y
                [0.000, 0.000, 0.666, 1.000], // DIRECTION_Z
                [0.666, 0.000, 0.000, 0.380], // PLANE_X
                [0.000, 0.666, 0.000, 0.380], // PLANE_Y
                [0.000, 0.000, 0.666, 0.380], // PLANE_Z
                [1.000, 0.500, 0.062, 0.541], // SELECTION
                [0.600, 0.600, 0.600, 0.600], // INACTIVE
                [0.666, 0.000, 0.000, 0.666], // TRANSLATION_LINE
                [0.666, 0.000, 0.000, 0.666], // SCALE_LINE
                [1.000, 1.000, 1.000, 1.000], // ROTATION_USING_BORDER
                [1.000, 1.000, 1.000, 0.500], // ROTATION_USING_FILL
                [0.000, 0.000, 0.000, 0.500], // HATCHED_AXIS_LINES
                [1.000, 1.000, 1.000, 1.000], // TEXT
                [0.000, 0.000, 0.000, 1.000], // TEXT_SHADOW
            ],
        }
    }
}

impl<'ui> GuizmoUi<'ui> {
    /// Get the current ImGuizmo style
    pub fn get_style(&self) -> Style {
        let mut style = Style::default();

        unsafe {
            let imgui_style = sys::ImGuizmo_GetStyle();
            style.translation_line_thickness = (*imgui_style).TranslationLineThickness;
            style.translation_line_arrow_size = (*imgui_style).TranslationLineArrowSize;
            style.rotation_line_thickness = (*imgui_style).RotationLineThickness;
            style.rotation_outer_line_thickness = (*imgui_style).RotationOuterLineThickness;
            style.scale_line_thickness = (*imgui_style).ScaleLineThickness;
            style.scale_line_circle_size = (*imgui_style).ScaleLineCircleSize;
            style.hatched_axis_line_thickness = (*imgui_style).HatchedAxisLineThickness;
            style.center_circle_size = (*imgui_style).CenterCircleSize;

            // Copy colors
            for i in 0..style.colors.len().min(15) {
                // COLOR::COUNT is 15
                style.colors[i] = [
                    (*imgui_style).Colors[i].x,
                    (*imgui_style).Colors[i].y,
                    (*imgui_style).Colors[i].z,
                    (*imgui_style).Colors[i].w,
                ];
            }
        }

        style
    }

    /// Set the ImGuizmo style
    pub fn set_style(&self, style: &Style) {
        unsafe {
            let imgui_style = sys::ImGuizmo_GetStyle();
            (*imgui_style).TranslationLineThickness = style.translation_line_thickness;
            (*imgui_style).TranslationLineArrowSize = style.translation_line_arrow_size;
            (*imgui_style).RotationLineThickness = style.rotation_line_thickness;
            (*imgui_style).RotationOuterLineThickness = style.rotation_outer_line_thickness;
            (*imgui_style).ScaleLineThickness = style.scale_line_thickness;
            (*imgui_style).ScaleLineCircleSize = style.scale_line_circle_size;
            (*imgui_style).HatchedAxisLineThickness = style.hatched_axis_line_thickness;
            (*imgui_style).CenterCircleSize = style.center_circle_size;

            // Copy colors
            for i in 0..style.colors.len().min(15) {
                // COLOR::COUNT is 15
                (*imgui_style).Colors[i].x = style.colors[i][0];
                (*imgui_style).Colors[i].y = style.colors[i][1];
                (*imgui_style).Colors[i].z = style.colors[i][2];
                (*imgui_style).Colors[i].w = style.colors[i][3];
            }
        }
    }

    /// Set a specific color in the style
    pub fn set_color(&self, color_type: ColorType, color: Color) {
        let mut style = self.get_style();
        style.colors[color_type as usize] = color;
        self.set_style(&style);
    }

    /// Get a specific color from the style
    pub fn get_color(&self, color_type: ColorType) -> Color {
        let style = self.get_style();
        style.colors[color_type as usize]
    }

    /// Set the gizmo size in clip space
    ///
    /// Controls the overall size of the gizmo. Default is typically around 0.1.
    pub fn set_gizmo_size_clip_space(&self, value: f32) {
        unsafe {
            sys::ImGuizmo_SetGizmoSizeClipSpace(value);
        }
    }

    /// Allow or disallow axis flipping
    ///
    /// When true (default), gizmo axes flip for better visibility.
    /// When false, they always stay along the positive world/local axis.
    pub fn allow_axis_flip(&self, value: bool) {
        unsafe {
            sys::ImGuizmo_AllowAxisFlip(value);
        }
    }

    /// Set the limit where axes are hidden
    ///
    /// Controls at what angle axes become hidden for better visibility.
    pub fn set_axis_limit(&self, value: f32) {
        unsafe {
            sys::ImGuizmo_SetAxisLimit(value);
        }
    }

    /// Set an axis mask to permanently hide given axes
    ///
    /// # Arguments
    /// * `x` - true to hide X axis, false to show
    /// * `y` - true to hide Y axis, false to show  
    /// * `z` - true to hide Z axis, false to show
    pub fn set_axis_mask(&self, x: bool, y: bool, z: bool) {
        unsafe {
            sys::ImGuizmo_SetAxisMask(x, y, z);
        }
    }

    /// Set the limit where planes are hidden
    ///
    /// Controls at what angle manipulation planes become hidden.
    pub fn set_plane_limit(&self, value: f32) {
        unsafe {
            sys::ImGuizmo_SetPlaneLimit(value);
        }
    }
}

/// Style builder for fluent configuration
pub struct StyleBuilder {
    style: Style,
}

impl StyleBuilder {
    /// Create a new style builder with default values
    pub fn new() -> Self {
        Self {
            style: Style::default(),
        }
    }

    /// Set translation line thickness
    pub fn translation_line_thickness(mut self, thickness: f32) -> Self {
        self.style.translation_line_thickness = thickness;
        self
    }

    /// Set translation line arrow size
    pub fn translation_line_arrow_size(mut self, size: f32) -> Self {
        self.style.translation_line_arrow_size = size;
        self
    }

    /// Set rotation line thickness
    pub fn rotation_line_thickness(mut self, thickness: f32) -> Self {
        self.style.rotation_line_thickness = thickness;
        self
    }

    /// Set rotation outer line thickness
    pub fn rotation_outer_line_thickness(mut self, thickness: f32) -> Self {
        self.style.rotation_outer_line_thickness = thickness;
        self
    }

    /// Set scale line thickness
    pub fn scale_line_thickness(mut self, thickness: f32) -> Self {
        self.style.scale_line_thickness = thickness;
        self
    }

    /// Set scale line circle size
    pub fn scale_line_circle_size(mut self, size: f32) -> Self {
        self.style.scale_line_circle_size = size;
        self
    }

    /// Set hatched axis line thickness
    pub fn hatched_axis_line_thickness(mut self, thickness: f32) -> Self {
        self.style.hatched_axis_line_thickness = thickness;
        self
    }

    /// Set center circle size
    pub fn center_circle_size(mut self, size: f32) -> Self {
        self.style.center_circle_size = size;
        self
    }

    /// Set a specific color
    pub fn color(mut self, color_type: ColorType, color: Color) -> Self {
        self.style.colors[color_type as usize] = color;
        self
    }

    /// Build the style
    pub fn build(self) -> Style {
        self.style
    }
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}
