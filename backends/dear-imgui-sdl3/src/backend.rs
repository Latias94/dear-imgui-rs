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
fn prepare_renderer_consumer(imgui: &mut Context) -> Result<RendererConsumer, Sdl3BackendError> {
    let consumer = imgui.create_renderer_consumer()?;
    imgui.reset_renderer_texture_bindings(&consumer)?;
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
) -> Result<(RuntimeRegistration, RendererConsumer), Sdl3BackendError> {
    let mut runtime = RuntimeRegistration::prepare(imgui, Some(renderer_shutdown))?;
    match prepare_renderer_consumer(imgui) {
        Ok(consumer) => Ok((runtime, consumer)),
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
    runtime.control().ensure_entry(imgui)?;
    let result = runtime
        .control()
        .binding()
        .try_with_bound_context(callback)?;
    runtime.control().finish_entry()?;
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
}

/// RAII owner for the SDL3 platform backend without an official renderer shim.
///
/// This owner and its Context attachment share one runtime control. Dropping either
/// side first runs the same idempotent teardown, and Context-first teardown releases
/// platform windows before the native Context is destroyed.
#[must_use = "dropping the backend owner shuts down the SDL3 platform backend"]
#[derive(Debug)]
pub struct Sdl3PlatformBackend {
    runtime: RuntimeRegistration,
}

impl Sdl3PlatformBackend {
    fn initialize(
        imgui: &mut Context,
        initialize_native: impl FnOnce(&mut Context) -> Result<(), Sdl3BackendError>,
    ) -> Result<Self, Sdl3BackendError> {
        let mut runtime = RuntimeRegistration::prepare(imgui, None)?;
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
        self.runtime.control().ensure_entry(imgui)?;
        let result = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(callback)?;
        self.runtime.control().finish_entry()?;
        Ok(result)
    }

    /// Initialize the SDL3 platform backend for non-OpenGL renderers.
    pub fn init_for_other(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        Self::initialize(imgui, |imgui| init_for_other(imgui, window))
    }

    /// Initialize the SDL3 platform backend for an OpenGL context without
    /// initializing the official OpenGL3 renderer.
    pub fn init_platform_for_opengl(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(imgui, |imgui| {
            init_platform_for_opengl(imgui, window, gl_context)
        })
    }

    /// Initialize the SDL3 platform backend for Vulkan renderers.
    pub fn init_for_vulkan(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        Self::initialize(imgui, |imgui| init_for_vulkan(imgui, window))
    }

    /// Initialize the SDL3 platform backend for Direct3D renderers.
    pub fn init_for_d3d(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        Self::initialize(imgui, |imgui| init_for_d3d(imgui, window))
    }

    /// Initialize the SDL3 platform backend for Metal renderers.
    pub fn init_for_metal(imgui: &mut Context, window: &Window) -> Result<Self, Sdl3BackendError> {
        Self::initialize(imgui, |imgui| init_for_metal(imgui, window))
    }

    /// Initialize the SDL3 platform backend for SDL_Renderer-based renderers.
    ///
    /// # Safety
    ///
    /// The caller must provide a valid `SDL_Renderer` pointer associated with `window`.
    pub unsafe fn init_for_sdl_renderer(
        imgui: &mut Context,
        window: &Window,
        renderer: *mut sdl3_sys::render::SDL_Renderer,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(imgui, |imgui| unsafe {
            init_for_sdl_renderer(imgui, window, renderer)
        })
    }

    /// Initialize the SDL3 platform backend for SDL GPU renderers.
    pub fn init_for_sdl_gpu(
        imgui: &mut Context,
        window: &Window,
    ) -> Result<Self, Sdl3BackendError> {
        Self::initialize(imgui, |imgui| init_for_sdl_gpu(imgui, window))
    }

    /// Begin a new SDL3 platform frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.entry(imgui, sdl3_new_frame_impl)
    }

    /// Process a single low-level SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        self.entry(imgui, || process_sys_event(event))
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

    /// Shut down the SDL3 platform backend.
    ///
    /// This operation is idempotent. Drop performs the same cleanup on a best-effort basis.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.runtime.shutdown_platform(imgui)
    }
}

