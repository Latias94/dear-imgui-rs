// Style and theming for plots

use crate::sys;

/// Style variables that can be modified
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StyleVar {
    LineWeight = sys::ImPlotStyleVar_LineWeight as i32,
    Marker = sys::ImPlotStyleVar_Marker as i32,
    MarkerSize = sys::ImPlotStyleVar_MarkerSize as i32,
    MarkerWeight = sys::ImPlotStyleVar_MarkerWeight as i32,
    FillAlpha = sys::ImPlotStyleVar_FillAlpha as i32,
    ErrorBarSize = sys::ImPlotStyleVar_ErrorBarSize as i32,
    ErrorBarWeight = sys::ImPlotStyleVar_ErrorBarWeight as i32,
    DigitalBitHeight = sys::ImPlotStyleVar_DigitalBitHeight as i32,
    DigitalBitGap = sys::ImPlotStyleVar_DigitalBitGap as i32,
    PlotBorderSize = sys::ImPlotStyleVar_PlotBorderSize as i32,
    MinorAlpha = sys::ImPlotStyleVar_MinorAlpha as i32,
    MajorTickLen = sys::ImPlotStyleVar_MajorTickLen as i32,
    MinorTickLen = sys::ImPlotStyleVar_MinorTickLen as i32,
    MajorTickSize = sys::ImPlotStyleVar_MajorTickSize as i32,
    MinorTickSize = sys::ImPlotStyleVar_MinorTickSize as i32,
    MajorGridSize = sys::ImPlotStyleVar_MajorGridSize as i32,
    MinorGridSize = sys::ImPlotStyleVar_MinorGridSize as i32,
    PlotPadding = sys::ImPlotStyleVar_PlotPadding as i32,
    LabelPadding = sys::ImPlotStyleVar_LabelPadding as i32,
    LegendPadding = sys::ImPlotStyleVar_LegendPadding as i32,
    LegendInnerPadding = sys::ImPlotStyleVar_LegendInnerPadding as i32,
    LegendSpacing = sys::ImPlotStyleVar_LegendSpacing as i32,
    MousePosPadding = sys::ImPlotStyleVar_MousePosPadding as i32,
    AnnotationPadding = sys::ImPlotStyleVar_AnnotationPadding as i32,
    FitPadding = sys::ImPlotStyleVar_FitPadding as i32,
    PlotDefaultSize = sys::ImPlotStyleVar_PlotDefaultSize as i32,
    PlotMinSize = sys::ImPlotStyleVar_PlotMinSize as i32,
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
            sys::ImVec2_c {
                x: value[0],
                y: value[1],
            },
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

        sys::ImPlot_PushStyleColor_U32(element as sys::ImPlotCol, color_u32);
    }
    StyleColorToken { was_popped: false }
}

/// Push a colormap to the stack
pub fn push_colormap(preset: crate::Colormap) {
    unsafe {
        sys::ImPlot_PushColormap_PlotColormap(preset as sys::ImPlotColormap);
    }
}

/// Pop a colormap from the stack
pub fn pop_colormap(count: i32) {
    unsafe {
        sys::ImPlot_PopColormap(count);
    }
}

/// Add a custom colormap from a vector of colors
pub fn add_colormap(name: &str, colors: &[sys::ImVec4], qualitative: bool) -> sys::ImPlotColormap {
    let name_cstr = std::ffi::CString::new(name).unwrap();
    unsafe {
        sys::ImPlot_AddColormap_Vec4Ptr(
            name_cstr.as_ptr(),
            colors.as_ptr(),
            colors.len() as i32,
            qualitative,
        ) as sys::ImPlotColormap
    }
}

// Style editor / selectors and input-map helpers

/// Show the ImPlot style editor window
pub fn show_style_editor() {
    unsafe { sys::ImPlot_ShowStyleEditor(std::ptr::null_mut()) }
}

/// Show the ImPlot style selector combo; returns true if selection changed
pub fn show_style_selector(label: &str) -> bool {
    let c = std::ffi::CString::new(label).unwrap_or_default();
    unsafe { sys::ImPlot_ShowStyleSelector(c.as_ptr()) }
}

/// Show the ImPlot colormap selector combo; returns true if selection changed
pub fn show_colormap_selector(label: &str) -> bool {
    let c = std::ffi::CString::new(label).unwrap_or_default();
    unsafe { sys::ImPlot_ShowColormapSelector(c.as_ptr()) }
}

/// Show the ImPlot input-map selector combo; returns true if selection changed
pub fn show_input_map_selector(label: &str) -> bool {
    let c = std::ffi::CString::new(label).unwrap_or_default();
    unsafe { sys::ImPlot_ShowInputMapSelector(c.as_ptr()) }
}

/// Map input to defaults
pub fn map_input_default() {
    unsafe { sys::ImPlot_MapInputDefault(sys::ImPlot_GetInputMap()) }
}

/// Map input to reversed scheme
pub fn map_input_reverse() {
    unsafe { sys::ImPlot_MapInputReverse(sys::ImPlot_GetInputMap()) }
}

// Colormap widgets

/// Draw a colormap scale widget
pub fn colormap_scale(
    label: &str,
    scale_min: f64,
    scale_max: f64,
    height: f32,
    cmap: Option<sys::ImPlotColormap>,
) {
    let c = std::ffi::CString::new(label).unwrap_or_default();
    let size = sys::ImVec2_c { x: 0.0, y: height };
    let fmt_ptr: *const i8 = std::ptr::null();
    let flags: i32 = 0; // ImPlotColormapScaleFlags_None
    unsafe {
        sys::ImPlot_ColormapScale(
            c.as_ptr(),
            scale_min,
            scale_max,
            size,
            fmt_ptr,
            flags,
            cmap.unwrap_or(0),
        )
    }
}

/// Draw a colormap slider; returns true if selection changed
pub fn colormap_slider(
    label: &str,
    t: &mut f32,
    out_color: &mut sys::ImVec4,
    format: Option<&str>,
    cmap: sys::ImPlotColormap,
) -> bool {
    let c = std::ffi::CString::new(label).unwrap_or_default();
    let fmt_c = format.and_then(|s| std::ffi::CString::new(s).ok());
    let fmt_ptr = fmt_c
        .as_ref()
        .map(|cs| cs.as_ptr())
        .unwrap_or(std::ptr::null());
    unsafe {
        sys::ImPlot_ColormapSlider(
            c.as_ptr(),
            t as *mut f32,
            out_color as *mut sys::ImVec4,
            fmt_ptr,
            cmap,
        )
    }
}

/// Draw a colormap picker button; returns true if clicked
pub fn colormap_button(label: &str, size: [f32; 2], cmap: sys::ImPlotColormap) -> bool {
    let c = std::ffi::CString::new(label).unwrap_or_default();
    let sz = sys::ImVec2_c {
        x: size[0],
        y: size[1],
    };
    unsafe { sys::ImPlot_ColormapButton(c.as_ptr(), sz, cmap) }
}
