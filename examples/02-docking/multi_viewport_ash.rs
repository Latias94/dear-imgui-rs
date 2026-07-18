//! Minimal multi-viewport sample using winit + ash (Vulkan) backends.
//!
//! ⚠️ **EXPERIMENTAL TEST EXAMPLE ONLY** ⚠️
//!
//! Run with:
//! ```bash
//! cargo run --bin multi_viewport_ash --features multi-viewport
//! ```
//!
//! Notes:
//! - This example targets desktop native (Windows/macOS/Linux).
//! - It uses Dear ImGui's multi-viewport system to create additional OS windows.
//! - Secondary viewports create their own Vulkan `SurfaceKHR` + swapchain.
//! - The ash renderer caches pipelines per swapchain format to handle per-viewport formats.

use ash::{
    Device, Entry, Instance,
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
use dear_imgui_ash::{AshRenderer, Options as AshOptions, multi_viewport as ash_mvp};
use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_winit::{HiDpiMode, WinitPlatform, multi_viewport as winit_mvp};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{ffi::CString, sync::Arc, time::Instant};
use tracing::{error, info};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

const FRAMES_IN_FLIGHT: usize = 2;

struct VulkanContext {
    entry: Entry,
    instance: Instance,
    surface_loader: khr_surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    device: Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
}

impl VulkanContext {
    fn new(window: &Window, title: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { Entry::load()? };

        let app_name = CString::new(title)?;
        let engine_name = CString::new("dear-imgui-examples")?;
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name.as_c_str())
            .engine_name(engine_name.as_c_str())
            .api_version(vk::make_api_version(0, 1, 0, 0));

        let extensions =
            ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())?.to_vec();

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions);
        let instance = unsafe { entry.create_instance(&instance_create_info, None)? };

        let surface_loader = khr_surface::Instance::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?
        };

        let (physical_device, queue_family_index) =
            pick_physical_device(&instance, &surface_loader, surface)?;

        let (device, queue) = create_device(&instance, physical_device, queue_family_index)?;

        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };

        Ok(Self {
            entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            queue_family_index,
            device,
            queue,
            command_pool,
        })
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

struct SwapchainState {
    loader: khr_swapchain::Device,
    swapchain: vk::SwapchainKHR,
    surface_format: vk::SurfaceFormatKHR,
    extent: vk::Extent2D,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    present_semaphores: Vec<vk::Semaphore>,
}

impl SwapchainState {
    fn new(
        ctx: &VulkanContext,
        window: &Window,
        render_pass: vk::RenderPass,
        surface_format: vk::SurfaceFormatKHR,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_old(
            ctx,
            window,
            render_pass,
            surface_format,
            vk::SwapchainKHR::null(),
        )
    }

    fn new_with_old(
        ctx: &VulkanContext,
        window: &Window,
        render_pass: vk::RenderPass,
        surface_format: vk::SurfaceFormatKHR,
        old_swapchain: vk::SwapchainKHR,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let loader = khr_swapchain::Device::new(&ctx.instance, &ctx.device);

        let caps = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_capabilities(ctx.physical_device, ctx.surface)?
        };
        let present_modes = unsafe {
            ctx.surface_loader
                .get_physical_device_surface_present_modes(ctx.physical_device, ctx.surface)?
        };

        let present_mode = pick_present_mode(&present_modes);
        let extent = pick_extent(&caps, window.inner_size());

        let min_image_count = {
            let desired = caps.min_image_count.saturating_add(1);
            if caps.max_image_count > 0 {
                desired.min(caps.max_image_count)
            } else {
                desired
            }
        };

        let composite_alpha = [
            vk::CompositeAlphaFlagsKHR::OPAQUE,
            vk::CompositeAlphaFlagsKHR::INHERIT,
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        ]
        .into_iter()
        .find(|c| caps.supported_composite_alpha.contains(*c))
        .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(ctx.surface)
            .min_image_count(min_image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old_swapchain);

