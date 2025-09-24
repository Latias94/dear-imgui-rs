//! Logging utilities for Dear ImGui
//!
//! This module provides convenient logging setup and utilities for Dear ImGui applications.
//! It supports both `tracing` (recommended) and `log` crate backends.

#[cfg(feature = "tracing")]
use tracing::{debug, info};

/// Initialize tracing subscriber with sensible defaults for Dear ImGui applications
#[cfg(feature = "tracing")]
pub fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Default filter: show info+ for dear_imgui, warn+ for everything else
            "dear_imgui=info,dear_imgui_wgpu=info,dear_imgui_winit=info,dear_imgui_glow=info,dear_imgui_bevy=info,warn".into()
        });

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

/// Initialize tracing subscriber with custom filter
#[cfg(feature = "tracing")]
pub fn init_tracing_with_filter(filter: &str) {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::new(filter);

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

/// Initialize tracing subscriber for development with more verbose output
#[cfg(feature = "tracing")]
pub fn init_tracing_dev() {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Development filter: show debug+ for dear_imgui, info+ for everything else
            "dear_imgui=debug,dear_imgui_wgpu=debug,dear_imgui_winit=debug,dear_imgui_glow=debug,dear_imgui_bevy=debug,info".into()
        });

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

/// Log context creation
#[cfg(feature = "tracing")]
pub fn log_context_created() {
    info!("Dear ImGui context created successfully");
}

/// Log context destruction
#[cfg(feature = "tracing")]
pub fn log_context_destroyed() {
    info!("Dear ImGui context destroyed");
}

/// Log renderer initialization
#[cfg(feature = "tracing")]
pub fn log_renderer_init(backend: &str) {
    info!("Dear ImGui {} renderer initialized", backend);
}

/// Log platform initialization
#[cfg(feature = "tracing")]
pub fn log_platform_init(platform: &str) {
    info!("Dear ImGui {} platform initialized", platform);
}

/// Log frame statistics
#[cfg(feature = "tracing")]
pub fn log_frame_stats(frame_time: f32, fps: f32) {
    debug!("Frame time: {:.2}ms, FPS: {:.1}", frame_time * 1000.0, fps);
}

/// Log memory usage
#[cfg(feature = "tracing")]
pub fn log_memory_usage(vertices: usize, indices: usize, draw_calls: usize) {
    debug!(
        "Render stats - Vertices: {}, Indices: {}, Draw calls: {}",
        vertices, indices, draw_calls
    );
}

// Fallback implementations when tracing is not available
#[cfg(not(feature = "tracing"))]
pub fn init_tracing() {
    eprintln!("Warning: tracing feature not enabled, logging disabled");
}

#[cfg(not(feature = "tracing"))]
pub fn init_tracing_with_filter(_filter: &str) {
    eprintln!("Warning: tracing feature not enabled, logging disabled");
}

#[cfg(not(feature = "tracing"))]
pub fn init_tracing_dev() {
    eprintln!("Warning: tracing feature not enabled, logging disabled");
}

#[cfg(not(feature = "tracing"))]
pub fn log_context_created() {}

#[cfg(not(feature = "tracing"))]
pub fn log_context_destroyed() {}

#[cfg(not(feature = "tracing"))]
pub fn log_renderer_init(_backend: &str) {}

#[cfg(not(feature = "tracing"))]
pub fn log_platform_init(_platform: &str) {}

#[cfg(not(feature = "tracing"))]
pub fn log_frame_stats(_frame_time: f32, _fps: f32) {}

#[cfg(not(feature = "tracing"))]
pub fn log_memory_usage(_vertices: usize, _indices: usize, _draw_calls: usize) {}

/// Macro for conditional tracing
#[macro_export]
macro_rules! imgui_trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::trace!($($arg)*);
    };
}

/// Macro for conditional debug logging
#[macro_export]
macro_rules! imgui_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!($($arg)*);
    };
}

/// Macro for conditional info logging
#[macro_export]
macro_rules! imgui_info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::info!($($arg)*);
    };
}

/// Macro for conditional warning logging
#[macro_export]
macro_rules! imgui_warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::warn!($($arg)*);
    };
}

/// Macro for conditional error logging
#[macro_export]
macro_rules! imgui_error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::error!($($arg)*);
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_logging_macros() {
        // Test that macros compile without tracing feature
        imgui_trace!("test trace");
        imgui_debug!("test debug");
        imgui_info!("test info");
        imgui_warn!("test warn");
        imgui_error!("test error");
    }
}
