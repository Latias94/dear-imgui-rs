use ash::{
    Device, Entry, Instance,
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
#[cfg(feature = "ash-dynamic-rendering")]
use dear_imgui_ash::DynamicRendering;
use dear_imgui_ash::{
    AshRenderer, AshRendererConfig, Options as AshOptions, RendererError, TextureRetirementBatch,
};
use dear_imgui_rs::*;
use dear_imgui_winit::WinitPlatform;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    ffi::CString,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Instant,
};
use tracing::{error, info};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[path = "../support/ash_frame_sync.rs"]
mod ash_frame_sync;
use ash_frame_sync::{
    FrameSync, clear_fence_references, create_frame_syncs, create_present_semaphores,
    destroy_frame_syncs, destroy_present_semaphores, replace_frame_sync,
};

const FRAMES_IN_FLIGHT: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeviceTeardownState {
    Active,
    Complete,
    Lost,
}

impl DeviceTeardownState {
    const fn permits_native_destruction(self) -> bool {
        matches!(self, Self::Complete | Self::Lost)
    }
}

fn classify_teardown_wait(
    result: Result<(), vk::Result>,
) -> Result<DeviceTeardownState, vk::Result> {
    match result {
        Ok(()) => Ok(DeviceTeardownState::Complete),
        Err(vk::Result::ERROR_DEVICE_LOST) => Ok(DeviceTeardownState::Lost),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod teardown_state_tests {
    use super::*;

    #[test]
    fn only_idle_and_device_lost_are_terminal_drop_proofs() {
        assert!(!DeviceTeardownState::Active.permits_native_destruction());
        assert!(DeviceTeardownState::Complete.permits_native_destruction());
        assert!(DeviceTeardownState::Lost.permits_native_destruction());
    }

    #[test]
    fn retryable_wait_errors_do_not_publish_a_terminal_proof() {
        assert_eq!(
            classify_teardown_wait(Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY)
        );
        assert_eq!(
            classify_teardown_wait(Err(vk::Result::ERROR_DEVICE_LOST)),
            Ok(DeviceTeardownState::Lost)
        );
        assert_eq!(
            classify_teardown_wait(Ok(())),
            Ok(DeviceTeardownState::Complete)
        );
    }
}

struct VulkanContextInit {
    entry: Option<Entry>,
    instance: Option<Instance>,
    surface_loader: Option<khr_surface::Instance>,
    surface: vk::SurfaceKHR,
    device: Option<Device>,
    command_pool: vk::CommandPool,
}

impl VulkanContextInit {
    fn new(entry: Entry) -> Self {
        Self {
            entry: Some(entry),
            instance: None,
            surface_loader: None,
            surface: vk::SurfaceKHR::null(),
            device: None,
            command_pool: vk::CommandPool::null(),
        }
    }

    fn entry(&self) -> &Entry {
        self.entry
            .as_ref()
            .expect("Vulkan initialization still owns its entry")
    }

    fn instance(&self) -> &Instance {
        self.instance
            .as_ref()
            .expect("Vulkan initialization already created its instance")
    }

    fn surface_loader(&self) -> &khr_surface::Instance {
        self.surface_loader
            .as_ref()
            .expect("Vulkan initialization already created its surface loader")
    }

    fn device(&self) -> &Device {
        self.device
            .as_ref()
            .expect("Vulkan initialization already created its device")
    }

    fn finish(mut self, physical_device: vk::PhysicalDevice, queue: vk::Queue) -> VulkanContext {
        let surface = std::mem::replace(&mut self.surface, vk::SurfaceKHR::null());
        let command_pool = std::mem::replace(&mut self.command_pool, vk::CommandPool::null());
        VulkanContext {
            _entry: self
                .entry
                .take()
                .expect("completed Vulkan initialization owns its entry"),
            instance: self
                .instance
                .take()
                .expect("completed Vulkan initialization owns its instance"),
            surface_loader: self
                .surface_loader
                .take()
                .expect("completed Vulkan initialization owns its surface loader"),
            surface,
            physical_device,
            device: self
                .device
                .take()
                .expect("completed Vulkan initialization owns its device"),
            queue,
            command_pool,
            teardown_state: DeviceTeardownState::Active,
        }
    }
}

impl Drop for VulkanContextInit {
    fn drop(&mut self) {
        unsafe {
            if let Some(device) = self.device.as_ref() {
                if self.command_pool != vk::CommandPool::null() {
                    device.destroy_command_pool(self.command_pool, None);
                    self.command_pool = vk::CommandPool::null();
                }
            }
            if let Some(device) = self.device.take() {
                device.destroy_device(None);
            }
            if self.surface != vk::SurfaceKHR::null() {
                if let Some(surface_loader) = self.surface_loader.as_ref() {
                    surface_loader.destroy_surface(self.surface, None);
                }
                self.surface = vk::SurfaceKHR::null();
            }
            if let Some(instance) = self.instance.take() {
                instance.destroy_instance(None);
            }
        }
    }
}

struct VulkanContext {
    _entry: Entry,
    instance: Instance,
    surface_loader: khr_surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    teardown_state: DeviceTeardownState,
}

impl VulkanContext {
    fn new(window: &Window, title: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { Entry::load()? };

        // Resolve every fallible native window handle before creating the first Vulkan object.
        let display_handle = window.display_handle()?.as_raw();
        let window_handle = window.window_handle()?.as_raw();

        let app_name = CString::new(title)?;
        let engine_name = CString::new("dear-imgui-examples")?;
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name.as_c_str())
            .engine_name(engine_name.as_c_str())
            .api_version(if cfg!(feature = "ash-dynamic-rendering") {
                vk::API_VERSION_1_3
            } else {
                vk::API_VERSION_1_0
            });

        let extensions = ash_window::enumerate_required_extensions(display_handle)?.to_vec();

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions);
        let mut init = VulkanContextInit::new(entry);
        let instance = unsafe { init.entry().create_instance(&instance_create_info, None)? };
        init.instance = Some(instance);

        let surface_loader = khr_surface::Instance::new(init.entry(), init.instance());
        init.surface_loader = Some(surface_loader);
        let surface = unsafe {
            ash_window::create_surface(
                init.entry(),
                init.instance(),
                display_handle,
                window_handle,
                None,
            )?
        };
        init.surface = surface;

        let (physical_device, queue_family_index) =
            pick_physical_device(init.instance(), init.surface_loader(), init.surface)?;

        let (device, queue) = create_device(init.instance(), physical_device, queue_family_index)?;
        init.device = Some(device);

        let command_pool = unsafe {
            init.device().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family_index)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?
        };
        init.command_pool = command_pool;

        Ok(init.finish(physical_device, queue))
    }

    fn mark_renderer_shutdown(&mut self, result: &Result<(), RendererError>) {
        match result {
            Ok(()) | Err(RendererError::RendererDestroyed) => {
                self.teardown_state = DeviceTeardownState::Complete;
            }
            Err(RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST)) => {
                self.teardown_state = DeviceTeardownState::Lost;
            }
            Err(_) => {}
        }
    }

    fn wait_idle_for_teardown(&mut self) -> Result<(), vk::Result> {
        if self.teardown_state != DeviceTeardownState::Active {
            return Ok(());
        }
        self.teardown_state = classify_teardown_wait(unsafe { self.device.device_wait_idle() })?;
        Ok(())
    }

    fn teardown_is_proven(&self) -> bool {
        self.teardown_state.permits_native_destruction()
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        if let Err(error) = self.wait_idle_for_teardown() {
            error!(
                ?error,
                "Vulkan device-idle proof failed during fallback teardown; leaking the native context"
            );
            return;
        }
        unsafe {
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

struct MainRenderTarget {
    #[cfg(not(feature = "ash-dynamic-rendering"))]
    device: Device,
    #[cfg(feature = "ash-dynamic-rendering")]
    format: vk::Format,
    #[cfg(not(feature = "ash-dynamic-rendering"))]
    render_pass: vk::RenderPass,
}

impl MainRenderTarget {
    fn new(device: &Device, format: vk::Format) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(feature = "ash-dynamic-rendering")]
        let _ = device;
        Ok(Self {
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            device: device.clone(),
            #[cfg(feature = "ash-dynamic-rendering")]
            format,
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            render_pass: create_render_pass(device, format)?,
        })
    }
}

