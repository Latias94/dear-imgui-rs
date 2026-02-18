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

// Bindgen output for `dear-implot-sys` can fluctuate between historical
// out-parameter signatures and the newer return-by-value signatures depending
// on which generated `OUT_DIR` file rust-analyzer happens to index.
//
// Keep the wrapper crate stable by calling a local extern declaration for the
// specific APIs we expose.
#[allow(non_snake_case)]
pub(crate) mod compat_ffi {
    use super::sys;
    use std::os::raw::c_char;

    unsafe extern "C" {
        pub fn ImPlot_GetPlotPos() -> sys::ImVec2;
        pub fn ImPlot_GetPlotSize() -> sys::ImVec2;
    }

    // Some targets (notably import-style wasm) cannot call C variadic (`...`) functions.
    // Declare the `*_Str0` convenience wrappers here to keep the safe layer independent
    // of bindgen fluctuations / pregenerated bindings.
    //
    // On wasm32, these must be provided by the `imgui-sys-v0` provider module.
    #[cfg(target_arch = "wasm32")]
    #[link(wasm_import_module = "imgui-sys-v0")]
    unsafe extern "C" {
        pub fn ImPlot_Annotation_Str0(
            x: f64,
            y: f64,
            col: sys::ImVec4_c,
            pix_offset: sys::ImVec2_c,
            clamp: bool,
            fmt: *const c_char,
        );
        pub fn ImPlot_TagX_Str0(x: f64, col: sys::ImVec4_c, fmt: *const c_char);
        pub fn ImPlot_TagY_Str0(y: f64, col: sys::ImVec4_c, fmt: *const c_char);
    }

    #[cfg(not(target_arch = "wasm32"))]
    unsafe extern "C" {
        pub fn ImPlot_Annotation_Str0(
            x: f64,
            y: f64,
            col: sys::ImVec4_c,
            pix_offset: sys::ImVec2_c,
            clamp: bool,
            fmt: *const c_char,
        );
        pub fn ImPlot_TagX_Str0(x: f64, col: sys::ImVec4_c, fmt: *const c_char);
        pub fn ImPlot_TagY_Str0(y: f64, col: sys::ImVec4_c, fmt: *const c_char);
    }
}

// Re-export essential types
pub use dear_imgui_rs::{Context, Ui};
pub use sys::{ImPlotPoint, ImPlotRange, ImPlotRect};
pub use sys::{ImTextureID, ImVec2, ImVec4};

mod advanced;
mod context;
mod style;
mod utils;

// New modular plot types
pub mod plots;

pub use context::*;
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

/// X axis selector matching ImPlot's ImAxis values
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum XAxis {
    X1 = 0,
    X2 = 1,
    X3 = 2,
}

/// Y axis selector matching ImPlot's ImAxis values
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum YAxis {
    Y1 = 3,
    Y2 = 4,
    Y3 = 5,
}

