//! Viewport backend traits for multi-viewport support
//!
//! This module provides traits that backends must implement to support
//! Dear ImGui's multi-viewport functionality.

use crate::{platform_io::Viewport, sys};
use std::cell::RefCell;
use std::ffi::{c_char, c_void};

// Thread-local storage for viewport contexts
thread_local! {
    static PLATFORM_VIEWPORT_CONTEXT: RefCell<Option<PlatformViewportContext>> = RefCell::new(None);
    static RENDERER_VIEWPORT_CONTEXT: RefCell<Option<RendererViewportContext>> = RefCell::new(None);
}

/// Trait that holds functions needed when the platform integration supports viewports.
///
/// Register it via [`Context::set_platform_backend()`](crate::Context::set_platform_backend())
#[cfg(feature = "multi-viewport")]
pub trait PlatformViewportBackend: 'static {
    /// Called by imgui when a new [`Viewport`] is created.
    ///
    /// # Notes
    /// This function should initiate the creation of a platform window.
    /// The window should be invisible.
    fn create_window(&mut self, viewport: &mut Viewport);

    /// Called by imgui when a [`Viewport`] is about to be destroyed.
    ///
    /// # Notes
    /// This function should initiate the destruction of the platform window.
    fn destroy_window(&mut self, viewport: &mut Viewport);

    /// Called by imgui to make a [`Viewport`] visible.
    fn show_window(&mut self, viewport: &mut Viewport);

    /// Called by imgui to reposition a [`Viewport`].
    ///
    /// # Notes
    /// `pos` specifies the position of the windows content area (excluding title bar etc.)
    fn set_window_pos(&mut self, viewport: &mut Viewport, pos: [f32; 2]);

    /// Called by imgui to get the position of a [`Viewport`].
    ///
    /// # Notes
    /// You should return the position of the window's content area (excluding title bar etc.)
    fn get_window_pos(&mut self, viewport: &mut Viewport) -> [f32; 2];

    /// Called by imgui to set the size of a [`Viewport`].
    ///
    /// # Notes
    /// `size` specifies the size of the window's content area (excluding title bar etc.)
    fn set_window_size(&mut self, viewport: &mut Viewport, size: [f32; 2]);

    /// Called by imgui to get the size of a [`Viewport`].
    ///
    /// # Notes
    /// you should return the size of the window's content area (excluding title bar etc.)
    fn get_window_size(&mut self, viewport: &mut Viewport) -> [f32; 2];

    /// Called by imgui to make a [`Viewport`] steal the focus.
    fn set_window_focus(&mut self, viewport: &mut Viewport);

    /// Called by imgui to query whether a [`Viewport`] is in focus.
    fn get_window_focus(&mut self, viewport: &mut Viewport) -> bool;

    /// Called by imgui to query whether a [`Viewport`] is minimized.
    fn get_window_minimized(&mut self, viewport: &mut Viewport) -> bool;

    /// Called by imgui to set a [`Viewport`] title.
    fn set_window_title(&mut self, viewport: &mut Viewport, title: &str);

    /// Called by imgui to set the opacity of an entire [`Viewport`].
    ///
    /// If your backend does not support opacity, it is safe to just do nothing in this function.
    fn set_window_alpha(&mut self, viewport: &mut Viewport, alpha: f32);

    /// Called by imgui to update a [`Viewport`].
    fn update_window(&mut self, viewport: &mut Viewport);

    /// Called by imgui to render a [`Viewport`].
    fn render_window(&mut self, viewport: &mut Viewport);

    /// Called by imgui to swap buffers for a [`Viewport`].
    fn swap_buffers(&mut self, viewport: &mut Viewport);

    /// Called by imgui to create a Vulkan surface for a [`Viewport`].
    ///
    /// Returns 0 on success, non-zero on failure.
    fn create_vk_surface(
        &mut self,
        viewport: &mut Viewport,
        instance: u64,
        out_surface: &mut u64,
    ) -> i32;
}

