//! Error bars plot implementation

use super::{
    Plot, PlotDataLayout, PlotDataOffset, PlotDataStride, PlotError, PlotItemStyle,
    plot_spec_with_style, validate_data_lengths, with_plot_str_or_empty,
};
use crate::{ErrorBarsFlags, ItemFlags, sys};

/// Builder for error bars plots
pub struct ErrorBarsPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    err_data: &'a [f64],
    style: PlotItemStyle,
    flags: ErrorBarsFlags,
    item_flags: ItemFlags,
    layout: PlotDataLayout,
}

impl<'a> super::PlotItemStyled for ErrorBarsPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> ErrorBarsPlot<'a> {
    /// Create a new error bars plot with symmetric errors
    ///
    /// # Arguments
    /// * `label` - The label for the error bars
    /// * `x_data` - X coordinates of the data points
    /// * `y_data` - Y coordinates of the data points
    /// * `err_data` - Deltas applied on either side of each value (`value ± err`)
    pub fn new(label: &'a str, x_data: &'a [f64], y_data: &'a [f64], err_data: &'a [f64]) -> Self {
        Self {
            label,
            x_data,
            y_data,
            err_data,
            style: PlotItemStyle::default(),
            flags: ErrorBarsFlags::NONE,
            item_flags: ItemFlags::NONE,
            layout: PlotDataLayout::DEFAULT,
        }
    }

