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
mod axis_types;
mod colormap;
mod colors;
mod context;
mod flags;
mod histogram_bins;
mod markers;
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
