//! SDL3 + Vulkan (Ash) multi-viewport example (native only).
//!
//! This demonstrates driving Dear ImGui with:
//! - SDL3 for window + events
//! - Official SDL3 platform backend (via `dear-imgui-sdl3`)
//! - Rust Vulkan renderer backend (`dear-imgui-ash`) with SDL3 multi-viewport callbacks
//!
//! Run with:
//!   cargo run -p dear-imgui-examples --bin sdl3_ash_multi_viewport --features sdl3-ash-multi-viewport
//!
//! Notes:
//! - This is experimental and intended for native desktop targets.
//! - Secondary viewports create their own Vulkan `SurfaceKHR` + swapchain.
//! - Per-viewport surface creation is delegated to SDL3 via `Platform_CreateVkSurface`.

use std::cell::RefCell;
use std::error::Error;
use std::ffi::CString;
use std::time::Instant;

use ash::khr::{surface as khr_surface, swapchain as khr_swapchain};
use ash::{Device, Entry, Instance, vk};
#[cfg(feature = "ash-dynamic-rendering")]
use dear_imgui_ash::DynamicRendering;
use dear_imgui_ash::multi_viewport_sdl3::{Sdl3ViewportRuntime, VulkanViewportConfig};
use dear_imgui_ash::{
    AshRenderer, AshRendererConfig, Options as AshOptions, TextureRetirementBatch,
};
use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_rs::{Condition, ConfigFlags, Context, RawDrawCallback, render::RenderedFrame};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, GamepadMode, Sdl3PlatformBackend};
use sdl3::video::{SwapInterval, WindowPos};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};

#[path = "../support/ash_frame_sync.rs"]
mod ash_frame_sync;
use ash_frame_sync::{
    FrameSync, clear_fence_references, create_frame_syncs, create_present_semaphores,
    destroy_frame_syncs, destroy_present_semaphores, replace_frame_sync,
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
    fn new(window: &sdl3::video::Window, title: &str) -> Result<Self, Box<dyn Error>> {
        // Use runtime loader mode so CI/users don't need `vulkan-1.lib` at link time.
        let entry = unsafe { Entry::load()? };

        let app_name = CString::new(title)?;
        let engine_name = CString::new("dear-imgui-examples")?;
        #[cfg(feature = "ash-dynamic-rendering")]
        let api_version = vk::API_VERSION_1_3;
        #[cfg(not(feature = "ash-dynamic-rendering"))]
        let api_version = vk::API_VERSION_1_0;
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name.as_c_str())
            .engine_name(engine_name.as_c_str())
            .api_version(api_version);

        let extension_names = window.vulkan_instance_extensions()?;
        let extensions_cstr: Vec<CString> = extension_names
            .into_iter()
            .map(CString::new)
            .collect::<Result<Vec<_>, _>>()?;
        let extension_ptrs: Vec<*const i8> = extensions_cstr.iter().map(|s| s.as_ptr()).collect();

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extension_ptrs);
        let instance = unsafe { entry.create_instance(&instance_create_info, None)? };

        let surface_loader = khr_surface::Instance::new(&entry, &instance);
        let surface = unsafe { window.vulkan_create_surface(instance.handle())? };

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

#[derive(Clone, Copy)]
struct MainRenderTarget {
    #[cfg(feature = "ash-dynamic-rendering")]
    format: vk::Format,
    #[cfg(not(feature = "ash-dynamic-rendering"))]
    render_pass: vk::RenderPass,
}

impl MainRenderTarget {
    fn new(_device: &Device, format: vk::Format) -> Result<Self, vk::Result> {
        Ok(Self {
            #[cfg(feature = "ash-dynamic-rendering")]
            format,
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            render_pass: create_render_pass(_device, format)?,
        })
    }

    fn destroy(self, _device: &Device) {
        #[cfg(not(feature = "ash-dynamic-rendering"))]
        unsafe {
            _device.destroy_render_pass(self.render_pass, None);
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
    #[cfg(feature = "ash-dynamic-rendering")]
    image_layouts: Vec<vk::ImageLayout>,
    #[cfg(not(feature = "ash-dynamic-rendering"))]
    framebuffers: Vec<vk::Framebuffer>,
    present_semaphores: Vec<vk::Semaphore>,
    present_mode: vk::PresentModeKHR,
}

impl SwapchainState {
    fn new(
        ctx: &VulkanContext,
        window: &sdl3::video::Window,
        render_target: MainRenderTarget,
        surface_format: vk::SurfaceFormatKHR,
    ) -> Result<Self, Box<dyn Error>> {
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
        window: &sdl3::video::Window,
        _render_target: MainRenderTarget,
        surface_format: vk::SurfaceFormatKHR,
        old_swapchain: vk::SwapchainKHR,
    ) -> Result<Self, Box<dyn Error>> {
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
        let extent = pick_extent(&caps, window.size_in_pixels());

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
                return Err(Box::new(error));
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
                return Err(Box::new(error));
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
            present_mode,
        })
    }

