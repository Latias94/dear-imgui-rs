//! Low-level FFI bindings for ImPlot via the cimplot C API
//!
//! This crate provides raw, unsafe bindings to the ImPlot library using the
//! cimplot C API, designed to work together with `dear-imgui-sys` (which uses
//! cimgui for Dear ImGui). This avoids C++ ABI issues and keeps builds
//! consistent across platforms and toolchains.
//!
//! ## Features
//!
//! - **docking**: Enable docking and multi-viewport features (default)
//! - **freetype**: Enable FreeType font rasterizer support
//! - **wasm**: Enable WebAssembly compatibility
//!
//! ## Safety
//!
//! This crate provides raw FFI bindings and is inherently unsafe. Users should
//! prefer the high-level `dear-implot` crate for safe Rust bindings.
//!
//! ## Usage
//!
//! This crate is typically not used directly. Instead, use the `dear-implot` crate
//! which provides safe, idiomatic Rust bindings built on top of these FFI bindings.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(clippy::all)]
#![allow(unpredictable_function_pointer_comparisons)]

// Re-export ImGui types from dear-imgui-sys to ensure compatibility
pub use dear_imgui_sys::{
    ImDrawData, ImDrawList, ImFontAtlas, ImGuiCond, ImGuiContext, ImGuiDragDropFlags, ImGuiIO,
    ImGuiMouseButton, ImGuiStyle, ImTextureID, ImVec2, ImVec4,
};

// Include the generated bindings from bindgen
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// TODO: Platform-specific function wrappers for MSVC ABI compatibility
// #[cfg(target_os = "windows")]
// pub mod wrapper_functions;

// Convenience type aliases and implementations
use std::ops::Range;

impl From<Range<f64>> for ImPlotRange {
    fn from(from: Range<f64>) -> Self {
        ImPlotRange {
            Min: from.start,
            Max: from.end,
        }
    }
}

impl From<[f64; 2]> for ImPlotRange {
    fn from(from: [f64; 2]) -> Self {
        ImPlotRange {
            Min: from[0],
            Max: from[1],
        }
    }
}

impl From<(f64, f64)> for ImPlotRange {
    fn from(from: (f64, f64)) -> Self {
        ImPlotRange {
            Min: from.0,
            Max: from.1,
        }
    }
}

impl From<ImVec2> for ImPlotRange {
    fn from(from: ImVec2) -> Self {
        ImPlotRange {
            Min: from.x as f64,
            Max: from.y as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implot_range_conversions() {
        let range1: ImPlotRange = (0.0..10.0).into();
        assert_eq!(range1.Min, 0.0);
        assert_eq!(range1.Max, 10.0);

        let range2: ImPlotRange = [1.0, 5.0].into();
        assert_eq!(range2.Min, 1.0);
        assert_eq!(range2.Max, 5.0);

        let range3: ImPlotRange = (2.0, 8.0).into();
        assert_eq!(range3.Min, 2.0);
        assert_eq!(range3.Max, 8.0);
    }
}
