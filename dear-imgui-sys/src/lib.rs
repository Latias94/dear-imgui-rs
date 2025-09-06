//! Low-level FFI bindings for Dear ImGui with docking support
//!
//! This crate provides raw, unsafe bindings to the Dear ImGui C++ library,
//! specifically the docking branch which includes docking and multi-viewport features.
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
//! prefer the high-level `dear-imgui` crate for safe Rust bindings.
//!
//! ## Usage
//!
//! This crate is typically not used directly. Instead, use the `dear-imgui` crate
//! which provides safe, idiomatic Rust bindings built on top of these FFI bindings.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(clippy::all)]

use std::ops::{Deref, DerefMut, Index, IndexMut};

// Include the generated bindings from bindgen
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Platform-specific function wrappers
pub mod wrapper_functions;

/// Implement indexing for ImVector to provide array-like access
impl<T> Index<usize> for ImVector<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.Size as usize {
            panic!(
                "ImVector index {} out of bounds (size: {})",
                index, self.Size
            );
        }
        unsafe { &*self.Data.add(index) }
    }
}

/// Implement mutable indexing for ImVector
impl<T> IndexMut<usize> for ImVector<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.Size as usize {
            panic!(
                "ImVector index {} out of bounds (size: {})",
                index, self.Size
            );
        }
        unsafe { &mut *self.Data.add(index) }
    }
}

/// Implement Deref to allow ImVector to be used as a slice
impl<T> Deref for ImVector<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe {
            if self.Size == 0 || self.Data.is_null() {
                // Handle empty vector or null data pointer
                &[]
            } else {
                std::slice::from_raw_parts(self.Data, self.Size as usize)
            }
        }
    }
}

/// Implement DerefMut to allow mutable slice access to ImVector
impl<T> DerefMut for ImVector<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            if self.Size == 0 || self.Data.is_null() {
                // Handle empty vector or null data pointer
                &mut []
            } else {
                std::slice::from_raw_parts_mut(self.Data, self.Size as usize)
            }
        }
    }
}

/// Implement iterator support for ImVector references
impl<'a, T> IntoIterator for &'a ImVector<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.deref().iter()
    }
}

/// Implement mutable iterator support for ImVector references
impl<'a, T> IntoIterator for &'a mut ImVector<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.deref_mut().iter_mut()
    }
}

// MSVC ABI compatibility for ImVec2-returning functions
#[cfg(target_env = "msvc")]
impl From<ImVec2_rr> for ImVec2 {
    #[inline]
    fn from(rr: ImVec2_rr) -> ImVec2 {
        ImVec2 { x: rr.x, y: rr.y }
    }
}

#[cfg(target_env = "msvc")]
impl From<ImVec2> for ImVec2_rr {
    #[inline]
    fn from(v: ImVec2) -> ImVec2_rr {
        ImVec2_rr { x: v.x, y: v.y }
    }
}

// Re-export commonly used types for convenience
pub use ImColor as Color;
pub use ImVec2 as Vector2;
pub use ImVec4 as Vector4;

/// Version information for the Dear ImGui library
pub const IMGUI_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if docking features are available
#[cfg(feature = "docking")]
pub const HAS_DOCKING: bool = true;

#[cfg(not(feature = "docking"))]
pub const HAS_DOCKING: bool = false;

/// Check if FreeType support is available
#[cfg(feature = "freetype")]
pub const HAS_FREETYPE: bool = true;

#[cfg(not(feature = "freetype"))]
pub const HAS_FREETYPE: bool = false;

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
