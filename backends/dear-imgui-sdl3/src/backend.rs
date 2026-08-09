#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use super::core::assert_current_draw_data;
use super::*;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::render::SynchronousRendererConsumer;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::sys;
#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
use dear_imgui_rs::{BackendFlags, ConfigFlags, FrameToken};
use sdl3::event::Event;
#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
use std::fmt;
#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn prepare_renderer_consumer(
    imgui: &mut Context,
) -> Result<SynchronousRendererConsumer, Sdl3BackendError> {
    let consumer = imgui.create_synchronous_renderer_consumer()?;
    // A new consumer has not submitted an epoch or produced a Context-managed texture mapping.
    // The empty transaction only clears bindings from an already-released predecessor before this
    // renderer can publish new texture requests.
    let reset = imgui.prepare_renderer_texture_reset(&consumer)?;
    reset.commit();
    Ok(consumer)
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn prepare_renderer_runtime(
    imgui: &mut Context,
    renderer_shutdown: fn(),
    renderer_device_objects_destroy: fn(),
    renderer_texture_update: fn(&mut dear_imgui_rs::TextureData),
    platform_graphics: PlatformGraphicsKind,
    native_renderer: NativeRendererKind,
) -> Result<RuntimeRegistration, Sdl3BackendError> {
    let mut runtime = RuntimeRegistration::prepare_with_backend(
        imgui,
        Some(renderer_shutdown),
        Some(renderer_device_objects_destroy),
        Some(renderer_texture_update),
        platform_graphics,
        native_renderer,
    )?;
    match prepare_renderer_consumer(imgui) {
        Ok(consumer) => {
            runtime.install_renderer_consumer(consumer);
            Ok(runtime)
        }
        Err(error) => {
            runtime.native_initialization_failed();
            Err(error)
        }
    }
}

fn run_backend_entry<R>(
    runtime: &RuntimeRegistration,
    imgui: &Context,
    callback: impl FnOnce() -> R,
) -> Result<R, Sdl3BackendError> {
    let entry = runtime.control().enter(imgui)?;
    let result = runtime
        .control()
        .binding()
        .try_with_bound_context(callback)?;
    entry.finish()?;
    Ok(result)
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn ensure_matching_pending_frame(
    context: &ContextBinding,
    frame: &PendingFrame<'_>,
) -> Result<(), Sdl3BackendError> {
    let expected = context.id();
    let actual = frame.context_id();
    if expected != actual {
        return Err(Sdl3BackendError::ContextMismatch { expected, actual });
    }
    Ok(())
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
fn ensure_matching_frame_token(
    context: &ContextBinding,
    frame: &FrameToken<'_>,
) -> Result<(), Sdl3BackendError> {
    let expected = context.id();
    let actual = frame.ui().context_id();
    if expected != actual {
        return Err(Sdl3BackendError::ContextMismatch { expected, actual });
    }
    Ok(())
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
fn native_viewport_route_enabled(config_flags: ConfigFlags, backend_flags: BackendFlags) -> bool {
    #[cfg(feature = "multi-viewport")]
    {
        let required = BackendFlags::PLATFORM_HAS_VIEWPORTS | BackendFlags::RENDERER_HAS_VIEWPORTS;
        config_flags.contains(ConfigFlags::VIEWPORTS_ENABLE) && backend_flags.contains(required)
    }

    #[cfg(not(feature = "multi-viewport"))]
    {
        let _ = (config_flags, backend_flags);
        false
    }
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
fn frame_has_native_viewport_route(frame: &FrameToken<'_>) -> bool {
    let io = frame.ui().io();
    native_viewport_route_enabled(io.config_flags(), io.backend_flags())
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
fn route_allows_native_viewport_pump(fault_free: bool, native_route_enabled: bool) -> bool {
    fault_free && native_route_enabled
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
fn capture_renderer_frame<'ctx>(
    runtime: &RuntimeRegistration,
    frame: FrameToken<'ctx>,
) -> Result<ReconciledFrame<'ctx>, Sdl3BackendError> {
    runtime.control().ensure_bound_entry()?;
    let frame = frame.try_render(runtime.renderer_consumer())?;
    reconcile_renderer_frame(runtime, frame)
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
fn pump_platform_windows(frame: &mut ReconciledFrame<'_>, enabled: bool) {
    #[cfg(feature = "multi-viewport")]
    if enabled {
        frame.update_and_render_platform_windows_default();
    }

    #[cfg(not(feature = "multi-viewport"))]
    let _ = (frame, enabled);
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
fn ensure_matching_reconciled_frame(
    context: &ContextBinding,
    frame: &ReconciledFrame<'_>,
) -> Result<(), Sdl3BackendError> {
    let expected = context.id();
    let actual = frame.context_id();
    if expected != actual {
        return Err(Sdl3BackendError::ContextMismatch { expected, actual });
    }
    Ok(())
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn reconcile_renderer_frame<'ctx>(
    runtime: &RuntimeRegistration,
    frame: PendingFrame<'ctx>,
) -> Result<ReconciledFrame<'ctx>, Sdl3BackendError> {
    ensure_matching_pending_frame(runtime.control().binding(), &frame)?;
    let request_epoch = frame.epoch().sequence();

    let entry = runtime.control().enter_bound()?;
    let processed = runtime.control().binding().try_with_bound_context(|| {
        runtime
            .control()
            .process_texture_requests(frame.texture_requests(), request_epoch)
    })??;
    let frame = frame.reconcile_texture_feedback(processed.feedback)?;
    runtime
        .control()
        .mark_textures_reconciled(&processed.installed);
    runtime
        .control()
        .prune_destroyed_textures(frame.completion_progress().watermark());
    entry.finish()?;
    Ok(frame)
}

/// Ordered SDL3 backend failures from one first-party viewport route.
///
/// The route returns every deferred failure observed during its transaction instead of hiding all
/// but the oldest queue entry. Faults retain the runtime's FIFO observation order.
#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
#[derive(Debug)]
pub struct Sdl3ViewportRouteError {
    faults: Vec<Sdl3BackendError>,
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
impl Sdl3ViewportRouteError {
    fn new(faults: Vec<Sdl3BackendError>) -> Self {
        debug_assert!(!faults.is_empty());
        Self { faults }
    }

    fn single(fault: Sdl3BackendError) -> Self {
        Self::new(vec![fault])
    }

    /// Returns every backend fault in reporting order.
    #[must_use]
    pub fn faults(&self) -> &[Sdl3BackendError] {
        &self.faults
    }

    /// Consumes the aggregate and returns every backend fault in reporting order.
    #[must_use]
    pub fn into_faults(self) -> Vec<Sdl3BackendError> {
        self.faults
    }
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
impl fmt::Display for Sdl3ViewportRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.faults[0])?;
        if self.faults.len() > 1 {
            write!(
                formatter,
                " ({} SDL3 viewport route faults in total)",
                self.faults.len()
            )?;
        }
        Ok(())
    }
}

#[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
impl std::error::Error for Sdl3ViewportRouteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.faults
            .first()
            .map(|fault| fault as &(dyn std::error::Error + 'static))
    }
}

/// One failure observed while preparing an SDL3 + OpenGL3 viewport frame.
#[cfg(feature = "opengl3-renderer")]
#[derive(Debug)]
#[non_exhaustive]
pub enum Sdl3OpenGl3ViewportRouteFault<RestoreError> {
    /// Frame capture, texture reconciliation, or an SDL3 native callback failed.
    Backend(Sdl3BackendError),
    /// The application failed to restore its main OpenGL context after the platform pass.
    MainContextRestore(RestoreError),
}

#[cfg(feature = "opengl3-renderer")]
impl<RestoreError> fmt::Display for Sdl3OpenGl3ViewportRouteFault<RestoreError>
where
    RestoreError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(error) => write!(formatter, "SDL3 OpenGL viewport route failed: {error}"),
            Self::MainContextRestore(error) => {
                write!(
                    formatter,
                    "failed to restore the main OpenGL context: {error}"
                )
            }
        }
    }
}

#[cfg(feature = "opengl3-renderer")]
impl<RestoreError> std::error::Error for Sdl3OpenGl3ViewportRouteFault<RestoreError>
where
    RestoreError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend(error) => Some(error),
            Self::MainContextRestore(error) => Some(error),
        }
    }
}

