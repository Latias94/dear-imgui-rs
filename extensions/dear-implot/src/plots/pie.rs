//! Pie chart plot implementation

use super::{
    Plot, PlotDataLayout, PlotError, PlotItemStyle, plot_spec_with_style,
    with_plot_str_slice_with_opt,
};
use crate::{FloatFormat, FloatFormatError, ItemFlags, PieChartFlags, sys};
use std::borrow::Cow;

fn validate_pie_chart<Value: Copy + Into<f64>>(
    label_count: usize,
    values: &[Value],
    center: [f64; 2],
    radius: f64,
    angle: f64,
) -> Result<(), PlotError> {
    if values.is_empty() {
        return Err(PlotError::EmptyData);
    }
    if label_count != values.len() {
        return Err(PlotError::DataLengthMismatch {
            x_len: label_count,
            y_len: values.len(),
        });
    }
    if !center.into_iter().all(f64::is_finite) || !angle.is_finite() {
        return Err(PlotError::InvalidData(
            "Pie chart center and start angle must be finite".into(),
        ));
    }
    if !radius.is_finite() || radius <= 0.0 {
        return Err(PlotError::InvalidData(
            "Pie chart radius must be positive and finite".into(),
        ));
    }
    if values
        .iter()
        .copied()
        .map(Into::into)
        .any(|value: f64| !value.is_finite() || value < 0.0)
    {
        return Err(PlotError::InvalidData(
            "Pie chart values must be finite and nonnegative".into(),
        ));
    }
    Ok(())
}

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

    /// Check the slice values, center, radius, and start angle before plotting.
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_pie_chart(
            self.label_ids.len(),
            self.values,
            [self.center_x, self.center_y],
            self.radius,
            self.angle0,
        )
    }
}

impl<F: AsRef<str>> Plot for PieChartPlot<'_, F> {
    fn plot(&self, plot_ui: &crate::PlotScope<'_>) {
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
                        plot_ui,
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

    /// Check the slice values, center, radius, and start angle before plotting.
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_pie_chart(
            self.label_ids.len(),
            self.values,
            [self.center_x, self.center_y],
            self.radius,
            self.angle0,
        )
    }
}

impl<F: AsRef<str>> Plot for PieChartPlotF32<'_, F> {
    fn plot(&self, plot_ui: &crate::PlotScope<'_>) {
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
                        plot_ui,
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
impl<'ui> crate::PlotScope<'ui> {
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

#[cfg(test)]
mod tests {
    use super::{PieChartPlot, PieChartPlotF32, PlotError};

    #[test]
    fn pie_charts_reject_non_finite_values_and_geometry() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for (center_x, center_y, radius, angle) in [
                (invalid, 0.0, 1.0, 90.0),
                (0.0, invalid, 1.0, 90.0),
                (0.0, 0.0, invalid, 90.0),
                (0.0, 0.0, 1.0, invalid),
            ] {
                assert!(matches!(
                    PieChartPlot::new(vec!["slice"], &[1.0], center_x, center_y, radius)
                        .with_start_angle(angle)
                        .validate(),
                    Err(PlotError::InvalidData(_))
                ));
                assert!(matches!(
                    PieChartPlotF32::new(vec!["slice"], &[1.0], center_x, center_y, radius)
                        .with_start_angle(angle)
                        .validate(),
                    Err(PlotError::InvalidData(_))
                ));
            }
            assert!(matches!(
                PieChartPlot::new(vec!["slice"], &[invalid], 0.0, 0.0, 1.0).validate(),
                Err(PlotError::InvalidData(_))
            ));
            assert!(matches!(
                PieChartPlotF32::new(vec!["slice"], &[invalid as f32], 0.0, 0.0, 1.0).validate(),
                Err(PlotError::InvalidData(_))
            ));
        }
        assert!(
            PieChartPlot::new(vec!["slice"], &[1.0], 0.0, 0.0, 1.0)
                .validate()
                .is_ok()
        );
        assert!(
            PieChartPlotF32::new(vec!["slice"], &[1.0], 0.0, 0.0, 1.0)
                .validate()
                .is_ok()
        );
    }
}