/// RAII owner for SDL3 platform + official OpenGL3 renderer backends.
#[cfg(feature = "opengl3-renderer")]
#[must_use = "dropping the backend owner shuts down the SDL3 + OpenGL3 backends"]
#[derive(Debug)]
pub struct Sdl3OpenGl3Backend {
    runtime: RuntimeRegistration,
    consumer: Option<RendererConsumer>,
    textures: RendererTextureStore,
}

#[cfg(feature = "opengl3-renderer")]
impl Sdl3OpenGl3Backend {
    fn from_initialized_context(runtime: RuntimeRegistration, consumer: RendererConsumer) -> Self {
        Self {
            runtime,
            consumer: Some(consumer),
            textures: RendererTextureStore::default(),
        }
    }

    /// Initialize the SDL3 platform backend and the official OpenGL3 renderer.
    pub fn init(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
        glsl_version: &str,
    ) -> Result<Self, Sdl3BackendError> {
        let (mut runtime, consumer) =
            prepare_renderer_runtime(imgui, shutdown_opengl3_renderer_impl)?;
        if let Err(error) = init_for_opengl(imgui, window, gl_context, glsl_version) {
            runtime.native_initialization_failed();
            drop(consumer);
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime, consumer))
    }

    /// Initialize the SDL3 + OpenGL3 backends with the upstream default GLSL version.
    pub fn init_default(
        imgui: &mut Context,
        window: &Window,
        gl_context: &GLContext,
    ) -> Result<Self, Sdl3BackendError> {
        let (mut runtime, consumer) =
            prepare_renderer_runtime(imgui, shutdown_opengl3_renderer_impl)?;
        if let Err(error) = init_for_opengl_default(imgui, window, gl_context) {
            runtime.native_initialization_failed();
            drop(consumer);
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime, consumer))
    }

    /// Begin a new SDL3 + OpenGL3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, new_frame_opengl3_impl)
    }

    /// Process a single low-level SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || process_sys_event(event))
    }

    /// Consume and render one synchronous frame using the official OpenGL3 renderer.
    pub fn render(&mut self, mut frame: RenderedFrame<'_>) -> Result<(), Sdl3BackendError> {
        ensure_matching_rendered_frame(self.runtime.control().binding(), &frame)?;
        self.runtime.poll_fault()?;
        if self.runtime.control().state() != RuntimeState::Attached {
            return Err(Sdl3BackendError::RuntimeDetached);
        }
        let feedback = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                self.textures
                    .process_requests(frame.texture_requests(), update_opengl3_texture)
            })??;
        frame.reconcile_texture_feedback(feedback)?;
        self.runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(frame.draw_data(), "Sdl3OpenGl3Backend::render()");
                render_opengl3_impl(frame.draw_data());
            })?;
        self.runtime.control().finish_entry()?;
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
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, destroy_opengl3_device_objects)?;
        self.textures.forget_destroyed_by_upstream();
        imgui.reset_renderer_texture_bindings(self.consumer())?;
        Ok(())
    }

    /// Shut down the official OpenGL3 renderer and SDL3 platform backend.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        let result = self
            .runtime
            .shutdown_renderer(imgui, self.consumer.as_ref(), || {
                self.textures.forget_destroyed_by_upstream()
            });
        if matches!(
            self.runtime.control().state(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.consumer.take();
        }
        result
    }

    /// Returns and clears the oldest pending SDL3 platform callback fault.
    pub fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.runtime.poll_fault()
    }

    fn consumer(&self) -> &RendererConsumer {
        self.consumer
            .as_ref()
            .expect("SDL3 OpenGL3 renderer consumer was already detached")
    }
}

#[cfg(feature = "sdlgpu3-renderer")]
#[must_use = "dropping the backend owner shuts down the SDL3 + SDLGPU3 backends"]
#[derive(Debug)]
pub struct SdlGpu3RendererBackend {
    runtime: RuntimeRegistration,
    consumer: Option<RendererConsumer>,
    textures: RendererTextureStore,
}

