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
use dear_imgui_rs::render::RendererConsumer;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::sys;
use sdl3::event::Event;

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn prepare_renderer_consumer(imgui: &mut Context) -> Result<RendererConsumer, Sdl3BackendError> {
    let consumer = imgui.create_renderer_consumer()?;
    // A new consumer has not submitted an epoch or produced a Context-managed texture mapping.
    // The empty transaction only clears bindings from an already-released predecessor before this
    // renderer can publish new texture requests.
    let reset = imgui.prepare_renderer_texture_reset(&consumer)?;
    let _ = reset.commit();
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
            runtime.control().install_renderer_consumer(consumer);
            Ok(runtime)
        }
        Err(error) => {
            runtime.native_initialization_failed();
            Err(error)
        }
    }
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn run_renderer_entry<R>(
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
fn ensure_matching_rendered_frame(
    context: &ContextBinding,
    frame: &RenderedFrame<'_>,
) -> Result<(), Sdl3BackendError> {
    let expected = context.id();
    let actual = frame.context_id();
    if expected != actual {
        return Err(Sdl3BackendError::ContextMismatch { expected, actual });
    }
    Ok(())
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
    fn rendered_frame_from_foreign_context_is_rejected_before_renderer_work() {
        let _guard = crate::tests::test_guard();
        let owner = Context::create();
        let owner_binding = owner.binding();
        let _owner = owner.suspend();
        let mut foreign = Context::create();
        foreign.io_mut().set_display_size([128.0, 128.0]);
        foreign.io_mut().set_delta_time(1.0 / 60.0);
        foreign
            .io_mut()
            .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
        let _consumer = foreign.create_renderer_consumer().unwrap();
        let frame = foreign.begin_frame().render();

        let error = ensure_matching_rendered_frame(&owner_binding, &frame).unwrap_err();

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

impl Sdl3PlatformBackend {
    /// Returns the Dear ImGui Context identity owned by this SDL3 platform backend.
    pub fn context_id(&self) -> ContextId {
        self.runtime.control().binding().id()
    }

    /// Validates that this backend still owns the active SDL3 viewport platform contract.
    ///
    /// Renderer backends use this before interpreting `PlatformHandle` values as SDL window IDs.
    /// A backend that has shut down cannot validate even if another platform later attaches to
    /// the same Context.
    pub fn validate_renderer_owner(&self, imgui: &Context) -> Result<(), Sdl3BackendError> {
        self.runtime.control().ensure_entry(imgui)
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

    fn entry<R>(
        &self,
        imgui: &mut Context,
        callback: impl FnOnce() -> R,
    ) -> Result<R, Sdl3BackendError> {
        let entry = self.runtime.control().enter(imgui)?;
        let result = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(callback)?;
        entry.finish()?;
        Ok(result)
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
        self.entry(imgui, || {
            sdl3_new_frame_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Process an owned SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &Event,
    ) -> Result<bool, Sdl3BackendError> {
        self.entry(imgui, || process_owned_event(event))?
    }

    /// Process a raw SDL3 event with the captured ImGui context.
    ///
    /// Prefer [`Self::process_event`] for normal SDL event loops.
    ///
    /// # Safety
    ///
    /// `event` must contain the active SDL union variant named by its type. Every pointer reachable
    /// from that variant must remain valid for the duration of this call. The call must execute on
    /// the SDL thread, and `event` must belong to the SDL runtime used by this backend.
    pub unsafe fn process_raw_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        self.entry(imgui, || unsafe { process_raw_sys_event(event) })
    }

    /// Configure how the SDL3 backend handles gamepads for the captured context.
    pub fn set_gamepad_mode(
        &mut self,
        imgui: &mut Context,
        mode: GamepadMode,
    ) -> Result<(), Sdl3BackendError> {
        self.entry(imgui, || set_gamepad_mode(mode))
    }

    /// Configure SDL3 backend to use manual gamepad selection for the captured context.
    ///
    /// # Safety
    ///
    /// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
    /// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
    /// - The slice itself is only read during this call; the backend copies the pointers.
    pub unsafe fn set_gamepad_mode_manual(
        &mut self,
        imgui: &mut Context,
        gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
    ) -> Result<(), Sdl3BackendError> {
        self.entry(imgui, || unsafe { set_gamepad_mode_manual(gamepads) })
    }

    /// Returns and clears the oldest pending SDL3 platform callback fault.
    pub fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.runtime.poll_fault()
    }

    /// Begins a scoped trace for one OpenGL secondary-platform-window pass.
    ///
    /// Start the trace before Dear ImGui invokes its platform render callbacks. Restore the main
    /// OpenGL context before finishing the returned guard, then call [`Self::poll_fault`] before
    /// swapping the main window. The report contains only native context and swap transactions
    /// that completed without bridge faults.
    pub fn begin_opengl_viewport_frame_trace(
        &self,
    ) -> Result<Sdl3OpenGlViewportFrameTrace<'_>, Sdl3OpenGlViewportFrameTraceError> {
        self.runtime.begin_opengl_viewport_frame_trace()
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
        run_renderer_entry(&self.runtime, imgui, || {
            new_frame_opengl3_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Process an owned SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || process_owned_event(event))?
    }

    /// Process a raw SDL3 event with the captured ImGui context.
    ///
    /// Prefer [`Self::process_event`] for normal SDL event loops.
    ///
    /// # Safety
    ///
    /// `event` must contain the active SDL union variant named by its type. Every pointer reachable
    /// from that variant must remain valid for the duration of this call. The call must execute on
    /// the SDL thread, and `event` must belong to the SDL runtime used by this backend.
    pub unsafe fn process_raw_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || unsafe {
            process_raw_sys_event(event)
        })
    }

    /// Consume and render one synchronous frame using the official OpenGL3 renderer.
    pub fn render(&mut self, mut frame: RenderedFrame<'_>) -> Result<(), Sdl3BackendError> {
        ensure_matching_rendered_frame(self.runtime.control().binding(), &frame)?;
        let entry = self.runtime.control().enter_bound()?;
        let request_epoch = frame.epoch().map_or(0, |epoch| epoch.sequence());
        let feedback = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                self.runtime
                    .control()
                    .process_texture_requests(frame.texture_requests(), request_epoch)
            })??;
        let progress = frame.reconcile_texture_feedback(feedback)?;
        self.runtime
            .control()
            .mark_textures_reconciled(frame.texture_requests());
        self.runtime
            .control()
            .prune_destroyed_textures(progress.watermark());
        self.runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(frame.draw_data(), "Sdl3OpenGl3Backend::render()");
                render_opengl3_impl(frame.draw_data());
            })?;
        entry.finish()?;
        Ok(())
    }

    /// Configure how the SDL3 backend handles gamepads for the captured context.
    pub fn set_gamepad_mode(
        &mut self,
        imgui: &mut Context,
        mode: GamepadMode,
    ) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || set_gamepad_mode(mode))
    }

    /// Configure SDL3 backend to use manual gamepad selection for the captured context.
    ///
    /// # Safety
    ///
    /// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
    /// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
    /// - The slice itself is only read during this call; the backend copies the pointers.
    pub unsafe fn set_gamepad_mode_manual(
        &mut self,
        imgui: &mut Context,
        gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
    ) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || unsafe {
            set_gamepad_mode_manual(gamepads)
        })
    }

    /// Create OpenGL3 renderer device objects.
    pub fn create_device_objects(&mut self, imgui: &mut Context) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, create_opengl3_device_objects)
    }

    /// Destroy OpenGL3 renderer device objects.
    ///
    /// This validates an idle Context renderer consumer before native destruction begins. If a
    /// detached [`dear_imgui_rs::render::FrameSnapshot`] is still outstanding, this returns an
    /// error without destroying any native object. Commit or drop every snapshot, then retry.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime
            .destroy_renderer_device_objects(imgui, destroy_opengl3_device_objects)
    }

    /// Shut down the official OpenGL3 renderer and SDL3 platform backend.
    ///
    /// Shutdown validates the Context renderer consumer before changing callbacks or releasing
    /// native resources. An outstanding detached snapshot must be committed or dropped before
    /// retrying this operation.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_renderer(imgui)
    }

    /// Returns and clears the oldest pending SDL3 platform callback fault.
    pub fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.runtime.poll_fault()
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
#[must_use = "call shutdown for reported cleanup errors, or retain the owner until Context teardown"]
#[derive(Debug)]
pub struct SdlGpu3RendererBackend {
    runtime: RuntimeRegistration,
}

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

    /// Initialize the SDL3 platform backend and the official SDLGPU3 renderer.
    ///
    /// # Safety
    ///
    /// `window` and `gpu` must remain valid until explicit shutdown succeeds or `imgui` finishes
    /// attachment teardown. Dropping this owner alone does not end their lifetime requirement.
    pub unsafe fn init_default(
        imgui: &mut Context,
        window: &Window,
        gpu: &Device,
    ) -> Result<Self, Sdl3BackendError> {
        let mut runtime = prepare_renderer_runtime(
            imgui,
            shutdown_sdlgpu3_renderer_impl,
            destroy_sdlgpu3_device_objects,
            update_sdlgpu3_texture,
            PlatformGraphicsKind::Other,
            NativeRendererKind::SdlGpu3,
        )?;
        if let Err(error) = init_for_sdlgpu3_default(imgui, window, gpu) {
            runtime.native_initialization_failed();
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime))
    }

    /// Process an owned SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || process_owned_event(event))?
    }

    /// Process a raw SDL3 event with the captured ImGui context.
    ///
    /// Prefer [`Self::process_event`] for normal SDL event loops.
    ///
    /// # Safety
    ///
    /// `event` must contain the active SDL union variant named by its type. Every pointer reachable
    /// from that variant must remain valid for the duration of this call. The call must execute on
    /// the SDL thread, and `event` must belong to the SDL runtime used by this backend.
    pub unsafe fn process_raw_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || unsafe {
            process_raw_sys_event(event)
        })
    }

    /// Begin a new SDL3 + SDLGPU3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || {
            new_frame_sdlgpu3_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Process texture requests and prepare one synchronous frame for an SDL GPU render pass.
    ///
    /// # Safety
    ///
    /// `command_buffer` must come from the same live `SDL_GPUDevice` supplied at backend
    /// initialization and must remain in a state that permits upload and render preparation
    /// commands. The `sdl3` wrapper does not expose enough provenance to validate this relation.
    pub unsafe fn prepare_render<'renderer, 'ctx, 'command>(
        &'renderer mut self,
        mut frame: RenderedFrame<'ctx>,
        command_buffer: &'command CommandBuffer,
    ) -> Result<SdlGpu3PreparedFrame<'renderer, 'ctx, 'command>, Sdl3BackendError> {
        ensure_matching_rendered_frame(self.runtime.control().binding(), &frame)?;
        let entry = self.runtime.control().enter_bound()?;
        let request_epoch = frame.epoch().map_or(0, |epoch| epoch.sequence());
        let feedback = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                self.runtime
                    .control()
                    .process_texture_requests(frame.texture_requests(), request_epoch)
            })??;
        let progress = frame.reconcile_texture_feedback(feedback)?;
        self.runtime
            .control()
            .mark_textures_reconciled(frame.texture_requests());
        self.runtime
            .control()
            .prune_destroyed_textures(progress.watermark());
        self.runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(
                    frame.draw_data(),
                    "SdlGpu3RendererBackend::prepare_render()",
                );
                prepare_render_sdlgpu3_impl(frame.draw_data(), command_buffer);
            })?;
        entry.finish()?;
        Ok(SdlGpu3PreparedFrame {
            backend: self,
            frame,
            command_buffer,
        })
    }

    /// Create SDL GPU3 renderer device objects.
    ///
    /// This first destroys the previous device objects and therefore requires the same idle
    /// renderer consumer as [`Self::destroy_device_objects`].
    pub fn create_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.destroy_device_objects(imgui)?;
        run_renderer_entry(&self.runtime, imgui, create_sdlgpu3_device_objects)?;
        Ok(())
    }

    /// Destroy SDL GPU3 renderer device objects.
    ///
    /// This validates an idle Context renderer consumer before native destruction begins. If a
    /// detached [`dear_imgui_rs::render::FrameSnapshot`] is still outstanding, this returns an
    /// error without destroying any native object. Commit or drop every snapshot, then retry.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime
            .destroy_renderer_device_objects(imgui, destroy_sdlgpu3_device_objects)
    }

    /// Shut down the official SDLGPU3 renderer and SDL3 platform backend.
    ///
    /// Shutdown validates the Context renderer consumer before changing callbacks or releasing
    /// native resources. An outstanding detached snapshot must be committed or dropped before
    /// retrying this operation.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_renderer(imgui)
    }

    /// Returns and clears the oldest pending SDL3 platform callback fault.
    pub fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.runtime.poll_fault()
    }
}

