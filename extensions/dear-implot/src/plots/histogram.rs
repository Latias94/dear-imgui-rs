//! Histogram plot implementation

use super::{
    Plot, PlotDataLayout, PlotError, PlotItemStyle, plot_spec_with_style, with_plot_str_or_empty,
};
use crate::sys;
use crate::{HistogramBins, HistogramFlags, ItemFlags};

/// Builder for 1D histogram plots
pub struct HistogramPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    style: PlotItemStyle,
    bins: HistogramBins,
    bar_scale: f64,
    range: Option<sys::ImPlotRange>,
    flags: HistogramFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for HistogramPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> HistogramPlot<'a> {
    /// Create a new histogram plot with the given label and data
    pub fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            label,
            values,
            style: PlotItemStyle::default(),
            bins: HistogramBins::DEFAULT,
            bar_scale: 1.0,
            range: None, // Auto-range
            flags: HistogramFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set a concrete positive bin count or automatic binning method.
    pub fn with_bins(mut self, bins: impl Into<HistogramBins>) -> Self {
        self.bins = bins.into();
        self
    }

    /// Set the bar scale factor
    pub fn with_bar_scale(mut self, scale: f64) -> Self {
        self.bar_scale = scale;
        self
    }

    /// Set the data range for binning
    /// Values outside this range will be treated as outliers
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.range = Some(sys::ImPlotRange { Min: min, Max: max });
        self
    }

    /// Set the data range using ImPlotRange
    pub fn with_range_struct(mut self, range: sys::ImPlotRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Set histogram flags for customization
    pub fn with_flags(mut self, flags: HistogramFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Make the histogram horizontal instead of vertical
    pub fn horizontal(mut self) -> Self {
        self.flags |= HistogramFlags::HORIZONTAL;
        self
    }

    /// Make the histogram cumulative
    pub fn cumulative(mut self) -> Self {
        self.flags |= HistogramFlags::CUMULATIVE;
        self
    }

    /// Normalize the histogram to show density (PDF)
    pub fn density(mut self) -> Self {
        self.flags |= HistogramFlags::DENSITY;
        self
    }

    /// Exclude outliers from normalization
    pub fn no_outliers(mut self) -> Self {
        self.flags |= HistogramFlags::NO_OUTLIERS;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        if self.values.is_empty() {
            return Err(PlotError::EmptyData);
        }
        Ok(())
    }
}

impl<'a> Plot for HistogramPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(count) = i32::try_from(self.values.len()) else {
            return;
        };

        let range = if let Some(range) = &self.range {
            *range
        } else {
            sys::ImPlotRange { Min: 0.0, Max: 0.0 }
        };

        let _guard = plot_ui.bind();
        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                PlotDataLayout::DEFAULT,
            );
            sys::ImPlot_PlotHistogram_doublePtr(
                label_ptr,
                self.values.as_ptr(),
                count,
                self.bins.raw("HistogramPlot::plot()"),
                self.bar_scale,
                range,
                spec,
            );
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Builder for 2D histogram plots (bivariate histograms as heatmaps)
pub struct Histogram2DPlot<'a> {
    label: &'a str,
    x_values: &'a [f64],
    y_values: &'a [f64],
    style: PlotItemStyle,
    x_bins: HistogramBins,
    y_bins: HistogramBins,
    range: Option<sys::ImPlotRect>,
    flags: HistogramFlags,
    item_flags: ItemFlags,
}

impl<'a> super::PlotItemStyled for Histogram2DPlot<'a> {
    fn style_mut(&mut self) -> &mut PlotItemStyle {
        &mut self.style
    }
}

impl<'a> Histogram2DPlot<'a> {
    /// Create a new 2D histogram plot with the given label and data
    pub fn new(label: &'a str, x_values: &'a [f64], y_values: &'a [f64]) -> Self {
        Self {
            label,
            x_values,
            y_values,
            style: PlotItemStyle::default(),
            x_bins: HistogramBins::DEFAULT,
            y_bins: HistogramBins::DEFAULT,
            range: None, // Auto-range
            flags: HistogramFlags::NONE,
            item_flags: ItemFlags::NONE,
        }
    }

    /// Set the number of bins for both X and Y axes
    pub fn with_bins(
        mut self,
        x_bins: impl Into<HistogramBins>,
        y_bins: impl Into<HistogramBins>,
    ) -> Self {
        self.x_bins = x_bins.into();
        self.y_bins = y_bins.into();
        self
    }

    /// Set the data range for binning
    pub fn with_range(mut self, x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        self.range = Some(sys::ImPlotRect {
            X: sys::ImPlotRange {
                Min: x_min,
                Max: x_max,
            },
            Y: sys::ImPlotRange {
                Min: y_min,
                Max: y_max,
            },
        });
        self
    }

    /// Set the data range using ImPlotRect
    pub fn with_range_struct(mut self, range: sys::ImPlotRect) -> Self {
        self.range = Some(range);
        self
    }