/// Ordered failures from one SDL3 + OpenGL3 viewport preparation transaction.
///
/// Existing backend faults prevent native viewport dispatch. The main context is then restored,
/// and every backend fault discovered by the final drain is retained in FIFO order.
#[cfg(feature = "opengl3-renderer")]
#[derive(Debug)]
pub struct Sdl3OpenGl3ViewportRouteError<RestoreError> {
    faults: Vec<Sdl3OpenGl3ViewportRouteFault<RestoreError>>,
}

#[cfg(feature = "opengl3-renderer")]
impl<RestoreError> Sdl3OpenGl3ViewportRouteError<RestoreError> {
    fn new(faults: Vec<Sdl3OpenGl3ViewportRouteFault<RestoreError>>) -> Self {
        debug_assert!(!faults.is_empty());
        Self { faults }
    }

    fn backend(error: Sdl3BackendError) -> Self {
        Self::new(vec![Sdl3OpenGl3ViewportRouteFault::Backend(error)])
    }

    /// Returns every route fault in reporting order.
    #[must_use]
    pub fn faults(&self) -> &[Sdl3OpenGl3ViewportRouteFault<RestoreError>] {
        &self.faults
    }

    /// Consumes the aggregate and returns every route fault in reporting order.
    #[must_use]
    pub fn into_faults(self) -> Vec<Sdl3OpenGl3ViewportRouteFault<RestoreError>> {
        self.faults
    }
}

#[cfg(feature = "opengl3-renderer")]
impl<RestoreError> fmt::Display for Sdl3OpenGl3ViewportRouteError<RestoreError>
where
    RestoreError: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.faults[0])?;
        if self.faults.len() > 1 {
            write!(
                formatter,
                " ({} SDL3 OpenGL viewport route faults in total)",
                self.faults.len()
            )?;
        }
        Ok(())
    }
}

#[cfg(feature = "opengl3-renderer")]
impl<RestoreError> std::error::Error for Sdl3OpenGl3ViewportRouteError<RestoreError>
where
    RestoreError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.faults
            .first()
            .map(|fault| fault as &(dyn std::error::Error + 'static))
    }
}

#[cfg(feature = "opengl3-renderer")]
fn finish_opengl_route_attempt<RestoreError>(
    route_attempt: std::thread::Result<()>,
    restore_main_context: impl FnOnce() -> Result<(), RestoreError>,
    drain_faults: impl FnOnce() -> Vec<Sdl3BackendError>,
) -> (Result<(), RestoreError>, Vec<Sdl3BackendError>) {
    let restore_attempt = catch_unwind(AssertUnwindSafe(restore_main_context));
    let deferred_faults = drain_faults();

    if let Err(payload) = route_attempt {
        resume_unwind(payload);
    }
    let restore_result = match restore_attempt {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    };
    (restore_result, deferred_faults)
}

#[cfg(all(test, any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer")))]
mod viewport_route_contract_tests {
    use super::*;
    #[cfg(feature = "opengl3-renderer")]
    use std::cell::Cell;

    #[test]
    fn route_error_preserves_backend_fault_fifo() {
        let error = Sdl3ViewportRouteError::new(vec![
            Sdl3BackendError::PlatformStateUnavailable,
            Sdl3BackendError::RuntimeDetached,
        ]);
        assert!(matches!(
            error.faults(),
            [
                Sdl3BackendError::PlatformStateUnavailable,
                Sdl3BackendError::RuntimeDetached
            ]
        ));
    }

    #[test]
    fn an_existing_fault_blocks_native_viewport_pump() {
        assert!(!route_allows_native_viewport_pump(false, true));
        assert!(!route_allows_native_viewport_pump(true, false));
        assert!(route_allows_native_viewport_pump(true, true));
    }

    #[test]
    fn foreign_frame_token_is_rejected_before_capture() {
        let _guard = crate::tests::test_guard();
        let expected = Context::create();
        let expected_binding = expected.binding();
        let expected = expected.suspend_or_panic();
        let mut foreign = Context::create();
        foreign
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        foreign.io_mut().set_display_size([128.0, 128.0]);
        foreign.io_mut().set_delta_time(1.0 / 60.0);
        let frame = foreign.begin_frame();
        let error = ensure_matching_frame_token(&expected_binding, &frame).unwrap_err();
        assert!(matches!(error, Sdl3BackendError::ContextMismatch { .. }));
        drop(frame);
        drop(foreign);
        drop(expected);
    }

    #[cfg(feature = "multi-viewport")]
    #[test]
    fn native_viewport_route_requires_actual_config_and_backend_capabilities() {
        let backend = BackendFlags::PLATFORM_HAS_VIEWPORTS | BackendFlags::RENDERER_HAS_VIEWPORTS;
        assert!(native_viewport_route_enabled(
            ConfigFlags::VIEWPORTS_ENABLE,
            backend
        ));
        assert!(!native_viewport_route_enabled(
            ConfigFlags::empty(),
            backend
        ));
        assert!(!native_viewport_route_enabled(
            ConfigFlags::VIEWPORTS_ENABLE,
            BackendFlags::PLATFORM_HAS_VIEWPORTS
        ));
    }

    #[cfg(not(feature = "multi-viewport"))]
    #[test]
    fn native_viewport_route_degrades_when_the_crate_capability_is_disabled() {
        let backend = BackendFlags::PLATFORM_HAS_VIEWPORTS | BackendFlags::RENDERER_HAS_VIEWPORTS;
        assert!(!native_viewport_route_enabled(
            ConfigFlags::VIEWPORTS_ENABLE,
            backend
        ));
    }

    #[cfg(feature = "opengl3-renderer")]
    #[test]
    fn opengl_route_restores_and_drains_after_success() {
        let restored = Cell::new(false);
        let drained = Cell::new(false);
        let (restore_result, faults) = finish_opengl_route_attempt(
            Ok(()),
            || {
                restored.set(true);
                Ok::<_, &'static str>(())
            },
            || {
                drained.set(true);
                vec![Sdl3BackendError::RuntimeDetached]
            },
        );
        assert!(restore_result.is_ok());
        assert_eq!(faults.len(), 1);
        assert!(restored.get());
        assert!(drained.get());
    }

    #[cfg(feature = "opengl3-renderer")]
    #[test]
    fn opengl_route_restores_and_drains_when_restore_fails() {
        let restored = Cell::new(false);
        let drained = Cell::new(false);
        let (restore_result, faults) = finish_opengl_route_attempt(
            Ok(()),
            || {
                restored.set(true);
                Err::<(), _>("restore failed")
            },
            || {
                drained.set(true);
                vec![Sdl3BackendError::RuntimeDetached]
            },
        );
        assert_eq!(restore_result, Err("restore failed"));
        assert_eq!(faults.len(), 1);
        assert!(restored.get());
        assert!(drained.get());
    }