        let swapchain = unsafe { loader.create_swapchain(&swapchain_create_info, None)? };
        let images = match unsafe { loader.get_swapchain_images(swapchain) } {
            Ok(images) => images,
            Err(error) => {
                unsafe { loader.destroy_swapchain(swapchain, None) };
                return Err(Box::new(error));
            }
        };
        let image_views = match create_image_views(&ctx.device, &images, surface_format.format) {
            Ok(image_views) => image_views,
            Err(error) => {
                unsafe { loader.destroy_swapchain(swapchain, None) };
                return Err(error);
            }
        };
        let framebuffers = match create_framebuffers(&ctx.device, render_pass, extent, &image_views)
        {
            Ok(framebuffers) => framebuffers,
            Err(error) => {
                destroy_image_views(&ctx.device, image_views);
                unsafe { loader.destroy_swapchain(swapchain, None) };
                return Err(error);
            }
        };
        let present_semaphores = match create_present_semaphores(&ctx.device, images.len()) {
            Ok(semaphores) => semaphores,
            Err(error) => {
                destroy_framebuffers(&ctx.device, framebuffers);
                destroy_image_views(&ctx.device, image_views);
                unsafe { loader.destroy_swapchain(swapchain, None) };
                return Err(error);
            }
        };

        Ok(Self {
            loader,
            swapchain,
            surface_format,
            extent,
            images,
            image_views,
            framebuffers,
            present_semaphores,
        })
    }

    fn destroy_resources(&mut self, device: &Device) {
        unsafe {
            for framebuffer in self.framebuffers.drain(..) {
                device.destroy_framebuffer(framebuffer, None);
            }
            for image_view in self.image_views.drain(..) {
                device.destroy_image_view(image_view, None);
            }
            for semaphore in self.present_semaphores.drain(..) {
                device.destroy_semaphore(semaphore, None);
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.loader.destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
    }

    fn recreate(
        &mut self,
        ctx: &VulkanContext,
        window: &Window,
        render_pass: vk::RenderPass,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { ctx.device.device_wait_idle()? };
        let new_format = pick_surface_format(ctx, window)?;
        let replacement =
            match Self::new_with_old(ctx, window, render_pass, new_format, self.swapchain) {
                Ok(replacement) => replacement,
                Err(error) => {
                    self.destroy_resources(&ctx.device);
                    return Err(error);
                }
            };
        let mut previous = std::mem::replace(self, replacement);
        previous.destroy_resources(&ctx.device);
        Ok(())
    }
}

struct FrameSync {
    image_available: vk::Semaphore,
    fence: vk::Fence,
    command_buffer: vk::CommandBuffer,
}

enum RendererRuntime {
    Single(AshRenderer),
    Viewports(ash_mvp::WinitViewportRuntime),
}

impl RendererRuntime {
    fn set_viewport_clear_color(
        &mut self,
        color: [f32; 4],
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => renderer.set_viewport_clear_color(color),
            Self::Viewports(runtime) => runtime.set_viewport_clear_color(color)?,
        }
        Ok(())
    }

    fn cmd_draw(
        &mut self,
        command_buffer: vk::CommandBuffer,
        frame: dear_imgui_rs::render::RenderedFrame<'_>,
    ) -> Result<Option<dear_imgui_ash::TextureRetirementBatch>, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Single(renderer) => renderer.cmd_draw(command_buffer, frame)?,
            Self::Viewports(runtime) => runtime.cmd_draw(command_buffer, frame)?,
        })
    }

    fn wait_for_texture_retirements(
        &mut self,
        batch: dear_imgui_ash::TextureRetirementBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => {
                renderer.wait_for_texture_retirements(batch)?;
            }
            Self::Viewports(runtime) => {
                runtime.wait_for_texture_retirements(batch)?;
            }
        }
        Ok(())
    }

    fn shutdown(&mut self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => renderer.shutdown(context)?,
            Self::Viewports(runtime) => runtime.shutdown(context)?,
        }
        Ok(())
    }
}

struct VulkanState {
    ctx: VulkanContext,
    render_pass: vk::RenderPass,
    swapchain: SwapchainState,
    frames: Vec<FrameSync>,
    images_in_flight: Vec<vk::Fence>,
    frame_index: usize,
    swapchain_dirty: bool,
}

impl Drop for VulkanState {
    fn drop(&mut self) {
        unsafe {
            let _ = self.ctx.device.device_wait_idle();
            destroy_frame_syncs(&self.ctx.device, self.ctx.command_pool, &mut self.frames);
            self.swapchain.destroy_resources(&self.ctx.device);
            self.ctx.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

struct ImguiState {
    renderer: RendererRuntime,
    viewport_runtime: Option<winit_mvp::WinitPlatformRuntime>,
    platform: WinitPlatform,
    context: Context,
    clear_color: [f32; 4],
    demo_open: bool,
    last_frame: Instant,
}

struct AppWindow {
    enable_viewports: bool,
    imgui: ImguiState,
    vk: VulkanState,
    // Keep the platform window alive until renderer, swapchains, and surfaces have been dropped.
    window: Arc<Window>,
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        let _ = self.imgui.renderer.shutdown(&mut self.imgui.context);
        if let Some(runtime) = self.imgui.viewport_runtime.as_mut() {
            let _ = runtime.shutdown();
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<Box<AppWindow>>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let enable_viewports = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));

        let version = env!("CARGO_PKG_VERSION");
        let size = LogicalSize::new(1280.0, 720.0);
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(format!("Dear ImGui Multi-Viewport (ash) - {version}"))
                    .with_inner_size(size),
            )?,
        );