    fn destroy_resources(&mut self, device: &Device) {
        unsafe {
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            for framebuffer in self.framebuffers.drain(..) {
                device.destroy_framebuffer(framebuffer, None);
            }
            for image_view in self.image_views.drain(..) {
                device.destroy_image_view(image_view, None);
            }
            destroy_present_semaphores(device, &mut self.present_semaphores);
            if self.swapchain != vk::SwapchainKHR::null() {
                self.loader.destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
    }

    fn recreate(
        &mut self,
        ctx: &VulkanContext,
        window: &sdl3::video::Window,
        render_target: MainRenderTarget,
    ) -> Result<(), Box<dyn Error>> {
        unsafe { ctx.device.device_wait_idle()? };
        self.recreate_after_device_idle(ctx, window, render_target)
    }

    fn recreate_after_device_idle(
        &mut self,
        ctx: &VulkanContext,
        window: &sdl3::video::Window,
        render_target: MainRenderTarget,
    ) -> Result<(), Box<dyn Error>> {
        if !surface_supports_format(ctx, self.surface_format)? {
            return Err(format!(
                "main surface no longer supports the renderer pair {:?}; rebuilding the renderer is required",
                self.surface_format
            )
            .into());
        }
        let replacement = match Self::new_with_old(
            ctx,
            window,
            render_target,
            self.surface_format,
            self.swapchain,
        ) {
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

fn record_command_buffer<F>(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    _render_target: MainRenderTarget,
    #[cfg(not(feature = "ash-dynamic-rendering"))] framebuffer: vk::Framebuffer,
    #[cfg(feature = "ash-dynamic-rendering")] image: vk::Image,
    #[cfg(feature = "ash-dynamic-rendering")] image_view: vk::ImageView,
    #[cfg(feature = "ash-dynamic-rendering")] old_layout: vk::ImageLayout,
    extent: vk::Extent2D,
    clear_color: [f32; 4],
    record: F,
) -> Result<Option<TextureRetirementBatch>, Box<dyn Error>>
where
    F: FnOnce(vk::CommandBuffer) -> Result<Option<TextureRetirementBatch>, Box<dyn Error>>,
{
    let texture_retirement;
    unsafe {
        device.begin_command_buffer(
            command_buffer,
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
            command_buffer,
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
                command_buffer,
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
                command_buffer,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_attachment)),
            );
        }

        texture_retirement = record(command_buffer)?;

        #[cfg(not(feature = "ash-dynamic-rendering"))]
        device.cmd_end_render_pass(command_buffer);
        #[cfg(feature = "ash-dynamic-rendering")]
        {
            device.cmd_end_rendering(command_buffer);
            ash_frame_sync::transition_swapchain_image(
                device,
                command_buffer,
                image,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );
        }
        device.end_command_buffer(command_buffer)?;
    }
    Ok(texture_retirement)
}

#[cfg(not(feature = "ash-dynamic-rendering"))]
fn create_render_pass(device: &Device, format: vk::Format) -> Result<vk::RenderPass, vk::Result> {
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

    let rp_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpass)
        .dependencies(&dependencies);
    unsafe { Ok(device.create_render_pass(&rp_info, None)?) }
}

fn create_image_views(
    device: &Device,
    images: &[vk::Image],
    format: vk::Format,
) -> Result<Vec<vk::ImageView>, vk::Result> {
    let mut image_views = Vec::with_capacity(images.len());
    for &image in images {
        let create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = match unsafe { device.create_image_view(&create_info, None) } {
            Ok(view) => view,
            Err(error) => {
                destroy_image_views(device, image_views);
                return Err(error);
            }
        };
        image_views.push(view);
    }
    Ok(image_views)
}

fn destroy_image_views(device: &Device, image_views: Vec<vk::ImageView>) {
    unsafe {
        for image_view in image_views {
            device.destroy_image_view(image_view, None);
        }
    }
}

#[cfg(not(feature = "ash-dynamic-rendering"))]
fn create_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    image_views: &[vk::ImageView],
) -> Result<Vec<vk::Framebuffer>, vk::Result> {
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
                return Err(error);
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

fn pick_surface_format(ctx: &VulkanContext) -> Result<vk::SurfaceFormatKHR, vk::Result> {
    let formats = unsafe {
        ctx.surface_loader
            .get_physical_device_surface_formats(ctx.physical_device, ctx.surface)?
    };
    if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
        if formats[0].color_space != vk::ColorSpaceKHR::SRGB_NONLINEAR {
            return Err(vk::Result::ERROR_FORMAT_NOT_SUPPORTED);
        }
        return Ok(vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_SRGB,
            color_space: formats[0].color_space,
        });
    }

    let preferred = [
        vk::Format::B8G8R8A8_SRGB,
        vk::Format::R8G8B8A8_SRGB,
        vk::Format::B8G8R8A8_UNORM,
        vk::Format::R8G8B8A8_UNORM,
    ];
    for p in preferred {
        if let Some(f) = formats
            .iter()
            .find(|f| f.format == p && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
        {
            return Ok(*f);
        }
    }
    Err(vk::Result::ERROR_FORMAT_NOT_SUPPORTED)
}

fn surface_supports_format(
    ctx: &VulkanContext,
    requested: vk::SurfaceFormatKHR,
) -> Result<bool, vk::Result> {
    let formats = unsafe {
        ctx.surface_loader
            .get_physical_device_surface_formats(ctx.physical_device, ctx.surface)?
    };
    Ok(
        if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
            formats[0].color_space == requested.color_space
        } else {
            formats.contains(&requested)
        },
    )
}

fn pick_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    if modes.contains(&vk::PresentModeKHR::MAILBOX) {
        vk::PresentModeKHR::MAILBOX
    } else {
        vk::PresentModeKHR::FIFO
    }
}

