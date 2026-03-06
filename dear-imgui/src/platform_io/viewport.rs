use crate::sys;
use std::cell::UnsafeCell;
use std::ffi::c_void;

// - Viewport management
// - Platform backend callbacks
// - Renderer backend callbacks
// - Monitor information
// - Platform-specific settings

/// Viewport structure for multi-viewport support
///
/// This is a transparent wrapper around `ImGuiViewport` that provides
/// safe access to viewport functionality.
#[repr(transparent)]
pub struct Viewport {
    raw: UnsafeCell<sys::ImGuiViewport>,
}

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiViewport>()] = [(); std::mem::size_of::<Viewport>()];
const _: [(); std::mem::align_of::<sys::ImGuiViewport>()] = [(); std::mem::align_of::<Viewport>()];

impl Viewport {
    #[inline]
    fn inner(&self) -> &sys::ImGuiViewport {
        // Safety: `Viewport` is a view into ImGui-owned viewport state which may be mutated by
        // Dear ImGui and platform/renderer backends while Rust holds `&Viewport`.
        unsafe { &*self.raw.get() }
    }

    #[inline]
    fn inner_mut(&mut self) -> &mut sys::ImGuiViewport {
        unsafe { &mut *self.raw.get() }
    }

    /// Get a reference to the viewport from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `raw` is non-null and points to a valid `ImGuiViewport`.
    /// - The viewport outlives the returned reference (e.g. it belongs to the
    ///   currently active ImGui context).
    pub(crate) unsafe fn from_raw<'a>(raw: *const sys::ImGuiViewport) -> &'a Self {
        unsafe { &*(raw as *const Self) }
    }

    /// Get a mutable reference to the viewport from a raw pointer
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `raw` is non-null and points to a valid `ImGuiViewport`.
    /// - The viewport outlives the returned reference (e.g. it belongs to the
    ///   currently active ImGui context).
    /// - No other references (shared or mutable) to the same viewport are alive.
    pub unsafe fn from_raw_mut<'a>(raw: *mut sys::ImGuiViewport) -> &'a mut Self {
        unsafe { &mut *(raw as *mut Self) }
    }

    /// Get the raw pointer to the underlying `ImGuiViewport`
    pub fn as_raw(&self) -> *const sys::ImGuiViewport {
        self.raw.get().cast_const()
    }

    /// Get the raw mutable pointer to the underlying `ImGuiViewport`
    pub fn as_raw_mut(&mut self) -> *mut sys::ImGuiViewport {
        self.raw.get()
    }

    /// Get the viewport ID
    pub fn id(&self) -> sys::ImGuiID {
        self.inner().ID
    }

    /// Set the viewport position
    pub fn set_pos(&mut self, pos: [f32; 2]) {
        self.inner_mut().Pos.x = pos[0];
        self.inner_mut().Pos.y = pos[1];
    }

    /// Get the viewport position
    pub fn pos(&self) -> [f32; 2] {
        [self.inner().Pos.x, self.inner().Pos.y]
    }

    /// Set the viewport size
    pub fn set_size(&mut self, size: [f32; 2]) {
        self.inner_mut().Size.x = size[0];
        self.inner_mut().Size.y = size[1];
    }

    /// Get the viewport size
    pub fn size(&self) -> [f32; 2] {
        [self.inner().Size.x, self.inner().Size.y]
    }

    /// Get the viewport work position (excluding menu bars, task bars, etc.)
    pub fn work_pos(&self) -> [f32; 2] {
        [self.inner().WorkPos.x, self.inner().WorkPos.y]
    }

    /// Get the viewport work size (excluding menu bars, task bars, etc.)
    pub fn work_size(&self) -> [f32; 2] {
        [self.inner().WorkSize.x, self.inner().WorkSize.y]
    }

