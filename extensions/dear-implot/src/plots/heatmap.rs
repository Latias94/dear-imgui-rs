//! Heatmap plot implementation

use super::{
    Plot, PlotDataLayout, PlotError, PlotItemStyle, plot_spec_with_style, with_plot_str_or_empty,
};
use crate::{HeatmapFlags, ItemFlags, sys};
use dear_imgui_rs::with_scratch_txt_two;

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

/// Builder for heatmap plots with extensive customization options
pub struct HeatmapPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    style: PlotItemStyle,
    rows: usize,
    cols: usize,
    scale_min: f64,
    scale_max: f64,
    label_fmt: Option<&'a str>,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    flags: HeatmapFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for HeatmapPlot<'a> {
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

    /// Set the color scale range (min, max)
    /// If both are 0.0, auto-scaling will be used
    pub fn with_scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    /// Set the label format for values (e.g., "%.2f", "%.1e")
    /// Set to None to disable labels
    pub fn with_label_format(mut self, fmt: Option<&'a str>) -> Self {
        self.label_fmt = fmt;
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

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        validate_grid_counts(
            "HeatmapPlot::validate()",
            self.rows,
            self.cols,
            self.values.len(),
        )
    }
}

impl<'a> Plot for HeatmapPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return; // Skip plotting if data is invalid
        }
        let Ok(rows) = heatmap_count_to_i32("HeatmapPlot::plot()", "rows", self.rows) else {
            return;
        };
        let Ok(cols) = heatmap_count_to_i32("HeatmapPlot::plot()", "cols", self.cols) else {
            return;
        };
        let label_fmt = self.label_fmt.filter(|s| !s.contains('\0'));
        let _guard = plot_ui.bind();
        match label_fmt {
            Some(label_fmt) => {
                let label = if self.label.contains('\0') {
                    ""
                } else {
                    self.label
                };
                with_scratch_txt_two(label, label_fmt, |label_ptr, label_fmt_ptr| unsafe {
                    let spec = plot_spec_with_style(
                        self.style,
                        self.flags.bits() | self.item_flags.bits(),
                        PlotDataLayout::DEFAULT,
                    );
                    sys::ImPlot_PlotHeatmap_doublePtr(
                        label_ptr,
                        self.values.as_ptr(),
                        rows,
                        cols,
                        self.scale_min,
                        self.scale_max,
                        label_fmt_ptr,
                        self.bounds_min,
                        self.bounds_max,
                        spec,
                    );
                })
            }
            None => with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotHeatmap_doublePtr(
                    label_ptr,
                    self.values.as_ptr(),
                    rows,
                    cols,
                    self.scale_min,
                    self.scale_max,
                    std::ptr::null(),
                    self.bounds_min,
                    self.bounds_max,
                    spec,
                );
            }),
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Float version of heatmap for better performance with f32 data
pub struct HeatmapPlotF32<'a> {
    label: &'a str,
    values: &'a [f32],
    style: PlotItemStyle,
    rows: usize,
    cols: usize,
    scale_min: f64,
    scale_max: f64,
    label_fmt: Option<&'a str>,
    bounds_min: sys::ImPlotPoint,
    bounds_max: sys::ImPlotPoint,
    flags: HeatmapFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for HeatmapPlotF32<'a> {
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

    /// Set the color scale range (min, max)
    pub fn with_scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    /// Set the label format for values
    pub fn with_label_format(mut self, fmt: Option<&'a str>) -> Self {
        self.label_fmt = fmt;
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

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }

        validate_grid_counts(
            "HeatmapPlotF32::validate()",
            self.rows,
            self.cols,
            self.values.len(),
        )
    }
}

impl<'a> Plot for HeatmapPlotF32<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(rows) = heatmap_count_to_i32("HeatmapPlotF32::plot()", "rows", self.rows) else {
            return;
        };
        let Ok(cols) = heatmap_count_to_i32("HeatmapPlotF32::plot()", "cols", self.cols) else {
            return;
        };
        let label_fmt = self.label_fmt.filter(|s| !s.contains('\0'));
        let _guard = plot_ui.bind();
        match label_fmt {
            Some(label_fmt) => {
                let label = if self.label.contains('\0') {
                    ""
                } else {
                    self.label
                };
                with_scratch_txt_two(label, label_fmt, |label_ptr, label_fmt_ptr| unsafe {
                    let spec = plot_spec_with_style(
                        self.style,
                        self.flags.bits() | self.item_flags.bits(),
                        PlotDataLayout::DEFAULT,
                    );
                    sys::ImPlot_PlotHeatmap_FloatPtr(
                        label_ptr,
                        self.values.as_ptr(),
                        rows,
                        cols,
                        self.scale_min,
                        self.scale_max,
                        label_fmt_ptr,
                        self.bounds_min,
                        self.bounds_max,
                        spec,
                    );
                })
            }
            None => with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotHeatmap_FloatPtr(
                    label_ptr,
                    self.values.as_ptr(),
                    rows,
                    cols,
                    self.scale_min,
                    self.scale_max,
                    std::ptr::null(),
                    self.bounds_min,
                    self.bounds_max,
                    spec,
                );
            }),
        }
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick heatmap plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a heatmap with f64 data
    pub fn heatmap_plot(
        &self,
        label: &str,
        values: &[f64],
        rows: usize,
        cols: usize,
    ) -> Result<(), PlotError> {
        let plot = HeatmapPlot::new(label, values, rows, cols);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a heatmap with f32 data
    pub fn heatmap_plot_f32(
        &self,
        label: &str,
        values: &[f32],
        rows: usize,
        cols: usize,
    ) -> Result<(), PlotError> {
        let plot = HeatmapPlotF32::new(label, values, rows, cols);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a heatmap with custom scale and bounds
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
        let plot = HeatmapPlot::new(label, values, rows, cols)
            .with_scale(scale_min, scale_max)
            .with_bounds_points(bounds_min, bounds_max);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{HeatmapPlot, HeatmapPlotF32};
    use crate::PlotError;

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
}