    #[cfg(feature = "opengl3-renderer")]
    #[test]
    fn opengl_route_restores_and_drains_before_resuming_panic() {
        let restored = Cell::new(false);
        let drained = Cell::new(false);
        let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let route_attempt = catch_unwind(AssertUnwindSafe(|| panic!("viewport panic")));
            let _ = finish_opengl_route_attempt(
                route_attempt,
                || {
                    restored.set(true);
                    Ok::<_, &'static str>(())
                },
                || {
                    drained.set(true);
                    Vec::new()
                },
            );
        }));
        assert!(panic.is_err());
        assert!(restored.get());
        assert!(drained.get());
    }

    #[cfg(feature = "sdlgpu3-renderer")]
    #[test]
    fn sdl_gpu_prepared_frame_has_an_explicit_main_surface_skip() {
        fn skip(frame: SdlGpu3PreparedViewportFrame<'_>) {
            frame.skip_main();
        }
        let _ = skip;
    }
}

macro_rules! impl_sdl3_input_controls {
    ($backend:ty) => {
        impl $backend {
            /// Configure how the SDL3 backend handles gamepads for the captured context.
            pub fn set_gamepad_mode(
                &mut self,
                imgui: &mut Context,
                mode: GamepadMode,
            ) -> Result<(), Sdl3BackendError> {
                run_backend_entry(&self.runtime, imgui, || set_gamepad_mode(mode))
            }

            /// Configure SDL3 backend to use manual gamepad selection for the captured context.
            ///
            /// # Safety
            ///
            /// - Every pointer in `gamepads` must be a valid, opened `SDL_Gamepad`.
            /// - Those gamepads must remain alive until another gamepad mode is selected or the
            ///   backend shuts down. The backend copies the pointers, not the gamepad objects.
            pub unsafe fn set_gamepad_mode_manual(
                &mut self,
                imgui: &mut Context,
                gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
            ) -> Result<(), Sdl3BackendError> {
                run_backend_entry(&self.runtime, imgui, || unsafe {
                    set_gamepad_mode_manual(gamepads)
                })
            }

            /// Override the SDL3 backend mouse-capture policy for the captured context.
            ///
            /// This controls whether drags continue to receive pointer updates outside an SDL
            /// window. It does not add global mouse or native viewport capabilities to an SDL
            /// video driver that lacks them, such as Wayland.
            pub fn set_mouse_capture_mode(
                &mut self,
                imgui: &mut Context,
                mode: MouseCaptureMode,
            ) -> Result<(), Sdl3BackendError> {
                run_backend_entry(&self.runtime, imgui, || set_mouse_capture_mode(mode))
            }
        }
    };
}

macro_rules! impl_sdl3_event_controls {
    ($backend:ty) => {
        impl $backend {
            /// Process an owned SDL3 event with the captured ImGui context.
            pub fn process_event(
                &mut self,
                imgui: &mut Context,
                event: &Event,
            ) -> Result<bool, Sdl3BackendError> {
                run_backend_entry(&self.runtime, imgui, || process_owned_event(event))?
            }

            /// Process an event copied by [`Sdl3CallbackEventHandoff`].
            ///
            /// Unlike [`Self::process_raw_event`], this path is safe because the handoff owns every
            /// pointer-bearing payload used by the official SDL3 backend.
            pub fn process_callback_event(
                &mut self,
                imgui: &mut Context,
                event: &Sdl3CallbackEvent,
            ) -> Result<bool, Sdl3BackendError> {
                run_backend_entry(&self.runtime, imgui, || process_callback_owned_event(event))
            }

            /// Process a raw SDL3 event with the captured ImGui context.
            ///
            /// Prefer [`Self::process_event`] for event-pump loops and
            /// [`Self::process_callback_event`] with [`Sdl3CallbackEventHandoff`] for SDL callback
            /// mode.
            ///
            /// # Safety
            ///
            /// `event` must contain the active SDL union variant named by its type. Every pointer
            /// reachable from that variant must remain valid for the duration of this call. The
            /// call must execute on the SDL thread, and `event` must belong to the SDL runtime used
            /// by this backend.
            pub unsafe fn process_raw_event(
                &mut self,
                imgui: &mut Context,
                event: &SDL_Event,
            ) -> Result<bool, Sdl3BackendError> {
                run_backend_entry(&self.runtime, imgui, || unsafe {
                    process_raw_sys_event(event)
                })
            }
        }
    };
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn ensure_matching_sdl_renderer(
    expected: *mut SDL_Renderer,
    actual: *mut SDL_Renderer,
) -> Result<(), Sdl3BackendError> {
    if expected != actual {
        return Err(Sdl3BackendError::RendererMismatch);
    }
    Ok(())
}

#[cfg(all(
    test,
    any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    )
))]
mod renderer_contract_tests {
    use super::*;

    #[test]
    fn pending_frame_from_foreign_context_is_rejected_before_renderer_work() {
        let _guard = crate::tests::test_guard();
        let owner = Context::create();
        let owner_binding = owner.binding();
        let _owner = owner.suspend_or_panic();
        let mut foreign = Context::create();
        foreign.io_mut().set_display_size([128.0, 128.0]);
        foreign.io_mut().set_delta_time(1.0 / 60.0);
        foreign
            .io_mut()
            .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
        let consumer = foreign.create_synchronous_renderer_consumer().unwrap();
        let frame = foreign.begin_frame().render(&consumer);

        let error = ensure_matching_pending_frame(&owner_binding, &frame).unwrap_err();

        assert!(matches!(
            error,
            Sdl3BackendError::ContextMismatch { expected, actual }
                if expected == owner_binding.id() && actual == frame.context_id()
        ));
    }

    #[cfg(any(feature = "opengl3-renderer", feature = "sdlgpu3-renderer"))]
    #[test]
    fn reconciled_frame_from_foreign_context_is_rejected_before_renderer_work() {
        let _guard = crate::tests::test_guard();
        let owner = Context::create();
        let owner_binding = owner.binding();
        let _owner = owner.suspend_or_panic();
        let mut foreign = Context::create();
        foreign.io_mut().set_display_size([128.0, 128.0]);
        foreign.io_mut().set_delta_time(1.0 / 60.0);
        foreign
            .io_mut()
            .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
        let consumer = foreign.create_synchronous_renderer_consumer().unwrap();
        let pending = foreign.begin_frame().render(&consumer);
        let feedback = pending
            .texture_requests()
            .iter()
            .map(dear_imgui_rs::render::TextureRequest::retry)
            .collect::<Vec<_>>();
        let frame = pending.reconcile_texture_feedback(feedback).unwrap();

        let error = ensure_matching_reconciled_frame(&owner_binding, &frame).unwrap_err();

        assert!(matches!(
            error,
            Sdl3BackendError::ContextMismatch { expected, actual }
                if expected == owner_binding.id() && actual == frame.context_id()
        ));
    }

    #[cfg(feature = "sdlrenderer3-renderer")]
    #[test]
    fn foreign_sdl_renderer_is_rejected_before_texture_or_draw_work() {
        let _guard = crate::tests::test_guard();
        let expected = 0x101_usize as *mut SDL_Renderer;
        let foreign = 0x102_usize as *mut SDL_Renderer;

        assert!(ensure_matching_sdl_renderer(expected, expected).is_ok());
        assert!(matches!(
            ensure_matching_sdl_renderer(expected, foreign),
            Err(Sdl3BackendError::RendererMismatch)
        ));
    }
}

#[cfg(test)]
mod input_control_surface_tests {
    use super::*;