        let ctx = VulkanContext::new(&window, "dear-imgui-multi-viewport-ash")?;
        let surface_format = pick_surface_format(&ctx, &window)?;
        let render_pass = create_render_pass(&ctx.device, surface_format.format)?;
        let swapchain = SwapchainState::new(&ctx, &window, render_pass, surface_format)?;

        let mut imgui = Context::create();
        imgui.set_ini_filename(None::<String>).unwrap();

        if enable_viewports {
            imgui.enable_multi_viewport();
        }
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);
        }

        let mut platform = WinitPlatform::new(&mut imgui);
        platform.attach_window(&window, HiDpiMode::Default, &mut imgui);

        let viewport_runtime = enable_viewports
            .then(|| winit_mvp::WinitPlatformRuntime::new(&mut imgui, Arc::clone(&window)))
            .transpose()?;

        let framebuffer_srgb = is_srgb_format(swapchain.surface_format.format);
        let mut renderer = AshRenderer::with_default_allocator(
            &ctx.instance,
            ctx.physical_device,
            ctx.device.clone(),
            ctx.queue,
            ctx.command_pool,
            render_pass,
            &mut imgui,
            Some(AshOptions {
                in_flight_frames: FRAMES_IN_FLIGHT,
                framebuffer_srgb,
                ..Default::default()
            }),
        )?;
        renderer.set_viewport_clear_color([0.1, 0.12, 0.15, 1.0]);
        let renderer = if enable_viewports {
            RendererRuntime::Viewports(unsafe {
                ash_mvp::WinitViewportRuntime::attach(
                    &mut imgui,
                    renderer,
                    ash_mvp::VulkanViewportConfig {
                        entry: ctx.entry.clone(),
                        instance: ctx.instance.clone(),
                        physical_device: ctx.physical_device,
                        validation_surface: ctx.surface,
                        present_queue: ctx.queue,
                        graphics_queue_family_index: ctx.queue_family_index,
                        present_queue_family_index: ctx.queue_family_index,
                    },
                )?
            })
        } else {
            RendererRuntime::Single(renderer)
        };

        let frames = create_frame_syncs(&ctx.device, ctx.command_pool, FRAMES_IN_FLIGHT)?;
        let images_in_flight = vec![vk::Fence::null(); swapchain.images.len()];
        Ok(Self {
            window,
            enable_viewports,
            imgui: ImguiState {
                renderer,
                viewport_runtime,
                platform,
                context: imgui,
                clear_color: [0.1, 0.12, 0.15, 1.0],
                demo_open: true,
                last_frame: Instant::now(),
            },
            vk: VulkanState {
                ctx,
                render_pass,
                swapchain,
                frames,
                images_in_flight,
                frame_index: 0,
                swapchain_dirty: false,
            },
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.vk.swapchain_dirty = true;
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window_size = self.window.inner_size();
        if window_size.width == 0 || window_size.height == 0 {
            return Ok(());
        }
        if self.vk.swapchain_dirty {
            self.vk
                .swapchain
                .recreate(&self.vk.ctx, &self.window, self.vk.render_pass)?;
            self.vk.images_in_flight = vec![vk::Fence::null(); self.vk.swapchain.images.len()];
            self.vk.swapchain_dirty = false;
        }

        let now = Instant::now();
        let dt = (now - self.imgui.last_frame).as_secs_f32();
        self.imgui.context.io_mut().set_delta_time(dt);
        self.imgui.last_frame = now;

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context);
        let ui = self.imgui.context.frame();

        ui.window("Multi-Viewport (ash)")
            .size([460.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Renderer: dear-imgui-ash (Vulkan)");
                ui.separator();

                ui.text(format!(
                    "Swapchain format: {:?}",
                    self.vk.swapchain.surface_format.format
                ));
                ui.text(format!(
                    "Framebuffer sRGB: {} (shader gamma path)",
                    is_srgb_format(self.vk.swapchain.surface_format.format)
                ));

                ui.color_edit4("Clear color", &mut self.imgui.clear_color);
                if let Err(error) = self
                    .imgui
                    .renderer
                    .set_viewport_clear_color(self.imgui.clear_color)
                {
                    error!("failed to update viewport clear color: {error}");
                }

                if ui.button("Show Demo Window") {
                    self.imgui.demo_open = true;
                }
            });

        if self.imgui.demo_open {
            // SAFETY: This demo assumes the destructive font-atlas controls are not activated.
            unsafe { ui.show_demo_window(&mut self.imgui.demo_open) };
        }

        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window);
        let draw_data = self.imgui.context.render();

        let frame = &self.vk.frames[self.vk.frame_index % self.vk.frames.len()];

        unsafe {
            self.vk
                .ctx
                .device
                .wait_for_fences(&[frame.fence], true, u64::MAX)?;
        }

        let acquire = unsafe {
            self.vk.swapchain.loader.acquire_next_image(
                self.vk.swapchain.swapchain,
                u64::MAX,
                frame.image_available,
                vk::Fence::null(),
            )
        };

        let (image_index, suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                self.vk.swapchain_dirty = true;
                return Ok(());
            }
            Err(e) => return Err(Box::new(e)),
        };
        self.vk.swapchain_dirty |= suboptimal;
        let present_semaphore = self.vk.swapchain.present_semaphores[image_index as usize];

        if self.vk.images_in_flight[image_index as usize] != vk::Fence::null() {
            unsafe {
                self.vk.ctx.device.wait_for_fences(
                    &[self.vk.images_in_flight[image_index as usize]],
                    true,
                    u64::MAX,
                )?;
            }
        }
        self.vk.images_in_flight[image_index as usize] = frame.fence;

        unsafe {
            self.vk.ctx.device.reset_fences(&[frame.fence])?;
            self.vk
                .ctx
                .device
                .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())?;
        }

        let texture_retirement = record_command_buffer(
            &self.vk.ctx.device,
            frame.command_buffer,
            self.vk.render_pass,
            self.vk.swapchain.framebuffers[image_index as usize],
            self.vk.swapchain.extent,
            self.imgui.clear_color,
            |cmd| self.imgui.renderer.cmd_draw(cmd, draw_data),
        )?;

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&frame.image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&frame.command_buffer))
            .signal_semaphores(std::slice::from_ref(&present_semaphore));

        unsafe {
            self.vk.ctx.device.queue_submit(
                self.vk.ctx.queue,
                std::slice::from_ref(&submit_info),
                frame.fence,
            )?;
        }

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&present_semaphore))
            .swapchains(std::slice::from_ref(&self.vk.swapchain.swapchain))
            .image_indices(std::slice::from_ref(&image_index));

        let present = unsafe {
            self.vk
                .swapchain
                .loader
                .queue_present(self.vk.ctx.queue, &present_info)
        };
        match present {
            Ok(suboptimal) => self.vk.swapchain_dirty |= suboptimal,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                self.vk.swapchain_dirty = true;
            }
            Err(e) => return Err(Box::new(e)),
        }

        self.vk.frame_index = (self.vk.frame_index + 1) % self.vk.frames.len();

        // Update + render all platform windows (secondary viewports).
        if self.enable_viewports {
            self.imgui.context.update_platform_windows();
            self.imgui.context.render_platform_windows_default();
        }

        if let Some(batch) = texture_retirement {
            self.imgui.renderer.wait_for_texture_retirements(batch)?;
        }

        Ok(())
    }

    fn render_with_event_loop(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let viewport_runtime = self.imgui.viewport_runtime.take();
        let result = match viewport_runtime.as_ref() {
            Some(runtime) => match runtime.with_event_loop(event_loop, |_| self.render()) {
                Ok(result) => result,
                Err(error) => Err(Box::new(error) as Box<dyn std::error::Error>),
            },
            None => self.render(),
        };
        self.imgui.viewport_runtime = viewport_runtime;
        result
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match AppWindow::new(event_loop) {
            Ok(win) => {
                win.window.request_redraw();
                self.window = Some(Box::new(win));
            }
            Err(e) => {
                error!("Failed to create window: {e}");
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app) = &self.window {
            app.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(app) = self.window.as_mut() else {
            return;
        };

        let is_main_window = window_id == app.window.id();
        let full: Event<()> = Event::WindowEvent {
            window_id,
            event: event.clone(),
        };

        if let Some(runtime) = app.imgui.viewport_runtime.as_ref() {
            if let Err(error) =
                runtime.handle_event(&mut app.imgui.platform, &mut app.imgui.context, &full)
            {
                error!("Winit viewport event error: {error}");
            }
        } else {
            let _ = app
                .imgui
                .platform
                .handle_event(&mut app.imgui.context, &app.window, &full);
        }

        match event {
            WindowEvent::CloseRequested => {
                // Only exit when the main application window is closed.
                if is_main_window {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if is_main_window {
                    app.resize(size);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if is_main_window {
                    app.resize(app.window.inner_size());
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if is_main_window && event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                // We drive rendering from the main window. Secondary viewport windows are
                // rendered via ImGui's platform callbacks during `app.render()`.
                if is_main_window {
                    if let Err(e) = app.render_with_event_loop(event_loop) {
                        error!("Render error: {e}");
                    }
                    app.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dear_imgui_examples::init_tracing_with_filter("dear_imgui=debug,multi_viewport_ash=info");
    info!("Starting Dear ImGui Multi-Viewport (ash) Example");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn pick_physical_device(
    instance: &Instance,
    surface_loader: &khr_surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32), Box<dyn std::error::Error>> {
    let devices = unsafe { instance.enumerate_physical_devices()? };
    for device in devices {
        let qfamilies = unsafe { instance.get_physical_device_queue_family_properties(device) };
        for (index, family) in qfamilies.iter().enumerate() {
            if !family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                continue;
            }
            let present_supported = unsafe {
                surface_loader.get_physical_device_surface_support(device, index as u32, surface)?
            };
            if present_supported {
                return Ok((device, index as u32));
            }
        }
    }
    Err("No suitable Vulkan device/queue family found".into())
}

fn create_device(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
) -> Result<(Device, vk::Queue), Box<dyn std::error::Error>> {
    let priorities = [1.0f32];
    let queue_create_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&priorities);

    let extensions = [khr_swapchain::NAME.as_ptr()];
    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_create_info))
        .enabled_extension_names(&extensions);

    let device = unsafe { instance.create_device(physical_device, &device_create_info, None)? };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
    Ok((device, queue))
}

fn pick_surface_format(
    ctx: &VulkanContext,
    _window: &Window,
) -> Result<vk::SurfaceFormatKHR, Box<dyn std::error::Error>> {
    let formats = unsafe {
        ctx.surface_loader
            .get_physical_device_surface_formats(ctx.physical_device, ctx.surface)?
    };
    let preferred = [
        vk::Format::B8G8R8A8_SRGB,
        vk::Format::R8G8B8A8_SRGB,
        vk::Format::B8G8R8A8_UNORM,
        vk::Format::R8G8B8A8_UNORM,
    ];
    for p in preferred {
        if let Some(f) = formats.iter().find(|f| f.format == p) {
            return Ok(*f);
        }
    }
    Ok(formats[0])
}

fn pick_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    if modes.contains(&vk::PresentModeKHR::MAILBOX) {
        vk::PresentModeKHR::MAILBOX
    } else {
        vk::PresentModeKHR::FIFO
    }
}

fn pick_extent(
    caps: &vk::SurfaceCapabilitiesKHR,
    size: winit::dpi::PhysicalSize<u32>,
) -> vk::Extent2D {
    if caps.current_extent.width != u32::MAX && caps.current_extent.height != u32::MAX {
        return caps.current_extent;
    }
    let w = size
        .width
        .clamp(caps.min_image_extent.width, caps.max_image_extent.width);
    let h = size
        .height
        .clamp(caps.min_image_extent.height, caps.max_image_extent.height);
    vk::Extent2D {
        width: w.max(1),
        height: h.max(1),
    }
}

fn create_image_views(
    device: &Device,
    images: &[vk::Image],
    format: vk::Format,
) -> Result<Vec<vk::ImageView>, Box<dyn std::error::Error>> {
    let mut views = Vec::with_capacity(images.len());
    for &image in images {
        let view = match unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    }),
                None,
            )
        } {
            Ok(view) => view,
            Err(error) => {
                destroy_image_views(device, views);
                return Err(Box::new(error));
            }
        };
        views.push(view);
    }
    Ok(views)
}

