//! Pie chart plot implementation

use super::{
    Plot, PlotDataLayout, PlotError, PlotItemStyle, plot_spec_with_style,
    with_plot_str_slice_with_opt,
};
use crate::{FloatFormat, FloatFormatError, ItemFlags, PieChartFlags, sys};
use std::borrow::Cow;

/// Builder for pie chart plots
pub struct PieChartPlot<'a, F = &'static str> {
    label_ids: Vec<&'a str>,
    values: &'a [f64],
    style: PlotItemStyle,
    center_x: f64,
    center_y: f64,
    radius: f64,
    label_fmt: Option<F>,
    angle0: f64,
    flags: PieChartFlags,
    item_flags: ItemFlags,
}

impl<F> super::PlotItemStyled for PieChartPlot<'_, F> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> PieChartPlot<'a> {
    /// Create a new pie chart plot
    ///
    /// # Arguments
    /// * `label_ids` - Labels for each slice of the pie
    /// * `values` - Values for each slice
    /// * `center_x` - X coordinate of the pie center in plot units
    /// * `center_y` - Y coordinate of the pie center in plot units
    /// * `radius` - Radius of the pie in plot units
    pub fn new(
        label_ids: Vec<&'a str>,
        values: &'a [f64],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Self {
        Self {
            label_ids,
            values,
            style: PlotItemStyle::default(),
            center_x,
            center_y,
            radius,
            label_fmt: Some("%.1f"),
            angle0: 90.0, // Start angle in degrees
            flags: PieChartFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }
}

impl<'a, F: AsRef<str>> PieChartPlot<'a, F> {
    /// Set the validated label format for slice values.
    pub fn with_label_format<'fmt>(
        self,
        format: FloatFormat<'fmt>,
    ) -> PieChartPlot<'a, FloatFormat<'fmt>> {
        PieChartPlot {
            label_ids: self.label_ids,
            values: self.values,
            style: self.style,
            center_x: self.center_x,
            center_y: self.center_y,
            radius: self.radius,
            label_fmt: Some(format),
            angle0: self.angle0,
            flags: self.flags,
            item_flags: self.item_flags,
        }
    }