    macro_rules! assert_input_controls {
        ($backend:ty) => {{
            let _ = <$backend>::set_gamepad_mode;
            let _ = <$backend>::set_gamepad_mode_manual;
            let _ = <$backend>::set_mouse_capture_mode;
            let _ = <$backend>::process_event;
            let _ = <$backend>::process_callback_event;
            let _ = <$backend>::process_raw_event;
        }};
    }

    #[test]
    fn platform_owner_exposes_the_complete_input_control_surface() {
        assert_input_controls!(Sdl3PlatformBackend);
        let _ = Sdl3PlatformBackend::drain_faults;
    }

    #[cfg(feature = "opengl3-renderer")]
    #[test]
    fn opengl_owner_exposes_the_complete_input_control_surface() {
        assert_input_controls!(Sdl3OpenGl3Backend);
    }

    #[cfg(feature = "sdlgpu3-renderer")]
    #[test]
    fn sdl_gpu_owner_exposes_the_complete_input_control_surface() {
        assert_input_controls!(SdlGpu3RendererBackend);
    }

    #[cfg(feature = "sdlrenderer3-renderer")]
    #[test]
    fn sdl_renderer_owner_exposes_the_complete_input_control_surface() {
        assert_input_controls!(SdlRenderer3Backend);
    }
}

/// RAII owner for the SDL3 platform backend without an official renderer shim.
///
/// This owner and its Context attachment share one runtime control. Explicit shutdown uses the
/// supplied Context to normalize any open frame. Dropping the owner defers native cleanup to the
/// Context attachment so Context-first teardown preserves the same ordering.
#[must_use = "call shutdown for reported cleanup errors, or retain the owner until Context teardown"]
#[derive(Debug)]
pub struct Sdl3PlatformBackend {
    runtime: RuntimeRegistration,
}

impl_sdl3_input_controls!(Sdl3PlatformBackend);
impl_sdl3_event_controls!(Sdl3PlatformBackend);

impl Sdl3PlatformBackend {
    /// Returns the Dear ImGui Context identity owned by this SDL3 platform backend.
    pub fn context_id(&self) -> ContextId {
        self.runtime.control().binding().id()
    }

    /// Returns and clears all pending SDL3 platform callback faults in observation order.
    ///
    /// First-party renderer routes already aggregate these failures into their frame result. This
    /// method is the advanced escape hatch for Glow and custom renderer routes, which must drain
    /// faults after every native platform-window pass.
    pub fn drain_faults(&self) -> Vec<Sdl3BackendError> {
        self.runtime.drain_faults()
    }

    /// Captures the exact platform generation used by a first-party renderer route.
    #[doc(hidden)]
    pub fn viewport_renderer_adapter(
        &self,
        imgui: &Context,
    ) -> Result<crate::Sdl3ViewportRendererAdapter, Sdl3BackendError> {
        self.runtime.viewport_renderer_adapter(imgui)
    }

    /// Lease the Vulkan surface capability owned by this exact SDL3 runtime generation.
    ///
    /// Only a backend initialized with [`Self::init_for_vulkan`] can issue this capability. While
    /// it is alive, SDL platform shutdown is rejected so a cached native callback can never outlive
    /// the `PlatformUserData` it interprets.
    #[cfg(feature = "multi-viewport")]
    pub fn acquire_vulkan_surface_provider(
        &self,
        imgui: &Context,
    ) -> Result<crate::Sdl3VulkanSurfaceProvider, Sdl3BackendError> {
        self.runtime.acquire_vulkan_surface_provider(imgui)
    }

    fn initialize(
        imgui: &mut Context,
        platform_graphics: PlatformGraphicsKind,
        gl_viewport_swap_interval: Sdl3OpenGlViewportSwapInterval,
        initialize_native: impl FnOnce(&mut Context) -> Result<(), Sdl3BackendError>,
    ) -> Result<Self, Sdl3BackendError> {
        let mut runtime = RuntimeRegistration::prepare_with_backend(
            imgui,
            None,
            None,
            None,
            platform_graphics,
            NativeRendererKind::None,
        )?;
        if platform_graphics == PlatformGraphicsKind::OpenGl {
            runtime.set_gl_viewport_swap_interval(gl_viewport_swap_interval);
        }
        if let Err(error) = initialize_native(imgui) {
            runtime.native_initialization_failed();
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self { runtime })
    }

    /// Initialize the SDL3 platform backend for non-OpenGL renderers.
    ///
    /// # Safety
    ///
    /// `window` must remain alive at the same native allocation until explicit shutdown succeeds
    /// or `imgui` finishes attachment teardown. Dropping this owner alone does not end that
    /// requirement because cleanup is deferred to the Context.
    pub unsafe fn init_for_other(
        imgui: &mut Context,
        window: &Window,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(
            imgui,
            PlatformGraphicsKind::Other,
            Sdl3OpenGlViewportSwapInterval::Immediate,
            |imgui| init_for_other(imgui, window),
        )
    }

    /// Initialize the SDL3 platform backend for an OpenGL context without
    /// initializing the official OpenGL3 renderer.
    ///
    /// # Safety
    ///
    /// `window` and `gl_context` must remain valid and associated with each other until explicit
    /// shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner alone does
    /// not end their lifetime requirement. The caller must also make `gl_context` current on
    /// `window` before every renderer operation that touches OpenGL state, including rendering,
    /// device-object changes, and shutdown.
    pub unsafe fn init_platform_for_opengl(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
    ) -> Result<Self, Sdl3BackendError> {
        unsafe {
            Self::init_platform_for_opengl_with_viewport_swap_interval(
                imgui,
                window,
                gl_context,
                Sdl3OpenGlViewportSwapInterval::default(),
            )
        }
    }

    /// Initialize the SDL3 OpenGL platform backend with an explicit secondary-viewport
    /// swap-interval policy.
    ///
    /// # Safety
    ///
    /// `window` and `gl_context` must remain valid and associated with each other until explicit
    /// shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner alone does
    /// not end their lifetime requirement. The caller must also make `gl_context` current on
    /// `window` before every renderer operation that touches OpenGL state, including rendering,
    /// device-object changes, and shutdown.
    pub unsafe fn init_platform_for_opengl_with_viewport_swap_interval(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
        viewport_swap_interval: Sdl3OpenGlViewportSwapInterval,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(
            imgui,
            PlatformGraphicsKind::OpenGl,
            viewport_swap_interval,
            |imgui| init_platform_for_opengl(imgui, window, gl_context),
        )
    }

    /// Initialize the SDL3 platform backend for Vulkan renderers.
    ///
    /// # Safety
    ///
    /// `window` must remain alive at the same native allocation until explicit shutdown succeeds
    /// or `imgui` finishes attachment teardown.
    pub unsafe fn init_for_vulkan(
        imgui: &mut Context,
        window: &Window,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(
            imgui,
            PlatformGraphicsKind::Vulkan,
            Sdl3OpenGlViewportSwapInterval::Immediate,
            |imgui| init_for_vulkan(imgui, window),
        )
    }

    /// Initialize the SDL3 platform backend for Direct3D renderers.
    ///
    /// # Safety
    ///
    /// `window` must remain alive at the same native allocation until explicit shutdown succeeds
    /// or `imgui` finishes attachment teardown.
    #[cfg(target_os = "windows")]
    pub unsafe fn init_for_d3d(
        imgui: &mut Context,
        window: &Window,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(
            imgui,
            PlatformGraphicsKind::Other,
            Sdl3OpenGlViewportSwapInterval::Immediate,
            |imgui| init_for_d3d(imgui, window),
        )
    }

