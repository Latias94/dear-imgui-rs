//! Low-level FFI bindings for Dear ImGui (via cimgui C API) with docking support
//!
//! This crate provides raw, unsafe bindings to Dear ImGui using the cimgui C API,
//! specifically targeting the docking branch (multi-viewport capable).
//!
//! ## Features
//!
//! - **docking**: Enable docking and multi-viewport features (default)
//! - **freetype**: Enable FreeType font rasterizer support
//! - **wasm**: Enable WebAssembly compatibility
//!
//! ## WebAssembly Support
//!
//! When the `wasm` feature is enabled, this crate provides full WASM compatibility:
//! - Disables platform-specific functions (file I/O, shell functions, etc.)
//! - Configures Dear ImGui for WASM environment
//! - Compatible with wasm-bindgen and web targets
//!
//! ## Safety
//!
//! This crate provides raw FFI bindings and is inherently unsafe. Users should
//! prefer the high-level `dear-imgui-rs` crate for safe Rust bindings.
//!
//! ## Usage
//!
//! This crate is typically not used directly. Instead, use the `dear-imgui-rs` crate
//! which provides safe, idiomatic Rust bindings built on top of these FFI bindings.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(clippy::all)]
// Bindgen may derive Eq/Hash for structs containing function pointers.
// New Clippy lint warns these comparisons are unpredictable; suppress for raw FFI types.
#![allow(unpredictable_function_pointer_comparisons)]

// Bindings are generated into OUT_DIR and included via a submodule so that
// possible inner attributes in the generated file are accepted at module root.
mod ffi;
pub use ffi::*;

// Ensure common ImGui typedefs are available even if bindgen doesn't emit them explicitly

// cimgui exposes typed vectors (e.g., ImVector_ImVec2) instead of a generic ImVector<T>.
// The sys crate intentionally avoids adding higher-level helpers here.

// cimgui C API avoids C++ ABI pitfalls; no MSVC-specific conversions are required.

// Re-export commonly used types for convenience
pub use ImColor as Color;
pub use ImVec2 as Vector2;
pub use ImVec4 as Vector4;

/// Version information for the Dear ImGui library
pub const IMGUI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Docking features are always available in this crate
pub const HAS_DOCKING: bool = true;

/// Check if FreeType support is available
#[cfg(feature = "freetype")]
pub const HAS_FREETYPE: bool = true;

#[cfg(not(feature = "freetype"))]
pub const HAS_FREETYPE: bool = false;

/// Check if WASM support is available
#[cfg(feature = "wasm")]
pub const HAS_WASM: bool = true;

#[cfg(not(feature = "wasm"))]
pub const HAS_WASM: bool = false;

// (No wasm-specific shims are required when using shared memory import style.)

impl ImVec2 {
    #[inline]
    pub const fn new(x: f32, y: f32) -> ImVec2 {
        ImVec2 { x, y }
    }

    #[inline]
    pub const fn zero() -> ImVec2 {
        ImVec2 { x: 0.0, y: 0.0 }
    }
}

impl From<[f32; 2]> for ImVec2 {
    #[inline]
    fn from(array: [f32; 2]) -> ImVec2 {
        ImVec2::new(array[0], array[1])
    }
}

impl From<(f32, f32)> for ImVec2 {
    #[inline]
    fn from((x, y): (f32, f32)) -> ImVec2 {
        ImVec2::new(x, y)
    }
}

impl From<ImVec2> for [f32; 2] {
    #[inline]
    fn from(v: ImVec2) -> [f32; 2] {
        [v.x, v.y]
    }
}

impl From<ImVec2> for (f32, f32) {
    #[inline]
    fn from(v: ImVec2) -> (f32, f32) {
        (v.x, v.y)
    }
}

impl From<mint::Vector2<f32>> for ImVec2 {
    #[inline]
    fn from(v: mint::Vector2<f32>) -> ImVec2 {
        ImVec2::new(v.x, v.y)
    }
}

impl ImVec4 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> ImVec4 {
        ImVec4 { x, y, z, w }
    }

    #[inline]
    pub const fn zero() -> ImVec4 {
        ImVec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }
}

impl From<[f32; 4]> for ImVec4 {
    #[inline]
    fn from(array: [f32; 4]) -> ImVec4 {
        ImVec4::new(array[0], array[1], array[2], array[3])
    }
}

impl From<(f32, f32, f32, f32)> for ImVec4 {
    #[inline]
    fn from((x, y, z, w): (f32, f32, f32, f32)) -> ImVec4 {
        ImVec4::new(x, y, z, w)
    }
}

impl From<ImVec4> for [f32; 4] {
    #[inline]
    fn from(v: ImVec4) -> [f32; 4] {
        [v.x, v.y, v.z, v.w]
    }
}

impl From<ImVec4> for (f32, f32, f32, f32) {
    #[inline]
    fn from(v: ImVec4) -> (f32, f32, f32, f32) {
        (v.x, v.y, v.z, v.w)
    }
}

impl From<mint::Vector4<f32>> for ImVec4 {
    #[inline]
    fn from(v: mint::Vector4<f32>) -> ImVec4 {
        ImVec4::new(v.x, v.y, v.z, v.w)
    }
}