#[cfg(feature = "sdlgpu3-renderer")]
impl SdlGpu3RendererBackend {
    fn from_initialized_context(runtime: RuntimeRegistration, consumer: RendererConsumer) -> Self {
        Self {
            runtime,
            consumer: Some(consumer),
            textures: RendererTextureStore::default(),
        }
    }

    /// Initialize the SDL3 platform backend and the official SDLGPU3 renderer.
    pub fn init(
        imgui: &mut Context,
        window: &Window,
        info: SdlGpu3InitInfo<'_>,
    ) -> Result<Self, Sdl3BackendError> {
        let (mut runtime, consumer) =
            prepare_renderer_runtime(imgui, shutdown_sdlgpu3_renderer_impl)?;
        if let Err(error) = init_for_sdlgpu3(imgui, window, info) {
            runtime.native_initialization_failed();
            drop(consumer);
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime, consumer))
    }

    /// Initialize the SDL3 platform backend and the official SDLGPU3 renderer.
    pub fn init_default(
        imgui: &mut Context,
        window: &Window,
        gpu: &Device,
    ) -> Result<Self, Sdl3BackendError> {
        let (mut runtime, consumer) =
            prepare_renderer_runtime(imgui, shutdown_sdlgpu3_renderer_impl)?;
        if let Err(error) = init_for_sdlgpu3_default(imgui, window, gpu) {
            runtime.native_initialization_failed();
            drop(consumer);
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime, consumer))
    }

    /// Process a single low-level SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || process_sys_event(event))
    }

    /// Begin a new SDL3 + SDLGPU3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, new_frame_sdlgpu3_impl)
    }

    /// Process texture requests and prepare one synchronous frame for an SDL GPU render pass.
    pub fn prepare_render<'renderer, 'ctx, 'command>(
        &'renderer mut self,
        mut frame: RenderedFrame<'ctx>,
        command_buffer: &'command CommandBuffer,
    ) -> Result<SdlGpu3PreparedFrame<'renderer, 'ctx, 'command>, Sdl3BackendError> {
        ensure_matching_rendered_frame(self.runtime.control().binding(), &frame)?;
        self.runtime.poll_fault()?;
        if self.runtime.control().state() != RuntimeState::Attached {
            return Err(Sdl3BackendError::RuntimeDetached);
        }
        let feedback = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                self.textures
                    .process_requests(frame.texture_requests(), update_sdlgpu3_texture)
            })??;
        frame.reconcile_texture_feedback(feedback)?;
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
        Ok(SdlGpu3PreparedFrame {
            backend: self,
            frame,
            command_buffer,
        })
    }

    /// Create SDL GPU3 renderer device objects.
    pub fn create_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        self.destroy_device_objects(imgui)?;
        run_renderer_entry(&self.runtime, imgui, create_sdlgpu3_device_objects)?;
        // Upstream CreateDeviceObjects starts by running its own destroy pass.
        imgui.reset_renderer_texture_bindings(self.consumer())?;
        Ok(())
    }

    /// Destroy SDL GPU3 renderer device objects.
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, destroy_sdlgpu3_device_objects)?;
        self.textures.forget_destroyed_by_upstream();
        imgui.reset_renderer_texture_bindings(self.consumer())?;
        Ok(())
    }

    /// Shut down the official SDLGPU3 renderer and SDL3 platform backend.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        let result = self
            .runtime
            .shutdown_renderer(imgui, self.consumer.as_ref(), || {
                self.textures.forget_destroyed_by_upstream()
            });
        if matches!(
            self.runtime.control().state(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.consumer.take();
        }
        result
    }

    /// Returns and clears the oldest pending SDL3 platform callback fault.
    pub fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.runtime.poll_fault()
    }

    fn consumer(&self) -> &RendererConsumer {
        self.consumer
            .as_ref()
            .expect("SDL3 GPU renderer consumer was already detached")
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
    pub fn render(self, render_pass: &mut RenderPass) -> Result<(), Sdl3BackendError> {
        self.backend
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(self.frame.draw_data(), "SdlGpu3PreparedFrame::render()");
                render_sdlgpu3_impl(self.frame.draw_data(), self.command_buffer, render_pass);
            })?;
        self.backend.runtime.control().finish_entry()
    }
}