/// Trait that holds optional functions for a rendering backend to support multiple viewports.
///
/// It is completely fine to not use this Backend at all, as all functions are optional.
#[cfg(feature = "multi-viewport")]
pub trait RendererViewportBackend: 'static {
    /// Called after [`PlatformViewportBackend::create_window()`].
    fn create_window(&mut self, viewport: &mut Viewport);

    /// Called before [`PlatformViewportBackend::destroy_window()`].
    fn destroy_window(&mut self, viewport: &mut Viewport);

    /// Called after [`PlatformViewportBackend::set_window_size()`].
    fn set_window_size(&mut self, viewport: &mut Viewport, size: [f32; 2]);

    /// Called to render a viewport.
    fn render_window(&mut self, viewport: &mut Viewport);

    /// Called to swap buffers for a viewport.
    fn swap_buffers(&mut self, viewport: &mut Viewport);
}

/// Default implementation of [`PlatformViewportBackend`] that does nothing.
#[cfg(feature = "multi-viewport")]
pub struct DummyPlatformViewportBackend;

#[cfg(feature = "multi-viewport")]
impl PlatformViewportBackend for DummyPlatformViewportBackend {
    fn create_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn destroy_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn show_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn set_window_pos(&mut self, _viewport: &mut Viewport, _pos: [f32; 2]) {
        // Do nothing
    }

    fn get_window_pos(&mut self, _viewport: &mut Viewport) -> [f32; 2] {
        [0.0, 0.0]
    }

    fn set_window_size(&mut self, _viewport: &mut Viewport, _size: [f32; 2]) {
        // Do nothing
    }

    fn get_window_size(&mut self, _viewport: &mut Viewport) -> [f32; 2] {
        [800.0, 600.0]
    }

    fn set_window_focus(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn get_window_focus(&mut self, _viewport: &mut Viewport) -> bool {
        false
    }

    fn get_window_minimized(&mut self, _viewport: &mut Viewport) -> bool {
        false
    }

    fn set_window_title(&mut self, _viewport: &mut Viewport, _title: &str) {
        // Do nothing
    }

    fn set_window_alpha(&mut self, _viewport: &mut Viewport, _alpha: f32) {
        // Do nothing
    }

    fn update_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn render_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn swap_buffers(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn create_vk_surface(
        &mut self,
        _viewport: &mut Viewport,
        _instance: u64,
        _out_surface: &mut u64,
    ) -> i32 {
        -1 // Not supported
    }
}

/// Default implementation of [`RendererViewportBackend`] that does nothing.
#[cfg(feature = "multi-viewport")]
pub struct DummyRendererViewportBackend;

#[cfg(feature = "multi-viewport")]
impl RendererViewportBackend for DummyRendererViewportBackend {
    fn create_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn destroy_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn set_window_size(&mut self, _viewport: &mut Viewport, _size: [f32; 2]) {
        // Do nothing
    }

    fn render_window(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }

    fn swap_buffers(&mut self, _viewport: &mut Viewport) {
        // Do nothing
    }
}

/// Context for platform viewport backend
#[cfg(feature = "multi-viewport")]
pub struct PlatformViewportContext {
    pub backend: Box<dyn PlatformViewportBackend>,
}

#[cfg(feature = "multi-viewport")]
impl PlatformViewportContext {
    pub fn new<T: PlatformViewportBackend>(backend: T) -> Self {
        Self {
            backend: Box::new(backend),
        }
    }

    pub fn dummy() -> Self {
        Self {
            backend: Box::new(DummyPlatformViewportBackend),
        }
    }
}

/// Context for renderer viewport backend
#[cfg(feature = "multi-viewport")]
pub struct RendererViewportContext {
    pub backend: Box<dyn RendererViewportBackend>,
}

#[cfg(feature = "multi-viewport")]
impl RendererViewportContext {
    pub fn new<T: RendererViewportBackend>(backend: T) -> Self {
        Self {
            backend: Box::new(backend),
        }
    }

    pub fn dummy() -> Self {
        Self {
            backend: Box::new(DummyRendererViewportBackend),
        }
    }
}

/// Set the platform viewport context for the current thread
#[cfg(feature = "multi-viewport")]
pub fn set_platform_viewport_context(context: PlatformViewportContext) {
    PLATFORM_VIEWPORT_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = Some(context);
    });
}

/// Set the renderer viewport context for the current thread
#[cfg(feature = "multi-viewport")]
pub fn set_renderer_viewport_context(context: RendererViewportContext) {
    RENDERER_VIEWPORT_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = Some(context);
    });
}