impl YAxis {
    /// Convert a Y axis (Y1..Y3) to the 0-based index used by ImPlotPlot_YAxis_Nil
    pub(crate) fn to_index(self) -> i32 {
        (self as i32) - 3
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
    FrameBg = sys::ImPlotCol_FrameBg as u32,
    PlotBg = sys::ImPlotCol_PlotBg as u32,
    PlotBorder = sys::ImPlotCol_PlotBorder as u32,
    LegendBg = sys::ImPlotCol_LegendBg as u32,
    LegendBorder = sys::ImPlotCol_LegendBorder as u32,
    LegendText = sys::ImPlotCol_LegendText as u32,
    TitleText = sys::ImPlotCol_TitleText as u32,
    InlayText = sys::ImPlotCol_InlayText as u32,
    AxisText = sys::ImPlotCol_AxisText as u32,
    AxisGrid = sys::ImPlotCol_AxisGrid as u32,
    AxisTick = sys::ImPlotCol_AxisTick as u32,
    AxisBg = sys::ImPlotCol_AxisBg as u32,
    AxisBgHovered = sys::ImPlotCol_AxisBgHovered as u32,
    AxisBgActive = sys::ImPlotCol_AxisBgActive as u32,
    Selection = sys::ImPlotCol_Selection as u32,
    Crosshairs = sys::ImPlotCol_Crosshairs as u32,
}

/// Built-in colormaps
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Colormap {
    Deep = sys::ImPlotColormap_Deep as u32,
    Dark = sys::ImPlotColormap_Dark as u32,
    Pastel = sys::ImPlotColormap_Pastel as u32,
    Paired = sys::ImPlotColormap_Paired as u32,
    Viridis = sys::ImPlotColormap_Viridis as u32,
    Plasma = sys::ImPlotColormap_Plasma as u32,
    Hot = sys::ImPlotColormap_Hot as u32,
    Cool = sys::ImPlotColormap_Cool as u32,
    Pink = sys::ImPlotColormap_Pink as u32,
    Jet = sys::ImPlotColormap_Jet as u32,
}

/// Plot location for legends, labels, etc.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlotLocation {
    Center = sys::ImPlotLocation_Center as u32,
    North = sys::ImPlotLocation_North as u32,
    South = sys::ImPlotLocation_South as u32,
    West = sys::ImPlotLocation_West as u32,
    East = sys::ImPlotLocation_East as u32,
    NorthWest = sys::ImPlotLocation_NorthWest as u32,
    NorthEast = sys::ImPlotLocation_NorthEast as u32,
    SouthWest = sys::ImPlotLocation_SouthWest as u32,
    SouthEast = sys::ImPlotLocation_SouthEast as u32,
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
        const NONE = sys::ImPlotHeatmapFlags_None as u32;
        const COL_MAJOR = sys::ImPlotHeatmapFlags_ColMajor as u32;
    }
}

bitflags::bitflags! {
    /// Flags for histogram plots
    pub struct HistogramFlags: u32 {
        const NONE = sys::ImPlotHistogramFlags_None as u32;
        const HORIZONTAL = sys::ImPlotHistogramFlags_Horizontal as u32;
        const CUMULATIVE = sys::ImPlotHistogramFlags_Cumulative as u32;
        const DENSITY = sys::ImPlotHistogramFlags_Density as u32;
        const NO_OUTLIERS = sys::ImPlotHistogramFlags_NoOutliers as u32;
        const COL_MAJOR = sys::ImPlotHistogramFlags_ColMajor as u32;
    }
}

bitflags::bitflags! {
    /// Flags for pie chart plots
    pub struct PieChartFlags: u32 {
        const NONE = sys::ImPlotPieChartFlags_None as u32;
        const NORMALIZE = sys::ImPlotPieChartFlags_Normalize as u32;
        const IGNORE_HIDDEN = sys::ImPlotPieChartFlags_IgnoreHidden as u32;
        const EXPLODING = sys::ImPlotPieChartFlags_Exploding as u32;
    }
}

bitflags::bitflags! {
    /// Flags for line plots
    pub struct LineFlags: u32 {
        const NONE = sys::ImPlotLineFlags_None as u32;
        const SEGMENTS = sys::ImPlotLineFlags_Segments as u32;
        const LOOP = sys::ImPlotLineFlags_Loop as u32;
        const SKIP_NAN = sys::ImPlotLineFlags_SkipNaN as u32;
        const NO_CLIP = sys::ImPlotLineFlags_NoClip as u32;
        const SHADED = sys::ImPlotLineFlags_Shaded as u32;
    }
}

bitflags::bitflags! {
    /// Flags for scatter plots
    pub struct ScatterFlags: u32 {
        const NONE = sys::ImPlotScatterFlags_None as u32;
        const NO_CLIP = sys::ImPlotScatterFlags_NoClip as u32;
    }
}

