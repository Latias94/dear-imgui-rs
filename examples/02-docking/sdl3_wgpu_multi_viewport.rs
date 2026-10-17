//! SDL3 + WGPU multi-viewport example (native only).
//!
//! This demonstrates driving Dear ImGui with:
//! - SDL3 for window + events;
//! - the official SDL3 platform backend via `dear-imgui-sdl3`;
//! - the Rust WGPU renderer backend with SDL3 multi-viewport callbacks.
//!
//! Run with:
//! `cargo run -p dear-imgui-examples --bin sdl3_wgpu_multi_viewport --features sdl3-wgpu-multi-viewport`
//! Add `-- --smoke-frames 120` for a bounded native target/texture attribution check.
//!
//! This is experimental and intended for native desktop targets. WebGPU/WASM multi-viewport is
//! not supported.

use std::cell::RefCell;
use std::error::Error;
use std::time::{Duration, Instant};

use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_examples::sdl3_dpi::DpiStyle;
use dear_imgui_rs::{Condition, ConfigFlags, Context, Id, NativeWindowTarget};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, GamepadMode, Sdl3PlatformBackend};
use dear_imgui_wgpu::multi_viewport_sdl3::{Sdl3ViewportRoute, WgpuViewportFrameReport};
use dear_imgui_wgpu::{
    ExternalTextureBinding, ExternalTextureId, FramebufferExtent, GammaMode, WgpuFrameStage,
    WgpuInitInfo, WgpuRenderer,
};
use sdl3::video::{SwapInterval, WindowPos};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};

const ENABLE_VIEWPORTS: bool = true;

struct SmokeScene {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    registration: ExternalTextureId,
}

impl SmokeScene {
    fn resource(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SDL3 smoke SceneColor"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        (texture, view)
    }
}

/// One bounded scenario in the existing native driver, without a parallel platform registry.
struct Smoke {
    frames: u64,
    limit: u64,
    started: Instant,
    phase: u8,
    scenes: [SmokeScene; 2],
    original: Option<[NativeWindowTarget; 2]>,
    replaced: bool,
    done: bool,
}

impl Smoke {
    fn new(
        device: &wgpu::Device,
        renderer: &Sdl3ViewportRoute,
        limit: u64,
    ) -> Result<Self, Box<dyn Error>> {
        let mut scenes = Vec::with_capacity(2);
        for _ in 0..2 {
            let (texture, view) = SmokeScene::resource(device);
            let registration = renderer.register_external_texture(&view)?;
            scenes.push(SmokeScene {
                _texture: texture,
                view,
                registration,
            });
        }
        Ok(Self {
            frames: 0,
            limit,
            started: Instant::now(),
            phase: 0,
            scenes: scenes.try_into().ok().expect("two scene textures"),
            original: None,
            replaced: false,
            done: false,
        })
    }