fn pick_extent(caps: &vk::SurfaceCapabilitiesKHR, size: (u32, u32)) -> vk::Extent2D {
    if caps.current_extent.width != u32::MAX && caps.current_extent.height != u32::MAX {
        return caps.current_extent;
    }

    let (w, h) = size;
    vk::Extent2D {
        width: w
            .clamp(caps.min_image_extent.width, caps.max_image_extent.width)
            .max(1),
        height: h
            .clamp(caps.min_image_extent.height, caps.max_image_extent.height)
            .max(1),
    }
}

fn pick_physical_device(
    instance: &Instance,
    surface_loader: &khr_surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32), Box<dyn Error>> {
    let pds = unsafe { instance.enumerate_physical_devices()? };
    for pd in pds {
        #[cfg(feature = "ash-dynamic-rendering")]
        {
            let properties = unsafe { instance.get_physical_device_properties(pd) };
            if properties.api_version < vk::API_VERSION_1_3 {
                continue;
            }
            let mut dynamic_rendering = vk::PhysicalDeviceDynamicRenderingFeatures::default();
            let mut features =
                vk::PhysicalDeviceFeatures2::default().push_next(&mut dynamic_rendering);
            unsafe { instance.get_physical_device_features2(pd, &mut features) };
            if dynamic_rendering.dynamic_rendering != vk::TRUE {
                continue;
            }
        }
        let qfs = unsafe { instance.get_physical_device_queue_family_properties(pd) };
        for (i, qf) in qfs.iter().enumerate() {
            if !qf.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                continue;
            }
            let supports_present = unsafe {
                surface_loader.get_physical_device_surface_support(pd, i as u32, surface)?
            };
            if supports_present {
                return Ok((pd, i as u32));
            }
        }
    }
    Err("no suitable Vulkan physical device found".into())
}

fn create_device(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
) -> Result<(Device, vk::Queue), Box<dyn Error>> {
    let priorities = [1.0f32];
    let queue_info = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family_index)
        .queue_priorities(&priorities)];

    let device_extensions = [khr_swapchain::NAME.as_ptr()];
    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_info)
        .enabled_extension_names(&device_extensions);
    #[cfg(feature = "ash-dynamic-rendering")]
    let mut dynamic_rendering =
        vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
    #[cfg(feature = "ash-dynamic-rendering")]
    let device_create_info = device_create_info.push_next(&mut dynamic_rendering);

    let device = unsafe { instance.create_device(physical_device, &device_create_info, None)? };
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
    Ok((device, queue))
}

fn is_srgb_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB | vk::Format::A8B8G8R8_SRGB_PACK32
    )
}

fn create_external_rgba_texture(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    device: &Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    renderer: &mut RendererRuntime,
) -> Result<ExternalTexture, Box<dyn Error>> {
    fn find_memory_type(
        props: &vk::PhysicalDeviceMemoryProperties,
        type_filter: u32,
        flags: vk::MemoryPropertyFlags,
    ) -> Option<u32> {
        for i in 0..props.memory_type_count {
            let i = i as u32;
            let matches = (type_filter & (1u32 << i)) != 0;
            let has_flags = props.memory_types[i as usize]
                .property_flags
                .contains(flags);
            if matches && has_flags {
                return Some(i);
            }
        }
        None
    }

    let memory_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
    let width: u32 = 64;
    let height: u32 = 64;

    // CPU pixels: checkerboard.
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            let i = ((y * width + x) * 4) as usize;
            let (r, g, b) = if on { (240, 240, 240) } else { (20, 20, 20) };
            pixels[i + 0] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = 255;
        }
    }

    // Staging buffer (host-visible).
    let buffer_info = vk::BufferCreateInfo::default()
        .size(pixels.len() as u64)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let staging_buffer = unsafe { device.create_buffer(&buffer_info, None)? };
    let staging_reqs = unsafe { device.get_buffer_memory_requirements(staging_buffer) };
    let staging_type = find_memory_type(
        &memory_props,
        staging_reqs.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )
    .ok_or("no suitable staging buffer memory type")?;
    let staging_mem = unsafe {
        device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(staging_reqs.size)
                .memory_type_index(staging_type),
            None,
        )?
    };
    unsafe {
        device.bind_buffer_memory(staging_buffer, staging_mem, 0)?;
        let ptr = device.map_memory(
            staging_mem,
            0,
            staging_reqs.size,
            vk::MemoryMapFlags::empty(),
        )? as *mut u8;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), ptr, pixels.len());
        device.unmap_memory(staging_mem);
    }

    // GPU image (device-local).
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = unsafe { device.create_image(&image_info, None)? };
    let image_reqs = unsafe { device.get_image_memory_requirements(image) };
    let image_type = find_memory_type(
        &memory_props,
        image_reqs.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .ok_or("no suitable image memory type")?;
    let image_mem = unsafe {
        device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(image_reqs.size)
                .memory_type_index(image_type),
            None,
        )?
    };
    unsafe {
        device.bind_image_memory(image, image_mem, 0)?;
    }

    // Upload + layout transitions.
    let cmd = unsafe {
        device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )?[0]
    };
    let fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None)? };

    unsafe {
        device.begin_command_buffer(
            cmd,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        let barrier_to_dst = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier_to_dst),
        );

        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            });
        device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            std::slice::from_ref(&region),
        );

        let barrier_to_shader = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier_to_shader),
        );

        device.end_command_buffer(cmd)?;

        let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
        device.queue_submit(queue, std::slice::from_ref(&submit), fence)?;
        device.wait_for_fences(&[fence], true, u64::MAX)?;

        device.free_command_buffers(command_pool, &[cmd]);
        device.destroy_fence(fence, None);
    }

    // Destroy staging resources.
    unsafe {
        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_mem, None);
    }

    let image_view = unsafe {
        device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::R8G8B8A8_UNORM)
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

    // SAFETY: the image view belongs to the renderer's device, remains alive until explicit
    // unregistration, and the upload above leaves it in the declared shader-read-only layout.
    let tex_id = unsafe {
        renderer.register_external_texture(image_view, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)?
    };

    Ok(ExternalTexture {
        tex_id,
        image,
        image_mem,
        image_view,
        use_linear_sampler: false,
    })
}