fn destroy_image_views(device: &Device, image_views: Vec<vk::ImageView>) {
    unsafe {
        for image_view in image_views {
            device.destroy_image_view(image_view, None);
        }
    }
}

fn create_render_pass(
    device: &Device,
    format: vk::Format,
) -> Result<vk::RenderPass, Box<dyn std::error::Error>> {
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];

    let color_attachment_refs = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];

    let subpass = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_attachment_refs)];

    let dependencies = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        )];

    unsafe {
        Ok(device.create_render_pass(
            &vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpass)
                .dependencies(&dependencies),
            None,
        )?)
    }
}

fn create_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    image_views: &[vk::ImageView],
) -> Result<Vec<vk::Framebuffer>, Box<dyn std::error::Error>> {
    let mut framebuffers = Vec::with_capacity(image_views.len());
    for &view in image_views {
        let framebuffer = match unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(std::slice::from_ref(&view))
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1),
                None,
            )
        } {
            Ok(framebuffer) => framebuffer,
            Err(error) => {
                destroy_framebuffers(device, framebuffers);
                return Err(Box::new(error));
            }
        };
        framebuffers.push(framebuffer);
    }
    Ok(framebuffers)
}

fn destroy_framebuffers(device: &Device, framebuffers: Vec<vk::Framebuffer>) {
    unsafe {
        for framebuffer in framebuffers {
            device.destroy_framebuffer(framebuffer, None);
        }
    }
}

