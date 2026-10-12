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
//! # Shared safety contract
//!
//! This build's current Dear ImGui context is process-global. Serialize all ImGui/backend calls
//! across contexts and threads; independent per-context locks are insufficient. Keep the intended
//! context current, pair every successful initialization with shutdown, and shut down the renderer
//! before the platform backend, context, and application-owned native resources. Android additionally
//! permits only one active backend instance in the process. Individual modules document their
//! native-handle and GPU requirements. Raw renderer calls do not reconcile safe managed frames.
//!
//! Platform / feature gating:
//!
//! - `android` requires `target_os = "android"` and feature
//!   `backend-shim-android`
//! - `opengl3` requires feature `backend-shim-opengl3` and is currently
//!   available on non-wasm targets
//! - `win32` requires `target_os = "windows"` and feature
//!   `backend-shim-win32`
//! - `dx11` requires `target_os = "windows"` and feature
//!   `backend-shim-dx11`
//! - `dx12` requires `target_os = "windows"` and feature `backend-shim-dx12`
//! - `metal` requires macOS or iOS and feature `backend-shim-metal`
//! - `osx` requires macOS and feature `backend-shim-osx`
//!
//! Typical usage patterns:
//!
//! - low-level Android apps can combine `backend_shim::android` and
//!   `backend_shim::opengl3` while keeping EGL / GLES setup and APK packaging
//!   in the application
//! - backend crates such as SDL3 wrappers may still own framework-specific
//!   build logic while reusing shared shim ABI where appropriate
//! - SDL3 renderer shims are owned by `dear-imgui-sdl3`; the sys-level
//!   renderer shim features are not exposed by `dear-imgui-sys`
//!
//! The repository's concrete Android proof-of-shape lives in
//! `examples-android/dear-imgui-android-smoke/`.

#[cfg(all(target_os = "android", feature = "backend-shim-android"))]
pub mod android;

#[cfg(all(target_os = "windows", feature = "backend-shim-dx11"))]
pub mod dx11;

#[cfg(all(target_os = "windows", feature = "backend-shim-dx12"))]
pub mod dx12;

#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    feature = "backend-shim-metal"
))]
pub mod metal;

#[cfg(all(not(target_arch = "wasm32"), feature = "backend-shim-opengl3"))]
pub mod opengl3;

#[cfg(all(target_os = "macos", feature = "backend-shim-osx"))]
pub mod osx;

#[cfg(all(target_os = "windows", feature = "backend-shim-win32"))]
pub mod win32;
