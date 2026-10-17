//! Low-level FFI bindings for ImPlot3D via the cimplot3d C API
//!
//! This crate pairs with `dear-imgui-sys` and exposes raw bindings to the
//! ImPlot3D library using the cimplot3d C API. Prefer using the higher-level
//! `dear-implot3d` crate for safe, idiomatic Rust wrappers.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::all)]
#![allow(unpredictable_function_pointer_comparisons)]

// Re-export Dear ImGui types for compatibility
pub use dear_imgui_sys::{ImDrawList, ImGuiContext, ImGuiID, ImTextureID, ImVec2, ImVec4};

// Include generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(all(feature = "surface-test-probe", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub mod surface_test_probe {
    unsafe extern "C" {
        pub fn dear_implot3d_legend_probe_frame(
            plot_context: *mut super::ImPlot3DContext,
            covered: bool,
            state: *mut f32,
        ) -> std::ffi::c_int;
        pub fn dear_implot3d_line_probe_plot(
            plot_context: *mut super::ImPlot3DContext,
            weight: f32,
            pixel_density: f32,
            anti_aliased: bool,
            positions: *mut f32,
            uvs: *mut f32,
            expected_uvs: *mut f32,
            expected_width: *mut f32,
        ) -> std::ffi::c_int;
        pub fn dear_implot3d_surface_probe_reset();
        pub fn dear_implot3d_surface_probe_plot(
            label_id: *const std::ffi::c_char,
            xs: *const f32,
            ys: *const f32,
            zs: *const f32,
            x_count: std::ffi::c_int,
            y_count: std::ffi::c_int,
            scale_min: f64,
            scale_max: f64,
            spec: super::ImPlot3DSpec_c,
        );
        pub fn dear_implot3d_surface_probe_read(
            xs: *mut f32,
            ys: *mut f32,
            zs: *mut f32,
            capacity: std::ffi::c_int,
            x_count: *mut std::ffi::c_int,
            y_count: *mut std::ffi::c_int,
        ) -> std::ffi::c_int;
    }
}