    /// Initialize the SDL3 platform backend for Metal renderers.
    ///
    /// # Safety
    ///
    /// `window` must remain alive at the same native allocation until explicit shutdown succeeds
    /// or `imgui` finishes attachment teardown.
    pub unsafe fn init_for_metal(
        imgui: &mut Context,
        window: &Window,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(
            imgui,
            PlatformGraphicsKind::Other,
            Sdl3OpenGlViewportSwapInterval::Immediate,
            |imgui| init_for_metal(imgui, window),
        )
    }

    /// Initialize the SDL3 platform backend for SDL_Renderer-based renderers.
    ///
    /// # Safety
    ///
    /// The caller must provide a valid `SDL_Renderer` pointer associated with `window`. Both
    /// native objects must remain alive until explicit shutdown succeeds or `imgui` finishes
    /// attachment teardown; dropping this owner alone does not end that requirement.
    pub unsafe fn init_for_sdl_renderer(
        imgui: &mut Context,
        window: &Window,
        renderer: *mut sdl3_sys::render::SDL_Renderer,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(
            imgui,
            PlatformGraphicsKind::Other,
            Sdl3OpenGlViewportSwapInterval::Immediate,
            |imgui| unsafe { init_for_sdl_renderer(imgui, window, renderer) },
        )
    }

    /// Initialize the SDL3 platform backend for SDL GPU renderers.
    ///
    /// # Safety
    ///
    /// `window` must remain alive at the same native allocation until explicit shutdown succeeds
    /// or `imgui` finishes attachment teardown.
    pub unsafe fn init_for_sdl_gpu(
        imgui: &mut Context,
        window: &Window,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(
            imgui,
            PlatformGraphicsKind::Other,
            Sdl3OpenGlViewportSwapInterval::Immediate,
            |imgui| init_for_sdl_gpu(imgui, window),
        )
    }

    /// Begin a new SDL3 platform frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_backend_entry(&self.runtime, imgui, || {
            sdl3_new_frame_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Shut down the SDL3 platform backend.
    ///
    /// This operation is idempotent. Drop defers native cleanup to Context teardown because it
    /// cannot safely normalize an open frame without the mutable Context. Any active renderer
    /// attachment rejects shutdown before the frame or native state changes; shut down that
    /// renderer and retry.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_platform(imgui)
    }
}

/// RAII owner for SDL3 platform + official OpenGL3 renderer backends.
#[cfg(feature = "opengl3-renderer")]
#[must_use = "call shutdown for reported cleanup errors, or retain the owner until Context teardown"]
#[derive(Debug)]
pub struct Sdl3OpenGl3Backend {
    runtime: RuntimeRegistration,
}

#[cfg(feature = "opengl3-renderer")]
impl_sdl3_input_controls!(Sdl3OpenGl3Backend);
#[cfg(feature = "opengl3-renderer")]
impl_sdl3_event_controls!(Sdl3OpenGl3Backend);

#[cfg(feature = "opengl3-renderer")]
impl Sdl3OpenGl3Backend {
    fn from_initialized_context(runtime: RuntimeRegistration) -> Self {
        Self { runtime }
    }

    /// Initialize the SDL3 platform backend and the official OpenGL3 renderer.
    ///
    /// # Safety
    ///
    /// `window` and `gl_context` must remain valid and associated with each other until explicit
    /// shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner alone does
    /// not end their lifetime requirement. The caller must also make `gl_context` current on
    /// `window` before every renderer operation that touches OpenGL state, including rendering,
    /// device-object changes, and shutdown.
    pub unsafe fn init(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
        glsl_version: &str,
    ) -> Result<Self, Sdl3BackendError> {
        unsafe {
            Self::init_with_viewport_swap_interval(
                imgui,
                window,
                gl_context,
                glsl_version,
                Sdl3OpenGlViewportSwapInterval::default(),
            )
        }
    }

