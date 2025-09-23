// Style and theming for plots

use crate::sys;

/// Style variables that can be modified
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StyleVar {
    LineWeight = sys::ImPlotStyleVar_LineWeight,
    Marker = sys::ImPlotStyleVar_Marker,
    MarkerSize = sys::ImPlotStyleVar_MarkerSize,
    MarkerWeight = sys::ImPlotStyleVar_MarkerWeight,
    FillAlpha = sys::ImPlotStyleVar_FillAlpha,
    ErrorBarSize = sys::ImPlotStyleVar_ErrorBarSize,
    ErrorBarWeight = sys::ImPlotStyleVar_ErrorBarWeight,
    DigitalBitHeight = sys::ImPlotStyleVar_DigitalBitHeight,
    DigitalBitGap = sys::ImPlotStyleVar_DigitalBitGap,
    PlotBorderSize = sys::ImPlotStyleVar_PlotBorderSize,
    MinorAlpha = sys::ImPlotStyleVar_MinorAlpha,
    MajorTickLen = sys::ImPlotStyleVar_MajorTickLen,
    MinorTickLen = sys::ImPlotStyleVar_MinorTickLen,
    MajorTickSize = sys::ImPlotStyleVar_MajorTickSize,
    MinorTickSize = sys::ImPlotStyleVar_MinorTickSize,
    MajorGridSize = sys::ImPlotStyleVar_MajorGridSize,
    MinorGridSize = sys::ImPlotStyleVar_MinorGridSize,
    PlotPadding = sys::ImPlotStyleVar_PlotPadding,
    LabelPadding = sys::ImPlotStyleVar_LabelPadding,
    LegendPadding = sys::ImPlotStyleVar_LegendPadding,
    LegendInnerPadding = sys::ImPlotStyleVar_LegendInnerPadding,
    LegendSpacing = sys::ImPlotStyleVar_LegendSpacing,
    MousePosPadding = sys::ImPlotStyleVar_MousePosPadding,
    AnnotationPadding = sys::ImPlotStyleVar_AnnotationPadding,
    FitPadding = sys::ImPlotStyleVar_FitPadding,
    PlotDefaultSize = sys::ImPlotStyleVar_PlotDefaultSize,
    PlotMinSize = sys::ImPlotStyleVar_PlotMinSize,
}

/// Token for managing style variable changes
pub struct StyleVarToken {
    was_popped: bool,
}

impl StyleVarToken {
    /// Pop this style variable from the stack
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop a style var token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot_PopStyleVar(1);
        }
    }
}

impl Drop for StyleVarToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot_PopStyleVar(1);
            }
        }
    }
}

/// Token for managing style color changes
pub struct StyleColorToken {
    was_popped: bool,
}

impl StyleColorToken {
    /// Pop this style color from the stack
    pub fn pop(mut self) {
        if self.was_popped {
            panic!("Attempted to pop a style color token twice.");
        }
        self.was_popped = true;
        unsafe {
            sys::ImPlot_PopStyleColor(1);
        }
    }
}

impl Drop for StyleColorToken {
    fn drop(&mut self) {
        if !self.was_popped {
            unsafe {
                sys::ImPlot_PopStyleColor(1);
            }
        }
    }
}

/// Push a float style variable to the stack
pub fn push_style_var_f32(var: StyleVar, value: f32) -> StyleVarToken {
    unsafe {
        sys::ImPlot_PushStyleVar_Float(var as sys::ImPlotStyleVar, value);
    }
    StyleVarToken { was_popped: false }
}

/// Push an integer style variable to the stack (converted to float)
pub fn push_style_var_i32(var: StyleVar, value: i32) -> StyleVarToken {
    unsafe {
        sys::ImPlot_PushStyleVar_Int(var as sys::ImPlotStyleVar, value);
    }
    StyleVarToken { was_popped: false }
}

/// Push a Vec2 style variable to the stack
pub fn push_style_var_vec2(var: StyleVar, value: [f32; 2]) -> StyleVarToken {
    unsafe {
        sys::ImPlot_PushStyleVar_Vec2(
            var as sys::ImPlotStyleVar,
            sys::ImVec2 { x: value[0], y: value[1] },
        );
    }
    StyleVarToken { was_popped: false }
}

/// Push a style color to the stack
pub fn push_style_color(element: crate::PlotColorElement, color: [f32; 4]) -> StyleColorToken {
    unsafe {
        // Convert color to ImU32 format (RGBA)
        let r = (color[0] * 255.0) as u32;
        let g = (color[1] * 255.0) as u32;
        let b = (color[2] * 255.0) as u32;
        let a = (color[3] * 255.0) as u32;
        let color_u32 = (a << 24) | (b << 16) | (g << 8) | r;

        sys::ImPlot_PushStyleColor_U32(element as sys::ImPlotCol_, color_u32);
    }
    StyleColorToken { was_popped: false }
}

/// Push a colormap to the stack
pub fn push_colormap(preset: crate::Colormap) {
    unsafe {
        sys::ImPlot_PushColormap_PlotColormap(preset as sys::ImPlotColormap_);
    }
}

/// Pop a colormap from the stack
pub fn pop_colormap(count: i32) {
    unsafe {
        sys::ImPlot_PopColormap(count);
    }
}

/// Add a custom colormap from a vector of colors
pub fn add_colormap(name: &str, colors: &[sys::ImVec4], qualitative: bool) -> sys::ImPlotColormap_ {
    let name_cstr = std::ffi::CString::new(name).unwrap();
    unsafe {
        sys::ImPlot_AddColormap_Vec4Ptr(name_cstr.as_ptr(), colors.as_ptr(), colors.len() as i32, qualitative)
    }
}
