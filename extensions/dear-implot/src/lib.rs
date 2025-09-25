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
//! use dear_imgui::*;
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
//! This crate is designed to work with the `dear-imgui` ecosystem:
//! - Uses the same context management patterns
//! - Compatible with dear-imgui's UI tokens and lifetime management
//! - Shares the same underlying Dear ImGui context

use dear_implot_sys as sys;

// Re-export essential types
pub use dear_imgui::{Context, Ui};
pub use sys::{ImPlotPoint, ImPlotRange, ImPlotRect};
pub use sys::{ImVec2, ImVec4};

mod advanced;
mod context;
mod plot;

mod style;
mod utils;

// New modular plot types
pub mod plots;

pub use context::*;
pub use plot::*;
pub use style::*;
pub use utils::*;

// Re-export new modular plot types for convenience
pub use plots::{
    Plot, PlotData, PlotError,
    bar::{BarPlot, PositionalBarPlot},
    error_bars::{AsymmetricErrorBarsPlot, ErrorBarsPlot, SimpleErrorBarsPlot},
    heatmap::{HeatmapPlot, HeatmapPlotF32},
    histogram::{Histogram2DPlot, HistogramPlot},
    line::{LinePlot, SimpleLinePlot},
    pie::{PieChartPlot, PieChartPlotF32},
    scatter::{ScatterPlot, SimpleScatterPlot},
    shaded::{ShadedBetweenPlot, ShadedPlot, SimpleShadedPlot},
    stems::{SimpleStemPlot, StemPlot},
};

// Constants
const IMPLOT_AUTO: i32 = -1;

/// Choice of Y axis for multi-axis plots
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum YAxisChoice {
    First = 0,
    Second = 1,
    Third = 2,
}

/// Convert an Option<YAxisChoice> into an i32. Picks IMPLOT_AUTO for None.
fn y_axis_choice_option_to_i32(y_axis_choice: Option<YAxisChoice>) -> i32 {
    match y_axis_choice {
        Some(choice) => choice as i32,
        None => IMPLOT_AUTO,
    }
}

/// Ui extension for obtaining a PlotUi from an ImPlot PlotContext
pub trait ImPlotExt {
    fn implot<'ui>(&'ui self, ctx: &'ui PlotContext) -> PlotUi<'ui>;
}

impl ImPlotExt for Ui {
    fn implot<'ui>(&'ui self, ctx: &'ui PlotContext) -> PlotUi<'ui> {
        ctx.get_plot_ui(self)
    }
}

/// Markers for plot points
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Marker {
    None = sys::ImPlotMarker_None,
    Circle = sys::ImPlotMarker_Circle,
    Square = sys::ImPlotMarker_Square,
    Diamond = sys::ImPlotMarker_Diamond,
    Up = sys::ImPlotMarker_Up,
    Down = sys::ImPlotMarker_Down,
    Left = sys::ImPlotMarker_Left,
    Right = sys::ImPlotMarker_Right,
    Cross = sys::ImPlotMarker_Cross,
    Plus = sys::ImPlotMarker_Plus,
    Asterisk = sys::ImPlotMarker_Asterisk,
}

/// Colorable plot elements
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotColorElement {
    Line = 0,
    Fill = 1,
    MarkerOutline = 2,
    MarkerFill = 3,
    ErrorBar = 4,
    FrameBg = 5,
    PlotBg = 6,
    PlotBorder = 7,
    LegendBackground = 8,
    LegendBorder = 9,
    LegendText = 10,
    TitleText = 11,
    InlayText = 12,
    XAxis = 13,
    XAxisGrid = 14,
    YAxis = 15,
    YAxisGrid = 16,
    YAxis2 = 17,
    YAxisGrid2 = 18,
    YAxis3 = 19,
    YAxisGrid3 = 20,
    Selection = 21,
    Crosshairs = 22,
    Query = 23,
}

/// Built-in colormaps
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Colormap {
    Deep = 0,
    Dark = 1,
    Pastel = 2,
    Paired = 3,
    Viridis = 4,
    Plasma = 5,
    Hot = 6,
    Cool = 7,
    Pink = 8,
    Jet = 9,
}

