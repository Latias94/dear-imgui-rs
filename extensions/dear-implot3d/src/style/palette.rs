use crate::flags::Marker3D;
use crate::sys;

use super::tokens::{StyleColorToken, StyleVarToken};
use super::types::{Plot3DColorElement, Plot3DStyleVar};
use std::marker::PhantomData;

#[inline]
pub fn style_colors_dark() {
    unsafe { sys::ImPlot3D_StyleColorsDark(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_light() {
    unsafe { sys::ImPlot3D_StyleColorsLight(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_classic() {
    unsafe { sys::ImPlot3D_StyleColorsClassic(std::ptr::null_mut()) }
}
#[inline]
pub fn style_colors_auto() {
    unsafe { sys::ImPlot3D_StyleColorsAuto(std::ptr::null_mut()) }
}

#[inline]
pub fn push_style_color(element: Plot3DColorElement, col: [f32; 4]) -> StyleColorToken {
    unsafe {
        sys::ImPlot3D_PushStyleColor_Vec4(
            element as sys::ImPlot3DCol,
            crate::imvec4(col[0], col[1], col[2], col[3]),
        );
    }
    StyleColorToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a typed style color override.
#[inline]
pub fn push_style_color_element(element: Plot3DColorElement, col: [f32; 4]) -> StyleColorToken {
    push_style_color(element, col)
}

/// Push a style variable (float variant)
#[inline]
pub fn push_style_var_f32(var: Plot3DStyleVar, val: f32) -> StyleVarToken {
    unsafe { sys::ImPlot3D_PushStyleVar_Float(var as sys::ImPlot3DStyleVar, val) }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push the default marker style variable.
#[inline]
pub fn push_style_var_marker(marker: Marker3D) -> StyleVarToken {
    unsafe {
        sys::ImPlot3D_PushStyleVar_Int(
            Plot3DStyleVar::Marker as sys::ImPlot3DStyleVar,
            marker as sys::ImPlot3DMarker,
        )
    }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

/// Push a style variable (Vec2 variant)
#[inline]
pub fn push_style_var_vec2(var: Plot3DStyleVar, val: [f32; 2]) -> StyleVarToken {
    unsafe {
        sys::ImPlot3D_PushStyleVar_Vec2(var as sys::ImPlot3DStyleVar, crate::imvec2(val[0], val[1]))
    }
    StyleVarToken {
        was_popped: false,
        _not_send_or_sync: PhantomData,
    }
}

#[inline]
pub fn set_next_line_style(col: [f32; 4], weight: f32) {
    crate::update_next_plot3d_spec(|spec| {
        spec.LineColor = crate::imvec4(col[0], col[1], col[2], col[3]);
        spec.LineWeight = weight;
    })
}

#[inline]
pub fn set_next_fill_style(col: [f32; 4], alpha_mod: f32) {
    crate::update_next_plot3d_spec(|spec| {
        spec.FillColor = crate::imvec4(col[0], col[1], col[2], col[3]);
        spec.FillAlpha = alpha_mod;
    })
}

#[inline]
pub fn set_next_marker_style(
    marker: Marker3D,
    size: f32,
    fill: [f32; 4],
    weight: f32,
    outline: [f32; 4],
) {
    crate::update_next_plot3d_spec(|spec| {
        spec.Marker = marker as sys::ImPlot3DMarker;
        spec.MarkerSize = size;
        spec.MarkerFillColor = crate::imvec4(fill[0], fill[1], fill[2], fill[3]);
        spec.MarkerLineColor = crate::imvec4(outline[0], outline[1], outline[2], outline[3]);
        spec.LineWeight = weight;
    })
}
