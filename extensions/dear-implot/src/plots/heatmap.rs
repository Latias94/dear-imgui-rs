//! Heatmap plot implementation

use super::{
    Plot, PlotDataLayout, PlotError, PlotItemStyle, plot_spec_with_style, with_plot_str_or_empty,
};
use crate::{FloatFormat, FloatFormatError, HeatmapFlags, ItemFlags, sys};
use dear_imgui_rs::with_scratch_txt_two;
use std::borrow::Cow;

fn validate_grid_counts(
    caller: &str,
    rows: usize,
    cols: usize,
    values_len: usize,
) -> Result<(), PlotError> {
    if rows == 0 || cols == 0 {
        return Err(PlotError::InvalidData(
            "Rows and columns must be positive".to_string(),
        ));
    }

    let expected_size = rows
        .checked_mul(cols)
        .ok_or_else(|| PlotError::InvalidData(format!("{caller} rows * cols overflowed usize")))?;
    let _ = heatmap_count_to_i32(caller, "rows", rows)?;
    let _ = heatmap_count_to_i32(caller, "cols", cols)?;
    let _ = heatmap_count_to_i32(caller, "rows * cols", expected_size)?;

    if values_len != expected_size {
        return Err(PlotError::DataLengthMismatch {
            x_len: expected_size,
            y_len: values_len,
        });
    }

    Ok(())
}

fn heatmap_count_to_i32(caller: &str, name: &str, value: usize) -> Result<i32, PlotError> {
    i32::try_from(value)
        .map_err(|_| PlotError::InvalidData(format!("{caller} {name} exceeded ImPlot's i32 range")))
}

fn heatmap_scale<T: Copy + Into<f64>>(
    caller: &str,
    values: &[T],
    rows: usize,
    cols: usize,
    mut min: f64,
    mut max: f64,
) -> Result<(f64, f64), PlotError> {
    if values.is_empty() {
        return Err(PlotError::EmptyData);
    }
    validate_grid_counts(caller, rows, cols, values.len())?;

    // Native RenderHeatmap uses the normalized value as an integer colormap index.
    if min == 0.0 && max == 0.0 {
        min = f64::INFINITY;
        max = f64::NEG_INFINITY;
        for &sample in values {
            let value: f64 = sample.into();
            if !value.is_finite() {
                return Err(PlotError::InvalidData(
                    "automatic heatmap scaling requires finite values".into(),
                ));
            }
            min = min.min(value);
            max = max.max(value);
        }
    } else if min != max && values.iter().any(|&v| f64::is_nan(v.into())) {
        return Err(PlotError::InvalidData(
            "cannot map a NaN heatmap value to a color".into(),
        ));
    }
    if !(max - min).is_finite() {
        return Err(PlotError::InvalidData(
            "heatmap color scale must have a finite span".into(),
        ));
    }
    Ok((min, max))
}

/// Builder for heatmap plots with extensive customization options
pub struct HeatmapPlot<'a, F = &'static str> {
    label: &'a str,
    values: &'a [f64],
    style: PlotItemStyle,
    rows: usize,
    cols: usize,
    scale_min: f64,
    scale_max: f64,
    label_fmt: Option<F>,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    flags: HeatmapFlags,
    item_flags: ItemFlags,
}

impl<F> super::PlotItemStyled for HeatmapPlot<'_, F> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> HeatmapPlot<'a> {
    /// Create a new heatmap plot with the given label and data
    ///
    /// # Arguments
    /// * `label` - The label for the heatmap
    /// * `values` - The data values in row-major order (unless ColMajor flag is set)
    /// * `rows` - Number of rows in the data
    /// * `cols` - Number of columns in the data
    pub fn new(label: &'a str, values: &'a [f64], rows: usize, cols: usize) -> Self {
        Self {
            label,
            values,
            style: PlotItemStyle::default(),
            rows,
            cols,
            scale_min: 0.0,
            scale_max: 0.0, // Auto-scale when both are 0
            label_fmt: Some("%.1f"),
            bounds_min: sys::ImPlotPoint { x: 0.0, y: 0.0 },
            bounds_max: sys::ImPlotPoint { x: 1.0, y: 1.0 },
            flags: HeatmapFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }
}

impl<'a, F: AsRef<str>> HeatmapPlot<'a, F> {
    /// Set the color scale range (min, max)
    /// If both are 0.0, auto-scaling will be used
    pub fn with_scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    /// Set the validated label format for values.
    pub fn with_label_format<'fmt>(
        self,
        format: FloatFormat<'fmt>,
    ) -> HeatmapPlot<'a, FloatFormat<'fmt>> {
        HeatmapPlot {
            label: self.label,
            values: self.values,
            style: self.style,
            rows: self.rows,
            cols: self.cols,
            scale_min: self.scale_min,
            scale_max: self.scale_max,
            label_fmt: Some(format),
            bounds_min: self.bounds_min,
            bounds_max: self.bounds_max,
            flags: self.flags,
            item_flags: self.item_flags,
        }
    }