enum RendererRuntime {
    Single(AshRenderer),
    Viewports(Sdl3ViewportRuntime),
}

impl RendererRuntime {
    fn poll_fault(&self) -> Result<(), Box<dyn Error>> {
        if let Self::Viewports(runtime) = self {
            runtime.poll_fault()?;
        }
        Ok(())
    }

    unsafe fn cmd_draw(
        &mut self,
        command_buffer: vk::CommandBuffer,
        frame: RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, Box<dyn Error>> {
        Ok(match self {
            Self::Single(renderer) => unsafe { renderer.cmd_draw(command_buffer, frame)? },
            Self::Viewports(runtime) => unsafe { runtime.cmd_draw(command_buffer, frame)? },
        })
    }

    fn prepare_frame(
        &mut self,
        frame: &mut RenderedFrame<'_>,
    ) -> Result<Option<TextureRetirementBatch>, Box<dyn Error>> {
        Ok(match self {
            Self::Single(renderer) => renderer.prepare_frame(frame)?,
            Self::Viewports(runtime) => runtime.prepare_frame(frame)?,
        })
    }

    fn pending_texture_retirement(&self) -> Result<Option<TextureRetirementBatch>, Box<dyn Error>> {
        Ok(match self {
            Self::Single(renderer) => renderer.pending_texture_retirement()?,
            Self::Viewports(runtime) => runtime.pending_texture_retirement()?,
        })
    }

    fn wait_for_texture_retirements(
        &mut self,
        batch: TextureRetirementBatch,
    ) -> Result<(), Box<dyn Error>> {
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

    fn set_viewport_clear_color(&mut self, color: [f32; 4]) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Single(renderer) => renderer.set_viewport_clear_color(color),
            Self::Viewports(runtime) => runtime.set_viewport_clear_color(color)?,
        }
        Ok(())
    }

    unsafe fn register_external_texture(
        &mut self,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> Result<dear_imgui_rs::TextureId, Box<dyn Error>> {
        Ok(match self {
            Self::Single(renderer) => unsafe {
                renderer.register_external_texture(image_view, image_layout)?
            },
            Self::Viewports(runtime) => unsafe {
                runtime.register_external_texture(image_view, image_layout)?
            },
        })
    }

    unsafe fn unregister_texture(
        &mut self,
        texture: dear_imgui_rs::TextureId,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Single(renderer) => unsafe { renderer.unregister_texture(texture)? },
            Self::Viewports(runtime) => unsafe { runtime.unregister_texture(texture)? },
        }
        Ok(())
    }

    fn shutdown(&mut self, context: &mut Context) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Single(renderer) => renderer.shutdown(context)?,
            Self::Viewports(runtime) => runtime.shutdown(context)?,
        }
        Ok(())
    }
}

struct ImguiState {
    context: Context,
    renderer: RendererRuntime,
    last_frame: Instant,
    clear_color: [f32; 4],
    img_tex: dear_imgui_rs::ManagedTextureId,
    tex_size: (u32, u32),
    frame: u32,
    show_demo: bool,
    external: Option<ExternalTexture>,
    sampler_linear_callback: RawDrawCallback,
    sampler_nearest_callback: RawDrawCallback,
}

struct VulkanState {
    ctx: VulkanContext,
    render_target: MainRenderTarget,
    swapchain: SwapchainState,
    frames: Vec<FrameSync>,
    images_in_flight: Vec<vk::Fence>,
    frame_index: usize,
    swapchain_dirty: bool,
}

struct ExternalTexture {
    tex_id: dear_imgui_rs::TextureId,
    image: vk::Image,
    image_mem: vk::DeviceMemory,
    image_view: vk::ImageView,
    use_linear_sampler: bool,
}

impl Drop for VulkanState {
    fn drop(&mut self) {
        unsafe {
            let _ = self.ctx.device.device_wait_idle();
            destroy_frame_syncs(&self.ctx.device, self.ctx.command_pool, &mut self.frames);
            self.swapchain.destroy_resources(&self.ctx.device);
        }
        self.render_target.destroy(&self.ctx.device);
    }
}

struct App {
    enable_viewports: bool,
    // Keep the backend owner before ImguiState so its Drop runs while Context is alive.
    sdl3_backend: Option<Sdl3PlatformBackend>,
    imgui: ImguiState,
    vk: VulkanState,
    // Keep the platform window alive until renderer, swapchains, and surfaces have been dropped.
    window: sdl3::video::Window,
    gpu_idle_for_shutdown: bool,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
}

struct Sdl3AshApp {
    events: Sdl3CallbackEventHandoff,
    main: MainThreadData<RefCell<MainData>>,
}

struct MainData {
    // Field order keeps renderer, swapchains, and windows alive only while SDL is initialized.
    app: App,
    _video: sdl3::VideoSubsystem,
    _sdl: sdl3::Sdl,
}