    /// Initialize the SDL3 platform backend and official OpenGL3 renderer with an explicit
    /// secondary-viewport swap-interval policy.
    ///
    /// # Safety
    ///
    /// `window` and `gl_context` must remain valid and associated with each other until explicit
    /// shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner alone does
    /// not end their lifetime requirement. The caller must also make `gl_context` current on
    /// `window` before every renderer operation that touches OpenGL state, including rendering,
    /// device-object changes, and shutdown.
    pub unsafe fn init_with_viewport_swap_interval(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
        glsl_version: &str,
        viewport_swap_interval: Sdl3OpenGlViewportSwapInterval,
    ) -> Result<Self, Sdl3BackendError> {
        let mut runtime = prepare_renderer_runtime(
            imgui,
            shutdown_opengl3_renderer_impl,
            destroy_opengl3_device_objects,
            update_opengl3_texture,
            PlatformGraphicsKind::OpenGl,
            NativeRendererKind::OpenGl3,
        )?;
        runtime.set_gl_viewport_swap_interval(viewport_swap_interval);
        if let Err(error) = init_for_opengl(imgui, window, gl_context, glsl_version) {
            runtime.native_initialization_failed();
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime))
    }

    /// Initialize the SDL3 + OpenGL3 backends with the upstream default GLSL version.
    ///
    /// # Safety
    ///
    /// `window` and `gl_context` must remain valid and associated with each other until explicit
    /// shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner alone does
    /// not end their lifetime requirement.
    pub unsafe fn init_default(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
    ) -> Result<Self, Sdl3BackendError> {
        unsafe {
            Self::init_default_with_viewport_swap_interval(
                imgui,
                window,
                gl_context,
                Sdl3OpenGlViewportSwapInterval::default(),
            )
        }
    }

    /// Initialize the SDL3 + OpenGL3 backends with the upstream default GLSL version and an
    /// explicit secondary-viewport swap-interval policy.
    ///
    /// # Safety
    ///
    /// `window` and `gl_context` must remain valid and associated with each other until explicit
    /// shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner alone does
    /// not end their lifetime requirement.
    pub unsafe fn init_default_with_viewport_swap_interval(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
        viewport_swap_interval: Sdl3OpenGlViewportSwapInterval,
    ) -> Result<Self, Sdl3BackendError> {
        let mut runtime = prepare_renderer_runtime(
            imgui,
            shutdown_opengl3_renderer_impl,
            destroy_opengl3_device_objects,
            update_opengl3_texture,
            PlatformGraphicsKind::OpenGl,
            NativeRendererKind::OpenGl3,
        )?;
        runtime.set_gl_viewport_swap_interval(viewport_swap_interval);
        if let Err(error) = init_for_opengl_default(imgui, window, gl_context) {
            runtime.native_initialization_failed();
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime))
    }

    /// Begin a new SDL3 + OpenGL3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_backend_entry(&self.runtime, imgui, || {
            new_frame_opengl3_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Captures an open frame, reconciles managed textures, and completes native viewports.
    ///
    /// The route reads the backend capabilities advertised for this exact frame. When the SDL
    /// video driver declines native platform viewports, preparation degrades to the main viewport
    /// without calling the native platform-window pump. Existing callback faults prevent the pump
    /// and are returned as one ordered batch.
    ///
    /// The application owns the main OpenGL context, so `restore_main_context` is always attempted
    /// after the route attempt. Deferred faults are drained only after restoration. If capture,
    /// reconciliation, or the native pump panics, restoration and the final drain still run before
    /// the original panic resumes unwinding.
    pub fn prepare<'ctx, RestoreError>(
        &mut self,
        frame: FrameToken<'ctx>,
        restore_main_context: impl FnOnce() -> Result<(), RestoreError>,
    ) -> Result<Sdl3OpenGl3PreparedViewportFrame<'ctx>, Sdl3OpenGl3ViewportRouteError<RestoreError>>
    {
        ensure_matching_frame_token(self.runtime.control().binding(), &frame)
            .map_err(Sdl3OpenGl3ViewportRouteError::backend)?;
        let native_viewports_pumped = frame_has_native_viewport_route(&frame);
        let mut faults = self
            .runtime
            .drain_faults()
            .into_iter()
            .map(Sdl3OpenGl3ViewportRouteFault::Backend)
            .collect::<Vec<_>>();
        let mut frame = Some(frame);
        let mut reconciled = None;

        let route_attempt = catch_unwind(AssertUnwindSafe(|| {
            if faults.is_empty() {
                match capture_renderer_frame(
                    &self.runtime,
                    frame.take().expect("the route owns one open frame"),
                ) {
                    Ok(captured) => reconciled = Some(captured),
                    Err(error) => faults.push(Sdl3OpenGl3ViewportRouteFault::Backend(error)),
                }
            }

            if route_allows_native_viewport_pump(faults.is_empty(), native_viewports_pumped) {
                pump_platform_windows(
                    reconciled
                        .as_mut()
                        .expect("successful frame capture retains a reconciled frame"),
                    true,
                );
            }
        }));
        let (restore_result, deferred_faults) =
            finish_opengl_route_attempt(route_attempt, restore_main_context, || {
                self.runtime.drain_faults()
            });
        if let Err(error) = restore_result {
            faults.push(Sdl3OpenGl3ViewportRouteFault::MainContextRestore(error));
        }
        faults.extend(
            deferred_faults
                .into_iter()
                .map(Sdl3OpenGl3ViewportRouteFault::Backend),
        );

        if faults.is_empty() {
            Ok(Sdl3OpenGl3PreparedViewportFrame {
                frame: reconciled.expect("a successful route retains its reconciled frame"),
            })
        } else {
            Err(Sdl3OpenGl3ViewportRouteError::new(faults))
        }
    }

    /// Renders the main viewport from the capability returned by [`Self::prepare`].
    ///
    /// A raw [`ReconciledFrame`] cannot enter this method, so safe code cannot render the main
    /// viewport without first completing the same-generation SDL platform transaction.
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::render::ReconciledFrame;
    /// use dear_imgui_sdl3::Sdl3OpenGl3Backend;
    ///
    /// fn bypass(backend: &mut Sdl3OpenGl3Backend, frame: ReconciledFrame<'_>) {
    ///     let _ = backend.render_main(frame);
    /// }
    /// ```
    pub fn render_main(
        &mut self,
        prepared: Sdl3OpenGl3PreparedViewportFrame<'_>,
    ) -> Result<(), Sdl3ViewportRouteError> {
        let frame = prepared.frame;
        ensure_matching_reconciled_frame(self.runtime.control().binding(), &frame)
            .map_err(Sdl3ViewportRouteError::single)?;
        let mut faults = self.runtime.drain_faults();
        if faults.is_empty() {
            let render_result = (|| {
                let entry = self.runtime.control().enter_bound()?;
                self.runtime
                    .control()
                    .binding()
                    .try_with_bound_context(|| {
                        assert_current_draw_data(
                            frame.draw_data(),
                            "Sdl3OpenGl3Backend::render_main()",
                        );
                        render_opengl3_impl(frame.draw_data());
                    })?;
                entry.finish()
            })();
            if let Err(error) = render_result {
                faults.push(error);
            }
        }
        faults.extend(self.runtime.drain_faults());

        if faults.is_empty() {
            Ok(())
        } else {
            Err(Sdl3ViewportRouteError::new(faults))
        }
    }

    /// Create OpenGL3 renderer device objects.
    pub fn create_device_objects(&mut self, imgui: &mut Context) -> Result<bool, Sdl3BackendError> {
        run_backend_entry(&self.runtime, imgui, create_opengl3_device_objects)
    }

    /// Destroy OpenGL3 renderer device objects.
    ///
    /// This validates the Context-bound synchronous consumer before native destruction begins.
    /// The reset is committed only after every renderer-owned texture has been released.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime
            .reset_renderer_device_objects(imgui, destroy_opengl3_device_objects)
    }

    /// Shut down the official OpenGL3 renderer and SDL3 platform backend.
    ///
    /// Shutdown validates the Context-bound synchronous consumer before changing callbacks or
    /// releasing native resources.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_renderer(imgui)
    }
}

/// OpenGL main-viewport frame prepared by the owning SDL3 platform transaction.
///
/// This capability is move-only and cannot be constructed outside the backend. Consuming it with
/// [`Sdl3OpenGl3Backend::render_main`] proves that texture reconciliation, capability-aware native
/// viewport dispatch, main-context restoration, and deferred-fault collection completed together.
///
/// ```compile_fail
/// use dear_imgui_sdl3::Sdl3OpenGl3PreparedViewportFrame;
///
/// fn duplicate(frame: Sdl3OpenGl3PreparedViewportFrame<'_>) {
///     let moved = frame;
///     drop(frame);
///     drop(moved);
/// }
/// ```
#[cfg(feature = "opengl3-renderer")]
#[must_use = "pass the prepared frame to Sdl3OpenGl3Backend::render_main"]
pub struct Sdl3OpenGl3PreparedViewportFrame<'ctx> {
    frame: ReconciledFrame<'ctx>,
}

