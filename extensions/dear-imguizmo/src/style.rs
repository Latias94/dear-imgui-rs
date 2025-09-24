use dear_imguizmo_sys as sys;
use dear_imgui::Ui;
use std::marker::PhantomData;

use crate::types::Color;

/// Safe wrapper over ImGuizmo global Style
pub struct Style<'ui> {
    pub(crate) ptr: *mut sys::Style,
    pub(crate) _phantom: PhantomData<&'ui Ui>,
}

impl<'ui> Style<'ui> {
    fn as_ref(&self) -> &sys::Style {
        unsafe { &*self.ptr }
    }
    fn as_mut(&mut self) -> &mut sys::Style {
        unsafe { &mut *self.ptr }
    }

    pub fn translation_line_thickness(&self) -> f32 {
        self.as_ref().TranslationLineThickness
    }
    pub fn set_translation_line_thickness(&mut self, v: f32) {
        self.as_mut().TranslationLineThickness = v;
    }
    pub fn rotation_line_thickness(&self) -> f32 {
        self.as_ref().RotationLineThickness
    }
    pub fn set_rotation_line_thickness(&mut self, v: f32) {
        self.as_mut().RotationLineThickness = v;
    }
    pub fn rotation_outer_line_thickness(&self) -> f32 {
        self.as_ref().RotationOuterLineThickness
    }
    pub fn set_rotation_outer_line_thickness(&mut self, v: f32) {
        self.as_mut().RotationOuterLineThickness = v;
    }
    pub fn scale_line_thickness(&self) -> f32 {
        self.as_ref().ScaleLineThickness
    }
    pub fn set_scale_line_thickness(&mut self, v: f32) {
        self.as_mut().ScaleLineThickness = v;
    }
    pub fn scale_line_circle_size(&self) -> f32 {
        self.as_ref().ScaleLineCircleSize
    }
    pub fn set_scale_line_circle_size(&mut self, v: f32) {
        self.as_mut().ScaleLineCircleSize = v;
    }
    pub fn center_circle_size(&self) -> f32 {
        self.as_ref().CenterCircleSize
    }
    pub fn set_center_circle_size(&mut self, v: f32) {
        self.as_mut().CenterCircleSize = v;
    }
    pub fn translation_line_arrow_size(&self) -> f32 {
        self.as_ref().TranslationLineArrowSize
    }
    pub fn set_translation_line_arrow_size(&mut self, v: f32) {
        self.as_mut().TranslationLineArrowSize = v;
    }
    pub fn hatched_axis_line_thickness(&self) -> f32 {
        self.as_ref().HatchedAxisLineThickness
    }
    pub fn set_hatched_axis_line_thickness(&mut self, v: f32) {
        self.as_mut().HatchedAxisLineThickness = v;
    }

    pub fn color(&self, idx: Color) -> [f32; 4] {
        let c = self.as_ref().Colors[idx as usize];
        [c.x, c.y, c.z, c.w]
    }
    pub fn set_color(&mut self, idx: Color, rgba: [f32; 4]) {
        self.as_mut().Colors[idx as usize] = sys::ImVec4 { x: rgba[0], y: rgba[1], z: rgba[2], w: rgba[3] };
    }
}
