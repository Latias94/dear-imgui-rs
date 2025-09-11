//! Modular plot types for ImPlot
//!
//! This module provides a modular approach to different plot types,
//! each with their own builder pattern and configuration options.

pub mod bar;
pub mod bar_groups;
pub mod digital;
pub mod dummy;
pub mod error_bars;
pub mod heatmap;
pub mod histogram;
pub mod line;
pub mod pie;
pub mod scatter;
pub mod shaded;
pub mod stairs;
pub mod stems;
pub mod text;

// Re-export all plot types for convenience
pub use bar::*;
pub use bar_groups::*;
pub use digital::*;
pub use dummy::*;
pub use error_bars::*;
pub use heatmap::*;
pub use histogram::*;
pub use line::*;
pub use pie::*;
pub use scatter::*;
pub use shaded::*;
pub use stairs::*;
pub use stems::*;
pub use text::*;

/// Common trait for all plot types
pub trait Plot {
    /// Plot this element
    fn plot(&self);

    /// Get the label for this plot
    fn label(&self) -> &str;
}

/// Common trait for plot data validation
pub trait PlotData {
    /// Get the label for this plot
    fn label(&self) -> &str;

    /// Get the length of the data
    fn data_len(&self) -> usize;

    /// Check if the data is empty
    fn is_empty(&self) -> bool {
        self.data_len() == 0
    }

    /// Validate the data for plotting
    fn validate(&self) -> Result<(), PlotError> {
        if self.is_empty() {
            Err(PlotError::EmptyData)
        } else {
            Ok(())
        }
    }
}

/// Errors that can occur during plotting
#[derive(Debug, Clone, PartialEq)]
pub enum PlotError {
    /// Data arrays have mismatched lengths
    DataLengthMismatch { x_len: usize, y_len: usize },
    /// Data is empty
    EmptyData,
    /// Invalid parameter value or data
    InvalidData(String),
    /// String conversion error
    StringConversion(String),
    /// Plot creation failed
    PlotCreationFailed(String),
}

impl std::fmt::Display for PlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlotError::DataLengthMismatch { x_len, y_len } => {
                write!(
                    f,
                    "Data length mismatch: x has {} elements, y has {} elements",
                    x_len, y_len
                )
            }
            PlotError::EmptyData => write!(f, "Data is empty"),
            PlotError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            PlotError::StringConversion(msg) => write!(f, "String conversion error: {}", msg),
            PlotError::PlotCreationFailed(msg) => write!(f, "Plot creation failed: {}", msg),
        }
    }
}

impl std::error::Error for PlotError {}

// Note: PlotData trait is implemented by individual plot types
// rather than raw data types, since each plot needs its own label

/// Helper function to validate data length consistency for slices
pub fn validate_data_lengths<T, U>(data1: &[T], data2: &[U]) -> Result<(), PlotError> {
    if data1.is_empty() || data2.is_empty() {
        return Err(PlotError::EmptyData);
    }

    if data1.len() != data2.len() {
        return Err(PlotError::DataLengthMismatch {
            x_len: data1.len(),
            y_len: data2.len(),
        });
    }

    Ok(())
}

/// Helper function to create a CString safely
/// This follows the pattern used in dear-imgui for safe string conversion
pub fn safe_cstring(s: &str) -> std::ffi::CString {
    // Remove any null bytes from the string to prevent CString creation errors
    let cleaned = s.replace('\0', "");
    std::ffi::CString::new(cleaned).unwrap_or_else(|_| {
        // If there's still an error, create an empty string
        std::ffi::CString::new("").unwrap()
    })
}

/// Universal plot builder that can create any plot type
pub struct PlotBuilder<'a> {
    plot_type: PlotType<'a>,
}

/// Enum representing different plot types
pub enum PlotType<'a> {
    Line {
        label: &'a str,
        x_data: &'a [f64],
        y_data: &'a [f64],
    },
    Scatter {
        label: &'a str,
        x_data: &'a [f64],
        y_data: &'a [f64],
    },
    Bar {
        label: &'a str,
        values: &'a [f64],
        width: f64,
    },
    Histogram {
        label: &'a str,
        values: &'a [f64],
        bins: i32,
    },
    Heatmap {
        label: &'a str,
        values: &'a [f64],
        rows: usize,
        cols: usize,
    },
    PieChart {
        labels: Vec<&'a str>,
        values: &'a [f64],
        center: (f64, f64),
        radius: f64,
    },
}

impl<'a> PlotBuilder<'a> {
    /// Create a line plot
    pub fn line(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            plot_type: PlotType::Line {
                label,
                x_data,
                y_data,
            },
        }
    }

    /// Create a scatter plot
    pub fn scatter(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            plot_type: PlotType::Scatter {
                label,
                x_data,
                y_data,
            },
        }
    }

    /// Create a bar plot
    pub fn bar(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            plot_type: PlotType::Bar {
                label,
                values,
                width: 0.67,
            },
        }
    }

    /// Create a histogram
    pub fn histogram(label: &'a str, values: &'a [f64]) -> Self {
        Self {
            plot_type: PlotType::Histogram {
                label,
                values,
                bins: crate::BinMethod::Sturges as i32,
            },
        }
    }

    /// Create a heatmap
    pub fn heatmap(label: &'a str, values: &'a [f64], rows: usize, cols: usize) -> Self {
        Self {
            plot_type: PlotType::Heatmap {
                label,
                values,
                rows,
                cols,
            },
        }
    }

    /// Create a pie chart
    pub fn pie_chart(
        labels: Vec<&'a str>,
        values: &'a [f64],
        center: (f64, f64),
        radius: f64,
    ) -> Self {
        Self {
            plot_type: PlotType::PieChart {
                labels,
                values,
                center,
                radius,
            },
        }
    }

    /// Build and plot the chart
    pub fn build(self) -> Result<(), PlotError> {
        match self.plot_type {
            PlotType::Line {
                label,
                x_data,
                y_data,
            } => {
                let plot = line::LinePlot::new(label, x_data, y_data);
                plot.validate()?;
                plot.plot();
            }
            PlotType::Scatter {
                label,
                x_data,
                y_data,
            } => {
                let plot = scatter::ScatterPlot::new(label, x_data, y_data);
                plot.validate()?;
                plot.plot();
            }
            PlotType::Bar {
                label,
                values,
                width,
            } => {
                let plot = bar::BarPlot::new(label, values).with_bar_size(width);
                plot.validate()?;
                plot.plot();
            }
            PlotType::Histogram {
                label,
                values,
                bins,
            } => {
                let plot = histogram::HistogramPlot::new(label, values).with_bins(bins);
                plot.validate()?;
                plot.plot();
            }
            PlotType::Heatmap {
                label,
                values,
                rows,
                cols,
            } => {
                let plot = heatmap::HeatmapPlot::new(label, values, rows, cols);
                plot.validate()?;
                plot.plot();
            }
            PlotType::PieChart {
                labels,
                values,
                center,
                radius,
            } => {
                let plot = pie::PieChartPlot::new(labels, values, center.0, center.1, radius);
                plot.validate()?;
                plot.plot();
            }
        }
        Ok(())
    }
}
