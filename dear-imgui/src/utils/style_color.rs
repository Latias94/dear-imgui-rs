use super::validation::{assert_finite_f32, assert_finite_vec4};
use crate::{StyleColor, sys};

impl crate::ui::Ui {
    /// Returns a single style color from the user interface style.
    ///
    /// Use this function if you need to access the colors, but don't want to clone the entire
    /// style object.
    #[doc(alias = "GetStyle", alias = "GetStyleColorVec4")]
    pub fn style_color(&self, style_color: StyleColor) -> [f32; 4] {
        unsafe {
            let color = sys::igGetStyleColorVec4(style_color as sys::ImGuiCol);
            let color = &*color;
            [color.x, color.y, color.z, color.w]
        }
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from a style color.
    ///
    /// This is a convenience wrapper over `ImGui::GetColorU32(ImGuiCol, alpha_mul)`.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32(&self, style_color: StyleColor) -> u32 {
        self.get_color_u32_with_alpha(style_color, 1.0)
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from a style color, with alpha multiplier.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32_with_alpha(&self, style_color: StyleColor, alpha_mul: f32) -> u32 {
        assert_finite_f32("Ui::get_color_u32_with_alpha()", "alpha_mul", alpha_mul);
        unsafe { sys::igGetColorU32_Col(style_color as sys::ImGuiCol, alpha_mul) }
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from an RGBA float color.
    ///
    /// Note: Dear ImGui applies the global style alpha when converting colors for rendering.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32_from_rgba(&self, rgba: [f32; 4]) -> u32 {
        assert_finite_vec4("Ui::get_color_u32_from_rgba()", "rgba", rgba);
        unsafe {
            sys::igGetColorU32_Vec4(sys::ImVec4_c {
                x: rgba[0],
                y: rgba[1],
                z: rgba[2],
                w: rgba[3],
            })
        }
    }

    /// Returns an ImGui-packed ABGR color (`ImU32`) from an existing packed color, with alpha multiplier.
    #[doc(alias = "GetColorU32")]
    pub fn get_color_u32_from_packed(&self, abgr: u32, alpha_mul: f32) -> u32 {
        assert_finite_f32("Ui::get_color_u32_from_packed()", "alpha_mul", alpha_mul);
        unsafe { sys::igGetColorU32_U32(abgr, alpha_mul) }
    }

    /// Returns the name of a style color.
    ///
    /// This is just a wrapper around calling [`name`] on [StyleColor].
    ///
    /// [`name`]: StyleColor::name
    #[doc(alias = "GetStyleColorName")]
    pub fn style_color_name(&self, style_color: StyleColor) -> &'static str {
        unsafe {
            let name_ptr = sys::igGetStyleColorName(style_color as sys::ImGuiCol);
            if name_ptr.is_null() {
                return "Unknown";
            }
            let c_str = std::ffi::CStr::from_ptr(name_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }
}