/// Plot location for legends, labels, etc.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotLocation {
    Center = 0,
    North = 1,
    South = 2,
    West = 4,
    East = 8,
    NorthWest = 5,
    NorthEast = 9,
    SouthWest = 6,
    SouthEast = 10,
}

/// Plot orientation
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotOrientation {
    Horizontal = 0,
    Vertical = 1,
}

/// Binning methods for histograms
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BinMethod {
    Sqrt = -1,
    Sturges = -2,
    Rice = -3,
    Scott = -4,
}

// Plot flags for different plot types
bitflags::bitflags! {
    /// Flags for heatmap plots
    pub struct HeatmapFlags: u32 {
        const NONE = 0;
        const COL_MAJOR = 1 << 10;
    }
}

bitflags::bitflags! {
    /// Flags for histogram plots
    pub struct HistogramFlags: u32 {
        const NONE = 0;
        const HORIZONTAL = 1 << 10;
        const CUMULATIVE = 1 << 11;
        const DENSITY = 1 << 12;
        const NO_OUTLIERS = 1 << 13;
        const COL_MAJOR = 1 << 14;
    }
}

bitflags::bitflags! {
    /// Flags for pie chart plots
    pub struct PieChartFlags: u32 {
        const NONE = 0;
        const NORMALIZE = 1 << 10;
        const IGNORE_HIDDEN = 1 << 11;
        const EXPLODING = 1 << 12;
    }
}

bitflags::bitflags! {
    /// Flags for line plots
    pub struct LineFlags: u32 {
        const NONE = 0;
        const SEGMENTS = 1 << 10;
        const LOOP = 1 << 11;
        const SKIP_NAN = 1 << 12;
        const NO_CLIP = 1 << 13;
        const SHADED = 1 << 14;
    }
}

bitflags::bitflags! {
    /// Flags for scatter plots
    pub struct ScatterFlags: u32 {
        const NONE = 0;
        const NO_CLIP = 1 << 10;
    }
}

bitflags::bitflags! {
    /// Flags for bar plots
    pub struct BarsFlags: u32 {
        const NONE = 0;
        const HORIZONTAL = 1 << 10;
    }
}

bitflags::bitflags! {
    /// Flags for shaded plots
    pub struct ShadedFlags: u32 {
        const NONE = 0;
    }
}

bitflags::bitflags! {
    /// Flags for stem plots
    pub struct StemsFlags: u32 {
        const NONE = 0;
        const HORIZONTAL = 1 << 10;
    }
}

bitflags::bitflags! {
    /// Flags for error bar plots
    pub struct ErrorBarsFlags: u32 {
        const NONE = 0;
        const HORIZONTAL = 1 << 10;
    }
}

bitflags::bitflags! {
    /// Flags for stairs plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StairsFlags: u32 {
        const NONE = 0;
        const PRE_STEP = 1 << 10;
        const SHADED = 1 << 11;
    }
}

bitflags::bitflags! {
    /// Flags for bar groups plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BarGroupsFlags: u32 {
        const NONE = 0;
        const HORIZONTAL = 1 << 10;
        const STACKED = 1 << 11;
    }
}

bitflags::bitflags! {
    /// Flags for digital plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DigitalFlags: u32 {
        const NONE = 0;
    }
}

bitflags::bitflags! {
    /// Flags for text plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextFlags: u32 {
        const NONE = 0;
        const VERTICAL = 1 << 10;
    }
}

bitflags::bitflags! {
    /// Flags for dummy plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DummyFlags: u32 {
        const NONE = 0;
    }
}

// Re-export all plot types for convenience
pub use plots::*;

// Re-export advanced features (explicit to avoid AxisFlags name clash)
pub use advanced::{
    LegendFlags, LegendLocation, LegendManager, LegendToken, MultiAxisPlot, MultiAxisToken,
    SubplotFlags, SubplotGrid, SubplotToken, YAxisConfig,
};
