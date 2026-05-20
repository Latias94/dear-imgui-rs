//! SDL3 clipboard seam.
//!
//! The upstream `imgui_impl_sdl3.cpp` backend installs clipboard callbacks internally.
//! This module keeps that responsibility named without exposing duplicate Rust-side state.