    /// Validate and set a C-style label format for slice values.
    pub fn try_label_format<'fmt>(
        self,
        format: impl Into<Cow<'fmt, str>>,
    ) -> Result<PieChartPlot<'a, FloatFormat<'fmt>>, FloatFormatError> {
        Ok(self.with_label_format(FloatFormat::new(format)?))
    }

    /// Disable per-slice value labels.
    pub fn without_value_labels(mut self) -> Self {
        self.label_fmt = None;
        self
    }

    /// Set the starting angle in degrees (default: 90.0)
    pub fn with_start_angle(mut self, angle: f64) -> Self {
        self.angle0 = angle;
        self
    }

    /// Set pie chart flags for customization
    pub fn with_flags(mut self, flags: PieChartFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Normalize the pie chart values (force full circle even if sum < 1.0)
    pub fn normalize(mut self) -> Self {
        self.flags |= PieChartFlags::NORMALIZE;
        self
    }

    /// Ignore hidden slices when drawing (as if they were not there)
    pub fn ignore_hidden(mut self) -> Self {
        self.flags |= PieChartFlags::IGNORE_HIDDEN;
        self
    }

    /// Enable exploding effect for legend-hovered slices
    pub fn exploding(mut self) -> Self {
        self.flags |= PieChartFlags::EXPLODING;
        self
    }

    /// Draw slices without the per-slice border stroke.
    pub fn no_slice_border(mut self) -> Self {
        self.flags |= PieChartFlags::NO_SLICE_BORDER;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        if self.label_ids.len() != self.values.len() {
            return Err(PlotError::DataLengthMismatch {
                x_len: self.label_ids.len(),
                y_len: self.values.len(),
            });
        }

        if self.radius <= 0.0 {
            return Err(PlotError::InvalidData(
                "Radius must be positive".to_string(),
            ));
        }

        // Check for negative values
        if self.values.iter().any(|&v| v < 0.0) {
            return Err(PlotError::InvalidData(
                "Pie chart values cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

impl<F: AsRef<str>> Plot for PieChartPlot<'_, F> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(count) = i32::try_from(self.values.len()) else {
            return;
        };
        plot_ui.with_bound_context(|| {
            with_plot_str_slice_with_opt(
                &self.label_ids,
                self.label_fmt.as_ref().map(AsRef::as_ref),
                |label_ptrs, label_fmt_ptr| unsafe {
                    let spec = plot_spec_with_style(
                        self.style,
                        self.flags.bits() | self.item_flags.bits(),
                        PlotDataLayout::DEFAULT,
                    );
                    sys::ImPlot_PlotPieChart_doublePtrStr(
                        label_ptrs.as_ptr(),
                        self.values.as_ptr(),
                        count,
                        self.center_x,
                        self.center_y,
                        self.radius,
                        label_fmt_ptr,
                        self.angle0,
                        spec,
                    );
                },
            )
        })
    }

    fn label(&self) -> &str {
        "PieChart" // Pie charts don't have a single label
    }
}

/// Float version of pie chart for better performance with f32 data
pub struct PieChartPlotF32<'a, F = &'static str> {
    label_ids: Vec<&'a str>,
    values: &'a [f32],
    style: PlotItemStyle,
    center_x: f64,
    center_y: f64,
    radius: f64,
    label_fmt: Option<F>,
    angle0: f64,
    flags: PieChartFlags,
    item_flags: ItemFlags,
}

impl<F> super::PlotItemStyled for PieChartPlotF32<'_, F> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> PieChartPlotF32<'a> {
    /// Create a new f32 pie chart plot
    pub fn new(
        label_ids: Vec<&'a str>,
        values: &'a [f32],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Self {
        Self {
            label_ids,
            values,
            style: PlotItemStyle::default(),
            center_x,
            center_y,
            radius,
            label_fmt: Some("%.1f"),
            angle0: 90.0,
            flags: PieChartFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }
}

impl<'a, F: AsRef<str>> PieChartPlotF32<'a, F> {
    /// Set the validated label format for slice values.
    pub fn with_label_format<'fmt>(
        self,
        format: FloatFormat<'fmt>,
    ) -> PieChartPlotF32<'a, FloatFormat<'fmt>> {
        PieChartPlotF32 {
            label_ids: self.label_ids,
            values: self.values,
            style: self.style,
            center_x: self.center_x,
            center_y: self.center_y,
            radius: self.radius,
            label_fmt: Some(format),
            angle0: self.angle0,
            flags: self.flags,
            item_flags: self.item_flags,
        }
    }

    /// Validate and set a C-style label format for slice values.
    pub fn try_label_format<'fmt>(
        self,
        format: impl Into<Cow<'fmt, str>>,
    ) -> Result<PieChartPlotF32<'a, FloatFormat<'fmt>>, FloatFormatError> {
        Ok(self.with_label_format(FloatFormat::new(format)?))
    }

    /// Disable per-slice value labels.
    pub fn without_value_labels(mut self) -> Self {
        self.label_fmt = None;
        self
    }

    /// Set the starting angle in degrees
    pub fn with_start_angle(mut self, angle: f64) -> Self {
        self.angle0 = angle;
        self
    }

    /// Set pie chart flags for customization
    pub fn with_flags(mut self, flags: PieChartFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Normalize the pie chart values
    pub fn normalize(mut self) -> Self {
        self.flags |= PieChartFlags::NORMALIZE;
        self
    }

    /// Ignore hidden slices when drawing
    pub fn ignore_hidden(mut self) -> Self {
        self.flags |= PieChartFlags::IGNORE_HIDDEN;
        self
    }

    /// Enable exploding effect for legend-hovered slices
    pub fn exploding(mut self) -> Self {
        self.flags |= PieChartFlags::EXPLODING;
        self
    }

    /// Draw slices without the per-slice border stroke.
    pub fn no_slice_border(mut self) -> Self {
        self.flags |= PieChartFlags::NO_SLICE_BORDER;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        if self.label_ids.len() != self.values.len() {
            return Err(PlotError::DataLengthMismatch {
                x_len: self.label_ids.len(),
                y_len: self.values.len(),
            });
        }

        if self.radius <= 0.0 {
            return Err(PlotError::InvalidData(
                "Radius must be positive".to_string(),
            ));
        }

        if self.values.iter().any(|&v| v < 0.0) {
            return Err(PlotError::InvalidData(
                "Pie chart values cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

impl<F: AsRef<str>> Plot for PieChartPlotF32<'_, F> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(count) = i32::try_from(self.values.len()) else {
            return;
        };
        plot_ui.with_bound_context(|| {
            with_plot_str_slice_with_opt(
                &self.label_ids,
                self.label_fmt.as_ref().map(AsRef::as_ref),
                |label_ptrs, label_fmt_ptr| unsafe {
                    let spec = plot_spec_with_style(
                        self.style,
                        self.flags.bits() | self.item_flags.bits(),
                        PlotDataLayout::DEFAULT,
                    );
                    sys::ImPlot_PlotPieChart_FloatPtrStr(
                        label_ptrs.as_ptr(),
                        self.values.as_ptr(),
                        count,
                        self.center_x,
                        self.center_y,
                        self.radius,
                        label_fmt_ptr,
                        self.angle0,
                        spec,
                    );
                },
            )
        })
    }

    fn label(&self) -> &str {
        "PieChart"
    }
}

/// Convenience functions for quick pie chart plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a pie chart with f64 data
    pub fn pie_chart_plot(
        &self,
        label_ids: Vec<&str>,
        values: &[f64],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Result<(), PlotError> {
        let plot = PieChartPlot::new(label_ids, values, center_x, center_y, radius);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a pie chart with f32 data
    pub fn pie_chart_plot_f32(
        &self,
        label_ids: Vec<&str>,
        values: &[f32],
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> Result<(), PlotError> {
        let plot = PieChartPlotF32::new(label_ids, values, center_x, center_y, radius);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a centered pie chart (center at 0.5, 0.5 with radius 0.4)
    pub fn centered_pie_chart(
        &self,
        label_ids: Vec<&str>,
        values: &[f64],
    ) -> Result<(), PlotError> {
        let plot = PieChartPlot::new(label_ids, values, 0.5, 0.5, 0.4);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }
}