/// RAII owner for SDL3 platform + official SDLRenderer3 renderer backends.
#[cfg(feature = "sdlrenderer3-renderer")]
#[must_use = "dropping the backend owner shuts down the SDL3 + SDLRenderer3 backends"]
#[derive(Debug)]
pub struct Sdl3RendererBackend {
    runtime: RuntimeRegistration,
    consumer: Option<RendererConsumer>,
    textures: RendererTextureStore,
}

#[cfg(feature = "sdlrenderer3-renderer")]
impl Sdl3RendererBackend {
    fn from_initialized_context(runtime: RuntimeRegistration, consumer: RendererConsumer) -> Self {
        Self {
            runtime,
            consumer: Some(consumer),
            textures: RendererTextureStore::default(),
        }
    }

    /// Initialize the SDL3 platform backend and the official SDLRenderer3 renderer.
    pub fn init(
        imgui: &mut Context,
        window: &Window,
        canvas: &WindowCanvas,
    ) -> Result<Self, Sdl3BackendError> {
        let (mut runtime, consumer) =
            prepare_renderer_runtime(imgui, shutdown_sdlrenderer3_renderer_impl)?;
        if let Err(error) = init_for_canvas(imgui, window, canvas) {
            runtime.native_initialization_failed();
            drop(consumer);
            return Err(error);
        }
        runtime.finish_native_initialization(imgui)?;
        Ok(Self::from_initialized_context(runtime, consumer))
    }

    /// Begin a new SDL3 + SDLRenderer3 frame.
    pub fn new_frame(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, new_frame_sdlrenderer3_impl)
    }

    /// Process a single low-level SDL3 event with the captured ImGui context.
    pub fn process_event(
        &mut self,
        imgui: &mut Context,
        event: &SDL_Event,
    ) -> Result<bool, Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, || process_sys_event(event))
    }

    /// Consume and render one synchronous frame using the official SDLRenderer3 renderer.
    pub fn render(
        &mut self,
        mut frame: RenderedFrame<'_>,
        canvas: &WindowCanvas,
    ) -> Result<(), Sdl3BackendError> {
        ensure_matching_rendered_frame(self.runtime.control().binding(), &frame)?;
        self.runtime.poll_fault()?;
        if self.runtime.control().state() != RuntimeState::Attached {
            return Err(Sdl3BackendError::RuntimeDetached);
        }
        let feedback = self
            .runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                self.textures
                    .process_requests(frame.texture_requests(), update_sdlrenderer3_texture)
            })??;
        frame.reconcile_texture_feedback(feedback)?;
        self.runtime
            .control()
            .binding()
            .try_with_bound_context(|| {
                assert_current_draw_data(frame.draw_data(), "Sdl3RendererBackend::render()");
                render_sdlrenderer3_impl(frame.draw_data(), canvas);
            })?;
        self.runtime.control().finish_entry()?;
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
    pub fn destroy_device_objects(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        run_renderer_entry(&self.runtime, imgui, destroy_sdlrenderer3_device_objects)?;
        self.textures.forget_destroyed_by_upstream();
        imgui.reset_renderer_texture_bindings(self.consumer())?;
        Ok(())
    }

    /// Shut down the official SDLRenderer3 renderer and SDL3 platform backend.
    pub fn shutdown(&mut self, imgui: &mut Context) -> Result<(), Sdl3BackendError> {
        let result = self
            .runtime
            .shutdown_renderer(imgui, self.consumer.as_ref(), || {
                self.textures.forget_destroyed_by_upstream()
            });
        if matches!(
            self.runtime.control().state(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.consumer.take();
        }
        result
    }

    /// Returns and clears the oldest pending SDL3 platform callback fault.
    pub fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.runtime.poll_fault()
    }

    fn consumer(&self) -> &RendererConsumer {
        self.consumer
            .as_ref()
            .expect("SDL3 renderer consumer was already detached")
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
