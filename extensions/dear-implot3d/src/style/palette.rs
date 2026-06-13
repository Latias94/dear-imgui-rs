use crate::flags::Marker3D;
use crate::sys;
use crate::{Plot3DContext, Plot3DUi};

use super::tokens::{StyleColorToken, StyleVarToken};
use super::types::{Plot3DColorElement, Plot3DStyleVar};
use std::marker::PhantomData;

impl Plot3DContext {
    #[inline]
    fn with_bound_palette<R>(&self, caller: &str, f: impl FnOnce() -> R) -> R {
        self.assert_imgui_alive(caller);
        let _guard = self.binding().bind();
        f()
    }

    /// Apply ImPlot3D's dark style palette to this context.
    #[inline]
    pub fn style_colors_dark(&self) {
        self.with_bound_palette(
            "dear-implot3d: Plot3DContext::style_colors_dark()",
            || unsafe { sys::ImPlot3D_StyleColorsDark(std::ptr::null_mut()) },
        )
    }

    /// Apply ImPlot3D's light style palette to this context.
    #[inline]
    pub fn style_colors_light(&self) {
        self.with_bound_palette(
            "dear-implot3d: Plot3DContext::style_colors_light()",
            || unsafe { sys::ImPlot3D_StyleColorsLight(std::ptr::null_mut()) },
        )
    }

    /// Apply ImPlot3D's classic style palette to this context.
    #[inline]
    pub fn style_colors_classic(&self) {
        self.with_bound_palette(
            "dear-implot3d: Plot3DContext::style_colors_classic()",
            || unsafe { sys::ImPlot3D_StyleColorsClassic(std::ptr::null_mut()) },
        )
    }

    /// Apply ImPlot3D's auto style palette to this context.
    #[inline]
    pub fn style_colors_auto(&self) {
        self.with_bound_palette(
            "dear-implot3d: Plot3DContext::style_colors_auto()",
            || unsafe { sys::ImPlot3D_StyleColorsAuto(std::ptr::null_mut()) },
        )
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Push a style color override to this ImPlot3D context's stack.
    #[inline]
    pub fn push_style_color(
        &self,
        element: Plot3DColorElement,
        col: [f32; 4],
    ) -> StyleColorToken<'_> {
        let _guard = self.binding.bind();
        unsafe {
            sys::ImPlot3D_PushStyleColor_Vec4(
                element as sys::ImPlot3DCol,
                crate::imvec4(col[0], col[1], col[2], col[3]),
            );
        }
        StyleColorToken {
            binding: self.binding,
            imgui_alive: self.imgui_alive.clone(),
            was_popped: false,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }

    /// Push a typed style color override.
    #[inline]
    pub fn push_style_color_element(
        &self,
        element: Plot3DColorElement,
        col: [f32; 4],
    ) -> StyleColorToken<'_> {
        self.push_style_color(element, col)
    }

    /// Push a style variable (float variant).
    #[inline]
    pub fn push_style_var_f32(&self, var: Plot3DStyleVar, val: f32) -> StyleVarToken<'_> {
        let _guard = self.binding.bind();
        unsafe { sys::ImPlot3D_PushStyleVar_Float(var as sys::ImPlot3DStyleVar, val) }
        StyleVarToken {
            binding: self.binding,
            imgui_alive: self.imgui_alive.clone(),
            was_popped: false,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }

    /// Push the default marker style variable.
    #[inline]
    pub fn push_style_var_marker(&self, marker: Marker3D) -> StyleVarToken<'_> {
        let _guard = self.binding.bind();
        unsafe {
            sys::ImPlot3D_PushStyleVar_Int(
                Plot3DStyleVar::Marker as sys::ImPlot3DStyleVar,
                marker as sys::ImPlot3DMarker,
            )
        }
        StyleVarToken {
            binding: self.binding,
            imgui_alive: self.imgui_alive.clone(),
            was_popped: false,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }

    /// Push a style variable (Vec2 variant).
    #[inline]
    pub fn push_style_var_vec2(&self, var: Plot3DStyleVar, val: [f32; 2]) -> StyleVarToken<'_> {
        let _guard = self.binding.bind();
        unsafe {
            sys::ImPlot3D_PushStyleVar_Vec2(
                var as sys::ImPlot3DStyleVar,
                crate::imvec2(val[0], val[1]),
            )
        }
        StyleVarToken {
            binding: self.binding,
            imgui_alive: self.imgui_alive.clone(),
            was_popped: false,
            _lifetime: PhantomData,
            _not_send_or_sync: PhantomData,
        }
    }
}

impl Plot3DUi<'_> {
    /// Set the line style for the next ImPlot3D item submitted through this context.
    #[inline]
    pub fn set_next_line_style(&self, col: [f32; 4], weight: f32) {
        let _guard = self.bind();
        crate::update_next_plot3d_spec(|spec| {
            spec.LineColor = crate::imvec4(col[0], col[1], col[2], col[3]);
            spec.LineWeight = weight;
        })
    }

    /// Set the fill style for the next ImPlot3D item submitted through this context.
    #[inline]
    pub fn set_next_fill_style(&self, col: [f32; 4], alpha_mod: f32) {
        let _guard = self.bind();
        crate::update_next_plot3d_spec(|spec| {
            spec.FillColor = crate::imvec4(col[0], col[1], col[2], col[3]);
            spec.FillAlpha = alpha_mod;
        })
    }

    /// Set the marker style for the next ImPlot3D item submitted through this context.
    #[inline]
    pub fn set_next_marker_style(
        &self,
        marker: Marker3D,
        size: f32,
        fill: [f32; 4],
        weight: f32,
        outline: [f32; 4],
    ) {
        let _guard = self.bind();
        crate::update_next_plot3d_spec(|spec| {
            spec.Marker = marker as sys::ImPlot3DMarker;
            spec.MarkerSize = size;
            spec.MarkerFillColor = crate::imvec4(fill[0], fill[1], fill[2], fill[3]);
            spec.MarkerLineColor = crate::imvec4(outline[0], outline[1], outline[2], outline[3]);
            spec.LineWeight = weight;
        })
    }
}
