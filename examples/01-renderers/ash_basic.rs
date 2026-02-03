use ash::{
    Device, Entry, Instance,
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
use dear_imgui_ash::{AshRenderer, Options as AshOptions};
use dear_imgui_rs::*;
use dear_imgui_winit::WinitPlatform;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{ffi::CString, sync::Arc, time::Instant};
use tracing::{error, info};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

const FRAMES_IN_FLIGHT: usize = 2;

struct VulkanContext {
    _entry: Entry,
    instance: Instance,
    surface_loader: khr_surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
}

impl VulkanContext {
    fn new(window: &Window, title: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::linked();

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
            _entry: entry,
            instance,
            surface_loader,
            surface,
            physical_device,
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
}

impl SwapchainState {
    fn new(
        ctx: &VulkanContext,
        window: &Window,
        render_pass: vk::RenderPass,
        surface_format: vk::SurfaceFormatKHR,
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
            .clipped(true);

        let swapchain = unsafe { loader.create_swapchain(&swapchain_create_info, None)? };
        let images = unsafe { loader.get_swapchain_images(swapchain)? };
        let image_views = create_image_views(&ctx.device, &images, surface_format.format)?;

        let framebuffers = create_framebuffers(&ctx.device, render_pass, extent, &image_views)?;

        Ok(Self {
            loader,
            swapchain,
            surface_format,
            extent,
            images,
            image_views,
            framebuffers,
        })
    }

    fn recreate(
        &mut self,
        ctx: &VulkanContext,
        window: &Window,
        render_pass: vk::RenderPass,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { ctx.device.device_wait_idle()? };
        self.destroy(&ctx.device);
        *self = Self::new(ctx, window, render_pass, self.surface_format)?;
        Ok(())
    }

    fn destroy(&mut self, device: &Device) {
        unsafe {
            for fb in self.framebuffers.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            for view in self.image_views.drain(..) {
                device.destroy_image_view(view, None);
            }
            self.loader.destroy_swapchain(self.swapchain, None);
        }
    }
}

impl Drop for SwapchainState {
    fn drop(&mut self) {
        // `destroy()` requires a `Device`; handled by `VulkanState::drop()`.
    }
}

struct FrameSync {
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    fence: vk::Fence,
    command_buffer: vk::CommandBuffer,
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: AshRenderer,
    clear_color: [f32; 4],
    demo_open: bool,
    last_frame: Instant,
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
        }

        unsafe {
            for f in &self.frames {
                self.ctx.device.destroy_semaphore(f.image_available, None);
                self.ctx.device.destroy_semaphore(f.render_finished, None);
                self.ctx.device.destroy_fence(f.fence, None);
            }
        }

        self.swapchain.destroy(&self.ctx.device);
        unsafe {
            self.ctx.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

struct AppWindow {
    window: Arc<Window>,
    imgui: ImguiState,
    vk: VulkanState,
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let version = env!("CARGO_PKG_VERSION");
        let size = LogicalSize::new(1280.0, 720.0);
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(format!("Dear ImGui Ash Basic - {version}"))
                    .with_inner_size(size),
            )?,
        );

        let ctx = VulkanContext::new(&window, "dear-imgui-ash-basic")?;
        let surface_format = pick_surface_format(&ctx, &window)?;
        let render_pass = create_render_pass(&ctx.device, surface_format.format)?;
        let swapchain = SwapchainState::new(&ctx, &window, render_pass, surface_format)?;

        // Setup ImGui
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, dear_imgui_winit::HiDpiMode::Default, &mut context);

        let framebuffer_srgb = is_srgb_format(swapchain.surface_format.format);
        let renderer = AshRenderer::with_default_allocator(
            &ctx.instance,
            ctx.physical_device,
            ctx.device.clone(),
            ctx.queue,
            ctx.command_pool,
            render_pass,
            &mut context,
            Some(AshOptions {
                in_flight_frames: FRAMES_IN_FLIGHT,
                framebuffer_srgb,
                ..Default::default()
            }),
        )?;

        // Frame sync objects
        let frames = (0..FRAMES_IN_FLIGHT)
            .map(|_| create_frame_sync(&ctx.device, ctx.command_pool))
            .collect::<Result<Vec<_>, _>>()?;

        let images_in_flight = vec![vk::Fence::null(); swapchain.images.len()];

        let imgui = ImguiState {
            context,
            platform,
            renderer,
            clear_color: [0.1, 0.2, 0.3, 1.0],
            demo_open: true,
            last_frame: Instant::now(),
        };

        Ok(Self {
            window,
            imgui,
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
                    self.vk.swapchain.surface_format.format
                ));
                ui.text(format!(
                    "Framebuffer sRGB: {} (shader gamma path)",
                    is_srgb_format(self.vk.swapchain.surface_format.format)
                ));

                ui.color_edit4("Clear color", &mut self.imgui.clear_color);

                if ui.button("Show Demo Window") {
                    self.imgui.demo_open = true;
                }

                ui.separator();
                ui.text("Modern texture management:");
                ui.bullet_text("ImGuiBackendFlags_RendererHasTextures");
                ui.bullet_text("DrawData::textures() create/update/destroy");
            });

        if self.imgui.demo_open {
            ui.show_demo_window(&mut self.imgui.demo_open);
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

        let (image_index, _suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.vk.swapchain_dirty = true;
                return Ok(());
            }
            Err(e) => return Err(Box::new(e)),
        };

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

        record_command_buffer(
            &self.vk.ctx.device,
            frame.command_buffer,
            self.vk.render_pass,
            self.vk.swapchain.framebuffers[image_index as usize],
            self.vk.swapchain.extent,
            self.imgui.clear_color,
            |cmd| self.imgui.renderer.cmd_draw(cmd, &draw_data),
        )?;

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&frame.image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&frame.command_buffer))
            .signal_semaphores(std::slice::from_ref(&frame.render_finished));

        unsafe {
            self.vk.ctx.device.queue_submit(
                self.vk.ctx.queue,
                std::slice::from_ref(&submit_info),
                frame.fence,
            )?;
        }

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&frame.render_finished))
            .swapchains(std::slice::from_ref(&self.vk.swapchain.swapchain))
            .image_indices(std::slice::from_ref(&image_index));

        let present = unsafe {
            self.vk
                .swapchain
                .loader
                .queue_present(self.vk.ctx.queue, &present_info)
        };
        match present {
            Ok(_) => {}
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                self.vk.swapchain_dirty = true;
            }
            Err(e) => return Err(Box::new(e)),
        }

        self.vk.frame_index = (self.vk.frame_index + 1) % self.vk.frames.len();
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

        let imgui = &mut window.imgui;
        imgui
            .platform
            .handle_window_event(&mut imgui.context, &window.window, &event);

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
}

