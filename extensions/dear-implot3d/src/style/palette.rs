use crate::flags::Marker3D;
use crate::sys;
use crate::{Plot3DContext, Plot3DScope, Plot3DUi};

use super::types::{Plot3DColorElement, Plot3DStyleVar};

impl Plot3DContext {
    /// Apply ImPlot3D's dark style palette to this context.
    #[inline]
    pub fn style_colors_dark(&self) {
        self.with_bound_context(|| unsafe { sys::ImPlot3D_StyleColorsDark(std::ptr::null_mut()) })
    }

    /// Apply ImPlot3D's light style palette to this context.
    #[inline]
    pub fn style_colors_light(&self) {
        self.with_bound_context(|| unsafe { sys::ImPlot3D_StyleColorsLight(std::ptr::null_mut()) })
    }

    /// Apply ImPlot3D's classic style palette to this context.
    #[inline]
    pub fn style_colors_classic(&self) {
        self.with_bound_context(|| unsafe {
            sys::ImPlot3D_StyleColorsClassic(std::ptr::null_mut())
        })
    }

    /// Apply ImPlot3D's auto style palette to this context.
    #[inline]
    pub fn style_colors_auto(&self) {
        self.with_bound_context(|| unsafe { sys::ImPlot3D_StyleColorsAuto(std::ptr::null_mut()) })
    }
}

impl Plot3DUi<'_> {
    /// Applies these colors to this context for the duration of `f`.
    pub fn with_style_colors<R>(
        &self,
        colors: &[(Plot3DColorElement, [f32; 4])],
        f: impl FnOnce() -> R,
    ) -> R {
        self.with_style_scope(
            colors.len(),
            || {
                for &(element, [r, g, b, a]) in colors {
                    unsafe {
                        sys::ImPlot3D_PushStyleColor_Vec4(element as _, crate::imvec4(r, g, b, a))
                    };
                }
            },
            sys::ImPlot3D_PopStyleColor,
            f,
        )
    }

    /// Applies the typed style variables in order and restores them when `f` returns or unwinds.
    pub fn with_style_vars<R>(&self, vars: &[Plot3DStyleVar], f: impl FnOnce() -> R) -> R {
        self.with_style_scope(
            vars.len(),
            || {
                for var in vars {
                    unsafe { var.push() };
                }
            },
            sys::ImPlot3D_PopStyleVar,
            f,
        )
    }
}

impl Plot3DScope<'_> {
    /// Set the line style for the next ImPlot3D item submitted to this plot.
    #[inline]
    pub fn set_next_line_style(&self, col: [f32; 4], weight: f32) {
        self.next_item.update(|spec| {
            spec.LineColor = crate::imvec4(col[0], col[1], col[2], col[3]);
            spec.LineWeight = weight;
        })
    }

    /// Set the fill style for the next ImPlot3D item submitted to this plot.
    #[inline]
    pub fn set_next_fill_style(&self, col: [f32; 4], alpha_mod: f32) {
        self.next_item.update(|spec| {
            spec.FillColor = crate::imvec4(col[0], col[1], col[2], col[3]);
            spec.FillAlpha = alpha_mod;
        })
    }

    /// Set the marker style for the next ImPlot3D item submitted to this plot.
    #[inline]
    pub fn set_next_marker_style(
        &self,
        marker: Marker3D,
        size: f32,
        fill: [f32; 4],
        weight: f32,
        outline: [f32; 4],
    ) {
        self.next_item.update(|spec| {
            spec.Marker = marker as sys::ImPlot3DMarker;
            spec.MarkerSize = size;
            spec.MarkerFillColor = crate::imvec4(fill[0], fill[1], fill[2], fill[3]);
            spec.MarkerLineColor = crate::imvec4(outline[0], outline[1], outline[2], outline[3]);
            spec.LineWeight = weight;
        })
    }
}