impl Drop for MainRenderTarget {
    fn drop(&mut self) {
        #[cfg(not(feature = "ash-dynamic-rendering"))]
        unsafe {
            self.device.destroy_render_pass(self.render_pass, None);
            self.render_pass = vk::RenderPass::null();
        }
    }
}

struct SwapchainState {
    device: Device,
    loader: khr_swapchain::Device,
    swapchain: vk::SwapchainKHR,
    surface_format: vk::SurfaceFormatKHR,
    extent: vk::Extent2D,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    #[cfg(feature = "ash-dynamic-rendering")]
    image_layouts: Vec<vk::ImageLayout>,
    #[cfg(not(feature = "ash-dynamic-rendering"))]
    framebuffers: Vec<vk::Framebuffer>,
    present_semaphores: Vec<vk::Semaphore>,
}

impl SwapchainState {
    fn new(
        ctx: &VulkanContext,
        window: &Window,
        render_target: &MainRenderTarget,
        surface_format: vk::SurfaceFormatKHR,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_old(
            ctx,
            window,
            render_target,
            surface_format,
            vk::SwapchainKHR::null(),
        )
    }

    fn new_with_old(
        ctx: &VulkanContext,
        window: &Window,
        _render_target: &MainRenderTarget,
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

        #[cfg(not(feature = "ash-dynamic-rendering"))]
        let framebuffers = match create_framebuffers(
            &ctx.device,
            _render_target.render_pass,
            extent,
            &image_views,
        ) {
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
                #[cfg(not(feature = "ash-dynamic-rendering"))]
                destroy_framebuffers(&ctx.device, framebuffers);
                destroy_image_views(&ctx.device, image_views);
                unsafe { loader.destroy_swapchain(swapchain, None) };
                return Err(Box::new(error));
            }
        };