    /// Check if this is the main viewport
    ///
    /// Note: Main viewport is typically identified by ID == 0 or by checking if it's not a platform window
    #[cfg(feature = "multi-viewport")]
    pub fn is_main(&self) -> bool {
        self.inner().ID == 0
            || (self.inner().Flags & (crate::ViewportFlags::IS_PLATFORM_WINDOW.bits())) == 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_main(&self) -> bool {
        self.inner().ID == 0
    }

    /// Check if this is a platform window (not the main viewport)
    #[cfg(feature = "multi-viewport")]
    pub fn is_platform_window(&self) -> bool {
        (self.inner().Flags & (crate::ViewportFlags::IS_PLATFORM_WINDOW.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_platform_window(&self) -> bool {
        false
    }

    /// Check if this is a platform monitor
    #[cfg(feature = "multi-viewport")]
    pub fn is_platform_monitor(&self) -> bool {
        (self.inner().Flags & (crate::ViewportFlags::IS_PLATFORM_MONITOR.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_platform_monitor(&self) -> bool {
        false
    }

    /// Check if this viewport is owned by the application
    #[cfg(feature = "multi-viewport")]
    pub fn is_owned_by_app(&self) -> bool {
        (self.inner().Flags & (crate::ViewportFlags::OWNED_BY_APP.bits())) != 0
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub fn is_owned_by_app(&self) -> bool {
        false
    }

    /// Get the platform user data
    pub fn platform_user_data(&self) -> *mut c_void {
        self.inner().PlatformUserData
    }

    /// Set the platform user data
    pub fn set_platform_user_data(&mut self, data: *mut c_void) {
        self.inner_mut().PlatformUserData = data;
    }

    /// Get the renderer user data
    pub fn renderer_user_data(&self) -> *mut c_void {
        self.inner().RendererUserData
    }

    /// Set the renderer user data
    pub fn set_renderer_user_data(&mut self, data: *mut c_void) {
        self.inner_mut().RendererUserData = data;
    }

    /// Get the platform handle
    pub fn platform_handle(&self) -> *mut c_void {
        self.inner().PlatformHandle
    }

    /// Set the platform handle
    pub fn set_platform_handle(&mut self, handle: *mut c_void) {
        self.inner_mut().PlatformHandle = handle;
    }

    /// Check if the platform window was created
    pub fn platform_window_created(&self) -> bool {
        self.inner().PlatformWindowCreated
    }

    /// Set whether the platform window was created
    pub fn set_platform_window_created(&mut self, created: bool) {
        self.inner_mut().PlatformWindowCreated = created;
    }

    /// Check if the platform requested move
    pub fn platform_request_move(&self) -> bool {
        self.inner().PlatformRequestMove
    }

    /// Set whether the platform requested move
    pub fn set_platform_request_move(&mut self, request: bool) {
        self.inner_mut().PlatformRequestMove = request;
    }

    /// Check if the platform requested resize
    pub fn platform_request_resize(&self) -> bool {
        self.inner().PlatformRequestResize
    }

    /// Set whether the platform requested resize
    pub fn set_platform_request_resize(&mut self, request: bool) {
        self.inner_mut().PlatformRequestResize = request;
    }

    /// Check if the platform requested close
    pub fn platform_request_close(&self) -> bool {
        self.inner().PlatformRequestClose
    }

    /// Set whether the platform requested close
    pub fn set_platform_request_close(&mut self, request: bool) {
        self.inner_mut().PlatformRequestClose = request;
    }

    /// Get the viewport flags
    pub fn flags(&self) -> sys::ImGuiViewportFlags {
        self.inner().Flags
    }

    /// Set the viewport flags
    pub fn set_flags(&mut self, flags: sys::ImGuiViewportFlags) {
        self.inner_mut().Flags = flags;
    }

    /// Get the DPI scale factor
    #[cfg(feature = "multi-viewport")]
    pub fn dpi_scale(&self) -> f32 {
        self.inner().DpiScale
    }

    /// Set the DPI scale factor
    #[cfg(feature = "multi-viewport")]
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.inner_mut().DpiScale = scale;
    }

    /// Get the parent viewport ID
    #[cfg(feature = "multi-viewport")]
    pub fn parent_viewport_id(&self) -> sys::ImGuiID {
        self.inner().ParentViewportId
    }

    /// Set the parent viewport ID
    #[cfg(feature = "multi-viewport")]
    pub fn set_parent_viewport_id(&mut self, id: sys::ImGuiID) {
        self.inner_mut().ParentViewportId = id;
    }

    /// Get the draw data pointer
    #[cfg(feature = "multi-viewport")]
    pub fn draw_data(&self) -> *mut sys::ImDrawData {
        self.inner().DrawData
    }

    /// Get the draw data as a reference (if available)
    #[cfg(feature = "multi-viewport")]
    pub fn draw_data_ref(&self) -> Option<&sys::ImDrawData> {
        if self.inner().DrawData.is_null() {
            None
        } else {
            Some(unsafe { &*self.inner().DrawData })
        }
    }

    /// Get the framebuffer scale
    #[cfg(feature = "multi-viewport")]
    pub fn framebuffer_scale(&self) -> [f32; 2] {
        [
            self.inner().FramebufferScale.x,
            self.inner().FramebufferScale.y,
        ]
    }

    /// Set the framebuffer scale
    #[cfg(feature = "multi-viewport")]
    pub fn set_framebuffer_scale(&mut self, scale: [f32; 2]) {
        self.inner_mut().FramebufferScale.x = scale[0];
        self.inner_mut().FramebufferScale.y = scale[1];
    }
}
