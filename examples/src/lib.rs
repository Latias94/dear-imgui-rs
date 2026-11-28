//! Shared application-level helpers for repository examples.

#[cfg(all(feature = "ash-winit-multi-viewport", not(target_arch = "wasm32")))]
#[path = "../support/ash_multi_viewport.rs"]
pub mod ash_multi_viewport;

#[cfg(feature = "sdl3-platform")]
pub mod sdl3_callbacks;

#[cfg(feature = "sdl3-platform")]
pub mod sdl3_dpi;

#[cfg(feature = "sdl3-platform")]
pub mod sdl3_gl;

#[cfg(feature = "sdl3-gpu-multi-viewport")]
pub mod sdl3_gpu_present;

/// Install the tracing subscriber used by native examples.
pub fn init_tracing_with_filter(filter: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_target(true)
        .init();
}