    /// Validate and set a C-style label format for values.
    pub fn try_label_format<'fmt>(
        self,
        format: impl Into<Cow<'fmt, str>>,
    ) -> Result<HeatmapPlot<'a, FloatFormat<'fmt>>, FloatFormatError> {
        Ok(self.with_label_format(FloatFormat::new(format)?))
    }

    /// Disable value labels.
    pub fn without_value_labels(mut self) -> Self {
        self.label_fmt = None;
        self
    }

    /// Set the drawing area bounds in plot coordinates
    pub fn with_bounds(mut self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        self.bounds_min = sys::ImPlotPoint { x: min_x, y: min_y };
        self.bounds_max = sys::ImPlotPoint { x: max_x, y: max_y };
        self
    }

    /// Set the drawing area bounds using ImPlotPoint
    pub fn with_bounds_points(mut self, min: sys::ImPlotPoint, max: sys::ImPlotPoint) -> Self {
        self.bounds_min = min;
        self.bounds_max = max;
        self
    }

    /// Set heatmap flags for customization
    pub fn with_flags(mut self, flags: HeatmapFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Use column-major data ordering instead of row-major
    pub fn column_major(mut self) -> Self {
        self.flags |= HeatmapFlags::COL_MAJOR;
        self
    }

    /// Checks the grid dimensions and that each value can be mapped to the selected color scale.
    pub fn validate(&self) -> Result<(), PlotError> {
        self.checked_scale().map(|_| ())
    }

    fn checked_scale(&self) -> Result<(f64, f64), PlotError> {
        heatmap_scale(
            "HeatmapPlot::validate()",
            self.values,
            self.rows,
            self.cols,
            self.scale_min,
            self.scale_max,
        )
    }

    pub(super) fn plot_checked(&self, plot_ui: &crate::PlotScope<'_>) -> Result<(), PlotError> {
        let (min, max) = self.checked_scale()?;
        let (rows, cols) = (self.rows as i32, self.cols as i32);
        let label_fmt = self.label_fmt.as_ref().map(AsRef::as_ref);
        plot_ui.with_bound_context(|| match label_fmt {
            Some(label_fmt) => {
                let label = if self.label.contains('\0') {
                    ""
                } else {
                    self.label
                };
                with_scratch_txt_two(label, label_fmt, |label_ptr, label_fmt_ptr| unsafe {
                    let spec = plot_spec_with_style(
                        plot_ui,
                        self.style,
                        self.flags.bits() | self.item_flags.bits(),
                        PlotDataLayout::DEFAULT,
                    );
                    sys::ImPlot_PlotHeatmap_doublePtr(
                        label_ptr,
                        self.values.as_ptr(),
                        rows,
                        cols,
                        min,
                        max,
                        label_fmt_ptr,
                        self.bounds_min,
                        self.bounds_max,
                        spec,
                    );
                })
            }
            None => with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    plot_ui,
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotHeatmap_doublePtr(
                    label_ptr,
                    self.values.as_ptr(),
                    rows,
                    cols,
                    min,
                    max,
                    std::ptr::null(),
                    self.bounds_min,
                    self.bounds_max,
                    spec,
                );
            }),
        });
        Ok(())
    }
}