    fn begin(
        &mut self,
        device: &wgpu::Device,
        renderer: &Sdl3ViewportRoute,
    ) -> Result<wgpu::CommandBuffer, Box<dyn Error>> {
        self.frames += 1;
        if self.frames > self.limit || self.started.elapsed() > Duration::from_secs(30) {
            return Err(format!(
                "SDL3 WGPU smoke timed out in phase {} after {} frames",
                self.phase, self.frames
            )
            .into());
        }
        if self.phase == 1 && !self.replaced {
            let (texture, view) = SmokeScene::resource(device);
            renderer.update_external_texture(self.scenes[0].registration, &view)?;
            self.scenes[0]._texture = texture;
            self.scenes[0].view = view;
            self.replaced = true;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SDL3 smoke scene producers before UI consumers"),
        });
        let first = if self.replaced {
            wgpu::Color::GREEN
        } else {
            wgpu::Color::RED
        };
        for (scene, color) in self.scenes.iter().zip([first, wgpu::Color::BLUE]) {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &scene.view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SDL3 smoke deterministic SceneColor"),
                color_attachments: &attachments,
                ..Default::default()
            });
        }
        Ok(encoder.finish())
    }

    fn draw(&self, ui: &dear_imgui_rs::Ui) -> [Option<Id>; 2] {
        let main = ui.main_viewport();
        let origin = main.pos();
        let size = main.size();
        let mut viewports = [None; 2];
        for (index, name) in ["SDL3 smoke Scene A", "SDL3 smoke Scene B"]
            .into_iter()
            .enumerate()
        {
            if self.phase >= 2 && index == 1 {
                continue;
            }
            let position = if self.phase >= 2 {
                ui.set_next_window_viewport(main.id());
                [origin[0] + 30.0, origin[1] + 300.0]
            } else {
                [
                    origin[0] + size[0] + 30.0,
                    origin[1] + 30.0 + index as f32 * 220.0,
                ]
            };
            let source = if self.phase == 0 { index } else { 0 };
            ui.window(name)
                .position(position, Condition::Always)
                .size([230.0, 200.0], Condition::Always)
                .build(|| {
                    viewports[index] = Some(ui.window_viewport().id());
                    ui.image(
                        self.scenes[source].registration.texture_id(),
                        [150.0, 125.0],
                    );
                    ui.text(format!(
                        "SceneColor {} / host frame {}",
                        source, self.frames
                    ));
                });
        }
        viewports
    }

    fn observe(
        &mut self,
        report: &WgpuViewportFrameReport,
        platform: &Sdl3PlatformBackend,
        viewports: [Option<Id>; 2],
        bindings: [ExternalTextureBinding; 2],
    ) -> Result<(), Box<dyn Error>> {
        if self.phase >= 2 {
            return Ok(());
        }
        let mut targets = Vec::with_capacity(2);
        for (index, viewport) in viewports.into_iter().enumerate() {
            let binding = bindings[if self.phase == 0 { index } else { 0 }];
            let Some(observation) = report.observations().iter().find(|observation| {
                Some(observation.target.viewport_id) == viewport
                    && !observation.target.is_main
                    && observation.stage == WgpuFrameStage::PresentationRequested
                    && observation.dependencies.external_bindings == [binding]
            }) else {
                return Ok(());
            };
            let live = platform
                .window_snapshot_for_instance(observation.target.instance_id)?
                .ok_or("reported SDL3 smoke target is no longer registered")?;
            if live.target != observation.target {
                return Err(
                    "SDL3 native target report disagrees with its exact registration".into(),
                );
            }
            targets.push(observation);
        }
        if targets[0].target.instance_id == targets[1].target.instance_id
            || targets[0].scope != targets[1].scope
        {
            return Err("SDL3 smoke needs two distinct native targets in the same UI epoch".into());
        }
        let captured = [targets[0].target, targets[1].target];
        if self.phase == 0 {
            if bindings[0].texture == bindings[1].texture
                || bindings.iter().any(|binding| binding.revision.get() != 1)
            {
                return Err(
                    "SDL3 distinct scenes must start with independent revision-one registrations"
                        .into(),
                );
            }
            self.original = Some(captured);
        } else {
            if self.original != Some(captured) || bindings[0].revision.get() != 2 {
                return Err("SDL3 shared scene must retain the exact native registrations and advance its binding revision".into());
            }
        }
        println!(
            "sdl3-wgpu-smoke phase={} host_frame={} targets={captured:?} bindings={:?} stage=PresentationRequested",
            self.phase,
            self.frames,
            targets
                .iter()
                .map(|observation| &observation.dependencies.external_bindings)
                .collect::<Vec<_>>()
        );
        self.phase += 1;
        Ok(())
    }

    fn finish_frame(
        &mut self,
        platform: &Sdl3PlatformBackend,
        merged_viewport: Option<Id>,
        main: NativeWindowTarget,
    ) -> Result<(), Box<dyn Error>> {
        if self.phase != 2 || merged_viewport != Some(main.viewport_id) {
            return Ok(());
        }
        for target in self.original.expect("distinct phase captured targets") {
            if platform
                .window_snapshot_for_instance(target.instance_id)?
                .is_some()
            {
                return Ok(());
            }
        }
        self.done = true;
        println!(
            "sdl3-wgpu-smoke PASS: distinct scenes -> shared revision 2; Scene A merged into {main:?}, Scene B closed; old native registrations retired; producer submitted before UI; presentation requests only"
        );
        Ok(())
    }
}

struct WgpuMultiViewportApp {
    events: Sdl3CallbackEventHandoff,
    main: MainThreadData<RefCell<MainData>>,
}

struct MainData {
    renderer: Sdl3ViewportRoute,
    sdl3_backend: Sdl3PlatformBackend,
    imgui: Context,
    dpi_style: DpiStyle,
    surface: wgpu::Surface<'static>,
    queue: wgpu::Queue,
    device: wgpu::Device,
    _adapter: wgpu::Adapter,
    _instance: wgpu::Instance,
    surface_config: wgpu::SurfaceConfiguration,
    window: sdl3::video::Window,
    _video: sdl3::VideoSubsystem,
    _sdl: sdl3::Sdl,
    last_frame: Instant,
    show_demo: bool,
    smoke: Option<Smoke>,
    shutdown_complete: bool,
}