impl Drop for App {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("SDL3 + Ash example fallback shutdown failed: {error}");
        }
    }
}

impl App {
    fn new(video: &sdl3::VideoSubsystem) -> Result<Self, Box<dyn Error>> {
        const ENABLE_VIEWPORTS: bool = true;

        // Create an SDL3 Vulkan window.
        let main_scale = video
            .get_primary_display()?
            .get_content_scale()
            .unwrap_or(1.0);

        let mut window = video
            .window(
                "Dear ImGui SDL3 + Ash (multi-viewport)",
                (1200.0 * main_scale) as u32,
                (720.0 * main_scale) as u32,
            )
            .resizable()
            .high_pixel_density()
            .vulkan()
            .build()
            .map_err(|e| format!("failed to create SDL3 window: {e}"))?;
        window.set_position(WindowPos::Centered, WindowPos::Centered);

        // Best-effort: disable vsync at SDL level (present mode controls timing).
        let _ = video.gl_set_swap_interval(SwapInterval::Immediate);

        // Vulkan instance/device/surface/swapchain.
        let ctx = VulkanContext::new(&window, "dear-imgui-sdl3-ash-multi-viewport")?;
        let surface_format = pick_surface_format(&ctx)?;
        let render_target = MainRenderTarget::new(&ctx.device, surface_format.format)?;
        let swapchain = SwapchainState::new(&ctx, &window, render_target, surface_format)?;

        // Dear ImGui context.
        let mut context = Context::create();
        context.set_ini_filename(None::<String>)?;

        {
            let io = context.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);

            let style = context.style_mut();
            style.set_font_scale_dpi(main_scale);
        }

        if ENABLE_VIEWPORTS {
            context.enable_multi_viewport();
        }

        // SDL3 platform backend for Vulkan (sets Platform_CreateVkSurface for multi-viewport).
        // SAFETY: `window` outlives ordered shutdown and Context teardown through App ownership.
        let mut sdl3_backend =
            unsafe { Sdl3PlatformBackend::init_for_vulkan(&mut context, &window)? };
        sdl3_backend.set_gamepad_mode(&mut context, GamepadMode::AutoAll)?;

        // Create a managed ImGui texture (CPU-side pixels; backend will create GPU texture).
        let tex_w: u32 = 128;
        let tex_h: u32 = 128;
        let mut img_tex = dear_imgui_rs::texture::OwnedTextureData::new();
        img_tex.create(dear_imgui_rs::texture::TextureFormat::RGBA32, tex_w, tex_h);
        let mut pixels = vec![0u8; (tex_w * tex_h * 4) as usize];
        for y in 0..tex_h {
            for x in 0..tex_w {
                let i = ((y * tex_w + x) * 4) as usize;
                pixels[i + 0] = (x as f32 / tex_w as f32 * 255.0) as u8;
                pixels[i + 1] = (y as f32 / tex_h as f32 * 255.0) as u8;
                pixels[i + 2] = 128;
                pixels[i + 3] = 255;
            }
        }
        img_tex.set_data(&pixels);

        let img_tex = context.register_texture(img_tex);

        // Renderer.
        let framebuffer_srgb = is_srgb_format(swapchain.surface_format.format);
        #[cfg(not(feature = "ash-dynamic-rendering"))]
        let renderer_config = AshRendererConfig::with_render_pass(
            ctx.device.clone(),
            ctx.queue,
            ctx.command_pool,
            render_target.render_pass,
        );
        #[cfg(feature = "ash-dynamic-rendering")]
        let renderer_config = AshRendererConfig::with_dynamic_rendering(
            ctx.device.clone(),
            ctx.queue,
            ctx.command_pool,
            DynamicRendering {
                color_attachment_format: render_target.format,
                depth_attachment_format: None,
            },
        );
        let renderer_config = renderer_config.with_options(AshOptions {
            in_flight_frames: FRAMES_IN_FLIGHT,
            framebuffer_srgb,
            ..Default::default()
        });
        // SAFETY: all handles share ctx's device lineage; the queue and command pool support the
        // upload/graphics work, and the render target matches the swapchain and viewport targets.
        let mut renderer = unsafe {
            AshRenderer::with_default_allocator(
                &ctx.instance,
                ctx.physical_device,
                renderer_config,
                &mut context,
            )?
        };
        renderer.set_viewport_clear_color([0.1, 0.12, 0.15, 1.0]);
        let sampler_linear_callback = context
            .platform_io()
            .draw_callback_set_sampler_linear_raw()
            .ok_or("Ash did not publish its linear sampler callback")?;
        let sampler_nearest_callback = context
            .platform_io()
            .draw_callback_set_sampler_nearest_raw()
            .ok_or("Ash did not publish its nearest sampler callback")?;
        let renderer = if ENABLE_VIEWPORTS {
            // SAFETY: the application serializes all host access to ctx.queue and ctx.device while
            // this runtime can submit, present, rebuild swapchains, or wait for device idle.
            RendererRuntime::Viewports(unsafe {
                Sdl3ViewportRuntime::attach(
                    &mut context,
                    &sdl3_backend,
                    renderer,
                    VulkanViewportConfig {
                        entry: ctx.entry.clone(),
                        instance: ctx.instance.clone(),
                        physical_device: ctx.physical_device,
                        validation_surface: ctx.surface,
                        present_queue: ctx.queue,
                        graphics_queue_family_index: ctx.queue_family_index,
                        present_queue_family_index: ctx.queue_family_index,
                        swapchain_policy:
                            dear_imgui_ash::multi_viewport_sdl3::ViewportSwapchainPolicy::from_main_surface(
                                swapchain.surface_format,
                                swapchain.present_mode,
                            ),
                        swapchain_image_usage: vk::ImageUsageFlags::empty(),
                    },
                )?
            })
        } else {
            RendererRuntime::Single(renderer)
        };