fn main() {
    dear_imgui_rs::logging::init_tracing_with_filter("dear_imgui=debug,ash_basic=info");
    info!("Starting Dear ImGui Ash Basic Example");

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
        let view = unsafe {
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
            )?
        };
        views.push(view);
    }
    Ok(views)
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
        let fb = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(std::slice::from_ref(&view))
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1),
                None,
            )?
        };
        framebuffers.push(fb);
    }
    Ok(framebuffers)
}

fn create_frame_sync(
    device: &Device,
    command_pool: vk::CommandPool,
) -> Result<FrameSync, Box<dyn std::error::Error>> {
    let semaphore_info = vk::SemaphoreCreateInfo::default();
    let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

    let image_available = unsafe { device.create_semaphore(&semaphore_info, None)? };
    let render_finished = unsafe { device.create_semaphore(&semaphore_info, None)? };
    let fence = unsafe { device.create_fence(&fence_info, None)? };

    let command_buffer = unsafe {
        device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )?[0]
    };

    Ok(FrameSync {
        image_available,
        render_finished,
        fence,
        command_buffer,
    })
}

fn record_command_buffer<F>(
    device: &Device,
    cmd: vk::CommandBuffer,
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    extent: vk::Extent2D,
    clear_color: [f32; 4],
    mut record_draws: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(vk::CommandBuffer) -> dear_imgui_ash::RendererResult<()>,
{
    unsafe {
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

        record_draws(cmd)?;

        device.cmd_end_render_pass(cmd);
        device.end_command_buffer(cmd)?;
    }
    Ok(())
}

fn is_srgb_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB | vk::Format::A8B8G8R8_SRGB_PACK32
    )
}