fn create_present_semaphores(
    device: &Device,
    count: usize,
) -> Result<Vec<vk::Semaphore>, Box<dyn std::error::Error>> {
    let mut semaphores = Vec::with_capacity(count);
    for _ in 0..count {
        match unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) } {
            Ok(semaphore) => semaphores.push(semaphore),
            Err(error) => {
                unsafe {
                    for semaphore in semaphores {
                        device.destroy_semaphore(semaphore, None);
                    }
                }
                return Err(Box::new(error));
            }
        }
    }
    Ok(semaphores)
}

fn create_frame_sync(
    device: &Device,
    command_pool: vk::CommandPool,
) -> Result<FrameSync, Box<dyn std::error::Error>> {
    let semaphore_info = vk::SemaphoreCreateInfo::default();
    let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

    let image_available = unsafe { device.create_semaphore(&semaphore_info, None)? };
    let fence = match unsafe { device.create_fence(&fence_info, None) } {
        Ok(fence) => fence,
        Err(error) => {
            unsafe { device.destroy_semaphore(image_available, None) };
            return Err(Box::new(error));
        }
    };

    let command_buffer = match unsafe {
        device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )
    } {
        Ok(command_buffers) => command_buffers[0],
        Err(error) => {
            unsafe {
                device.destroy_fence(fence, None);
                device.destroy_semaphore(image_available, None);
            }
            return Err(Box::new(error));
        }
    };

    Ok(FrameSync {
        image_available,
        fence,
        command_buffer,
    })
}

