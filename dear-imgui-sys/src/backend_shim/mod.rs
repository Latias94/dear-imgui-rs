//! Optional backend shim entry points for downstream integrations.
//!
//! These modules expose the repository-owned C shim ABI for selected official
//! Dear ImGui backends. They are intentionally low-level and unsafe.
//!
//! Important boundary:
//!
//! - this is not the upstream `imgui_impl_*` ABI
//! - the Rust-facing symbol names belong to this repository
//! - enabling a backend shim feature does not imply a safe wrapper exists in
//!   `dear-imgui-rs`
//!
//! Platform / feature gating:
//!
//! - `android` requires `target_os = "android"` and feature
//!   `backend-shim-android`
//! - `opengl3` requires feature `backend-shim-opengl3` and is currently
//!   available on non-wasm targets
//! - `sdlrenderer3` requires feature `backend-shim-sdlrenderer3` and is
//!   currently available on non-wasm targets
//! - `win32` requires `target_os = "windows"` and feature
//!   `backend-shim-win32`
//! - `dx11` requires `target_os = "windows"` and feature
//!   `backend-shim-dx11`
//!
//! Typical usage patterns:
//!
//! - low-level Android apps can combine `backend_shim::android` and
//!   `backend_shim::opengl3` while keeping EGL / GLES setup and APK packaging
//!   in the application
//! - backend crates such as SDL3 wrappers may still own framework-specific
//!   build logic while reusing shared shim ABI where appropriate
//!
//! The repository's concrete Android proof-of-shape lives in
//! `examples-android/dear-imgui-android-smoke/`.

#[cfg(all(target_os = "android", feature = "backend-shim-android"))]
pub mod android;

#[cfg(all(target_os = "windows", feature = "backend-shim-dx11"))]
pub mod dx11;

#[cfg(all(not(target_arch = "wasm32"), feature = "backend-shim-opengl3"))]
pub mod opengl3;

#[cfg(all(not(target_arch = "wasm32"), feature = "backend-shim-sdlrenderer3"))]
pub mod sdlrenderer3;

#[cfg(all(target_os = "windows", feature = "backend-shim-win32"))]
pub mod win32;
