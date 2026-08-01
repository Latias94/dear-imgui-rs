//! # Dear ImPlot - Rust Bindings with Dear ImGui Compatibility
//!
//! High-level Rust bindings for ImPlot, the immediate mode plotting library.
//! This crate provides safe, idiomatic Rust bindings designed to work seamlessly
//! with dear-imgui (C++ bindgen) rather than imgui-rs (cimgui).
//!
//! ## Features
//!
//! - Safe, idiomatic Rust API
//! - Full compatibility with dear-imgui
//! - Builder pattern for plots and plot elements
//! - Memory-safe string handling
//! - Support for all major plot types
//!
//! ## Quick Start
//!
//! ```no_run
//! use dear_imgui_rs::*;
//! use dear_implot::*;
//!
//! let mut ctx = Context::create();
//! let mut plot_ctx = PlotContext::create(&ctx);
//!
//! let ui = ctx.frame();
//! let plot_ui = plot_ctx.get_plot_ui(&ui);
//!
//! if let Some(token) = plot_ui.begin_plot("My Plot") {
//!     plot_ui.plot_line("Line", &[1.0, 2.0, 3.0, 4.0], &[1.0, 4.0, 2.0, 3.0]);
//!     token.end();
//! }
//! ```
//!
//! ## Integration with Dear ImGui
//!
//! This crate is designed to work with the `dear-imgui-rs` ecosystem:
//! - Uses the same context management patterns
//! - Compatible with dear-imgui's UI tokens and lifetime management
//! - Shares the same underlying Dear ImGui context

use dear_implot_sys as sys;

// Re-export essential types
pub use dear_imgui_rs::{Context, Ui};
pub use sys::{ImPlotPoint, ImPlotRange, ImPlotRect};
pub use sys::{ImTextureID, ImVec2, ImVec4};

mod advanced;
mod axis_types;
mod colormap;
mod colors;
mod context;
mod flags;
mod histogram_bins;
mod markers;
mod numeric_format;
mod plot_types;
mod style;
mod ui_ext;
mod utils;

// New modular plot types
pub mod plots;

pub use axis_types::{Axis, XAxis, YAxis, YAxisChoice};
pub(crate) use axis_types::{IMPLOT_AUTO, y_axis_choice_option_to_i32};
pub use colormap::Colormap;
pub use colors::PlotColorElement;
pub use context::*;
pub use flags::*;
pub use histogram_bins::{BinMethod, HistogramBins};
pub use markers::Marker;
pub use numeric_format::{AxisFormat, AxisFormatError, FloatFormat, FloatFormatError};
pub use plot_types::{PlotCond, PlotLocation, PlotOrientation};
pub use style::*;
pub use ui_ext::ImPlotExt;
pub use utils::*;

// Re-export new modular plot types for convenience
pub use plots::{
    Plot, PlotData, PlotDataLayout, PlotDataOffset, PlotDataStride, PlotError,
    bar::{BarPlot, PositionalBarPlot},
    error_bars::{AsymmetricErrorBarsPlot, ErrorBarsPlot, SimpleErrorBarsPlot},
    heatmap::{HeatmapPlot, HeatmapPlotF32},
    histogram::{Histogram2DPlot, HistogramPlot},
    line::{LinePlot, SimpleLinePlot},
    pie::{PieChartPlot, PieChartPlotF32},
    polygon::PolygonPlot,
    scatter::{ScatterPlot, SimpleScatterPlot},
    shaded::{ShadedBetweenPlot, ShadedPlot, SimpleShadedPlot},
    stems::{SimpleStemPlot, StemPlot},
};

// Re-export all plot types for convenience
pub use plots::*;

// Re-export advanced features (explicit to avoid AxisFlags name clash)
pub use advanced::{
    LegendFlags, LegendLocation, LegendManager, LegendToken, MultiAxisPlot, MultiAxisToken,
    SubplotFlags, SubplotGrid, SubplotToken, YAxisConfig,
};
