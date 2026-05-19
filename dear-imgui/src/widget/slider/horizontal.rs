use std::ffi::c_void;

use crate::Ui;
use crate::internal::{DataTypeKind, component_count_i32};
use crate::sys;

use super::SliderFlags;
use super::validation::validate_slider_preconditions;

/// Builder for slider widgets
#[derive(Clone, Debug)]
#[must_use]
pub struct Slider<'ui, Label, Data, Format = &'static str> {
    pub(super) ui: &'ui Ui,
    pub(super) label: Label,
    pub(super) min: Data,
    pub(super) max: Data,
    pub(super) display_format: Option<Format>,
    pub(super) flags: SliderFlags,
}

impl<'ui, Label, Data> Slider<'ui, Label, Data>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
{
    /// Creates a new slider builder
    #[doc(alias = "SliderScalar", alias = "SliderScalarN")]
    #[deprecated(note = "Use `Ui::slider` or `Ui::slider_config`.", since = "0.1.0")]
    pub fn new(ui: &'ui Ui, label: Label, min: Data, max: Data) -> Self {
        Self {
            ui,
            label,
            min,
            max,
            display_format: None,
            flags: SliderFlags::NONE,
        }
    }
}

impl<'ui, Label, Data, Format> Slider<'ui, Label, Data, Format>
where
    Label: AsRef<str>,
    Data: DataTypeKind,
    Format: AsRef<str>,
{
    /// Sets the range inclusively, such that both values given
    /// are valid values which the slider can be dragged to.
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.slider_config("Example", i8::MIN, i8::MAX)
    ///     .range(4, 8)
    ///     // Remember to call .build()
    ///     ;
    /// ```
    ///
    /// It is safe, though up to C++ Dear ImGui, on how to handle when
    /// `min > max`.
    ///
    /// Note for f32 and f64 sliders, Dear ImGui limits each range endpoint to
    /// finite values within half their full range (e.g.
    /// `-f32::MAX/2.0..=f32::MAX/2.0`). Specifying a value outside this range
    /// will panic before calling into Dear ImGui.
    /// For large ranged values, consider using input widgets instead
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
    ) -> Slider<'ui, Label, Data, Format2> {
        Slider {
            ui: self.ui,
            label: self.label,
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

    /// Builds a slider that is bound to the given value.
    ///
    /// Returns true if the slider value was changed.
    pub fn build(self, value: &mut Data) -> bool {
        validate_slider_preconditions("Slider::build()", &self.min, &self.max, self.flags);
        unsafe {
            let (label, display_format) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igSliderScalar(
                label,
                Data::KIND as i32,
                value as *mut Data as *mut c_void,
                &self.min as *const Data as *const c_void,
                &self.max as *const Data as *const c_void,
                display_format,
                self.flags.bits(),
            )
        }
    }

    /// Builds a horizontal array of multiple sliders attached to the given slice.
    ///
    /// Returns true if any slider value was changed.
    pub fn build_array(self, values: &mut [Data]) -> bool {
        validate_slider_preconditions("Slider::build_array()", &self.min, &self.max, self.flags);
        let count = component_count_i32("Slider::build_array()", values.len());
        if self.flags.contains(SliderFlags::COLOR_MARKERS) {
            assert!(
                count <= 4,
                "Slider::build_array() supports at most 4 components with COLOR_MARKERS"
            );
        }
        unsafe {
            let (label, display_format) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igSliderScalarN(
                label,
                Data::KIND as i32,
                values.as_mut_ptr() as *mut c_void,
                count,
                &self.min as *const Data as *const c_void,
                &self.max as *const Data as *const c_void,
                display_format,
                self.flags.bits(),
            )
        }
    }
}
