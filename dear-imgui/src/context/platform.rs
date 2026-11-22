use crate::sys;

use super::Context;
#[cfg(feature = "multi-viewport")]
use super::attachment::{ContextPlatformWindowTeardown, ContextPlatformWindowTeardownError};
use super::binding::{CTX_MUTEX, with_bound_context};

#[cfg(feature = "multi-viewport")]
struct PlatformDrawDataTextureMask {
    entries: Vec<(
        std::ptr::NonNull<sys::ImDrawData>,
        *mut sys::ImVector_ImTextureDataPtr,
    )>,
}

#[cfg(feature = "multi-viewport")]
impl PlatformDrawDataTextureMask {
    unsafe fn install(platform_io: *mut sys::ImGuiPlatformIO) -> Self {
        let mut entries = Vec::new();
        let viewports = unsafe { &(*platform_io).Viewports };
        if viewports.Size <= 0 {
            return Self { entries };
        }
        assert!(
            viewports.Capacity >= viewports.Size && !viewports.Data.is_null(),
            "ImGuiPlatformIO.Viewports has invalid native storage"
        );
        for index in 0..viewports.Size as usize {
            let viewport = unsafe { *viewports.Data.add(index) };
            let Some(viewport) = std::ptr::NonNull::new(viewport) else {
                continue;
            };
            let Some(draw_data) = std::ptr::NonNull::new(unsafe { viewport.as_ref().DrawData })
            else {
                continue;
            };
            let textures = unsafe { draw_data.as_ref().Textures };
            if textures.is_null() {
                continue;
            }
            unsafe { (*draw_data.as_ptr()).Textures = std::ptr::null_mut() };
            entries.push((draw_data, textures));
        }
        Self { entries }
    }
}

#[cfg(feature = "multi-viewport")]
impl Drop for PlatformDrawDataTextureMask {
    fn drop(&mut self) {
        for (draw_data, textures) in self.entries.drain(..) {
            unsafe { (*draw_data.as_ptr()).Textures = textures };
        }
    }
}