#[cfg(feature = "opengl3-renderer")]
impl Sdl3OpenGl3PreparedViewportFrame<'_> {
    /// Returns the Context identity carried by this prepared frame.
    #[must_use]
    pub fn context_id(&self) -> ContextId {
        self.frame.context_id()
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
#[must_use = "call shutdown for reported cleanup errors, or retain the owner until Context teardown"]
#[derive(Debug)]
pub struct SdlGpu3RendererBackend {
    runtime: RuntimeRegistration,
}

#[cfg(feature = "sdlgpu3-renderer")]
impl_sdl3_input_controls!(SdlGpu3RendererBackend);
#[cfg(feature = "sdlgpu3-renderer")]
impl_sdl3_event_controls!(SdlGpu3RendererBackend);

#[cfg(feature = "sdlgpu3-renderer")]
impl SdlGpu3RendererBackend {
    fn from_initialized_context(runtime: RuntimeRegistration) -> Self {
        Self { runtime }
    }

    /// Initialize the SDL3 platform backend and the official SDLGPU3 renderer.
    ///
    /// # Safety
    ///
    /// `window` and every native GPU object referenced by `info` must remain valid until explicit
    /// shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner alone does
    /// not end their lifetime requirement.
    pub unsafe fn init(
        imgui: &mut Context,
        window: &Window,
        info: SdlGpu3InitInfo<'_>,
    ) -> Result<Self, Sdl3BackendError> {
        let mut runtime = prepare_renderer_runtime(
            imgui,
            shutdown_sdlgpu3_renderer_impl,
            destroy_sdlgpu3_device_objects,
            update_sdlgpu3_texture,
            PlatformGraphicsKind::Other,
            NativeRendererKind::SdlGpu3,
        )?;
        if let Err(error) = init_for_sdlgpu3(imgui, window, info) {
            runtime.native_initialization_failed();
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime))
    }

    /// Begin a new SDL3 + SDLGPU3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_backend_entry(&self.runtime, imgui, || {
            new_frame_sdlgpu3_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Captures an open frame, reconciles managed textures, and completes native viewports.
    ///
    /// Call this before acquiring the main swapchain image. Secondary viewports remain independent
    /// of main-surface availability, while targets that do not advertise the complete native
    /// viewport capability naturally degrade to main-window rendering. Existing faults prevent
    /// the native pump, and all deferred failures are returned in FIFO order.
    pub fn prepare<'ctx>(
        &mut self,
        frame: FrameToken<'ctx>,
    ) -> Result<SdlGpu3PreparedViewportFrame<'ctx>, Sdl3ViewportRouteError> {
        ensure_matching_frame_token(self.runtime.control().binding(), &frame)
            .map_err(Sdl3ViewportRouteError::single)?;
        let native_viewports_pumped = frame_has_native_viewport_route(&frame);
        let mut faults = self.runtime.drain_faults();
        let mut frame = Some(frame);
        let mut reconciled = None;

        let route_attempt = catch_unwind(AssertUnwindSafe(|| {
            if faults.is_empty() {
                match capture_renderer_frame(
                    &self.runtime,
                    frame.take().expect("the route owns one open frame"),
                ) {
                    Ok(captured) => reconciled = Some(captured),
                    Err(error) => faults.push(error),
                }
            }

            if route_allows_native_viewport_pump(faults.is_empty(), native_viewports_pumped) {
                pump_platform_windows(
                    reconciled
                        .as_mut()
                        .expect("successful frame capture retains a reconciled frame"),
                    true,
                );
            }
        }));
        let deferred_faults = self.runtime.drain_faults();
        if let Err(payload) = route_attempt {
            resume_unwind(payload);
        }
        faults.extend(deferred_faults);

        if faults.is_empty() {
            Ok(SdlGpu3PreparedViewportFrame {
                frame: reconciled.expect("a successful route retains its reconciled frame"),
            })
        } else {
            Err(Sdl3ViewportRouteError::new(faults))
        }
    }

    /// Records the SDL GPU preparation commands for the main viewport.
    ///
    /// The returned capability keeps the renderer, Context frame, and command buffer transaction
    /// together until [`SdlGpu3RenderPassFrame::render_main`] consumes it inside the active pass.
    /// A raw [`ReconciledFrame`] cannot enter this method.
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::render::ReconciledFrame;
    /// use dear_imgui_sdl3::SdlGpu3RendererBackend;
    /// use sdl3::gpu::CommandBuffer;
    ///
    /// unsafe fn bypass<'a>(
    ///     backend: &'a mut SdlGpu3RendererBackend,
    ///     frame: ReconciledFrame<'a>,
    ///     command_buffer: &'a CommandBuffer,
    /// ) {
    ///     let _ = backend.prepare_render_main(frame, command_buffer);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// `command_buffer` must come from the same live `SDL_GPUDevice` supplied at backend
    /// initialization and remain able to accept upload and render preparation commands. The SDL3
    /// Rust wrapper does not expose enough provenance to validate that native relation.
    pub unsafe fn prepare_render_main<'renderer, 'ctx, 'command>(
        &'renderer mut self,
        prepared: SdlGpu3PreparedViewportFrame<'ctx>,
        command_buffer: &'command CommandBuffer,
    ) -> Result<SdlGpu3RenderPassFrame<'renderer, 'ctx, 'command>, Sdl3ViewportRouteError> {
        let frame = prepared.frame;
        ensure_matching_reconciled_frame(self.runtime.control().binding(), &frame)
            .map_err(Sdl3ViewportRouteError::single)?;
        let mut faults = self.runtime.drain_faults();
        if faults.is_empty() {
            let prepare_result = (|| {
                let entry = self.runtime.control().enter_bound()?;
                self.runtime
                    .control()
                    .binding()
                    .try_with_bound_context(|| {
                        assert_current_draw_data(
                            frame.draw_data(),
                            "SdlGpu3RendererBackend::prepare_render_main()",
                        );
                        prepare_render_sdlgpu3_impl(frame.draw_data(), command_buffer);
                    })?;
                entry.finish()
            })();
            if let Err(error) = prepare_result {
                faults.push(error);
            }
        }
        faults.extend(self.runtime.drain_faults());

        if faults.is_empty() {
            Ok(SdlGpu3RenderPassFrame {
                backend: self,
                frame,
                command_buffer,
            })
        } else {
            Err(Sdl3ViewportRouteError::new(faults))
        }
    }

    /// Create SDL GPU3 renderer device objects.
    ///
    /// This first destroys the previous device objects and therefore requires the same idle
    /// renderer consumer as [`Self::destroy_device_objects`].
    pub fn create_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime
            .reset_renderer_device_objects(imgui, create_sdlgpu3_device_objects)
    }

    /// Destroy SDL GPU3 renderer device objects.
    ///
    /// This validates the Context-bound synchronous consumer before native destruction begins.
    /// The reset is committed only after every renderer-owned texture has been released.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime
            .reset_renderer_device_objects(imgui, destroy_sdlgpu3_device_objects)
    }

    /// Shut down the official SDLGPU3 renderer and SDL3 platform backend.
    ///
    /// Shutdown validates the Context-bound synchronous consumer before changing callbacks or
    /// releasing native resources.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_renderer(imgui)
    }
}

/// SDL GPU main-viewport frame prepared after the owning native viewport transaction.
///
/// This capability remains independent of the application's main swapchain. The application may
/// acquire a surface and pass it to [`SdlGpu3RendererBackend::prepare_render_main`], or explicitly
/// consume the frame with [`Self::skip_main`] when no main image is available.
///
/// ```compile_fail
/// use dear_imgui_sdl3::SdlGpu3PreparedViewportFrame;
///
/// fn duplicate(frame: SdlGpu3PreparedViewportFrame<'_>) {
///     let moved = frame;
///     frame.skip_main();
///     moved.skip_main();
/// }
/// ```
#[cfg(feature = "sdlgpu3-renderer")]
#[must_use = "render or explicitly skip the prepared main viewport frame"]
pub struct SdlGpu3PreparedViewportFrame<'ctx> {
    frame: ReconciledFrame<'ctx>,
}

#[cfg(feature = "sdlgpu3-renderer")]
impl SdlGpu3PreparedViewportFrame<'_> {
    /// Returns the Context identity carried by this prepared frame.
    #[must_use]
    pub fn context_id(&self) -> ContextId {
        self.frame.context_id()
    }

    /// Returns the logical main-viewport display size reported by Dear ImGui.
    #[must_use]
    pub fn main_display_size(&self) -> [f32; 2] {
        self.frame.draw_data().display_size()
    }

    /// Returns whether the main viewport has a positive logical draw area.
    #[must_use]
    pub fn main_is_drawable(&self) -> bool {
        self.main_display_size().into_iter().all(|size| size > 0.0)
    }

    /// Completes the frame without drawing the main viewport.
    ///
    /// Secondary native viewports and managed-texture reconciliation have already completed, so a
    /// missing or minimized main swapchain does not invalidate their work.
    pub fn skip_main(self) {}
}

/// SDL GPU frame prepared for one application-owned command buffer and render pass.
#[cfg(feature = "sdlgpu3-renderer")]
#[must_use = "call render_main while the SDL GPU render pass is active"]
pub struct SdlGpu3RenderPassFrame<'renderer, 'ctx, 'command> {
    backend: &'renderer mut SdlGpu3RendererBackend,
    frame: ReconciledFrame<'ctx>,
    command_buffer: &'command CommandBuffer,
}

