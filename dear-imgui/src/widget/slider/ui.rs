use crate::Ui;
use crate::internal::DataTypeKind;

use super::{AngleSlider, Slider, SliderFlags, VerticalSlider};

impl Ui {
    /// Creates a new slider widget. Returns true if the value has been edited.
    pub fn slider<T: AsRef<str>, K: DataTypeKind>(
        &self,
        label: T,
        min: K,
        max: K,
        value: &mut K,
    ) -> bool {
        self.slider_config(label, min, max).build(value)
    }

    /// Creates a new unbuilt Slider.
    pub fn slider_config<T: AsRef<str>, K: DataTypeKind>(
        &self,
        label: T,
        min: K,
        max: K,
    ) -> Slider<'_, T, K> {
        Slider {
            ui: self,
            label,
            min,
            max,
            display_format: Option::<&'static str>::None,
            flags: SliderFlags::NONE,
        }
    }

    /// Creates a float slider
    #[doc(alias = "SliderFloat")]
    pub fn slider_f32(&self, label: impl AsRef<str>, value: &mut f32, min: f32, max: f32) -> bool {
        self.slider_config(label, min, max).build(value)
    }

    /// Creates an integer slider
    #[doc(alias = "SliderInt")]
    pub fn slider_i32(&self, label: impl AsRef<str>, value: &mut i32, min: i32, max: i32) -> bool {
        self.slider_config(label, min, max).build(value)
    }

    /// Creates a float2 slider
    #[doc(alias = "SliderFloat2")]
    pub fn slider_float2(
        &self,
        label: impl AsRef<str>,
        value: &mut [f32; 2],
        min: f32,
        max: f32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates a float3 slider
    #[doc(alias = "SliderFloat3")]
    pub fn slider_float3(
        &self,
        label: impl AsRef<str>,
        value: &mut [f32; 3],
        min: f32,
        max: f32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates a float4 slider
    #[doc(alias = "SliderFloat4")]
    pub fn slider_float4(
        &self,
        label: impl AsRef<str>,
        value: &mut [f32; 4],
        min: f32,
        max: f32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates an int2 slider
    #[doc(alias = "SliderInt2")]
    pub fn slider_int2(
        &self,
        label: impl AsRef<str>,
        value: &mut [i32; 2],
        min: i32,
        max: i32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates an int3 slider
    #[doc(alias = "SliderInt3")]
    pub fn slider_int3(
        &self,
        label: impl AsRef<str>,
        value: &mut [i32; 3],
        min: i32,
        max: i32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates an int4 slider
    #[doc(alias = "SliderInt4")]
    pub fn slider_int4(
        &self,
        label: impl AsRef<str>,
        value: &mut [i32; 4],
        min: i32,
        max: i32,
    ) -> bool {
        self.slider_config(label, min, max)
            .build_array(value.as_mut_slice())
    }

    /// Creates a vertical slider
    #[doc(alias = "VSliderFloat")]
    pub fn v_slider_f32(
        &self,
        label: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        value: &mut f32,
        min: f32,
        max: f32,
    ) -> bool {
        VerticalSlider::new(label, size, min, max).build(self, value)
    }

    /// Creates a vertical integer slider
    #[doc(alias = "VSliderInt")]
    pub fn v_slider_i32(
        &self,
        label: impl AsRef<str>,
        size: impl Into<[f32; 2]>,
        value: &mut i32,
        min: i32,
        max: i32,
    ) -> bool {
        VerticalSlider::new(label, size, min, max).build(self, value)
    }

    /// Creates an angle slider (value in radians)
    #[doc(alias = "SliderAngle")]
    pub fn slider_angle(&self, label: impl AsRef<str>, value_rad: &mut f32) -> bool {
        AngleSlider::new(label).build(self, value_rad)
    }
}