        #[cfg(feature = "ash-dynamic-rendering")]
        let image_count = images.len();
        Ok(Self {
            device: ctx.device.clone(),
            loader,
            swapchain,
            surface_format,
            extent,
            images,
            image_views,
            #[cfg(feature = "ash-dynamic-rendering")]
            image_layouts: vec![vk::ImageLayout::UNDEFINED; image_count],
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            framebuffers,
            present_semaphores,
        })
    }

    fn recreate(
        &mut self,
        ctx: &VulkanContext,
        window: &Window,
        render_target: &MainRenderTarget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { ctx.device.device_wait_idle()? };
        self.recreate_after_device_idle(ctx, window, render_target)
    }

    fn recreate_after_device_idle(
        &mut self,
        ctx: &VulkanContext,
        window: &Window,
        render_target: &MainRenderTarget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let replacement = match Self::new_with_old(
            ctx,
            window,
            render_target,
            self.surface_format,
            self.swapchain,
        ) {
            Ok(replacement) => replacement,
            Err(error) => {
                self.destroy();
                return Err(error);
            }
        };
        let mut previous = std::mem::replace(self, replacement);
        previous.destroy();
        Ok(())
    }

    fn destroy(&mut self) {
        unsafe {
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            for fb in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(fb, None);
            }
            for view in self.image_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            destroy_present_semaphores(&self.device, &mut self.present_semaphores);
            if self.swapchain != vk::SwapchainKHR::null() {
                self.loader.destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
    }
}

impl Drop for SwapchainState {
    fn drop(&mut self) {
        self.destroy();
    }
}

struct FrameSyncState {
    device: Device,
    command_pool: vk::CommandPool,
    frames: Vec<FrameSync>,
}

impl FrameSyncState {
    fn new(ctx: &VulkanContext, count: usize) -> Result<Self, vk::Result> {
        Ok(Self {
            device: ctx.device.clone(),
            command_pool: ctx.command_pool,
            frames: create_frame_syncs(&ctx.device, ctx.command_pool, count)?,
        })
    }
}

impl Deref for FrameSyncState {
    type Target = [FrameSync];

    fn deref(&self) -> &Self::Target {
        &self.frames
    }
}

impl DerefMut for FrameSyncState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.frames
    }
}