#[cfg(feature = "sdlgpu3-renderer")]
impl SdlGpu3RenderPassFrame<'_, '_, '_> {
    /// Submits the prepared Dear ImGui draw data into the active SDL GPU render pass.
    ///
    /// # Safety
    ///
    /// `render_pass` must be active on `self`'s command buffer, originate from the same live
    /// `SDL_GPUDevice` used to initialize the backend, and have attachments compatible with the
    /// backend's configured color format and sample count.
    pub unsafe fn render_main(
        self,
        render_pass: &mut RenderPass,
    ) -> Result<(), Sdl3ViewportRouteError> {
        let mut faults = self.backend.runtime.drain_faults();
        if faults.is_empty() {
            let render_result = (|| {
                let entry = self.backend.runtime.control().enter_bound()?;
                self.backend
                    .runtime
                    .control()
                    .binding()
                    .try_with_bound_context(|| {
                        assert_current_draw_data(
                            self.frame.draw_data(),
                            "SdlGpu3RenderPassFrame::render_main()",
                        );
                        render_sdlgpu3_impl(
                            self.frame.draw_data(),
                            self.command_buffer,
                            render_pass,
                        );
                    })?;
                entry.finish()
            })();
            if let Err(error) = render_result {
                faults.push(error);
            }
        }
        faults.extend(self.backend.runtime.drain_faults());

        if faults.is_empty() {
            Ok(())
        } else {
            Err(Sdl3ViewportRouteError::new(faults))
        }
    }
}

/// RAII owner for SDL3 platform + official SDLRenderer3 renderer backends.
#[cfg(feature = "sdlrenderer3-renderer")]
#[must_use = "call shutdown for reported cleanup errors, or retain the owner until Context teardown"]
#[derive(Debug)]
pub struct SdlRenderer3Backend {
    runtime: RuntimeRegistration,
    renderer: *mut sdl3_sys::render::SDL_Renderer,
}

#[cfg(feature = "sdlrenderer3-renderer")]
impl_sdl3_input_controls!(SdlRenderer3Backend);
#[cfg(feature = "sdlrenderer3-renderer")]
impl_sdl3_event_controls!(SdlRenderer3Backend);

#[cfg(feature = "sdlrenderer3-renderer")]
impl SdlRenderer3Backend {
    fn from_initialized_context(
        runtime: RuntimeRegistration,
        renderer: *mut sdl3_sys::render::SDL_Renderer,
    ) -> Self {
        Self { runtime, renderer }
    }

    /// Synchronous consumer owned by this renderer.
    ///
    /// Pass it to [`Context::render`] to produce the [`PendingFrame`] consumed by this backend.
    ///
    /// # Panics
    ///
    /// Panics after this renderer has completed explicit shutdown.
    #[must_use]
    pub fn consumer(&self) -> &SynchronousRendererConsumer {
        self.runtime.renderer_consumer()
    }

    /// Initialize the SDL3 platform backend and the official SDLRenderer3 renderer.
    ///
    /// # Safety
    ///
    /// `canvas`, its window, and their associated native `SDL_Renderer` must remain valid until
    /// explicit shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner
    /// alone does not end their lifetime requirement.
    pub unsafe fn init(
        imgui: &mut Context,
        canvas: &WindowCanvas,
    ) -> Result<Self, Sdl3BackendError> {
        let mut runtime = prepare_renderer_runtime(
            imgui,
            shutdown_sdlrenderer3_renderer_impl,
            destroy_sdlrenderer3_device_objects,
            update_sdlrenderer3_texture,
            PlatformGraphicsKind::Other,
            NativeRendererKind::SdlRenderer3,
        )?;
        if let Err(error) = init_for_canvas(imgui, canvas) {
            runtime.native_initialization_failed();
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime, canvas.raw()))
    }

    /// Begin a new SDL3 + SDLRenderer3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_backend_entry(&self.runtime, imgui, || {
            new_frame_sdlrenderer3_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Consume and render one synchronous frame using the official SDLRenderer3 renderer.
    pub fn render(
        &mut self,
        frame: PendingFrame<'_>,
        canvas: &WindowCanvas,
    ) -> Result<(), Sdl3BackendError> {
        ensure_matching_sdl_renderer(self.renderer, canvas.raw())?;
        let frame = reconcile_renderer_frame(&self.runtime, frame)?;
        let entry = self.runtime.control().enter_bound()?;
        self.runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(frame.draw_data(), "SdlRenderer3Backend::render()");
                render_sdlrenderer3_impl(frame.draw_data(), canvas);
            })?;
        entry.finish()?;
        Ok(())
    }

    /// Destroy SDLRenderer3 renderer device objects.
    ///
    /// This validates the Context-bound synchronous consumer before native destruction begins.
    /// The reset is committed only after every renderer-owned texture has been released.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime
            .reset_renderer_device_objects(imgui, destroy_sdlrenderer3_device_objects)
    }

    /// Shut down the official SDLRenderer3 renderer and SDL3 platform backend.
    ///
    /// Shutdown validates the Context-bound synchronous consumer before changing callbacks or
    /// releasing native resources.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_renderer(imgui)
    }
}

#[cfg(feature = "opengl3-renderer")]
fn render_opengl3_impl(draw_data: &DrawData) {
    unsafe {
        let raw = draw_data as *const DrawData as *mut sys::ImDrawData;
        opengl3_backend::dear_imgui_backend_opengl3_render_draw_data(raw);
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
fn render_sdlgpu3_impl(
    draw_data: &DrawData,
    command_buffer: &CommandBuffer,
    render_pass: &RenderPass,
) {
    unsafe {
        let raw = draw_data as *const DrawData as *mut sys::ImDrawData;
        ffi::dear_imgui_sdl3_backend_sdlgpu3_render_draw_data(
            raw,
            command_buffer.raw(),
            render_pass.raw(),
            std::ptr::null_mut(),
        );
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
fn prepare_render_sdlgpu3_impl(draw_data: &DrawData, command_buffer: &CommandBuffer) {
    unsafe {
        let raw = draw_data as *const DrawData as *mut sys::ImDrawData;
        ffi::dear_imgui_sdl3_backend_sdlgpu3_prepare_draw_data(raw, command_buffer.raw());
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn render_sdlrenderer3_impl(draw_data: &DrawData, canvas: &WindowCanvas) {
    let sdl_renderer = canvas.raw();
    unsafe {
        let raw = draw_data as *const DrawData as *mut sys::ImDrawData;
        ffi::dear_imgui_sdl3_backend_sdlrenderer3_render_draw_data(raw, sdl_renderer);
    }
}

#[cfg(feature = "opengl3-renderer")]
fn update_opengl3_texture(tex: &mut dear_imgui_rs::TextureData) {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_update_texture(tex.as_raw_mut());
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
fn update_sdlgpu3_texture(tex: &mut dear_imgui_rs::TextureData) {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlgpu3_update_texture(tex.as_raw_mut());
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
fn create_sdlgpu3_device_objects() {
    unsafe { ffi::dear_imgui_sdl3_backend_sdlgpu3_create_device_objects() }
}

#[cfg(feature = "sdlgpu3-renderer")]
fn destroy_sdlgpu3_device_objects() {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlgpu3_destroy_device_objects();
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn update_sdlrenderer3_texture(tex: &mut dear_imgui_rs::TextureData) {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlrenderer3_update_texture(tex.as_raw_mut());
    }
}

#[cfg(feature = "opengl3-renderer")]
fn create_opengl3_device_objects() -> bool {
    unsafe { opengl3_backend::dear_imgui_backend_opengl3_create_device_objects() }
}

#[cfg(feature = "opengl3-renderer")]
fn destroy_opengl3_device_objects() {
    unsafe {
        opengl3_backend::dear_imgui_backend_opengl3_destroy_device_objects();
    }
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn destroy_sdlrenderer3_device_objects() {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlrenderer3_destroy_device_objects();
    }
}
