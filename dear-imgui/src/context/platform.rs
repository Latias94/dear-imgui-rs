use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

impl Context {
    /// Get shared access to the platform IO.
    ///
    /// Note: `ImGuiPlatformIO` exists even when multi-viewport is disabled. We expose it
    /// unconditionally so callers can use ImGui 1.92+ texture management via `PlatformIO.Textures[]`.
    #[doc(alias = "GetPlatformIO")]
    pub fn platform_io(&self) -> &crate::platform_io::PlatformIo {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let pio = self.platform_io_ptr("Context::platform_io()");
            crate::platform_io::PlatformIo::from_raw(pio)
        }
    }

    /// Get mutable access to the platform IO.
    ///
    /// Note: `ImGuiPlatformIO` exists even when multi-viewport is disabled. We expose it
    /// unconditionally so callers can use ImGui 1.92+ texture management via `PlatformIO.Textures[]`.
    pub fn platform_io_mut(&mut self) -> &mut crate::platform_io::PlatformIo {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let pio = self.platform_io_ptr("Context::platform_io_mut()");
            crate::platform_io::PlatformIo::from_raw_mut(pio)
        }
    }

    /// Returns a reference to the main Dear ImGui viewport.
    ///
    /// The returned reference is owned by this ImGui context and
    /// must not be used after the context is destroyed.
    #[doc(alias = "GetMainViewport")]
    pub fn main_viewport(&mut self) -> &mut crate::platform_io::Viewport {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let ptr = sys::igGetMainViewport();
                if ptr.is_null() {
                    panic!("Context::main_viewport() requires a valid ImGui context");
                }
                crate::platform_io::Viewport::from_raw_mut(ptr)
            })
        }
    }

    /// Enable Dear ImGui's multi-viewport capability.
    ///
    /// Docking is an independent capability and must be enabled explicitly by the caller.
    /// A platform and renderer backend that advertise multi-viewport support must be installed
    /// before the next frame begins.
    ///
    /// Prefer enabling this before the first frame so Dear ImGui can load settings in the correct
    /// coordinate space. Enabling it between the first and second frames is rejected by Dear
    /// ImGui. When it is enabled after a later completed frame, [`Context::frame`] automatically
    /// advances the platform-window lifecycle for the preceding disabled frame.
    #[cfg(feature = "multi-viewport")]
    pub fn enable_multi_viewport(&mut self) {
        let io = self.io_mut();
        let mut flags = io.config_flags();
        flags.insert(crate::ConfigFlags::VIEWPORTS_ENABLE);
        io.set_config_flags(flags);
    }

    /// Update platform windows
    ///
    /// This function should be called every frame when multi-viewport is enabled.
    /// It updates all platform windows and handles viewport management.
    ///
    /// # Panics
    ///
    /// Panics if the current frame has not ended, this frame was already updated, or the active
    /// multi-viewport backend contract is incomplete.
    #[cfg(feature = "multi-viewport")]
    #[doc(alias = "UpdatePlatformWindows")]
    pub fn update_platform_windows(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                self.assert_can_update_platform_windows_unlocked(
                    "Context::update_platform_windows()",
                );
                sys::igUpdatePlatformWindows();
            });
        }
    }

    /// Render platform windows with default implementation
    ///
    /// This function renders all platform windows using the default implementation.
    /// It calls the platform and renderer backends to render each viewport.
    ///
    /// # Panics
    ///
    /// Panics unless [`Self::update_platform_windows`] completed for the current frame after
    /// [`Self::render`], or unless the installed backends provide `Platform_RenderWindow` or
    /// `Renderer_RenderWindow`. Snapshot-driven renderers should render their detached viewport
    /// data directly instead of calling this default callback pump.
    #[cfg(feature = "multi-viewport")]
    #[doc(alias = "RenderPlatformWindowsDefault")]
    pub fn render_platform_windows_default(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let raw = &*self.raw;
                assert!(
                    raw.FrameCount > 0
                        && raw.FrameCountRendered == raw.FrameCount
                        && raw.FrameCountPlatformEnded == raw.FrameCount,
                    "Context::render_platform_windows_default() requires a rendered frame followed by Context::update_platform_windows()"
                );
                let platform_io = sys::igGetPlatformIO_Nil();
                assert!(
                    !platform_io.is_null()
                        && ((*platform_io).Platform_RenderWindow.is_some()
                            || (*platform_io).Renderer_RenderWindow.is_some()),
                    "Context::render_platform_windows_default() requires Platform_RenderWindow or Renderer_RenderWindow; render snapshots directly when the renderer does not use default callbacks"
                );
                sys::igRenderPlatformWindowsDefault(std::ptr::null_mut(), std::ptr::null_mut());
            });
        }
    }

    /// Destroy all platform windows
    ///
    /// This function should be called during shutdown to properly clean up
    /// all platform windows and their associated resources.
    ///
    /// Any open frame is ended first through the same idempotent lifecycle path used by Context
    /// destruction. This lets backends revoke UI access before destroying secondary windows.
    #[cfg(feature = "multi-viewport")]
    #[doc(alias = "DestroyPlatformWindows")]
    pub fn destroy_platform_windows(&mut self) {
        let _guard = CTX_MUTEX.lock();
        self.end_frame_for_teardown_unlocked();
        unsafe {
            with_bound_context(self.raw, || {
                sys::igDestroyPlatformWindows();
            });
        }
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn prepare_multi_viewport_new_frame_contract_unlocked(&self, caller: &str) {
        unsafe {
            let config_flags = (*self.io_ptr(caller)).ConfigFlags;
            let viewports_enabled = config_flags & sys::ImGuiConfigFlags_ViewportsEnable != 0;
            if !viewports_enabled {
                return;
            }

            let frame_count = (*self.raw).FrameCount;
            let frame_count_ended = (*self.raw).FrameCountEnded;
            let frame_count_platform_ended = (*self.raw).FrameCountPlatformEnded;
            let config_flags_current_frame = (*self.raw).ConfigFlagsCurrFrame;

            if frame_count == 1
                && config_flags_current_frame & sys::ImGuiConfigFlags_ViewportsEnable == 0
            {
                panic!(
                    "{caller} cannot enable multi-viewport on the second frame; enable it before the first frame or after the second frame so Dear ImGui preserves its settings contract"
                );
            }
            if !self.multi_viewport_backends_advertised_unlocked() {
                // Dear ImGui intentionally clears ViewportsEnable when either backend declines
                // support. Preserve that graceful fallback instead of turning it into an error.
                return;
            }
            if frame_count > 0 && frame_count_platform_ended != frame_count {
                if config_flags_current_frame & sys::ImGuiConfigFlags_ViewportsEnable == 0 {
                    assert_eq!(
                        frame_count_ended, frame_count,
                        "{caller} cannot enable multi-viewport while the previous frame is still open"
                    );
                    // The preceding frame had viewports disabled, so native UpdatePlatformWindows
                    // only advances its frame watermark and cannot invoke backend callbacks.
                    sys::igUpdatePlatformWindows();
                } else {
                    panic!(
                        "{caller} cannot begin a new multi-viewport frame before Context::update_platform_windows() completes the previous frame"
                    );
                }
            }
            self.assert_multi_viewport_backend_contract_unlocked(
                caller,
                sys::ImGuiConfigFlags_ViewportsEnable,
            );
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn assert_can_update_platform_windows_unlocked(&self, caller: &str) {
        unsafe {
            let frame_count = (*self.raw).FrameCount;
            let frame_count_ended = (*self.raw).FrameCountEnded;
            let frame_count_platform_ended = (*self.raw).FrameCountPlatformEnded;
            let config_flags_current_frame = (*self.raw).ConfigFlagsCurrFrame;
            assert!(
                frame_count_ended == frame_count,
                "{caller} requires Context::render() or an ended frame first"
            );
            assert!(
                frame_count_platform_ended < frame_count,
                "{caller} was already called for frame {}",
                frame_count
            );
            self.assert_multi_viewport_backend_contract_unlocked(
                caller,
                config_flags_current_frame,
            );
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn assert_multi_viewport_backend_contract_unlocked(&self, caller: &str, config_flags: i32) {
        if config_flags & sys::ImGuiConfigFlags_ViewportsEnable == 0 {
            return;
        }

        unsafe {
            let io = &*self.io_ptr(caller);
            assert!(
                self.multi_viewport_backends_advertised_unlocked(),
                "{caller} requires platform and renderer backends that advertise multi-viewport support"
            );

            let platform_io = &*self.platform_io_ptr(caller);
            for (name, installed) in [
                (
                    "Platform_CreateWindow",
                    platform_io.Platform_CreateWindow.is_some(),
                ),
                (
                    "Platform_DestroyWindow",
                    platform_io.Platform_DestroyWindow.is_some(),
                ),
                (
                    "Platform_ShowWindow",
                    platform_io.Platform_ShowWindow.is_some(),
                ),
                (
                    "Platform_GetWindowPos",
                    platform_io.Platform_GetWindowPos.is_some(),
                ),
                (
                    "Platform_SetWindowPos",
                    platform_io.Platform_SetWindowPos.is_some(),
                ),
                (
                    "Platform_GetWindowSize",
                    platform_io.Platform_GetWindowSize.is_some(),
                ),
                (
                    "Platform_SetWindowSize",
                    platform_io.Platform_SetWindowSize.is_some(),
                ),
                (
                    "Platform_SetWindowTitle",
                    platform_io.Platform_SetWindowTitle.is_some(),
                ),
            ] {
                assert!(
                    installed,
                    "{caller} requires the {name} callback before multi-viewport can run"
                );
            }

            let monitor_count = platform_io.Monitors.Size;
            assert!(
                monitor_count > 0 && !platform_io.Monitors.Data.is_null(),
                "{caller} requires at least one valid PlatformIO monitor"
            );
            assert!(
                platform_io.Monitors.Capacity >= monitor_count,
                "{caller} rejected a corrupt PlatformIO monitor vector"
            );
            let monitors = std::slice::from_raw_parts(
                platform_io.Monitors.Data,
                usize::try_from(monitor_count)
                    .expect("positive PlatformIO monitor count must fit usize"),
            );
            crate::platform_io::assert_monitor_contract(monitors, caller);

            let main_viewport = sys::igGetMainViewport();
            assert!(
                !main_viewport.is_null(),
                "{caller} requires a valid main viewport"
            );
            assert!(
                !(*main_viewport).PlatformUserData.is_null()
                    || !(*main_viewport).PlatformHandle.is_null(),
                "{caller} requires the platform backend to initialize the main viewport"
            );

            let transparent_docking = io.ConfigDockingTransparentPayload
                && config_flags & sys::ImGuiConfigFlags_DockingEnable != 0;
            assert!(
                !transparent_docking || platform_io.Platform_SetWindowAlpha.is_some(),
                "{caller} requires Platform_SetWindowAlpha when transparent docking payloads are enabled"
            );
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn multi_viewport_backends_advertised_unlocked(&self) -> bool {
        unsafe {
            let backend_flags = (*self.io_ptr("multi-viewport backend validation")).BackendFlags;
            backend_flags & sys::ImGuiBackendFlags_PlatformHasViewports != 0
                && backend_flags & sys::ImGuiBackendFlags_RendererHasViewports != 0
        }
    }
}