impl Drop for FrameSyncState {
    fn drop(&mut self) {
        destroy_frame_syncs(&self.device, self.command_pool, &mut self.frames);
    }
}

struct ImguiState {
    renderer: AshRenderer,
    platform: WinitPlatform,
    clear_color: [f32; 4],
    demo_open: bool,
    last_frame: Instant,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    // Context must outlive every attachment, including fallback field drops after a failed shutdown.
    context: Context,
}

struct VulkanState {
    // These fields are manually dropped only after device-idle or device-loss proves teardown is
    // terminal. A retryable wait failure intentionally leaks the complete Vulkan ownership tree.
    frames: ManuallyDrop<FrameSyncState>,
    swapchain: ManuallyDrop<SwapchainState>,
    render_target: ManuallyDrop<MainRenderTarget>,
    images_in_flight: Vec<vk::Fence>,
    frame_index: usize,
    swapchain_dirty: bool,
    ctx: ManuallyDrop<VulkanContext>,
    // Keep the native window alive if fallback teardown has to leak the Vulkan tree.
    window_keepalive: ManuallyDrop<Arc<Window>>,
}

impl VulkanState {
    fn new(window: &Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let ctx = VulkanContext::new(window, "dear-imgui-winit-ash")?;
        let surface_format = pick_surface_format(&ctx, window)?;
        let render_target = MainRenderTarget::new(&ctx.device, surface_format.format)?;
        let swapchain = SwapchainState::new(&ctx, window, &render_target, surface_format)?;
        let frames = FrameSyncState::new(&ctx, FRAMES_IN_FLIGHT)?;
        let images_in_flight = vec![vk::Fence::null(); swapchain.images.len()];

        Ok(Self {
            frames: ManuallyDrop::new(frames),
            swapchain: ManuallyDrop::new(swapchain),
            render_target: ManuallyDrop::new(render_target),
            images_in_flight,
            frame_index: 0,
            swapchain_dirty: false,
            ctx: ManuallyDrop::new(ctx),
            window_keepalive: ManuallyDrop::new(Arc::clone(window)),
        })
    }
}

impl Drop for VulkanState {
    fn drop(&mut self) {
        if let Err(error) = self.ctx.wait_idle_for_teardown() {
            error!(
                ?error,
                "Vulkan device-idle proof failed; leaking frame, swapchain, render-target, and device ownership"
            );
            return;
        }

        // SAFETY: every field is dropped exactly once here, and only after a terminal teardown
        // proof. Declaration order mirrors Vulkan parentage: GPU children precede the device.
        unsafe {
            ManuallyDrop::drop(&mut self.frames);
            ManuallyDrop::drop(&mut self.swapchain);
            ManuallyDrop::drop(&mut self.render_target);
            ManuallyDrop::drop(&mut self.ctx);
            ManuallyDrop::drop(&mut self.window_keepalive);
        }
    }
}