bitflags::bitflags! {
    /// Flags for bar plots
    pub struct BarsFlags: u32 {
        const NONE = sys::ImPlotBarsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotBarsFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for shaded plots
    pub struct ShadedFlags: u32 {
        const NONE = sys::ImPlotShadedFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Flags for stem plots
    pub struct StemsFlags: u32 {
        const NONE = sys::ImPlotStemsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotStemsFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for error bar plots
    pub struct ErrorBarsFlags: u32 {
        const NONE = sys::ImPlotErrorBarsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotErrorBarsFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for stairs plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StairsFlags: u32 {
        const NONE = sys::ImPlotStairsFlags_None as u32;
        const PRE_STEP = sys::ImPlotStairsFlags_PreStep as u32;
        const SHADED = sys::ImPlotStairsFlags_Shaded as u32;
    }
}

bitflags::bitflags! {
    /// Flags for bar groups plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BarGroupsFlags: u32 {
        const NONE = sys::ImPlotBarGroupsFlags_None as u32;
        const HORIZONTAL = sys::ImPlotBarGroupsFlags_Horizontal as u32;
        const STACKED = sys::ImPlotBarGroupsFlags_Stacked as u32;
    }
}

bitflags::bitflags! {
    /// Flags for digital plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DigitalFlags: u32 {
        const NONE = sys::ImPlotDigitalFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Flags for text plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextFlags: u32 {
        const NONE = sys::ImPlotTextFlags_None as u32;
        const VERTICAL = sys::ImPlotTextFlags_Vertical as u32;
    }
}

bitflags::bitflags! {
    /// Flags for dummy plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DummyFlags: u32 {
        const NONE = sys::ImPlotDummyFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Flags for drag tools (points/lines)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DragToolFlags: u32 {
        const NONE = sys::ImPlotDragToolFlags_None as u32;
        const NO_CURSORS = sys::ImPlotDragToolFlags_NoCursors as u32;
        const NO_FIT = sys::ImPlotDragToolFlags_NoFit as u32;
        const NO_INPUTS = sys::ImPlotDragToolFlags_NoInputs as u32;
        const DELAYED = sys::ImPlotDragToolFlags_Delayed as u32;
    }
}

bitflags::bitflags! {
    /// Flags for infinite lines plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InfLinesFlags: u32 {
        const NONE = sys::ImPlotInfLinesFlags_None as u32;
        const HORIZONTAL = sys::ImPlotInfLinesFlags_Horizontal as u32;
    }
}

bitflags::bitflags! {
    /// Flags for image plots
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ImageFlags: u32 {
        const NONE = sys::ImPlotImageFlags_None as u32;
    }
}

bitflags::bitflags! {
    /// Axis flags matching ImPlotAxisFlags_ (see cimplot.h)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AxisFlags: u32 {
        const NONE           = sys::ImPlotAxisFlags_None as u32;
        const NO_LABEL       = sys::ImPlotAxisFlags_NoLabel as u32;
        const NO_GRID_LINES  = sys::ImPlotAxisFlags_NoGridLines as u32;
        const NO_TICK_MARKS  = sys::ImPlotAxisFlags_NoTickMarks as u32;
        const NO_TICK_LABELS = sys::ImPlotAxisFlags_NoTickLabels as u32;
        const NO_INITIAL_FIT = sys::ImPlotAxisFlags_NoInitialFit as u32;
        const NO_MENUS       = sys::ImPlotAxisFlags_NoMenus as u32;
        const NO_SIDE_SWITCH = sys::ImPlotAxisFlags_NoSideSwitch as u32;
        const NO_HIGHLIGHT   = sys::ImPlotAxisFlags_NoHighlight as u32;
        const OPPOSITE       = sys::ImPlotAxisFlags_Opposite as u32;
        const FOREGROUND     = sys::ImPlotAxisFlags_Foreground as u32;
        const INVERT         = sys::ImPlotAxisFlags_Invert as u32;
        const AUTO_FIT       = sys::ImPlotAxisFlags_AutoFit as u32;
        const RANGE_FIT      = sys::ImPlotAxisFlags_RangeFit as u32;
        const PAN_STRETCH    = sys::ImPlotAxisFlags_PanStretch as u32;
        const LOCK_MIN       = sys::ImPlotAxisFlags_LockMin as u32;
        const LOCK_MAX       = sys::ImPlotAxisFlags_LockMax as u32;
    }
}

/// Plot condition (setup/next) matching ImPlotCond (ImGuiCond)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PlotCond {
    None = 0,
    Always = 1,
    Once = 2,
}

// Re-export all plot types for convenience
pub use plots::*;

// Re-export advanced features (explicit to avoid AxisFlags name clash)
pub use advanced::{
    LegendFlags, LegendLocation, LegendManager, LegendToken, MultiAxisPlot, MultiAxisToken,
    SubplotFlags, SubplotGrid, SubplotToken, YAxisConfig,
};