/// Prepared SDLGPU3 frame that keeps both its renderer and Context render lease alive.
#[cfg(feature = "sdlgpu3-renderer")]
#[must_use = "call render() while the SDL GPU render pass is active"]
pub struct SdlGpu3PreparedFrame<'renderer, 'ctx, 'command> {
    backend: &'renderer mut SdlGpu3RendererBackend,
    frame: RenderedFrame<'ctx>,
    command_buffer: &'command CommandBuffer,
}

#[cfg(feature = "sdlgpu3-renderer")]
impl SdlGpu3PreparedFrame<'_, '_, '_> {
    /// Submit the prepared Dear ImGui draw data into the active SDL GPU render pass.
    ///
    /// # Safety
    ///
    /// `render_pass` must be active on `self`'s command buffer, originate from the same live
    /// `SDL_GPUDevice` used to initialize the backend, and have attachments compatible with the
    /// backend's configured color format and sample count.
    pub unsafe fn render(self, render_pass: &mut RenderPass) -> Result<(), Sdl3BackendError> {
        let entry = self.backend.runtime.control().enter_bound()?;
        self.backend
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(self.frame.draw_data(), "SdlGpu3PreparedFrame::render()");
                render_sdlgpu3_impl(self.frame.draw_data(), self.command_buffer, render_pass);
            })?;
        entry.finish()
    }
}