        // Frame sync objects.
        let frames = create_frame_syncs(&ctx.device, ctx.command_pool, FRAMES_IN_FLIGHT)?;
        let images_in_flight = vec![vk::Fence::null(); swapchain.images.len()];

        Ok(Self {
            window,
            enable_viewports: ENABLE_VIEWPORTS,
            sdl3_backend: Some(sdl3_backend),
            imgui: ImguiState {
                context,
                renderer,
                last_frame: Instant::now(),
                clear_color: [0.1, 0.12, 0.15, 1.0],
                img_tex,
                tex_size: (tex_w, tex_h),
                frame: 0,
                show_demo: true,
                external: None,
                sampler_linear_callback,
                sampler_nearest_callback,
            },
            vk: VulkanState {
                ctx,
                render_target,
                swapchain,
                frames,
                images_in_flight,
                frame_index: 0,
                swapchain_dirty: false,
            },
            gpu_idle_for_shutdown: false,
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
        })
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.gpu_idle_for_shutdown {
            unsafe { self.vk.ctx.device.device_wait_idle()? };
            self.gpu_idle_for_shutdown = true;
        }

        self.destroy_external_texture()?;

        if !self.renderer_shutdown_complete {
            self.imgui.renderer.shutdown(&mut self.imgui.context)?;
            self.renderer_shutdown_complete = true;
            self.enable_viewports = false;
        }

        if !self.platform_shutdown_complete {
            self.shutdown_platform_backend()?;
            self.platform_shutdown_complete = true;
        }

        Ok(())
    }

    fn init_external_texture(&mut self) -> Result<(), Box<dyn Error>> {
        if self.imgui.external.is_some() {
            return Ok(());
        }

        let external = create_external_rgba_texture(
            &self.vk.ctx.instance,
            self.vk.ctx.physical_device,
            &self.vk.ctx.device,
            self.vk.ctx.queue,
            self.vk.ctx.command_pool,
            &mut self.imgui.renderer,
        )?;
        self.imgui.external = Some(external);
        Ok(())
    }

    fn destroy_external_texture(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(external) = self.imgui.external.take() else {
            return Ok(());
        };

        // SAFETY: shutdown has waited for device idle and no recorded frame command buffer will be
        // submitted again, so the external descriptor has no remaining submitted or future users.
        if let Err(error) = unsafe { self.imgui.renderer.unregister_texture(external.tex_id) } {
            self.imgui.external = Some(external);
            return Err(error);
        }

        unsafe {
            self.vk
                .ctx
                .device
                .destroy_image_view(external.image_view, None);
            self.vk.ctx.device.destroy_image(external.image, None);
            self.vk.ctx.device.free_memory(external.image_mem, None);
        }
        Ok(())
    }

    fn shutdown_platform_backend(&mut self) -> Result<(), imgui_sdl3_backend::Sdl3BackendError> {
        if let Some(mut backend) = self.sdl3_backend.take() {
            if let Err(error) = backend.shutdown(&mut self.imgui.context) {
                self.sdl3_backend = Some(backend);
                return Err(error);
            }
        }
        Ok(())
    }

    fn update_texture(&mut self) {
        let (w, h) = self.imgui.tex_size;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let t = self.imgui.frame as f32 * 0.08;
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let fx = x as f32 / w as f32;
                let fy = y as f32 / h as f32;
                pixels[i + 0] = ((fx * 255.0 + t.sin() * 128.0).clamp(0.0, 255.0)) as u8;
                pixels[i + 1] = ((fy * 255.0 + (t * 1.7).cos() * 128.0).clamp(0.0, 255.0)) as u8;
                pixels[i + 2] = (((fx + fy + t * 0.1).sin().abs()) * 255.0) as u8;
                pixels[i + 3] = 255;
            }
        }
        self.imgui
            .context
            .with_texture_mut(self.imgui.img_tex, |mut texture| texture.set_data(&pixels))
            .expect("animated texture should remain active");
        self.imgui.frame = self.imgui.frame.wrapping_add(1);
    }

    fn process_event(
        &mut self,
        event: &dear_imgui_examples::sdl3_callbacks::QueuedSdl3Event,
    ) -> Result<AppResult, Box<dyn Error>> {
        event.with_imgui_event(|raw| -> Result<(), Box<dyn Error>> {
            if let Some(raw) = raw {
                let backend = self
                    .sdl3_backend
                    .as_mut()
                    .expect("SDL3 backend must be active while the app is running");
                let _ = backend.process_event(&mut self.imgui.context, raw)?;
            }
            Ok(())
        })?;

        if requests_exit(event, self.window.id()) {
            return Ok(AppResult::Success);
        }
        if event.is_pixel_size_changed_for(self.window.id()) {
            let (width, height) = self.window.size_in_pixels();
            if width > 0 && height > 0 {
                self.vk.swapchain_dirty = true;
            }
        }
        Ok(AppResult::Continue)
    }

    fn iterate(&mut self) -> Result<(), Box<dyn Error>> {
        self.init_external_texture()?;

        let now = Instant::now();
        let dt = (now - self.imgui.last_frame).as_secs_f32();
        self.imgui.last_frame = now;
        self.imgui.context.io_mut().set_delta_time(dt);

        // Update animated texture (marks WantUpdates).
        self.update_texture();

        self.sdl3_backend
            .as_mut()
            .expect("SDL3 backend must be active while the app is running")
            .new_frame(&mut self.imgui.context)?;
        let ui = self.imgui.context.frame();

        ui.dockspace_over_main_viewport();

        let sampler_linear_callback = self.imgui.sampler_linear_callback;
        let sampler_nearest_callback = self.imgui.sampler_nearest_callback;
        ui.window("SDL3 + Ash (multi-viewport)")
            .size([460.0, 280.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Drag ImGui windows outside to spawn OS windows.");
                ui.separator();
                ui.checkbox("Show demo window", &mut self.imgui.show_demo);
                ui.color_edit4("Clear color", &mut self.imgui.clear_color);
                ui.separator();
                ui.text("Animated ImGui-managed texture:");
                ui.image(self.imgui.img_tex, [256.0, 256.0]);

                if let Some(external) = self.imgui.external.as_mut() {
                    ui.separator();
                    ui.text("External Vulkan texture (legacy TextureId):");

                    let mut use_linear = external.use_linear_sampler;
                    ui.checkbox("Use linear sampler", &mut use_linear);
                    external.use_linear_sampler = use_linear;

                    let selected_callback = if use_linear {
                        sampler_linear_callback
                    } else {
                        sampler_nearest_callback
                    };
                    let draw_list = ui.get_window_draw_list();
                    unsafe {
                        draw_list.add_callback(selected_callback, std::ptr::null_mut(), 0);
                    }
                    ui.image(external.tex_id, [256.0, 256.0]);
                    let draw_list = ui.get_window_draw_list();
                    unsafe {
                        draw_list.add_callback(sampler_linear_callback, std::ptr::null_mut(), 0);
                    }
                } else {
                    ui.separator();
                    ui.text("External texture not available.");
                }
                ui.text(format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));
            });

        if self.imgui.show_demo {
            ui.show_demo_window(&mut self.imgui.show_demo);
        }

        self.imgui
            .renderer
            .set_viewport_clear_color(self.imgui.clear_color)?;

        let mut frame = self.imgui.context.render();
        let prepared_texture_retirement = self.imgui.renderer.prepare_frame(&mut frame)?;

        // Reconcile managed textures before any viewport records commands, then submit secondary
        // swapchains before acquiring the main surface to avoid overlapping WSI ownership.
        if self.enable_viewports {
            frame.update_and_render_platform_windows_default();
        }
        self.imgui.renderer.poll_fault()?;

        let clear_color = self.imgui.clear_color;
        let recorded_texture_retirement = render_main_window(
            &mut self.vk,
            &mut self.imgui.renderer,
            &self.window,
            clear_color,
            frame,
        )?;
        let texture_retirement = ash_frame_sync::merge_texture_retirement_batches(
            prepared_texture_retirement,
            recorded_texture_retirement,
        )?;

        if let Some(retirement) = texture_retirement {
            self.imgui
                .renderer
                .wait_for_texture_retirements(retirement)?;
        }
        Ok(())
    }
}

