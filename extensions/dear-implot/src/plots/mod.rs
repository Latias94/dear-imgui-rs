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
pub mod image;
pub mod inf_lines;
pub mod line;
pub mod pie;
pub mod polygon;
pub mod scatter;
pub mod shaded;
pub mod stairs;
pub mod stems;
pub mod text;

use crate::sys;
use dear_imgui_rs::{with_scratch_txt, with_scratch_txt_slice, with_scratch_txt_slice_with_opt};
use std::cell::RefCell;
use std::os::raw::c_char;

// Re-export all plot types for convenience
pub use bar::*;
pub use bar_groups::*;
pub use digital::*;
pub use dummy::*;
pub use error_bars::*;
pub use heatmap::*;
pub use histogram::*;
pub use image::*;
pub use inf_lines::*;
pub use line::*;
pub use pie::*;
pub use polygon::*;
pub use scatter::*;
pub use shaded::*;
pub use stairs::*;
pub use stems::*;
pub use text::*;

thread_local! {
    static NEXT_PLOT_SPEC: RefCell<Option<sys::ImPlotSpec_c>> = RefCell::new(None);
}

fn color4(rgba: [f32; 4]) -> sys::ImVec4_c {
    sys::ImVec4_c {
        x: rgba[0],
        y: rgba[1],
        z: rgba[2],
        w: rgba[3],
    }
}

/// Common style overrides for plot items backed by `ImPlotSpec`.
///
/// This provides a stable high-level Rust entry point for ImPlot v0.18 item
/// styling without exposing raw FFI structs at every call site.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlotItemStyle {
    pub(crate) line_color: Option<sys::ImVec4_c>,
    pub(crate) line_weight: Option<f32>,
    pub(crate) fill_color: Option<sys::ImVec4_c>,
    pub(crate) fill_alpha: Option<f32>,
    pub(crate) marker: Option<sys::ImPlotMarker>,
    pub(crate) marker_size: Option<f32>,
    pub(crate) marker_line_color: Option<sys::ImVec4_c>,
    pub(crate) marker_fill_color: Option<sys::ImVec4_c>,
    pub(crate) size: Option<f32>,
}

impl PlotItemStyle {
    /// Create an empty style override that keeps ImPlot defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the plot item's line color. Use the alpha channel for line transparency.
    pub fn with_line_color(mut self, color: [f32; 4]) -> Self {
        self.line_color = Some(color4(color));
        self
    }

    /// Override the plot item's line width in pixels.
    pub fn with_line_weight(mut self, weight: f32) -> Self {
        self.line_weight = Some(weight);
        self
    }

    /// Override the plot item's fill color.
    pub fn with_fill_color(mut self, color: [f32; 4]) -> Self {
        self.fill_color = Some(color4(color));
        self
    }

    /// Override the fill alpha multiplier used by filled regions and marker faces.
    pub fn with_fill_alpha(mut self, alpha: f32) -> Self {
        self.fill_alpha = Some(alpha);
        self
    }

    /// Override the marker type.
    pub fn with_marker(mut self, marker: crate::Marker) -> Self {
        self.marker = Some(marker as sys::ImPlotMarker);
        self
    }

    /// Override the marker size in pixels.
    pub fn with_marker_size(mut self, size: f32) -> Self {
        self.marker_size = Some(size);
        self
    }

    /// Override the marker outline color.
    pub fn with_marker_line_color(mut self, color: [f32; 4]) -> Self {
        self.marker_line_color = Some(color4(color));
        self
    }

    /// Override the marker fill color.
    pub fn with_marker_fill_color(mut self, color: [f32; 4]) -> Self {
        self.marker_fill_color = Some(color4(color));
        self
    }

    /// Override the generic size field used by some item types such as error bars.
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    pub(crate) fn apply_to_spec(self, spec: &mut sys::ImPlotSpec_c) {
        if let Some(line_color) = self.line_color {
            spec.LineColor = line_color;
        }
        if let Some(line_weight) = self.line_weight {
            spec.LineWeight = line_weight;
        }
        if let Some(fill_color) = self.fill_color {
            spec.FillColor = fill_color;
        }
        if let Some(fill_alpha) = self.fill_alpha {
            spec.FillAlpha = fill_alpha;
        }
        if let Some(marker) = self.marker {
            spec.Marker = marker;
        }
        if let Some(marker_size) = self.marker_size {
            spec.MarkerSize = marker_size;
        }
        if let Some(marker_line_color) = self.marker_line_color {
            spec.MarkerLineColor = marker_line_color;
        }
        if let Some(marker_fill_color) = self.marker_fill_color {
            spec.MarkerFillColor = marker_fill_color;
        }
        if let Some(size) = self.size {
            spec.Size = size;
        }
    }
}

/// Shared ImPlot item-style builder methods for plot builders backed by `ImPlotSpec`.
///
/// Importing `dear_implot::*` brings this trait into scope, so every supported
/// plot builder exposes the same styling methods.
pub trait PlotItemStyled: Sized {
    fn style_mut(&mut self) -> &mut PlotItemStyle;

    /// Replace the entire item style override for this plot.
    fn with_style(mut self, style: PlotItemStyle) -> Self {
        *self.style_mut() = style;
        self
    }

    /// Set the line color. Use the alpha channel to control line transparency.
    fn with_line_color(mut self, color: [f32; 4]) -> Self {
        self.style_mut().line_color = Some(color4(color));
        self
    }

