//! Histogram plot implementation

use super::{Plot, PlotError, safe_cstring};
use crate::sys;
use crate::{BinMethod, HistogramFlags};

/// Builder for 1D histogram plots
pub struct HistogramPlot<'a> {
    label: &'a str,
    values: &'a [f64],
    bins: i32,
    bar_scale: f64,
    range: Option<sys::ImPlotRange>,
    flags: HistogramFlags,
}

impl<'a> HistogramPlot<'a> {
    /// Create a new histogram plot with the given label and data
    pub fn new(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            label,
            values,
            bins: BinMethod::Sturges as i32,
            bar_scale: 1.0,
            range: None, // Auto-range
            flags: HistogramFlags::NONE,
        }
    }

    /// Set the number of bins (positive integer) or binning method (negative value)
    /// Common binning methods:
    /// - ImPlotBin_Sqrt = -1
    /// - ImPlotBin_Sturges = -2  (default)
    /// - ImPlotBin_Rice = -3
    /// - ImPlotBin_Scott = -4
    pub fn with_bins(mut self, bins: i32) -> Self {
        self.bins = bins;
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
    fn plot(&self) {
        if self.validate().is_err() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        let range = if let Some(range) = &self.range {
            *range
        } else {
            sys::ImPlotRange { Min: 0.0, Max: 0.0 }
        };

        unsafe {
            sys::ImPlot_PlotHistogram_doublePtr(
                label_cstr.as_ptr(),
                self.values.as_ptr(),
                self.values.len() as i32,
                self.bins,
                self.bar_scale,
                range,
                self.flags.bits() as i32,
            );
        }
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
    x_bins: i32,
    y_bins: i32,
    range: Option<sys::ImPlotRect>,
    flags: HistogramFlags,
}

impl<'a> Histogram2DPlot<'a> {
    /// Create a new 2D histogram plot with the given label and data
    pub fn new(label: &'a str, x_values: &'a [f64], y_values: &'a [f64]) -> Self {
        Self {
            label,
            x_values,
            y_values,
            x_bins: BinMethod::Sturges as i32,
            y_bins: BinMethod::Sturges as i32,
            range: None, // Auto-range
            flags: HistogramFlags::NONE,
        }
    }

    /// Set the number of bins for both X and Y axes
    pub fn with_bins(mut self, x_bins: i32, y_bins: i32) -> Self {
        self.x_bins = x_bins;
        self.y_bins = y_bins;
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
    fn plot(&self) {
        if self.validate().is_err() {
            return;
        }

        let label_cstr = safe_cstring(self.label);

        let range = if let Some(range) = &self.range {
            *range
        } else {
            sys::ImPlotRect {
                X: sys::ImPlotRange { Min: 0.0, Max: 0.0 },
                Y: sys::ImPlotRange { Min: 0.0, Max: 0.0 },
            }
        };

        unsafe {
            sys::ImPlot_PlotHistogram2D_doublePtr(
                label_cstr.as_ptr(),
                self.x_values.as_ptr(),
                self.y_values.as_ptr(),
                self.x_values.len() as i32,
                self.x_bins,
                self.y_bins,
                range,
                self.flags.bits() as i32,
            );
        }
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
        plot.plot();
        Ok(())
    }

    /// Plot a 1D histogram with custom bin count
    pub fn histogram_plot_with_bins(
        &self,
        label: &str,
        values: &[f64],
        bins: i32,
    ) -> Result<(), PlotError> {
        let plot = HistogramPlot::new(label, values).with_bins(bins);
        plot.validate()?;
        plot.plot();
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
        plot.plot();
        Ok(())
    }

    /// Plot a 2D histogram with custom bin counts
    pub fn histogram_2d_plot_with_bins(
        &self,
        label: &str,
        x_values: &[f64],
        y_values: &[f64],
        x_bins: i32,
        y_bins: i32,
    ) -> Result<(), PlotError> {
        let plot = Histogram2DPlot::new(label, x_values, y_values).with_bins(x_bins, y_bins);
        plot.validate()?;
        plot.plot();
        Ok(())
    }
}