impl<F: AsRef<str>> Plot for HeatmapPlot<'_, F> {
    fn plot(&self, plot_ui: &crate::PlotScope<'_>) {
        let _ = self.plot_checked(plot_ui);
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Float version of heatmap for better performance with f32 data
pub struct HeatmapPlotF32<'a, F = &'static str> {
    label: &'a str,
    values: &'a [f32],
    style: PlotItemStyle,
    rows: usize,
    cols: usize,
    scale_min: f64,
    scale_max: f64,
    label_fmt: Option<F>,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    flags: HeatmapFlags,
    item_flags: ItemFlags,
}

impl<F> super::PlotItemStyled for HeatmapPlotF32<'_, F> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> HeatmapPlotF32<'a> {
    /// Create a new f32 heatmap plot
    pub fn new(label: &'a str, values: &'a [f32], rows: usize, cols: usize) -> Self {
        Self {
            label,
            values,
            style: PlotItemStyle::default(),
            rows,
            cols,
            scale_min: 0.0,
            scale_max: 0.0,
            label_fmt: Some("%.1f"),
            bounds_min: sys::ImPlotPoint { x: 0.0, y: 0.0 },
            bounds_max: sys::ImPlotPoint { x: 1.0, y: 1.0 },
            flags: HeatmapFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }
}

impl<'a, F: AsRef<str>> HeatmapPlotF32<'a, F> {
    /// Set the color scale range (min, max)
    pub fn with_scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    /// Set the validated label format for values.
    pub fn with_label_format<'fmt>(
        self,
        format: FloatFormat<'fmt>,
    ) -> HeatmapPlotF32<'a, FloatFormat<'fmt>> {
        HeatmapPlotF32 {
            label: self.label,
            values: self.values,
            style: self.style,
            rows: self.rows,
            cols: self.cols,
            scale_min: self.scale_min,
            scale_max: self.scale_max,
            label_fmt: Some(format),
            bounds_min: self.bounds_min,
            bounds_max: self.bounds_max,
            flags: self.flags,
            item_flags: self.item_flags,
        }
    }

    /// Validate and set a C-style label format for values.
    pub fn try_label_format<'fmt>(
        self,
        format: impl Into<Cow<'fmt, str>>,
    ) -> Result<HeatmapPlotF32<'a, FloatFormat<'fmt>>, FloatFormatError> {
        Ok(self.with_label_format(FloatFormat::new(format)?))
    }

    /// Disable value labels.
    pub fn without_value_labels(mut self) -> Self {
        self.label_fmt = None;
        self
    }

    /// Set the drawing area bounds in plot coordinates
    pub fn with_bounds(mut self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        self.bounds_min = sys::ImPlotPoint { x: min_x, y: min_y };
        self.bounds_max = sys::ImPlotPoint { x: max_x, y: max_y };
        self
    }

    /// Set heatmap flags for customization
    pub fn with_flags(mut self, flags: HeatmapFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Use column-major data ordering
    pub fn column_major(mut self) -> Self {
        self.flags |= HeatmapFlags::COL_MAJOR;
        self
    }

    /// Checks the grid dimensions and that each value can be mapped to the selected color scale.
    pub fn validate(&self) -> Result<(), PlotError> {
        self.checked_scale().map(|_| ())
    }

    fn checked_scale(&self) -> Result<(f64, f64), PlotError> {
        heatmap_scale(
            "HeatmapPlotF32::validate()",
            self.values,
            self.rows,
            self.cols,
            self.scale_min,
            self.scale_max,
        )
    }

    fn plot_checked(&self, plot_ui: &crate::PlotScope<'_>) -> Result<(), PlotError> {
        let (min, max) = self.checked_scale()?;
        let (rows, cols) = (self.rows as i32, self.cols as i32);
        let label_fmt = self.label_fmt.as_ref().map(AsRef::as_ref);
        plot_ui.with_bound_context(|| match label_fmt {
            Some(label_fmt) => {
                let label = if self.label.contains('\0') {
                    ""
                } else {
                    self.label
                };
                with_scratch_txt_two(label, label_fmt, |label_ptr, label_fmt_ptr| unsafe {
                    let spec = plot_spec_with_style(
                        plot_ui,
                        self.style,
                        self.flags.bits() | self.item_flags.bits(),
                        PlotDataLayout::DEFAULT,
                    );
                    sys::ImPlot_PlotHeatmap_FloatPtr(
                        label_ptr,
                        self.values.as_ptr(),
                        rows,
                        cols,
                        min,
                        max,
                        label_fmt_ptr,
                        self.bounds_min,
                        self.bounds_max,
                        spec,
                    );
                })
            }
            None => with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    plot_ui,
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotHeatmap_FloatPtr(
                    label_ptr,
                    self.values.as_ptr(),
                    rows,
                    cols,
                    min,
                    max,
                    std::ptr::null(),
                    self.bounds_min,
                    self.bounds_max,
                    spec,
                );
            }),
        });
        Ok(())
    }
}