/// Execute a closure with access to the platform viewport context
#[cfg(feature = "multi-viewport")]
pub fn with_platform_viewport_context<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut dyn PlatformViewportBackend) -> R,
{
    PLATFORM_VIEWPORT_CONTEXT.with(|ctx| {
        ctx.borrow_mut()
            .as_mut()
            .map(|context| f(context.backend.as_mut()))
    })
}

/// Execute a closure with access to the renderer viewport context
#[cfg(feature = "multi-viewport")]
pub fn with_renderer_viewport_context<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut dyn RendererViewportBackend) -> R,
{
    RENDERER_VIEWPORT_CONTEXT.with(|ctx| {
        ctx.borrow_mut()
            .as_mut()
            .map(|context| f(context.backend.as_mut()))
    })
}

/// Clear the platform viewport context for the current thread
#[cfg(feature = "multi-viewport")]
pub fn clear_platform_viewport_context() {
    PLATFORM_VIEWPORT_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = None;
    });
}

/// Clear the renderer viewport context for the current thread
#[cfg(feature = "multi-viewport")]
pub fn clear_renderer_viewport_context() {
    RENDERER_VIEWPORT_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = None;
    });
}

/// Check if a platform viewport context is available
#[cfg(feature = "multi-viewport")]
pub fn has_platform_viewport_context() -> bool {
    PLATFORM_VIEWPORT_CONTEXT.with(|ctx| ctx.borrow().is_some())
}

/// Check if a renderer viewport context is available
#[cfg(feature = "multi-viewport")]
pub fn has_renderer_viewport_context() -> bool {
    RENDERER_VIEWPORT_CONTEXT.with(|ctx| ctx.borrow().is_some())
}

/// Execute a closure with access to the platform viewport context, with error handling
#[cfg(feature = "multi-viewport")]
pub fn try_with_platform_viewport_context<F, R, E>(f: F) -> Result<R, E>
where
    F: FnOnce(&mut dyn PlatformViewportBackend) -> Result<R, E>,
    E: From<ViewportError>,
{
    PLATFORM_VIEWPORT_CONTEXT.with(|ctx| {
        ctx.borrow_mut()
            .as_mut()
            .ok_or_else(|| E::from(ViewportError::NoContext))
            .and_then(|context| f(context.backend.as_mut()))
    })
}

/// Execute a closure with access to the renderer viewport context, with error handling
#[cfg(feature = "multi-viewport")]
pub fn try_with_renderer_viewport_context<F, R, E>(f: F) -> Result<R, E>
where
    F: FnOnce(&mut dyn RendererViewportBackend) -> Result<R, E>,
    E: From<ViewportError>,
{
    RENDERER_VIEWPORT_CONTEXT.with(|ctx| {
        ctx.borrow_mut()
            .as_mut()
            .ok_or_else(|| E::from(ViewportError::NoContext))
            .and_then(|context| f(context.backend.as_mut()))
    })
}

/// Errors that can occur when working with viewports
#[cfg(feature = "multi-viewport")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewportError {
    /// No viewport context is available
    NoContext,
    /// Invalid viewport handle
    InvalidViewport,
    /// Platform-specific error
    PlatformError(String),
    /// Renderer-specific error
    RendererError(String),
}

#[cfg(feature = "multi-viewport")]
impl std::fmt::Display for ViewportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewportError::NoContext => write!(f, "No viewport context available"),
            ViewportError::InvalidViewport => write!(f, "Invalid viewport handle"),
            ViewportError::PlatformError(msg) => write!(f, "Platform error: {}", msg),
            ViewportError::RendererError(msg) => write!(f, "Renderer error: {}", msg),
        }
    }
}

#[cfg(feature = "multi-viewport")]
impl std::error::Error for ViewportError {}

#[cfg(test)]
mod tests;

// Add a dummy viewport implementation for testing
#[cfg(feature = "multi-viewport")]
impl crate::platform_io::Viewport {
    /// Create a dummy viewport for testing
    pub fn dummy() -> Self {
        unsafe {
            let raw = std::mem::zeroed::<sys::ImGuiViewport>();
            std::mem::transmute(raw)
        }
    }
}
