use std::ffi::c_void;

use crate::Ui;
use crate::internal::DataTypeKind;
use crate::sys;

use super::SliderFlags;
use super::validation::validate_slider_preconditions;

/// Builder for a vertical slider widget.
#[derive(Clone, Debug)]
#[must_use]
pub struct VerticalSlider<Label, Data, Format = &'static str> {
    label: Label,
    size: [f32; 2],
    min: Data,
    max: Data,
    display_format: Option<Format>,
    flags: SliderFlags,
}

impl<Label, Data> VerticalSlider<Label, Data>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
{
    /// Constructs a new vertical slider builder with the given size and range.
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// VerticalSlider::new("Example", [20.0, 20.0], i8::MIN, i8::MAX)
    ///     .range(4, 8)
    ///     // Remember to call .build(&ui)
    ///     ;
    /// ```
    ///
    /// It is safe, though up to C++ Dear ImGui, on how to handle when
    /// `min > max`.
    #[doc(alias = "VSliderScalar")]
    pub fn new(label: Label, size: impl Into<[f32; 2]>, min: Data, max: Data) -> Self {
        VerticalSlider {
            label,
            size: size.into(),
            min,
            max,
            display_format: None,
            flags: SliderFlags::NONE,
        }
    }
}

impl<Label, Data, Format> VerticalSlider<Label, Data, Format>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
    Format: AsRef<str>,
{
    /// Sets the range for the vertical slider.
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// VerticalSlider::new("Example", [20.0, 20.0], i8::MIN, i8::MAX)
    ///     .range(4, 8)
    ///     // Remember to call .build(&ui)
    ///     ;
    /// ```
    ///
    /// It is safe, though up to C++ Dear ImGui, on how to handle when
    /// `min > max`.
    #[inline]
    pub fn range(mut self, min: Data, max: Data) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// Sets the display format using *a C-style printf string*
    #[inline]
    pub fn display_format<Format2: AsRef<str>>(
        self,
        display_format: Format2,
    ) -> VerticalSlider<Label, Data, Format2> {
        VerticalSlider {
            label: self.label,
            size: self.size,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Replaces all current settings with the given flags
    #[inline]
    pub fn flags(mut self, flags: SliderFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a vertical slider that is bound to the given value.
    ///
    /// Returns true if the slider value was changed.
    pub fn build(self, ui: &Ui, value: &mut Data) -> bool {
        validate_slider_preconditions("VerticalSlider::build()", &self.min, &self.max, self.flags);
        let (label, display_format) = ui.scratch_txt_with_opt(self.label, self.display_format);
        let size = sys::ImVec2::new(self.size[0], self.size[1]);

        ui.run_with_bound_context(|| unsafe {
            sys::igVSliderScalar(
                label,
                size,
                Data::KIND as i32,
                value as *mut Data as *mut c_void,
                &self.min as *const Data as *const c_void,
                &self.max as *const Data as *const c_void,
                display_format,
                self.flags.bits(),
            )
        })
    }
}
