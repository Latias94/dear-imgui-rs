//! Raw backend entry points for downstream integrations.
//!
//! This module intentionally exposes only low-level unsafe declarations. It is
//! a shared bridge for backend crates and third-party ecosystems, not a signal
//! that the core crates own full backend integration or backend source builds.
//!
//! Important ABI note:
//! - these declarations are only the shared low-level surface
//! - downstream crates still own backend compilation and link strategy
//! - official Dear ImGui backend entry points come from C++ backend code, so
//!   many integrations still use a tiny crate-local C shim before calling them
//!   from Rust

#[cfg(all(target_os = "android", feature = "raw-backend-android"))]
#[path = "android.rs"]
pub mod android;

#[cfg(all(target_os = "windows", feature = "raw-backend-dx11"))]
#[path = "dx11.rs"]
pub mod dx11;

#[cfg(feature = "raw-backend-opengl3")]
#[path = "opengl3.rs"]
pub mod opengl3;

#[cfg(all(target_os = "windows", feature = "raw-backend-win32"))]
#[path = "win32.rs"]
pub mod win32;