fn recover_aborted_main_acquire(
    vk_state: &mut VulkanState,
    renderer: &mut RendererRuntime,
    window: &sdl3::video::Window,
    frame_index: usize,
) -> Result<(), Box<dyn Error>> {
    vk_state.swapchain_dirty = true;
    unsafe { vk_state.ctx.device.device_wait_idle()? };

    let frame = vk_state
        .frames
        .get_mut(frame_index)
        .ok_or("Ash main frame disappeared during acquire recovery")?;
    let abandoned_fence = frame.fence;
    clear_fence_references(&mut vk_state.images_in_flight, abandoned_fence);
    let abandoned_retirement =
        replace_frame_sync(&vk_state.ctx.device, vk_state.ctx.command_pool, frame)?;

    vk_state
        .swapchain
        .recreate_after_device_idle(&vk_state.ctx, window, vk_state.render_target)?;
    vk_state.images_in_flight = vec![vk::Fence::null(); vk_state.swapchain.images.len()];
    vk_state.swapchain_dirty = false;

    if let Some(retirement) = abandoned_retirement {
        renderer.wait_for_texture_retirements(retirement)?;
    }
    if let Some(retirement) = renderer.pending_texture_retirement()? {
        renderer.wait_for_texture_retirements(retirement)?;
    }
    Ok(())
}