impl<F: AsRef<str>> Plot for HeatmapPlotF32<'_, F> {
    fn plot(&self, plot_ui: &crate::PlotScope<'_>) {
        let _ = self.plot_checked(plot_ui);
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick heatmap plotting
impl<'ui> crate::PlotScope<'ui> {
    /// Plot a heatmap with f64 data
    pub fn heatmap_plot(
        &self,
        label: &str,
        values: &[f64],
        rows: usize,
        cols: usize,
    ) -> Result<(), PlotError> {
        HeatmapPlot::new(label, values, rows, cols).plot_checked(self)
    }

    /// Plot a heatmap with f32 data
    pub fn heatmap_plot_f32(
        &self,
        label: &str,
        values: &[f32],
        rows: usize,
        cols: usize,
    ) -> Result<(), PlotError> {
        HeatmapPlotF32::new(label, values, rows, cols).plot_checked(self)
    }

    /// Plot a heatmap with custom scale and bounds
    #[allow(
        clippy::too_many_arguments,
        reason = "Retains the public convenience call for callers supplying explicit grid, scale, and bounds"
    )]
    pub fn heatmap_plot_scaled(
        &self,
        label: &str,
        values: &[f64],
        rows: usize,
        cols: usize,
        scale_min: f64,
        scale_max: f64,
        bounds_min: sys::ImPlotPoint,
        bounds_max: sys::ImPlotPoint,
    ) -> Result<(), PlotError> {
        HeatmapPlot::new(label, values, rows, cols)
            .with_scale(scale_min, scale_max)
            .with_bounds_points(bounds_min, bounds_max)
            .plot_checked(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{HeatmapPlot, HeatmapPlotF32, validate_grid_counts};
    use crate::PlotError;

    #[test]
    fn heatmap_values_must_produce_defined_colormap_indices() {
        assert!(
            HeatmapPlot::new("auto", &[0.0, f64::NAN], 1, 2)
                .validate()
                .is_err()
        );
        assert!(
            HeatmapPlotF32::new("explicit", &[f32::NAN], 1, 1)
                .with_scale(1.0, 0.0)
                .validate()
                .is_err()
        );
        assert!(
            HeatmapPlot::new("extremes", &[-f64::MAX, f64::MAX], 1, 2)
                .validate()
                .is_err()
        );
        // An explicit reversed scale still saturates both infinite endpoints.
        assert_eq!(
            HeatmapPlotF32::new("clamped", &[f32::INFINITY, f32::NEG_INFINITY], 1, 2)
                .with_scale(1.0, 0.0)
                .checked_scale(),
            Ok((1.0, 0.0))
        );
    }

    fn invalid_data_message(err: PlotError) -> String {
        match err {
            PlotError::InvalidData(message) => message,
            other => panic!("expected invalid data error, got {other:?}"),
        }
    }

    #[test]
    fn heatmap_rejects_zero_counts_before_ffi() {
        let values = [1.0];
        let err = HeatmapPlot::new("heat", &values, 0, 1)
            .validate()
            .expect_err("zero row count must be rejected");
        assert!(invalid_data_message(err).contains("Rows and columns must be positive"));
    }

    #[test]
    fn heatmap_rejects_count_multiplication_overflow_before_ffi() {
        let values = [1.0];
        let err = HeatmapPlot::new("heat", &values, usize::MAX, 2)
            .validate()
            .expect_err("overflowing grid size must be rejected");
        assert!(invalid_data_message(err).contains("rows * cols overflowed"));
    }

    #[test]
    fn heatmap_rejects_i32_count_overflow_before_ffi() {
        let values = [1.0];
        let err = HeatmapPlot::new("heat", &values, i32::MAX as usize + 1, 1)
            .validate()
            .expect_err("oversized row count must be rejected");
        assert!(invalid_data_message(err).contains("rows exceeded ImPlot's i32 range"));
    }

    #[test]
    fn heatmap_f32_uses_checked_grid_counts() {
        let values = [1.0f32];
        let err = HeatmapPlotF32::new("heat", &values, 1, i32::MAX as usize + 1)
            .validate()
            .expect_err("oversized column count must be rejected");
        assert!(invalid_data_message(err).contains("cols exceeded ImPlot's i32 range"));
    }

    #[test]
    fn heatmap_rejects_a_cell_count_outside_native_index_range() {
        let cols = i32::MAX as usize / 2 + 1;
        let err = validate_grid_counts("heatmap", 2, cols, 2 * cols).unwrap_err();
        assert!(invalid_data_message(err).contains("rows * cols exceeded ImPlot's i32 range"));
    }
}