struct AppWindow {
    imgui: ManuallyDrop<ImguiState>,
    vk: ManuallyDrop<VulkanState>,
    window: ManuallyDrop<Arc<Window>>,
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            error!("Winit/Ash fallback shutdown failed: {error}");
        }
        if !self.vk.ctx.teardown_is_proven() {
            error!(
                "Winit/Ash fallback teardown lacks a terminal GPU proof; leaking the ImGui, Vulkan, and window ownership tree"
            );
            return;
        }

        // SAFETY: the terminal GPU proof permits ordered destruction. ImGui attachments are
        // released while the Vulkan device and platform window are alive, then Vulkan children
        // and their device are destroyed before the window.
        unsafe {
            ManuallyDrop::drop(&mut self.imgui);
            ManuallyDrop::drop(&mut self.vk);
            ManuallyDrop::drop(&mut self.window);
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let ImguiState {
            renderer,
            platform,
            renderer_shutdown_complete,
            platform_shutdown_complete,
            context,
            ..
        } = &mut *self.imgui;

        context.end_frame();
        let mut errors = Vec::new();

        if !*renderer_shutdown_complete {
            let result = renderer.shutdown(context);
            self.vk.ctx.mark_renderer_shutdown(&result);
            match result {
                Ok(()) | Err(RendererError::RendererDestroyed) => {
                    *renderer_shutdown_complete = true;
                }
                Err(error @ RendererError::Vulkan(vk::Result::ERROR_DEVICE_LOST)) => {
                    // Device loss is terminal: Ash already reclaimed its resources and committed
                    // the Context texture reset. Preserve the first diagnostic, but never retry it.
                    *renderer_shutdown_complete = true;
                    errors.push(format!(
                        "Ash renderer shutdown completed after device loss: {error}"
                    ));
                }
                Err(error) => errors.push(format!("Ash renderer shutdown failed: {error}")),
            }
        }
        if !self.vk.ctx.teardown_is_proven()
            && let Err(error) = self.vk.ctx.wait_idle_for_teardown()
        {
            errors.push(format!("Ash device-idle wait failed: {error}"));
            return Err(errors.join("; ").into());
        }
        if *renderer_shutdown_complete && !*platform_shutdown_complete {
            match platform.shutdown(context) {
                Ok(()) => *platform_shutdown_complete = true,
                Err(error) => errors.push(format!("Winit platform shutdown failed: {error}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; ").into())
        }
    }

    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let version = env!("CARGO_PKG_VERSION");
        let size = LogicalSize::new(1280.0, 720.0);
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(format!("Dear ImGui - Winit + Ash - {version}"))
                    .with_inner_size(size),
            )?,
        );

        let vk = VulkanState::new(&window)?;

        // Setup ImGui
        let mut context = Context::create();
        context.set_ini_filename(None::<String>)?;
        let mut platform = WinitPlatform::new(&mut context)?;
        platform.attach_window(
            Arc::clone(&window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        )?;

        let framebuffer_srgb = is_srgb_format(vk.swapchain.surface_format.format);
        #[cfg(not(feature = "ash-dynamic-rendering"))]
        let renderer_config = AshRendererConfig::with_render_pass(
            vk.ctx.device.clone(),
            vk.ctx.queue,
            vk.ctx.command_pool,
            vk.render_target.render_pass,
        );
        #[cfg(feature = "ash-dynamic-rendering")]
        let renderer_config = AshRendererConfig::with_dynamic_rendering(
            vk.ctx.device.clone(),
            vk.ctx.queue,
            vk.ctx.command_pool,
            DynamicRendering {
                color_attachment_format: vk.render_target.format,
                depth_attachment_format: None,
            },
        );
        let renderer_config = renderer_config.with_options(AshOptions {
            in_flight_frames: FRAMES_IN_FLIGHT,
            framebuffer_srgb,
            ..Default::default()
        });
        // Keep renderer creation as the final fallible step. The backend rolls back its own
        // partial initialization, while `platform`, `context`, and `vk` then unwind in that order.
        // SAFETY: all handles were created from ctx's device lineage; the graphics queue and
        // command pool are compatible, and the render target matches the swapchain format.
        let renderer = unsafe {
            AshRenderer::with_default_allocator(
                &vk.ctx.instance,
                vk.ctx.physical_device,
                renderer_config,
                &mut context,
            )?
        };

        let imgui = ImguiState {
            platform,
            renderer,
            clear_color: [0.1, 0.2, 0.3, 1.0],
            demo_open: true,
            last_frame: Instant::now(),
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
            context,
        };

        Ok(Self {
            imgui: ManuallyDrop::new(imgui),
            vk: ManuallyDrop::new(vk),
            window: ManuallyDrop::new(window),
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.vk.swapchain_dirty = true;
    }

    fn recover_aborted_acquire(
        vk: &mut VulkanState,
        renderer: &mut AshRenderer,
        window: &Arc<Window>,
        frame_slot: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        vk.swapchain_dirty = true;
        unsafe { vk.ctx.device.device_wait_idle()? };

        let frame = vk
            .frames
            .get_mut(frame_slot)
            .ok_or("Ash frame slot disappeared during acquire recovery")?;
        let abandoned_fence = frame.fence;
        clear_fence_references(&mut vk.images_in_flight, abandoned_fence);
        let abandoned_retirement = replace_frame_sync(&vk.ctx.device, vk.ctx.command_pool, frame)?;

        vk.swapchain
            .recreate_after_device_idle(&vk.ctx, window, &vk.render_target)?;
        vk.images_in_flight = vec![vk::Fence::null(); vk.swapchain.images.len()];
        vk.swapchain_dirty = false;

        if let Some(retirement) = abandoned_retirement {
            renderer.wait_for_texture_retirements(retirement)?;
        }
        if let Some(retirement) = renderer.pending_texture_retirement()? {
            renderer.wait_for_texture_retirements(retirement)?;
        }
        Ok(())
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window = &*self.window;
        let vk = &mut *self.vk;
        let ImguiState {
            renderer,
            platform,
            clear_color,
            demo_open,
            last_frame,
            context,
            ..
        } = &mut *self.imgui;

        if vk.swapchain_dirty {
            vk.swapchain.recreate(&vk.ctx, window, &vk.render_target)?;
            vk.images_in_flight = vec![vk::Fence::null(); vk.swapchain.images.len()];
            vk.swapchain_dirty = false;
        }

        let now = Instant::now();
        let dt = (now - *last_frame).as_secs_f32();
        context.io_mut().set_delta_time(dt);
        *last_frame = now;

        platform.prepare_frame(context, window)?;
        let ui = context.frame();

        ui.window("Hello, Dear ImGui (Ash)!")
            .size([420.0, 240.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Vulkan renderer: dear-imgui-ash");
                ui.separator();

                ui.text(format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));

                ui.text(format!(
                    "Swapchain format: {:?}",
                    vk.swapchain.surface_format.format
                ));
                ui.text(format!(
                    "Framebuffer sRGB: {} (shader gamma path)",
                    is_srgb_format(vk.swapchain.surface_format.format)
                ));

                ui.color_edit4("Clear color", clear_color);

                if ui.button("Show Demo Window") {
                    *demo_open = true;
                }

                ui.separator();
                ui.text("Modern texture management:");
                ui.bullet_text("ImGuiBackendFlags_RendererHasTextures");
                ui.bullet_text("PendingFrame create/update/destroy reconciliation");
            });

        if *demo_open {
            ui.show_demo_window(demo_open);
        }

        platform.prepare_render(&ui, window)?;
        let pending_frame = context.render(renderer.renderer_consumer()?);

        let frame_slot = vk.frame_index % vk.frames.len();
        let frame_fence = vk.frames[frame_slot].fence;
        let image_available = vk.frames[frame_slot].image_available;
        let command_buffer = vk.frames[frame_slot].command_buffer;

        unsafe {
            vk.ctx
                .device
                .wait_for_fences(&[frame_fence], true, u64::MAX)?;
        }
        if let Some(retirement) = vk.frames[frame_slot].texture_retirement.take() {
            // SAFETY: this frame fence covers its draw submission and every earlier upload on the
            // same renderer queue.
            unsafe {
                renderer.complete_texture_retirements_with_fences(retirement, &[frame_fence])?;
            }
        }

        let acquire = unsafe {
            vk.swapchain.loader.acquire_next_image(
                vk.swapchain.swapchain,
                u64::MAX,
                image_available,
                vk::Fence::null(),
            )
        };

        let (image_index, acquire_suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                vk.swapchain_dirty = true;
                return Ok(());
            }
            Err(e) => return Err(Box::new(e)),
        };
        let image_index_usize = image_index as usize;
        let submission = (|| -> Result<(Option<TextureRetirementBatch>, vk::Semaphore), Box<dyn std::error::Error>> {
            let image_fence = vk
                .images_in_flight
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no in-flight fence slot")?;
            let present_semaphore = vk
                .swapchain
                .present_semaphores
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no present semaphore")?;
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            let framebuffer = vk
                .swapchain
                .framebuffers
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no framebuffer")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image = vk
                .swapchain
                .images
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image is missing")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image_view = vk
                .swapchain
                .image_views
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no image view")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let old_layout = vk
                .swapchain
                .image_layouts
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no tracked layout")?;

            if image_fence != vk::Fence::null() {
                unsafe {
                    vk.ctx
                        .device
                        .wait_for_fences(&[image_fence], true, u64::MAX)?;
                }
            }
            unsafe {
                vk.ctx.device.reset_command_buffer(
                    command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )?;
            }

            let (reconciled_frame, texture_retirement) = renderer.prepare_frame(pending_frame)?;

            let texture_retirement = record_command_buffer(
                &vk.ctx.device,
                command_buffer,
                &vk.render_target,
                #[cfg(not(feature = "ash-dynamic-rendering"))]
                framebuffer,
                #[cfg(feature = "ash-dynamic-rendering")]
                image,
                #[cfg(feature = "ash-dynamic-rendering")]
                image_view,
                #[cfg(feature = "ash-dynamic-rendering")]
                old_layout,
                vk.swapchain.extent,
                *clear_color,
                // SAFETY: cmd is recording inside the compatible render pass and is submitted
                // before any renderer resource can be retired or destroyed.
                |cmd| {
                    unsafe { renderer.cmd_draw(cmd, reconciled_frame) }?;
                    Ok(texture_retirement)
                },
            )?;

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(std::slice::from_ref(&image_available))
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(std::slice::from_ref(&command_buffer))
                .signal_semaphores(std::slice::from_ref(&present_semaphore));
            unsafe {
                vk.ctx.device.reset_fences(&[frame_fence])?;
                vk.ctx.device.queue_submit(
                    vk.ctx.queue,
                    std::slice::from_ref(&submit_info),
                    frame_fence,
                )?;
            }
            Ok((texture_retirement, present_semaphore))
        })();
        let (texture_retirement, present_semaphore) = match submission {
            Ok(submission) => submission,
            Err(error) => {
                if let Err(recovery_error) =
                    Self::recover_aborted_acquire(vk, renderer, window, frame_slot)
                {
                    return Err(format!(
                        "Ash main acquire failed before submit: {error}; recovery also failed: {recovery_error}"
                    )
                    .into());
                }
                return Err(error);
            }
        };

        #[cfg(feature = "ash-dynamic-rendering")]
        {
            vk.swapchain.image_layouts[image_index_usize] = vk::ImageLayout::PRESENT_SRC_KHR;
        }
        vk.images_in_flight[image_index_usize] = frame_fence;
        vk.frames[frame_slot].texture_retirement = texture_retirement;
        vk.swapchain_dirty |= acquire_suboptimal;

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&present_semaphore))
            .swapchains(std::slice::from_ref(&vk.swapchain.swapchain))
            .image_indices(std::slice::from_ref(&image_index));

        let present = unsafe {
            vk.swapchain
                .loader
                .queue_present(vk.ctx.queue, &present_info)
        };
        match present {
            Ok(suboptimal) => vk.swapchain_dirty |= suboptimal,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                vk.swapchain_dirty = true;
            }
            Err(error) => {
                vk.swapchain_dirty = true;
                return Err(Box::new(error));
            }
        }

        vk.frame_index = (vk.frame_index + 1) % vk.frames.len();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        match AppWindow::new(event_loop) {
            Ok(window) => {
                self.window = Some(window);
                info!("Window created successfully in resumed");
            }
            Err(e) => {
                error!("Failed to create window in resumed: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.window.as_mut() {
            Some(window) => window,
            None => return,
        };

        let ImguiState {
            platform, context, ..
        } = &mut *window.imgui;
        if let Err(error) = platform.handle_window_event(context, &window.window, &event) {
            error!("Winit platform event error: {error}");
            event_loop.exit();
            return;
        }

        match event {
            WindowEvent::Resized(physical_size) => {
                window.resize(physical_size);
                window.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                window.vk.swapchain_dirty = true;
                window.window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                info!("Close requested");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = window.render() {
                    error!("Render error: {e}");
                    event_loop.exit();
                    return;
                }
                window.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_mut()
            && let Err(error) = window.shutdown()
        {
            error!("Winit/Ash shutdown failed: {error}");
        }
    }
}

fn main() {
    dear_imgui_examples::init_tracing_with_filter("dear_imgui=debug,winit_ash=info");
    info!("Starting Dear ImGui Winit + Ash lifecycle reference");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}

fn pick_physical_device(
    instance: &Instance,
    surface_loader: &khr_surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32), Box<dyn std::error::Error>> {
    let devices = unsafe { instance.enumerate_physical_devices()? };
    for device in devices {
        #[cfg(feature = "ash-dynamic-rendering")]
        {
            let properties = unsafe { instance.get_physical_device_properties(device) };
            if properties.api_version < vk::API_VERSION_1_3 {
                continue;
            }
            let mut dynamic_rendering = vk::PhysicalDeviceDynamicRenderingFeatures::default();
            let mut features =
                vk::PhysicalDeviceFeatures2::default().push_next(&mut dynamic_rendering);
            unsafe { instance.get_physical_device_features2(device, &mut features) };
            if dynamic_rendering.dynamic_rendering != vk::TRUE {
                continue;
            }
        }
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
    #[cfg(feature = "ash-dynamic-rendering")]
    let mut dynamic_rendering =
        vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
    #[cfg(feature = "ash-dynamic-rendering")]
    let device_create_info = device_create_info.push_next(&mut dynamic_rendering);

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
    if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
        return Ok(vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_SRGB,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        });
    }

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
    Ok(*formats.first().unwrap_or(&vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_UNORM,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    }))
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
    let clamp = |v: u32, min: u32, max: u32| v.max(min).min(max);
    vk::Extent2D {
        width: clamp(
            size.width,
            caps.min_image_extent.width,
            caps.max_image_extent.width,
        ),
        height: clamp(
            size.height,
            caps.min_image_extent.height,
            caps.max_image_extent.height,
        ),
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

#[cfg(not(feature = "ash-dynamic-rendering"))]
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

#[cfg(not(feature = "ash-dynamic-rendering"))]
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

#[cfg(not(feature = "ash-dynamic-rendering"))]
fn destroy_framebuffers(device: &Device, framebuffers: Vec<vk::Framebuffer>) {
    unsafe {
        for framebuffer in framebuffers {
            device.destroy_framebuffer(framebuffer, None);
        }
    }
}

fn record_command_buffer<F>(
    device: &Device,
    cmd: vk::CommandBuffer,
    _render_target: &MainRenderTarget,
    #[cfg(not(feature = "ash-dynamic-rendering"))] framebuffer: vk::Framebuffer,
    #[cfg(feature = "ash-dynamic-rendering")] image: vk::Image,
    #[cfg(feature = "ash-dynamic-rendering")] image_view: vk::ImageView,
    #[cfg(feature = "ash-dynamic-rendering")] old_layout: vk::ImageLayout,
    extent: vk::Extent2D,
    clear_color: [f32; 4],
    record_draws: F,
) -> Result<Option<TextureRetirementBatch>, Box<dyn std::error::Error>>
where
    F: FnOnce(vk::CommandBuffer) -> dear_imgui_ash::RendererResult<Option<TextureRetirementBatch>>,
{
    let texture_retirement;
    unsafe {
        device.begin_command_buffer(
            cmd,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        let clear_value = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: clear_color,
            },
        };

        #[cfg(not(feature = "ash-dynamic-rendering"))]
        device.cmd_begin_render_pass(
            cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(_render_target.render_pass)
                .framebuffer(framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                })
                .clear_values(std::slice::from_ref(&clear_value)),
            vk::SubpassContents::INLINE,
        );

        #[cfg(feature = "ash-dynamic-rendering")]
        {
            ash_frame_sync::transition_swapchain_image(
                device,
                cmd,
                image,
                old_layout,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            let color_attachment = vk::RenderingAttachmentInfo::default()
                .image_view(image_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear_value);
            device.cmd_begin_rendering(
                cmd,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_attachment)),
            );
        }

        texture_retirement = record_draws(cmd)?;

        #[cfg(not(feature = "ash-dynamic-rendering"))]
        device.cmd_end_render_pass(cmd);
        #[cfg(feature = "ash-dynamic-rendering")]
        {
            device.cmd_end_rendering(cmd);
            ash_frame_sync::transition_swapchain_image(
                device,
                cmd,
                image,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );
        }
        device.end_command_buffer(cmd)?;
    }
    Ok(texture_retirement)
}

fn is_srgb_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB | vk::Format::A8B8G8R8_SRGB_PACK32
    )
}