fn render_main_window(
    vk_state: &mut VulkanState,
    renderer: &mut RendererRuntime,
    window: &sdl3::video::Window,
    clear_color: [f32; 4],
    rendered_frame: RenderedFrame<'_>,
) -> Result<Option<TextureRetirementBatch>, Box<dyn Error>> {
    let (width, height) = window.size_in_pixels();
    if width == 0 || height == 0 {
        return renderer.pending_texture_retirement();
    }
    if vk_state.swapchain_dirty {
        vk_state
            .swapchain
            .recreate(&vk_state.ctx, window, vk_state.render_target)?;
        vk_state.images_in_flight = vec![vk::Fence::null(); vk_state.swapchain.images.len()];
        vk_state.swapchain_dirty = false;
    }

    let frame_index = vk_state.frame_index % vk_state.frames.len();
    let frame_fence = vk_state.frames[frame_index].fence;
    let image_available = vk_state.frames[frame_index].image_available;
    let command_buffer = vk_state.frames[frame_index].command_buffer;
    unsafe {
        vk_state
            .ctx
            .device
            .wait_for_fences(&[frame_fence], true, u64::MAX)?;
    }

    let acquire = unsafe {
        vk_state.swapchain.loader.acquire_next_image(
            vk_state.swapchain.swapchain,
            u64::MAX,
            image_available,
            vk::Fence::null(),
        )
    };

    let (image_index, suboptimal) = match acquire {
        Ok(v) => v,
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
            vk_state.swapchain_dirty = true;
            return renderer.pending_texture_retirement();
        }
        Err(e) => return Err(Box::new(e)),
    };
    let image_index_usize = image_index as usize;
    let submission =
        (|| -> Result<(Option<TextureRetirementBatch>, vk::Semaphore), Box<dyn Error>> {
            let image_fence = vk_state
                .images_in_flight
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no in-flight fence slot")?;
            let present_semaphore = vk_state
                .swapchain
                .present_semaphores
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no present semaphore")?;
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            let framebuffer = vk_state
                .swapchain
                .framebuffers
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no framebuffer")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image = vk_state
                .swapchain
                .images
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image is missing")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image_view = vk_state
                .swapchain
                .image_views
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no image view")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let old_layout = vk_state
                .swapchain
                .image_layouts
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no tracked layout")?;

            if image_fence != vk::Fence::null() {
                unsafe {
                    vk_state
                        .ctx
                        .device
                        .wait_for_fences(&[image_fence], true, u64::MAX)?;
                }
            }
            unsafe {
                vk_state
                    .ctx
                    .device
                    .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())?;
            }

            let texture_retirement = record_command_buffer(
                &vk_state.ctx.device,
                command_buffer,
                vk_state.render_target,
                #[cfg(not(feature = "ash-dynamic-rendering"))]
                framebuffer,
                #[cfg(feature = "ash-dynamic-rendering")]
                image,
                #[cfg(feature = "ash-dynamic-rendering")]
                image_view,
                #[cfg(feature = "ash-dynamic-rendering")]
                old_layout,
                vk_state.swapchain.extent,
                clear_color,
                // SAFETY: cmd is recording inside the compatible render pass and is submitted before
                // any renderer resource can be retired or destroyed.
                |cmd| unsafe { renderer.cmd_draw(cmd, rendered_frame) },
            )?;

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(std::slice::from_ref(&image_available))
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(std::slice::from_ref(&command_buffer))
                .signal_semaphores(std::slice::from_ref(&present_semaphore));
            unsafe {
                vk_state.ctx.device.reset_fences(&[frame_fence])?;
                vk_state.ctx.device.queue_submit(
                    vk_state.ctx.queue,
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
                recover_aborted_main_acquire(vk_state, renderer, window, frame_index)
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
        vk_state.swapchain.image_layouts[image_index_usize] = vk::ImageLayout::PRESENT_SRC_KHR;
    }
    vk_state.images_in_flight[image_index_usize] = frame_fence;
    vk_state.swapchain_dirty |= suboptimal;

    let present_info = vk::PresentInfoKHR::default()
        .wait_semaphores(std::slice::from_ref(&present_semaphore))
        .swapchains(std::slice::from_ref(&vk_state.swapchain.swapchain))
        .image_indices(std::slice::from_ref(&image_index));

    let present = unsafe {
        vk_state
            .swapchain
            .loader
            .queue_present(vk_state.ctx.queue, &present_info)
    };
    match present {
        Ok(suboptimal) => vk_state.swapchain_dirty |= suboptimal,
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
            vk_state.swapchain_dirty = true;
        }
        Err(error) => {
            vk_state.swapchain_dirty = true;
            return Err(Box::new(error));
        }
    }

    vk_state.frame_index = (vk_state.frame_index + 1) % vk_state.frames.len();
    Ok(texture_retirement)
}

#[app_impl]
impl Sdl3AshApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        let result = (|| -> Result<Self, Box<dyn Error>> {
            imgui_sdl3_backend::enable_native_ime_ui();
            configure_main_callback_rate();

            let sdl = sdl3::init()?;
            let video = sdl.video()?;
            // Optional: ensure SDL loads Vulkan loader early (the first Vulkan window would also
            // load it, but doing so here makes initialization failures deterministic).
            let _ = video.vulkan_load_library_default();
            let app = App::new(&video)?;
            Ok(Self {
                events: Sdl3CallbackEventHandoff::default(),
                main: MainThreadData::assert_new(RefCell::new(MainData {
                    app,
                    _video: video,
                    _sdl: sdl,
                })),
            })
        })();

        match result {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 Ash example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        let mut events = self.events.drain();
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut main_guard.app;
        while let Some(event) = events.pop() {
            match main.process_event(&event) {
                Ok(AppResult::Continue) => {}
                Ok(result) => return result,
                Err(error) => {
                    eprintln!("SDL3 Ash event processing failed: {error}");
                    return AppResult::Failure;
                }
            }
        }
        match main.iterate() {
            Ok(()) => AppResult::Continue,
            Err(error) => {
                eprintln!("SDL3 Ash frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        self.events.push(raw);
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
        if let Some(app) = state {
            if let Err(error) = app.main.assert_get().borrow_mut().app.shutdown() {
                eprintln!("SDL3 Ash shutdown failed: {error}");
            }
        }
    }
}
