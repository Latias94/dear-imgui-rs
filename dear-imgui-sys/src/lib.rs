//! Raw FFI bindings for Dear ImGui
//!
//! This crate provides low-level, unsafe bindings to the Dear ImGui C++ library.
//! Most users should use the higher-level `dear-imgui` crate instead.
//!
//! # Safety
//!
//! All functions in this crate are unsafe and require careful handling of
//! memory management, lifetimes, and thread safety. The higher-level
//! `dear-imgui` crate provides safe abstractions over these raw bindings.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        unsafe {
            let ctx = ImGui_CreateContext(std::ptr::null_mut());
            assert!(!ctx.is_null());
            ImGui_DestroyContext(ctx);
        }
    }

    #[test]
    fn test_version_info() {
        // Test that we can access Dear ImGui version information
        unsafe {
            let version = ImGui_GetVersion();
            assert!(!version.is_null());

            let version_str = std::ffi::CStr::from_ptr(version);
            let version_string = version_str.to_string_lossy();

            // Should contain version number
            assert!(version_string.contains('.'));
        }
    }
}