impl WgpuMultiViewportApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        let mut arguments = std::env::args().skip(1);
        let mut smoke_frames = None;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--smoke-frames" => {
                    let count: u64 = arguments
                        .next()
                        .ok_or("--smoke-frames requires a count")?
                        .parse()?;
                    if !(8..=600).contains(&count) {
                        return Err("smoke frame count must be 8..=600".into());
                    }
                    smoke_frames = Some(count);
                }
                _ => return Err(format!("unknown argument: {argument}").into()),
            }
        }
        configure_main_callback_rate();
        imgui_sdl3_backend::enable_native_ime_ui();

        let sdl = sdl3::init()?;
        let video = sdl.video()?;
        let main_scale = video
            .get_primary_display()?
            .get_content_scale()
            .unwrap_or(1.0);
        let main_scale = if main_scale.is_finite() && main_scale > 0.0 {
            main_scale
        } else {
            1.0
        };
        let mut window = video
            .window(
                "Dear ImGui SDL3 + WGPU (multi-viewport)",
                (1200.0 * main_scale) as u32,
                (720.0 * main_scale) as u32,
            )
            .resizable()
            .high_pixel_density()
            .build()
            .map_err(|error| format!("failed to create SDL3 window: {error}"))?;
        window.set_position(WindowPos::Centered, WindowPos::Centered);
        let _ = video.gl_set_swap_interval(SwapInterval::Immediate);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        // SAFETY: the retained SDL3 window stays valid until after this surface is dropped.
        let surface = unsafe {
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_display_and_window(&window, &window)
                    .expect("failed to create SurfaceTarget from SDL3 window"),
            )?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))
        .expect("failed to find suitable WGPU adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let (width, height) = window.size_in_pixels();
        let capabilities = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .copied()
            .find(|format| capabilities.formats.contains(format))
            .unwrap_or(capabilities.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let mut imgui = Context::create();
        imgui.set_ini_filename(None::<String>)?;
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);
        }
        let dpi_style = DpiStyle::new(&mut imgui, &window);
        if ENABLE_VIEWPORTS {
            imgui.enable_multi_viewport();
        }
        if smoke_frames.is_some() {
            imgui.io_mut().set_config_viewports_no_auto_merge(true);
        }
        // SAFETY: the retained window outlives explicit platform shutdown.
        let mut sdl3_backend = unsafe { Sdl3PlatformBackend::init_for_other(&mut imgui, &window)? };
        sdl3_backend.set_gamepad_mode(&mut imgui, GamepadMode::AutoAll)?;
        let mut renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_config.format)
                .with_instance(instance.clone())
                .with_adapter(adapter.clone())
                .with_viewport_surface_config((&surface_config).into()),
            &mut imgui,
        )?;
        renderer.set_gamma_mode(GammaMode::Auto);
        let renderer = Sdl3ViewportRoute::attach(&mut imgui, &sdl3_backend, renderer)?;
        let smoke = smoke_frames
            .map(|limit| Smoke::new(&device, &renderer, limit))
            .transpose()?;

        Ok(Self {
            events: Sdl3CallbackEventHandoff::default(),
            main: MainThreadData::assert_new(RefCell::new(MainData {
                renderer,
                sdl3_backend,
                imgui,
                dpi_style,
                surface,
                queue,
                device,
                _adapter: adapter,
                _instance: instance,
                surface_config,
                window,
                _video: video,
                _sdl: sdl,
                last_frame: Instant::now(),
                show_demo: smoke.is_none(),
                smoke,
                shutdown_complete: false,
            })),
        })
    }

    fn process_events(&self) -> AppResult {
        let mut events = self.events.drain();
        if let Some(error) = events.first_fault() {
            eprintln!("SDL3 callback event handoff failed: {error}");
            return AppResult::Failure;
        }
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        while let Some(event) = events.pop() {
            let backend_result = main
                .sdl3_backend
                .process_callback_event(&mut main.imgui, &event);
            if let Err(error) = backend_result {
                eprintln!("SDL3 backend event processing failed: {error}");
                return AppResult::Failure;
            }
            if requests_exit(&event, main.window.id()) {
                if main.smoke.as_ref().is_some_and(|smoke| !smoke.done) {
                    eprintln!("SDL3 WGPU smoke interrupted before its native evidence completed");
                    return AppResult::Failure;
                }
                return AppResult::Success;
            }
            if event.is_pixel_size_changed_for(main.window.id()) {
                Self::reconfigure_surface(main);
            }
        }
        AppResult::Continue
    }

    fn reconfigure_surface(main: &mut MainData) {
        let (width, height) = main.window.size_in_pixels();
        if width > 0 && height > 0 {
            main.surface_config.width = width;
            main.surface_config.height = height;
            main.surface.configure(&main.device, &main.surface_config);
        }
    }

    fn render(&self) -> Result<bool, Box<dyn Error>> {
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        let now = Instant::now();
        main.imgui
            .io_mut()
            .set_delta_time((now - main.last_frame).as_secs_f32());
        main.last_frame = now;
        main.sdl3_backend.new_frame(&mut main.imgui)?;
        main.dpi_style.update(&mut main.imgui, &main.window);
        let scale = main.dpi_style.scale();
        let producer = main
            .smoke
            .as_mut()
            .map(|smoke| smoke.begin(&main.device, &main.renderer))
            .transpose()?;
        let bindings = main
            .smoke
            .as_ref()
            .map(|smoke| {
                Ok::<_, Box<dyn Error>>([
                    main.renderer
                        .external_texture_binding(smoke.scenes[0].registration)?,
                    main.renderer
                        .external_texture_binding(smoke.scenes[1].registration)?,
                ])
            })
            .transpose()?;
        let frame = main.imgui.begin_frame();
        let ui = frame.ui();
        ui.dockspace().build()?;
        ui.window("SDL3 + WGPU (multi-viewport)")
            .size([420.0 * scale, 260.0 * scale], Condition::FirstUseEver)
            .build(|| {
                ui.text("Drag ImGui windows outside to spawn OS windows.");
                ui.separator();
                ui.checkbox("Show demo window", &mut main.show_demo);
                ui.text("Gamepad: SDL3 backend in AutoAll mode.");
                ui.text(format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));
            });
        if main.show_demo {
            ui.show_demo_window(&mut main.show_demo);
        }
        let scene_viewports = main
            .smoke
            .as_ref()
            .map(|smoke| smoke.draw(ui))
            .unwrap_or([None; 2]);
        // Route preparation finishes secondary native windows before main-surface acquisition, so
        // temporary main-surface failures do not stall the other viewports.
        let prepared = main.renderer.prepare(frame)?;
        let prepared = main.renderer.complete_secondary(prepared, producer)?;
        if let (Some(smoke), Some(bindings)) = (&mut main.smoke, bindings) {
            smoke.observe(
                prepared.secondary_report(),
                &main.sdl3_backend,
                scene_viewports,
                bindings,
            )?;
        }

        let (frame, reconfigure_after_present) = match main.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                drop(prepared);
                Self::reconfigure_surface(main);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                drop(prepared);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface acquisition failed with a WGPU validation error".into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = main
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("sdl3_wgpu_mv_encoder"),
            });
        let mut main_observation = {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sdl3_wgpu_mv_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.12,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            main.renderer.record_main(
                prepared,
                &mut render_pass,
                FramebufferExtent::from_texture(&frame.texture),
            )?
        };
        main.queue.submit(std::iter::once(encoder.finish()));
        main_observation.stage = main_observation.stage.max(WgpuFrameStage::Submitted);
        main.queue.present(frame);
        main_observation.stage = main_observation
            .stage
            .max(WgpuFrameStage::PresentationRequested);
        if reconfigure_after_present {
            Self::reconfigure_surface(main);
        }
        if let Some(smoke) = &mut main.smoke {
            smoke.finish_frame(
                &main.sdl3_backend,
                scene_viewports[0],
                main_observation.target,
            )?;
            return Ok(smoke.done);
        }
        Ok(false)
    }

    fn shutdown(&self) -> Result<(), Box<dyn Error>> {
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        if main.shutdown_complete {
            return Ok(());
        }
        main.renderer.shutdown(&mut main.imgui)?;
        main.sdl3_backend.shutdown(&mut main.imgui)?;
        main.shutdown_complete = true;
        if main.smoke.is_some() {
            println!("sdl3-wgpu-smoke teardown=renderer-before-platform complete");
        }
        Ok(())
    }
}

#[app_impl]
impl WgpuMultiViewportApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 WGPU multi-viewport example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        let event_result = self.process_events();
        if event_result != AppResult::Continue {
            return event_result;
        }
        match self.render() {
            Ok(true) => match self.shutdown() {
                Ok(()) => AppResult::Success,
                Err(error) => {
                    eprintln!("SDL3 WGPU smoke teardown failed: {error}");
                    AppResult::Failure
                }
            },
            Ok(false) => AppResult::Continue,
            Err(error) => {
                eprintln!("SDL3 WGPU multi-viewport frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        // SAFETY: SDL supplies a valid event whose transient payload remains live for this call.
        unsafe { self.events.push_from_callback(raw) };
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
        if let Some(app) = state
            && let Err(error) = app.shutdown()
        {
            eprintln!("SDL3 WGPU multi-viewport shutdown failed: {error}");
        }
    }
}