/// RAII owner for SDL3 platform + official SDLRenderer3 renderer backends.
#[cfg(feature = "sdlrenderer3-renderer")]
#[must_use = "call shutdown for reported cleanup errors, or retain the owner until Context teardown"]
#[derive(Debug)]
pub struct Sdl3RendererBackend {
    runtime: RuntimeRegistration,
    renderer: *mut sdl3_sys::render::SDL_Renderer,
}

#[cfg(feature = "sdlrenderer3-renderer")]
impl Sdl3RendererBackend {
    fn from_initialized_context(
        runtime: RuntimeRegistration,
        renderer: *mut sdl3_sys::render::SDL_Renderer,
    ) -> Self {
        Self { runtime, renderer }
    }

    /// Initialize the SDL3 platform backend and the official SDLRenderer3 renderer.
    ///
    /// # Safety
    ///
    /// `window`, `canvas`, and their associated native `SDL_Renderer` must remain valid until
    /// explicit shutdown succeeds or `imgui` finishes attachment teardown. Dropping this owner
    /// alone does not end their lifetime requirement.
    pub unsafe fn init(
        imgui: &mut Context,
        window: &Window,
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
        if let Err(error) = init_for_canvas(imgui, window, canvas) {
            runtime.native_initialization_failed();
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime, canvas.raw()))
    }

    /// Begin a new SDL3 + SDLRenderer3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || {
            new_frame_sdlrenderer3_impl();
            self.runtime.control().refresh_platform_monitors_bound();
        })
    }

    /// Process an owned SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || process_owned_event(event))?
    }

    /// Process a raw SDL3 event with the captured ImGui context.
    ///
    /// Prefer [`Self::process_event`] for normal SDL event loops.
    ///
    /// # Safety
    ///
    /// `event` must contain the active SDL union variant named by its type. Every pointer reachable
    /// from that variant must remain valid for the duration of this call. The call must execute on
    /// the SDL thread, and `event` must belong to the SDL runtime used by this backend.
    pub unsafe fn process_raw_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || unsafe {
            process_raw_sys_event(event)
        })
    }

    /// Consume and render one synchronous frame using the official SDLRenderer3 renderer.
    pub fn render(
        &mut self,
        mut frame: RenderedFrame<'_>,
        canvas: &WindowCanvas,
    ) -> Result<(), Sdl3BackendError> {
        ensure_matching_sdl_renderer(self.renderer, canvas.raw())?;
        ensure_matching_rendered_frame(self.runtime.control().binding(), &frame)?;
        let entry = self.runtime.control().enter_bound()?;
        let request_epoch = frame.epoch().map_or(0, |epoch| epoch.sequence());
        let feedback = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                self.runtime
                    .control()
                    .process_texture_requests(frame.texture_requests(), request_epoch)
            })??;
        let progress = frame.reconcile_texture_feedback(feedback)?;
        self.runtime
            .control()
            .mark_textures_reconciled(frame.texture_requests());
        self.runtime
            .control()
            .prune_destroyed_textures(progress.watermark());
        self.runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(frame.draw_data(), "Sdl3RendererBackend::render()");
                render_sdlrenderer3_impl(frame.draw_data(), canvas);
            })?;
        entry.finish()?;
        Ok(())
    }

    /// Configure how the SDL3 backend handles gamepads for the captured context.
    pub fn set_gamepad_mode(
        &mut self,
        imgui: &mut Context,
        mode: GamepadMode,
    ) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || set_gamepad_mode(mode))
    }

    /// Configure SDL3 backend to use manual gamepad selection for the captured context.
    ///
    /// # Safety
    ///
    /// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
    /// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
    /// - The slice itself is only read during this call; the backend copies the pointers.
    pub unsafe fn set_gamepad_mode_manual(
        &mut self,
        imgui: &mut Context,
        gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
    ) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || unsafe {
            set_gamepad_mode_manual(gamepads)
        })
    }

    /// Create SDLRenderer3 renderer device objects.
    pub fn create_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, create_sdlrenderer3_device_objects)
    }

    /// Destroy SDLRenderer3 renderer device objects.
    ///
    /// This validates an idle Context renderer consumer before native destruction begins. If a
    /// detached [`dear_imgui_rs::render::FrameSnapshot`] is still outstanding, this returns an
    /// error without destroying any native object. Commit or drop every snapshot, then retry.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime
            .destroy_renderer_device_objects(imgui, destroy_sdlrenderer3_device_objects)
    }

    /// Shut down the official SDLRenderer3 renderer and SDL3 platform backend.
    ///
    /// Shutdown validates the Context renderer consumer before changing callbacks or releasing
    /// native resources. An outstanding detached snapshot must be committed or dropped before
    /// retrying this operation.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_renderer(imgui)
    }

    /// Returns and clears the oldest pending SDL3 platform callback fault.
    pub fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.runtime.poll_fault()
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
fn create_sdlrenderer3_device_objects() {
    unsafe { ffi::dear_imgui_sdl3_backend_sdlrenderer3_create_device_objects() }
}

#[cfg(feature = "sdlrenderer3-renderer")]
fn destroy_sdlrenderer3_device_objects() {
    unsafe {
        ffi::dear_imgui_sdl3_backend_sdlrenderer3_destroy_device_objects();
    }
}