    /// Set the line width in pixels.
    fn with_line_weight(mut self, weight: f32) -> Self {
        self.style_mut().line_weight = Some(weight);
        self
    }

    /// Set the fill color.
    fn with_fill_color(mut self, color: [f32; 4]) -> Self {
        self.style_mut().fill_color = Some(color4(color));
        self
    }

    /// Set the fill alpha multiplier used for fills and marker faces.
    fn with_fill_alpha(mut self, alpha: f32) -> Self {
        self.style_mut().fill_alpha = Some(alpha);
        self
    }

    /// Set the marker type.
    fn with_marker(mut self, marker: crate::Marker) -> Self {
        self.style_mut().marker = Some(marker as sys::ImPlotMarker);
        self
    }

    /// Set the marker size in pixels.
    fn with_marker_size(mut self, size: f32) -> Self {
        self.style_mut().marker_size = Some(size);
        self
    }

    /// Set the marker outline color.
    fn with_marker_line_color(mut self, color: [f32; 4]) -> Self {
        self.style_mut().marker_line_color = Some(color4(color));
        self
    }

    /// Set the marker fill color.
    fn with_marker_fill_color(mut self, color: [f32; 4]) -> Self {
        self.style_mut().marker_fill_color = Some(color4(color));
        self
    }

    /// Set the generic size field used by some plot types such as error bars and digital plots.
    fn with_size(mut self, size: f32) -> Self {
        self.style_mut().size = Some(size);
        self
    }
}

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

pub(crate) fn with_plot_str<R>(s: &str, f: impl FnOnce(*const c_char) -> R) -> Option<R> {
    if s.contains('\0') {
        None
    } else {
        Some(with_scratch_txt(s, f))
    }
}

pub(crate) fn with_plot_str_or_empty<R>(s: &str, f: impl FnOnce(*const c_char) -> R) -> R {
    let s = if s.contains('\0') { "" } else { s };
    with_scratch_txt(s, f)
}

pub(crate) fn with_plot_str_slice<R>(txts: &[&str], f: impl FnOnce(&[*const c_char]) -> R) -> R {
    let cleaned: Vec<&str> = txts
        .iter()
        .map(|&s| if s.contains('\0') { "" } else { s })
        .collect();
    with_scratch_txt_slice(&cleaned, f)
}

pub(crate) fn with_plot_str_slice_with_opt<R>(
    txts: &[&str],
    txt_opt: Option<&str>,
    f: impl FnOnce(&[*const c_char], *const c_char) -> R,
) -> R {
    let cleaned: Vec<&str> = txts
        .iter()
        .map(|&s| if s.contains('\0') { "" } else { s })
        .collect();
    let txt_opt = txt_opt.filter(|s| !s.contains('\0'));
    with_scratch_txt_slice_with_opt(&cleaned, txt_opt, f)
}

pub(crate) fn default_plot_spec() -> sys::ImPlotSpec_c {
    let auto_col = sys::ImVec4_c {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: -1.0,
    };

    sys::ImPlotSpec_c {
        LineColor: auto_col,
        LineColors: std::ptr::null_mut(),
        LineWeight: 1.0,
        FillColor: auto_col,
        FillColors: std::ptr::null_mut(),
        FillAlpha: 1.0,
        Marker: sys::ImPlotMarker_None as _,
        MarkerSize: 4.0,
        MarkerSizes: std::ptr::null_mut(),
        MarkerLineColor: auto_col,
        MarkerLineColors: std::ptr::null_mut(),
        MarkerFillColor: auto_col,
        MarkerFillColors: std::ptr::null_mut(),
        Size: 4.0,
        Offset: 0,
        Stride: crate::IMPLOT_AUTO,
        Flags: sys::ImPlotItemFlags_None as _,
    }
}

pub(crate) fn take_next_plot_spec() -> Option<sys::ImPlotSpec_c> {
    NEXT_PLOT_SPEC.with(|cell| cell.borrow_mut().take())
}

pub(crate) fn set_next_plot_spec(spec: Option<sys::ImPlotSpec_c>) {
    NEXT_PLOT_SPEC.with(|cell| {
        *cell.borrow_mut() = spec;
    })
}

pub(crate) fn plot_spec_from(flags: u32, offset: i32, stride: i32) -> sys::ImPlotSpec_c {
    let mut spec = take_next_plot_spec().unwrap_or_else(default_plot_spec);
    spec.Flags = flags as sys::ImPlotItemFlags;
    spec.Offset = offset;
    spec.Stride = stride;
    spec
}

pub(crate) fn plot_spec_with_style(
    style: PlotItemStyle,
    flags: u32,
    offset: i32,
    stride: i32,
) -> sys::ImPlotSpec_c {
    let mut spec = plot_spec_from(flags, offset, stride);
    style.apply_to_spec(&mut spec);
    spec
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
    Polygon {
        label: &'a str,
        x_data: &'a [f64],
        y_data: &'a [f64],
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

    /// Create a polygon plot.
    pub fn polygon(label: &'a str, x_data: &'a [f64], y_data: &'a [f64]) -> Self {
        Self {
            plot_type: PlotType::Polygon {
                label,
                x_data,
                y_data,
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
            PlotType::Polygon {
                label,
                x_data,
                y_data,
            } => {
                let plot = polygon::PolygonPlot::new(label, x_data, y_data);
                plot.validate()?;
                plot.plot();
            }
        }
        Ok(())
    }
}