    /// Set error bar flags for customization
    pub fn with_flags(mut self, flags: ErrorBarsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Set the data layout used to read values.
    ///
    /// # Safety
    ///
    /// Every sample address computed from `layout` must refer to an initialized, properly aligned
    /// `f64` within every data allocation retained by this builder.
    pub unsafe fn with_data_layout(mut self, layout: PlotDataLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Set the sample-index offset used to read values.
    pub fn with_offset(mut self, offset: PlotDataOffset) -> Self {
        self.layout = self.layout.with_offset(offset);
        self
    }

    /// Set the byte stride used to read values.
    ///
    /// # Safety
    ///
    /// Every strided sample read must remain initialized, aligned, and within every data allocation
    /// retained by this builder.
    pub unsafe fn with_stride(mut self, stride: PlotDataStride) -> Self {
        self.layout = self.layout.with_stride(stride);
        self
    }

    /// Make error bars horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= ErrorBarsFlags::HORIZONTAL;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(self.x_data, self.y_data)?;
        validate_data_lengths(self.x_data, self.err_data)
    }
}

impl<'a> Plot for ErrorBarsPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Some(count) = self.layout.sample_count(self.x_data.len()) else {
            return;
        };
        plot_ui.with_bound_context(|| {
            with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    self.layout,
                );
                sys::ImPlot_PlotErrorBars_doublePtrdoublePtrdoublePtrInt(
                    label_ptr,
                    self.x_data.as_ptr(),
                    self.y_data.as_ptr(),
                    self.err_data.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Builder for asymmetric error bars plots
pub struct AsymmetricErrorBarsPlot<'a> {
    label: &'a str,
    x_data: &'a [f64],
    y_data: &'a [f64],
    err_neg: &'a [f64],
    err_pos: &'a [f64],
    style: PlotItemStyle,
    flags: ErrorBarsFlags,
    item_flags: ItemFlags,
    layout: PlotDataLayout,
}

impl<'a> super::PlotItemStyled for AsymmetricErrorBarsPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> AsymmetricErrorBarsPlot<'a> {
    /// Create a new asymmetric error bars plot
    ///
    /// # Arguments
    /// * `label` - The label for the error bars
    /// * `x_data` - X coordinates of the data points
    /// * `y_data` - Y coordinates of the data points
    /// * `err_neg` - Signed deltas subtracted from the plotted coordinate for the first endpoint
    /// * `err_pos` - Signed deltas added to the plotted coordinate for the second endpoint
    pub fn new(
        label: &'a str,
        x_data: &'a [f64],
        y_data: &'a [f64],
        err_neg: &'a [f64],
        err_pos: &'a [f64],
    ) -> Self {
        Self {
            label,
            x_data,
            y_data,
            err_neg,
            err_pos,
            style: PlotItemStyle::default(),
            flags: ErrorBarsFlags::NONE,
            item_flags: ItemFlags::NONE,
            layout: PlotDataLayout::DEFAULT,
        }
    }

    /// Set error bar flags for customization
    pub fn with_flags(mut self, flags: ErrorBarsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Set the data layout used to read values.
    ///
    /// # Safety
    ///
    /// Every sample address computed from `layout` must refer to an initialized, properly aligned
    /// `f64` within every data allocation retained by this builder.
    pub unsafe fn with_data_layout(mut self, layout: PlotDataLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Set the sample-index offset used to read values.
    pub fn with_offset(mut self, offset: PlotDataOffset) -> Self {
        self.layout = self.layout.with_offset(offset);
        self
    }

    /// Set the byte stride used to read values.
    ///
    /// # Safety
    ///
    /// Every strided sample read must remain initialized, aligned, and within every data allocation
    /// retained by this builder.
    pub unsafe fn with_stride(mut self, stride: PlotDataStride) -> Self {
        self.layout = self.layout.with_stride(stride);
        self
    }

    /// Make error bars horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= ErrorBarsFlags::HORIZONTAL;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(self.x_data, self.y_data)?;
        validate_data_lengths(self.x_data, self.err_neg)?;
        validate_data_lengths(self.x_data, self.err_pos)
    }
}

impl<'a> Plot for AsymmetricErrorBarsPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Some(count) = self.layout.sample_count(self.x_data.len()) else {
            return;
        };
        plot_ui.with_bound_context(|| {
            with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    self.layout,
                );
                sys::ImPlot_PlotErrorBars_doublePtrdoublePtrdoublePtrdoublePtr(
                    label_ptr,
                    self.x_data.as_ptr(),
                    self.y_data.as_ptr(),
                    self.err_neg.as_ptr(),
                    self.err_pos.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Simple error bars plot for quick plotting
pub struct SimpleErrorBarsPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    errors: &'a [f64],
    style: PlotItemStyle,
    flags: ErrorBarsFlags,
    item_flags: ItemFlags,
    x_scale: f64,
    x_start: f64,
}

impl<'a> super::PlotItemStyled for SimpleErrorBarsPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> SimpleErrorBarsPlot<'a> {
    /// Create a simple error bars plot with Y values only (X will be indices)
    pub fn new(label: &'a str, values: &'a [f64], errors: &'a [f64]) -> Self {
        Self {
            label,
            values,
            errors,
            style: PlotItemStyle::default(),
            flags: ErrorBarsFlags::NONE,
            item_flags: ItemFlags::NONE,
            x_scale: 1.0,
            x_start: 0.0,
        }
    }

    /// Set error bar flags for customization
    pub fn with_flags(mut self, flags: ErrorBarsFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make error bars horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= ErrorBarsFlags::HORIZONTAL;
        self
    }

    /// Set X scale factor
    pub fn with_x_scale(mut self, scale: f64) -> Self {
        self.x_scale = scale;
        self
    }

    /// Set X start value
    pub fn with_x_start(mut self, start: f64) -> Self {
        self.x_start = start;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        validate_data_lengths(self.values, self.errors)
    }
}

impl<'a> Plot for SimpleErrorBarsPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(count) = i32::try_from(self.values.len()) else {
            return;
        };

        // Create temporary X data
        let x_data: Vec<f64> = (0..self.values.len())
            .map(|i| self.x_start + i as f64 * self.x_scale)
            .collect();

        plot_ui.with_bound_context(|| {
            with_plot_str_or_empty(self.label, |label_ptr| unsafe {
                let spec = plot_spec_with_style(
                    self.style,
                    self.flags.bits() | self.item_flags.bits(),
                    PlotDataLayout::DEFAULT,
                );
                sys::ImPlot_PlotErrorBars_doublePtrdoublePtrdoublePtrInt(
                    label_ptr,
                    x_data.as_ptr(),
                    self.values.as_ptr(),
                    self.errors.as_ptr(),
                    count,
                    spec,
                );
            })
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick error bars plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot error bars with symmetric errors
    pub fn error_bars_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
        err_data: &[f64],
    ) -> Result<(), PlotError> {
        let plot = ErrorBarsPlot::new(label, x_data, y_data, err_data);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot error bars with asymmetric errors
    pub fn asymmetric_error_bars_plot(
        &self,
        label: &str,
        x_data: &[f64],
        y_data: &[f64],
        err_neg: &[f64],
        err_pos: &[f64],
    ) -> Result<(), PlotError> {
        let plot = AsymmetricErrorBarsPlot::new(label, x_data, y_data, err_neg, err_pos);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot simple error bars with Y values only (X will be indices)
    pub fn simple_error_bars_plot(
        &self,
        label: &str,
        values: &[f64],
        errors: &[f64],
    ) -> Result<(), PlotError> {
        let plot = SimpleErrorBarsPlot::new(label, values, errors);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strided_errors_validate_the_logical_series_lengths() {
        let rows = [[1.0, 5.0, 0.1, 0.3], [-2.0, 6.0, 0.2, 0.4]];
        let data = rows.as_flattened();
        let field = |column| &data[column..column + rows.len()];
        let stride = PlotDataStride::for_type::<[f64; 4]>();
        // Each field has two samples 32 bytes apart. The next row's negative X lies in
        // the packed prefix of the upper-error view, between its two actual error values.
        let symmetric =
            unsafe { ErrorBarsPlot::new("sym", field(0), field(1), field(3)).with_stride(stride) };
        let asymmetric = unsafe {
            AsymmetricErrorBarsPlot::new("asym", field(0), field(1), field(2), field(3))
                .with_stride(stride)
        };
        assert_eq!(symmetric.validate(), Ok(()));
        assert_eq!(asymmetric.validate(), Ok(()));
    }

    #[test]
    fn test_simple_error_bars_plot_flags() {
        let values = [1.0, 2.0, 3.0, 4.0];
        let errors = [0.1, 0.2, 0.3, 0.4];
        let plot = SimpleErrorBarsPlot::new("test", &values, &errors)
            .horizontal()
            .with_item_flags(ItemFlags::NO_LEGEND);
        assert_eq!(plot.label(), "test");
        assert_eq!(plot.flags.bits(), ErrorBarsFlags::HORIZONTAL.bits());
        assert_eq!(plot.item_flags, ItemFlags::NO_LEGEND);
    }
}