impl Context {
    /// Get shared access to the platform IO.
    ///
    /// Note: `ImGuiPlatformIO` exists even when multi-viewport is disabled. We expose it
    /// unconditionally for platform configuration. Managed renderer texture work is exposed through
    /// request-bound `PendingFrame` or `FrameSnapshot` APIs, not raw texture pointers.
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
    /// Platform configuration is available even when multi-viewport is disabled. Callback
    /// registration always targets this Context, including inside another Context's binding scope.
    /// Managed renderer texture work uses the request-bound `PendingFrame` or `FrameSnapshot` APIs.
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
    /// It calls the installed render callbacks for visible secondary viewports, then calls their
    /// swap callbacks. Backends that render the viewports themselves may install only a swap callback.
    ///
    /// # Panics
    ///
    /// Panics unless [`Self::update_platform_windows`] completed for the current frame after
    /// [`Self::render`], or if the backends provide no render or swap callback. Managed-texture
    /// renderers must also reconcile the current frame before using this method.
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
                let platform_io =
                    self.platform_io_ptr("Context::render_platform_windows_default()");
                assert!(
                    (*platform_io).Platform_RenderWindow.is_some()
                        || (*platform_io).Renderer_RenderWindow.is_some()
                        || (*platform_io).Platform_SwapBuffers.is_some()
                        || (*platform_io).Renderer_SwapBuffers.is_some(),
                    "Context::render_platform_windows_default() requires a platform or renderer render or swap callback"
                );
                let renderer_has_textures =
                    (*self.io_ptr("Context::render_platform_windows_default()")).BackendFlags
                        & sys::ImGuiBackendFlags_RendererHasTextures as i32
                        != 0;
                if renderer_has_textures {
                    assert!(
                        self.snapshot_hub
                            .is_synchronous_frame_reconciled(raw.FrameCount),
                        "Context::render_platform_windows_default() requires managed-texture reconciliation for the current rendered frame"
                    );
                }
                let _texture_mask = renderer_has_textures
                    .then(|| PlatformDrawDataTextureMask::install(platform_io));
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
    /// destruction. This lets backends revoke UI access before destroying secondary windows. A
    /// rejected observer preflight still leaves that frame ended.
    ///
    /// Platform backends receive a bounded observer scope with this Context current so they can
    /// validate callback ownership and keep their teardown state coherent. If that preflight
    /// fails, native teardown does not begin.
    ///
    /// This is a platform-backend shutdown primitive. A registered backend may transition its
    /// own multi-viewport runtime to a released state; prefer its explicit shutdown API when one
    /// is available.
    ///
    /// # Errors
    ///
    /// Returns an error when the active platform attachment rejects the transaction, cannot
    /// complete post-teardown cleanup, or when the operation is re-entered. Post-teardown errors
    /// report that native platform windows have already been destroyed.
    #[cfg(feature = "multi-viewport")]
    #[doc(alias = "DestroyPlatformWindows")]
    pub fn destroy_platform_windows(&mut self) -> Result<(), ContextPlatformWindowTeardownError> {
        with_bound_context(self.raw, || {
            self.end_frame_for_teardown_unlocked();
            let teardown = ContextPlatformWindowTeardown::new(&self.state);
            let invocation = self.attachments.begin_platform_window_teardown(&teardown)?;
            unsafe { sys::igDestroyPlatformWindows() };
            invocation.finish(&teardown)
        })
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
                config_flags | config_flags_current_frame,
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

#[cfg(all(test, feature = "multi-viewport"))]
mod tests {
    use super::*;

    #[test]
    fn platform_teardown_observers_run_with_the_receiver_bound() {
        use crate::context::{
            ContextAttachment, ContextAttachmentRole, ContextAttachmentTeardownError,
        };
        use std::{cell::RefCell, rc::Rc};

        thread_local! {
            static CALLS: RefCell<Vec<(&'static str, *mut sys::ImGuiContext)>> = const { RefCell::new(Vec::new()) };
        }
        fn record(phase: &'static str) {
            CALLS.with_borrow_mut(|calls| {
                calls.push((phase, unsafe { sys::igGetCurrentContext() }))
            });
        }
        unsafe extern "C" fn destroy(_: *mut crate::Viewport) {
            record("destroy");
        }
        struct Observer;

        impl ContextAttachment for Observer {
            fn begin_platform_window_teardown(
                &self,
                _: &ContextPlatformWindowTeardown<'_>,
            ) -> Result<(), ContextAttachmentTeardownError> {
                record("begin");
                Ok(())
            }

            fn end_platform_window_teardown(
                &self,
                _: &ContextPlatformWindowTeardown<'_>,
            ) -> Result<(), ContextAttachmentTeardownError> {
                record("end");
                Ok(())
            }
        }

        let _guard = crate::test_support::imgui_context_guard();
        let first = Context::create();
        let binding = first.binding();
        let first_raw = first.as_raw();
        let _first = first.suspend_or_panic();
        let mut target = Context::create();
        let target_raw = target.as_raw();
        let _lease = target
            .register_attachment::<Observer>(ContextAttachmentRole::Platform, Rc::new(Observer))
            .unwrap();
        unsafe {
            target
                .platform_io_mut()
                .set_platform_destroy_window(Some(destroy))
        };

        binding.with_bound_context(|| {
            target.destroy_platform_windows().unwrap();
            assert_eq!(unsafe { sys::igGetCurrentContext() }, first_raw);
        });
        assert_eq!(
            CALLS.take(),
            [
                ("begin", target_raw),
                ("destroy", target_raw),
                ("end", target_raw)
            ]
        );
        assert_eq!(unsafe { sys::igGetCurrentContext() }, target_raw);
    }

    #[test]
    fn default_platform_render_dispatches_swap_only_backends() {
        use crate::context::{
            ContextAttachment, ContextAttachmentRole, ContextAttachmentTeardownError,
            ContextTeardown, FramePrepareOptions,
        };
        use std::{cell::Cell, ffi::c_void, rc::Rc};

        struct Platform;
        impl ContextAttachment for Platform {
            fn release_platform_windows(
                &self,
                context: &ContextTeardown<'_>,
            ) -> Result<(), ContextAttachmentTeardownError> {
                context.with_bound_context(|| unsafe { sys::igDestroyPlatformWindows() });
                Ok(())
            }
        }
        thread_local! { static SWAPPED: Cell<Option<crate::Id>> = const { Cell::new(None) }; }
        unsafe extern "C" fn window(_: *mut sys::ImGuiViewport) {}
        unsafe extern "C" fn title(_: *mut sys::ImGuiViewport, _: *const std::ffi::c_char) {}
        unsafe extern "C" fn set_vec(_: *mut sys::ImGuiViewport, _: *const sys::ImVec2) {}
        unsafe extern "C" fn get_pos(vp: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            unsafe { out.write((*vp).Pos) };
        }
        unsafe extern "C" fn get_size(vp: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
            unsafe { out.write((*vp).Size) };
        }
        unsafe extern "C" fn swap(vp: *mut crate::platform_io::Viewport, _: *mut c_void) {
            SWAPPED.set(Some(unsafe { (*vp).id() }));
        }

        let _guard = crate::test_support::imgui_context_guard();
        let mut context = Context::create();
        context
            .register_attachment::<Platform>(ContextAttachmentRole::Platform, Rc::new(Platform))
            .unwrap()
            .defer_to_context();
        context.set_ini_filename(None::<&str>).unwrap();
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        context.prepare_frame(FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
        let flags = context.io().backend_flags()
            | crate::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | crate::BackendFlags::RENDERER_HAS_VIEWPORTS;
        context.io_mut().set_backend_flags(flags);
        context.io_mut().set_config_viewports_no_auto_merge(true);
        context.enable_multi_viewport();
        let screen = sys::ImVec2::new(1920.0, 1080.0);
        unsafe {
            context
                .main_viewport()
                .set_platform_handle(std::ptr::dangling_mut::<u8>().cast());
            let io = context.platform_io_mut();
            io.set_monitors(&[sys::ImGuiPlatformMonitor {
                MainSize: screen,
                WorkSize: screen,
                DpiScale: 1.0,
                ..Default::default()
            }]);
            io.set_platform_create_window_raw(Some(window));
            io.set_platform_destroy_window_raw(Some(window));
            io.set_platform_show_window_raw(Some(window));
            io.set_platform_set_window_title_raw(Some(title));
            io.set_platform_set_window_pos_raw(Some(set_vec));
            io.set_platform_set_window_size_raw(Some(set_vec));
            io.set_platform_get_window_pos_raw(Some(get_pos));
            io.set_platform_get_window_size_raw(Some(get_size));
        }
        for _ in 0..2 {
            let ui = context.frame();
            ui.window("Secondary swap target")
                .position([200.0, 20.0], crate::Condition::Always)
                .size([80.0, 80.0], crate::Condition::Always)
                .build(|| ui.text("secondary"));
            drop(context.render_legacy());
            context.update_platform_windows();
        }
        let secondary = context
            .platform_io()
            .viewports_iter()
            .find(|v| !v.is_main())
            .unwrap()
            .id();
        unsafe {
            context
                .platform_io_mut()
                .set_platform_swap_buffers(Some(swap))
        };
        SWAPPED.set(None);
        context.render_platform_windows_default();
        assert_eq!(SWAPPED.get(), Some(secondary));

        unsafe {
            context.platform_io_mut().set_platform_swap_buffers(None);
            context
                .platform_io_mut()
                .set_renderer_swap_buffers(Some(swap));
        }
        SWAPPED.set(None);
        context.render_platform_windows_default();
        assert_eq!(SWAPPED.get(), Some(secondary));
    }

    #[test]
    fn managed_platform_draw_texture_mask_restores_every_pointer() {
        let mut textures = sys::ImVector_ImTextureDataPtr::default();
        let textures_ptr = &mut textures as *mut sys::ImVector_ImTextureDataPtr;
        let mut draw_data = sys::ImDrawData {
            Textures: textures_ptr,
            ..Default::default()
        };
        let mut viewport = sys::ImGuiViewport {
            DrawData: &mut draw_data,
            ..Default::default()
        };
        let viewport = &mut viewport as *mut sys::ImGuiViewport;
        let mut viewports = [viewport, viewport];
        let mut platform_io = sys::ImGuiPlatformIO {
            Viewports: sys::ImVector_ImGuiViewportPtr {
                Size: viewports.len() as i32,
                Capacity: viewports.len() as i32,
                Data: viewports.as_mut_ptr(),
            },
            ..Default::default()
        };

        let mask = unsafe { PlatformDrawDataTextureMask::install(&mut platform_io) };
        assert!(draw_data.Textures.is_null());
        drop(mask);
        assert_eq!(draw_data.Textures, textures_ptr);
    }
}