fn create_frame_syncs(
    device: &Device,
    command_pool: vk::CommandPool,
    count: usize,
) -> Result<Vec<FrameSync>, Box<dyn std::error::Error>> {
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        match create_frame_sync(device, command_pool) {
            Ok(frame) => frames.push(frame),
            Err(error) => {
                destroy_frame_syncs(device, command_pool, &mut frames);
                return Err(error);
            }
        }
    }
    Ok(frames)
}

fn destroy_frame_syncs(
    device: &Device,
    command_pool: vk::CommandPool,
    frames: &mut Vec<FrameSync>,
) {
    unsafe {
        for frame in frames.drain(..) {
            device.destroy_semaphore(frame.image_available, None);
            device.destroy_fence(frame.fence, None);
            device.free_command_buffers(command_pool, &[frame.command_buffer]);
        }
    }
}

fn record_command_buffer<F, T>(
    device: &Device,
    cmd: vk::CommandBuffer,
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    extent: vk::Extent2D,
    clear_color: [f32; 4],
    record_draws: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(vk::CommandBuffer) -> Result<T, Box<dyn std::error::Error>>,
{
    let result = unsafe {
        device.begin_command_buffer(
            cmd,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: clear_color,
            },
        }];

        device.cmd_begin_render_pass(
            cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(render_pass)
                .framebuffer(framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                })
                .clear_values(&clear_values),
            vk::SubpassContents::INLINE,
        );

        let result = record_draws(cmd)?;

        device.cmd_end_render_pass(cmd);
        device.end_command_buffer(cmd)?;
        result
    };
    Ok(result)
}

fn is_srgb_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB | vk::Format::A8B8G8R8_SRGB_PACK32
    )
}
