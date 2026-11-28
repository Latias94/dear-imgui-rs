use dear_imgui_rs::Ui;
use dear_imguizmo_sys as sys;
use std::marker::PhantomData;

use crate::types::Color;

/// Safe wrapper over ImGuizmo global Style
pub struct Style<'ui> {
    pub(crate) ptr: *mut sys::Style,
    pub(crate) _phantom: PhantomData<&'ui Ui>,
}

impl<'ui> Style<'ui> {
    pub fn translation_line_thickness(&self) -> f32 {
        unsafe { (*self.ptr).TranslationLineThickness }
    }
    pub fn set_translation_line_thickness(&mut self, v: f32) {
        unsafe { (*self.ptr).TranslationLineThickness = v };
    }
    pub fn rotation_line_thickness(&self) -> f32 {
        unsafe { (*self.ptr).RotationLineThickness }
    }
    pub fn set_rotation_line_thickness(&mut self, v: f32) {
        unsafe { (*self.ptr).RotationLineThickness = v };
    }
    pub fn rotation_outer_line_thickness(&self) -> f32 {
        unsafe { (*self.ptr).RotationOuterLineThickness }
    }
    pub fn set_rotation_outer_line_thickness(&mut self, v: f32) {
        unsafe { (*self.ptr).RotationOuterLineThickness = v };
    }
    pub fn scale_line_thickness(&self) -> f32 {
        unsafe { (*self.ptr).ScaleLineThickness }
    }
    pub fn set_scale_line_thickness(&mut self, v: f32) {
        unsafe { (*self.ptr).ScaleLineThickness = v };
    }
    pub fn scale_line_circle_size(&self) -> f32 {
        unsafe { (*self.ptr).ScaleLineCircleSize }
    }
    pub fn set_scale_line_circle_size(&mut self, v: f32) {
        unsafe { (*self.ptr).ScaleLineCircleSize = v };
    }
    pub fn center_circle_size(&self) -> f32 {
        unsafe { (*self.ptr).CenterCircleSize }
    }
    pub fn set_center_circle_size(&mut self, v: f32) {
        unsafe { (*self.ptr).CenterCircleSize = v };
    }
    pub fn translation_line_arrow_size(&self) -> f32 {
        unsafe { (*self.ptr).TranslationLineArrowSize }
    }
    pub fn set_translation_line_arrow_size(&mut self, v: f32) {
        unsafe { (*self.ptr).TranslationLineArrowSize = v };
    }
    pub fn hatched_axis_line_thickness(&self) -> f32 {
        unsafe { (*self.ptr).HatchedAxisLineThickness }
    }
    pub fn set_hatched_axis_line_thickness(&mut self, v: f32) {
        unsafe { (*self.ptr).HatchedAxisLineThickness = v };
    }

    pub fn color(&self, idx: Color) -> [f32; 4] {
        let color = unsafe { (*self.ptr).Colors[idx as usize] };
        [color.x, color.y, color.z, color.w]
    }
    pub fn set_color(&mut self, idx: Color, rgba: [f32; 4]) {
        unsafe {
            (*self.ptr).Colors[idx as usize] = sys::ImVec4 {
                x: rgba[0],
                y: rgba[1],
                z: rgba[2],
                w: rgba[3],
            };
        }
    }
}