    /// Set histogram flags for customization
    pub fn with_flags(mut self, flags: HistogramFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set common item flags for this plot item (applies to all plot types)
    pub fn with_item_flags(mut self, flags: ItemFlags) -> Self {
        self.item_flags = flags;
        self
    }

    /// Normalize the histogram to show density
    pub fn density(mut self) -> Self {
        self.flags |= HistogramFlags::DENSITY;
        self
    }

    /// Exclude outliers from normalization
    pub fn no_outliers(mut self) -> Self {
        self.flags |= HistogramFlags::NO_OUTLIERS;
        self
    }

    /// Use column-major data ordering
    pub fn column_major(mut self) -> Self {
        self.flags |= HistogramFlags::COL_MAJOR;
        self
    }

    /// Validate the plot data
    pub fn validate(&self) -> Result<(), PlotError> {
        super::validate_data_lengths(self.x_values, self.y_values)
    }
}

impl<'a> Plot for Histogram2DPlot<'a> {
    fn plot(&self, plot_ui: &crate::PlotUi<'_>) {
        if self.validate().is_err() {
            return;
        }
        let Ok(count) = i32::try_from(self.x_values.len()) else {
            return;
        };

        let range = if let Some(range) = &self.range {
            *range
        } else {
            sys::ImPlotRect {
                X: sys::ImPlotRange { Min: 0.0, Max: 0.0 },
                Y: sys::ImPlotRange { Min: 0.0, Max: 0.0 },
            }
        };

        let _guard = plot_ui.bind();
        with_plot_str_or_empty(self.label, |label_ptr| unsafe {
            let spec = plot_spec_with_style(
                self.style,
                self.flags.bits() | self.item_flags.bits(),
                PlotDataLayout::DEFAULT,
            );
            sys::ImPlot_PlotHistogram2D_doublePtr(
                label_ptr,
                self.x_values.as_ptr(),
                self.y_values.as_ptr(),
                count,
                self.x_bins.raw("Histogram2DPlot::plot()"),
                self.y_bins.raw("Histogram2DPlot::plot()"),
                range,
                spec,
            );
        })
    }

    fn label(&self) -> &str {
        self.label
    }
}

/// Convenience functions for quick histogram plotting
impl<'ui> crate::PlotUi<'ui> {
    /// Plot a 1D histogram with default settings
    pub fn histogram_plot(&self, label: &str, values: &[f64]) -> Result<(), PlotError> {
        let plot = HistogramPlot::new(label, values);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a 1D histogram with custom bin count
    pub fn histogram_plot_with_bins(
        &self,
        label: &str,
        values: &[f64],
        bins: impl Into<HistogramBins>,
    ) -> Result<(), PlotError> {
        let plot = HistogramPlot::new(label, values).with_bins(bins);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a 2D histogram (bivariate histogram as heatmap)
    pub fn histogram_2d_plot(
        &self,
        label: &str,
        x_values: &[f64],
        y_values: &[f64],
    ) -> Result<(), PlotError> {
        let plot = Histogram2DPlot::new(label, x_values, y_values);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }

    /// Plot a 2D histogram with custom bin counts
    pub fn histogram_2d_plot_with_bins(
        &self,
        label: &str,
        x_values: &[f64],
        y_values: &[f64],
        x_bins: impl Into<HistogramBins>,
        y_bins: impl Into<HistogramBins>,
    ) -> Result<(), PlotError> {
        let plot = Histogram2DPlot::new(label, x_values, y_values).with_bins(x_bins, y_bins);
        plot.validate()?;
        plot.plot(self);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Histogram2DPlot, HistogramPlot};
    use crate::{BinMethod, HistogramBins};

    #[test]
    fn histogram_bins_distinguish_counts_from_methods() {
        assert_eq!(HistogramBins::from(8usize).raw("test"), 8);
        assert_eq!(
            HistogramBins::from(BinMethod::Rice).raw("test"),
            BinMethod::Rice as i32
        );
        assert_eq!(
            HistogramBins::DEFAULT.raw("test"),
            BinMethod::Sturges as i32
        );
    }

    #[test]
    #[should_panic(expected = "test bin count must be positive")]
    fn histogram_bins_reject_zero_counts_before_ffi() {
        let _ = HistogramBins::from(0usize).raw("test");
    }

    #[test]
    #[should_panic(expected = "test bin count exceeded ImPlot's i32 range")]
    fn histogram_bins_reject_oversized_counts_before_ffi() {
        let _ = HistogramBins::from(i32::MAX as usize + 1).raw("test");
    }

    #[test]
    fn histogram_builders_accept_typed_bins() {
        let values = [1.0, 2.0, 3.0];
        let _ = HistogramPlot::new("hist", &values).with_bins(8usize);
        let _ = HistogramPlot::new("hist", &values).with_bins(BinMethod::Scott);
        let _ = Histogram2DPlot::new("hist2d", &values, &values).with_bins(4usize, BinMethod::Rice);
    }
}
