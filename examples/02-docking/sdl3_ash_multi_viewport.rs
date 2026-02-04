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

use std::error::Error;
use std::ffi::CString;
use std::time::Instant;

use ash::khr::{surface as khr_surface, swapchain as khr_swapchain};
use ash::{Device, Entry, Instance, vk};
use dear_imgui_ash::{AshRenderer, Options as AshOptions};
use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, GamepadMode};
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::video::{SwapInterval, WindowPos};

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
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name.as_c_str())
            .engine_name(engine_name.as_c_str())
            .api_version(vk::make_api_version(0, 1, 0, 0));

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
        window: &sdl3::video::Window,
        render_pass: vk::RenderPass,
        surface_format: vk::SurfaceFormatKHR,
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
        window: &sdl3::video::Window,
        render_pass: vk::RenderPass,
    ) -> Result<(), Box<dyn Error>> {
        unsafe {
            let _ = ctx.device.device_wait_idle();
            for fb in self.framebuffers.drain(..) {
                ctx.device.destroy_framebuffer(fb, None);
            }
            for v in self.image_views.drain(..) {
                ctx.device.destroy_image_view(v, None);
            }
            self.loader.destroy_swapchain(self.swapchain, None);
        }

        let new_format = pick_surface_format(ctx)?;
        *self = Self::new(ctx, window, render_pass, new_format)?;
        Ok(())
    }
}

struct FrameSync {
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    fence: vk::Fence,
    command_buffer: vk::CommandBuffer,
}

fn create_frame_sync(
    device: &Device,
    command_pool: vk::CommandPool,
) -> Result<FrameSync, vk::Result> {
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

fn record_command_buffer<
    F: FnOnce(vk::CommandBuffer) -> Result<(), dear_imgui_ash::RendererError>,
>(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    extent: vk::Extent2D,
    clear_color: [f32; 4],
    record: F,
) -> Result<(), Box<dyn Error>> {
    unsafe {
        device.begin_command_buffer(
            command_buffer,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: clear_color,
            },
        }];

        device.cmd_begin_render_pass(
            command_buffer,
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

        record(command_buffer)?;

        device.cmd_end_render_pass(command_buffer);
        device.end_command_buffer(command_buffer)?;
    }
    Ok(())
}

fn create_render_pass(device: &Device, format: vk::Format) -> Result<vk::RenderPass, vk::Result> {
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        // Swapchain images are in PRESENT_SRC_KHR when acquired.
        .initial_layout(vk::ImageLayout::PRESENT_SRC_KHR)
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
        let view = unsafe { device.create_image_view(&create_info, None)? };
        image_views.push(view);
    }
    Ok(image_views)
}

fn create_framebuffers(
    device: &Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    image_views: &[vk::ImageView],
) -> Result<Vec<vk::Framebuffer>, vk::Result> {
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

fn pick_surface_format(ctx: &VulkanContext) -> Result<vk::SurfaceFormatKHR, vk::Result> {
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
    Ok(*formats.first().unwrap())
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
    renderer: &mut AshRenderer,
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

    fn create_sampler(
        device: &Device,
        mag: vk::Filter,
        min: vk::Filter,
    ) -> Result<vk::Sampler, vk::Result> {
        let info = vk::SamplerCreateInfo::default()
            .mag_filter(mag)
            .min_filter(min)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .max_lod(0.0);
        unsafe { device.create_sampler(&info, None) }
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

    let sampler_nearest = create_sampler(device, vk::Filter::NEAREST, vk::Filter::NEAREST)?;
    let sampler_linear = create_sampler(device, vk::Filter::LINEAR, vk::Filter::LINEAR)?;

    let tex_id = renderer.register_external_texture_with_sampler(image_view, sampler_nearest)?;

    Ok(ExternalTexture {
        tex_id,
        image,
        image_mem,
        image_view,
        sampler_nearest,
        sampler_linear,
        use_linear_sampler: false,
    })
}

struct ImguiState {
    // Ensure registered textures are unregistered before the ImGui context is destroyed.
    _registered_user_textures: Vec<dear_imgui_rs::RegisteredUserTexture>,
    context: Context,
    renderer: AshRenderer,
    last_frame: Instant,
    clear_color: [f32; 4],
    img_tex: dear_imgui_rs::texture::OwnedTextureData,
    tex_size: (u32, u32),
    frame: u32,
    show_demo: bool,
    external: Option<ExternalTexture>,
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

struct ExternalTexture {
    tex_id: dear_imgui_rs::TextureId,
    image: vk::Image,
    image_mem: vk::DeviceMemory,
    image_view: vk::ImageView,
    sampler_nearest: vk::Sampler,
    sampler_linear: vk::Sampler,
    use_linear_sampler: bool,
}

impl Drop for VulkanState {
    fn drop(&mut self) {
        unsafe {
            let _ = self.ctx.device.device_wait_idle();
            for f in self.frames.drain(..) {
                self.ctx.device.destroy_semaphore(f.image_available, None);
                self.ctx.device.destroy_semaphore(f.render_finished, None);
                self.ctx.device.destroy_fence(f.fence, None);
                self.ctx
                    .device
                    .free_command_buffers(self.ctx.command_pool, &[f.command_buffer]);
            }
            for fb in self.swapchain.framebuffers.drain(..) {
                self.ctx.device.destroy_framebuffer(fb, None);
            }
            for v in self.swapchain.image_views.drain(..) {
                self.ctx.device.destroy_image_view(v, None);
            }
            self.swapchain
                .loader
                .destroy_swapchain(self.swapchain.swapchain, None);
            self.ctx.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

struct App {
    window: sdl3::video::Window,
    enable_viewports: bool,
    imgui: ImguiState,
    vk: VulkanState,
}

impl Drop for App {
    fn drop(&mut self) {
        self.destroy_external_texture();
        if self.enable_viewports {
            // Avoid shutdown assertions by ensuring platform windows are destroyed before the
            // ImGui context and renderer are dropped.
            dear_imgui_ash::multi_viewport_sdl3::shutdown_multi_viewport_support(
                &mut self.imgui.context,
            );
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
        let render_pass = create_render_pass(&ctx.device, surface_format.format)?;
        let swapchain = SwapchainState::new(&ctx, &window, render_pass, surface_format)?;

        // Dear ImGui context.
        let mut context = Context::create();
        context.set_ini_filename(None::<String>)?;

        {
            let io = context.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            if ENABLE_VIEWPORTS {
                flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
            }
            io.set_config_flags(flags);

            let style = context.style_mut();
            style.set_font_scale_dpi(main_scale);
        }

        if ENABLE_VIEWPORTS {
            context.enable_multi_viewport();
        }

        // SDL3 platform backend for Vulkan (sets Platform_CreateVkSurface for multi-viewport).
        imgui_sdl3_backend::init_for_vulkan(&mut context, &window)?;
        imgui_sdl3_backend::set_gamepad_mode(GamepadMode::AutoAll);

        // Create a managed ImGui texture (CPU-side pixels; backend will create GPU texture).
        let tex_w: u32 = 128;
        let tex_h: u32 = 128;
        let mut img_tex = dear_imgui_rs::texture::TextureData::new();
        img_tex.create(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            tex_w as i32,
            tex_h as i32,
        );
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
        img_tex.set_status(dear_imgui_rs::texture::TextureStatus::WantCreate);

        // Register user-created textures so renderer backends can see them via DrawData::textures().
        // This avoids TexID==0 assertions and lets the backend handle Create/Update/Destroy.
        let mut registered_user_textures = Vec::new();
        registered_user_textures.push(context.register_user_texture_token(&mut *img_tex));

        // Renderer.
        let framebuffer_srgb = is_srgb_format(swapchain.surface_format.format);
        let mut renderer = AshRenderer::with_default_allocator(
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
        renderer.set_viewport_clear_color([0.1, 0.12, 0.15, 1.0]);

        // Frame sync objects.
        let frames = (0..FRAMES_IN_FLIGHT)
            .map(|_| create_frame_sync(&ctx.device, ctx.command_pool))
            .collect::<Result<Vec<_>, _>>()?;
        let images_in_flight = vec![vk::Fence::null(); swapchain.images.len()];

        Ok(Self {
            window,
            enable_viewports: ENABLE_VIEWPORTS,
            imgui: ImguiState {
                _registered_user_textures: registered_user_textures,
                context,
                renderer,
                last_frame: Instant::now(),
                clear_color: [0.1, 0.12, 0.15, 1.0],
                img_tex,
                tex_size: (tex_w, tex_h),
                frame: 0,
                show_demo: true,
                external: None,
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

    fn destroy_external_texture(&mut self) {
        let Some(external) = self.imgui.external.take() else {
            return;
        };

        self.imgui.renderer.unregister_texture(external.tex_id);

        unsafe {
            self.vk
                .ctx
                .device
                .destroy_sampler(external.sampler_nearest, None);
            self.vk
                .ctx
                .device
                .destroy_sampler(external.sampler_linear, None);
            self.vk
                .ctx
                .device
                .destroy_image_view(external.image_view, None);
            self.vk.ctx.device.destroy_image(external.image, None);
            self.vk.ctx.device.free_memory(external.image_mem, None);
        }
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
        self.imgui.img_tex.set_data(&pixels);
        self.imgui.frame = self.imgui.frame.wrapping_add(1);
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Best-effort: create external texture once (will be shown in UI).
        let _ = self.init_external_texture();

        'main: loop {
            while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
                let _ = imgui_sdl3_backend::process_sys_event(&raw);

                let event = Event::from_ll(raw);
                match event {
                    Event::Quit { .. } => break 'main,
                    Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'main,
                    Event::Window {
                        win_event: sdl3::event::WindowEvent::CloseRequested,
                        window_id,
                        ..
                    } if window_id == self.window.id() => break 'main,
                    Event::Window {
                        win_event: sdl3::event::WindowEvent::PixelSizeChanged(_, _),
                        window_id,
                        ..
                    } if window_id == self.window.id() => {
                        let (w, h) = self.window.size_in_pixels();
                        if w > 0 && h > 0 {
                            self.vk.swapchain_dirty = true;
                        }
                    }
                    _ => {}
                }
            }

            let now = Instant::now();
            let dt = (now - self.imgui.last_frame).as_secs_f32();
            self.imgui.last_frame = now;
            self.imgui.context.io_mut().set_delta_time(dt);

            // Update animated texture (marks WantUpdates).
            self.update_texture();

            imgui_sdl3_backend::sdl3_new_frame(&mut self.imgui.context);
            let ui = self.imgui.context.frame();

            ui.dockspace_over_main_viewport();

            ui.window("SDL3 + Ash (multi-viewport)")
                .size([460.0, 280.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text("Drag ImGui windows outside to spawn OS windows.");
                    ui.separator();
                    ui.checkbox("Show demo window", &mut self.imgui.show_demo);
                    ui.color_edit4("Clear color", &mut self.imgui.clear_color);
                    ui.separator();
                    ui.text("Animated ImGui-managed texture:");
                    ui.image(&mut *self.imgui.img_tex, [256.0, 256.0]);

                    if let Some(external) = self.imgui.external.as_mut() {
                        ui.separator();
                        ui.text("External Vulkan texture (legacy TextureId):");

                        let mut use_linear = external.use_linear_sampler;
                        ui.checkbox("Use linear sampler", &mut use_linear);
                        if use_linear != external.use_linear_sampler {
                            external.use_linear_sampler = use_linear;
                            let sampler = if use_linear {
                                external.sampler_linear
                            } else {
                                external.sampler_nearest
                            };
                            let _ = self
                                .imgui
                                .renderer
                                .update_external_texture_sampler(external.tex_id, sampler);
                        }

                        ui.image(external.tex_id, [256.0, 256.0]);
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

            {
                let draw_data = self.imgui.context.render();
                let clear_color = self.imgui.clear_color;
                render_main_window(
                    &mut self.vk,
                    &mut self.imgui.renderer,
                    &self.window,
                    clear_color,
                    &draw_data,
                )?;
            }

            if self.enable_viewports {
                let io_flags = self.imgui.context.io().config_flags();
                if io_flags.contains(ConfigFlags::VIEWPORTS_ENABLE) {
                    self.imgui.context.update_platform_windows();
                    self.imgui.context.render_platform_windows_default();
                }
            }
        }

        self.destroy_external_texture();
        imgui_sdl3_backend::shutdown(&mut self.imgui.context);
        Ok(())
    }
}

fn render_main_window(
    vk_state: &mut VulkanState,
    renderer: &mut AshRenderer,
    window: &sdl3::video::Window,
    clear_color: [f32; 4],
    draw_data: &dear_imgui_rs::render::DrawData,
) -> Result<(), Box<dyn Error>> {
    if vk_state.swapchain_dirty {
        vk_state
            .swapchain
            .recreate(&vk_state.ctx, window, vk_state.render_pass)?;
        vk_state.images_in_flight = vec![vk::Fence::null(); vk_state.swapchain.images.len()];
        vk_state.swapchain_dirty = false;
    }

    let frame = &vk_state.frames[vk_state.frame_index % vk_state.frames.len()];
    unsafe {
        vk_state
            .ctx
            .device
            .wait_for_fences(&[frame.fence], true, u64::MAX)?;
    }

    let acquire = unsafe {
        vk_state.swapchain.loader.acquire_next_image(
            vk_state.swapchain.swapchain,
            u64::MAX,
            frame.image_available,
            vk::Fence::null(),
        )
    };

    let (image_index, _suboptimal) = match acquire {
        Ok(v) => v,
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
            vk_state.swapchain_dirty = true;
            return Ok(());
        }
        Err(e) => return Err(Box::new(e)),
    };

    if vk_state.images_in_flight[image_index as usize] != vk::Fence::null() {
        unsafe {
            vk_state.ctx.device.wait_for_fences(
                &[vk_state.images_in_flight[image_index as usize]],
                true,
                u64::MAX,
            )?;
        }
    }
    vk_state.images_in_flight[image_index as usize] = frame.fence;

    unsafe {
        vk_state.ctx.device.reset_fences(&[frame.fence])?;
        vk_state
            .ctx
            .device
            .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())?;
    }

    record_command_buffer(
        &vk_state.ctx.device,
        frame.command_buffer,
        vk_state.render_pass,
        vk_state.swapchain.framebuffers[image_index as usize],
        vk_state.swapchain.extent,
        clear_color,
        |cmd| renderer.cmd_draw(cmd, draw_data),
    )?;

    let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
    let submit_info = vk::SubmitInfo::default()
        .wait_semaphores(std::slice::from_ref(&frame.image_available))
        .wait_dst_stage_mask(&wait_stages)
        .command_buffers(std::slice::from_ref(&frame.command_buffer))
        .signal_semaphores(std::slice::from_ref(&frame.render_finished));

    unsafe {
        vk_state.ctx.device.queue_submit(
            vk_state.ctx.queue,
            std::slice::from_ref(&submit_info),
            frame.fence,
        )?;
    }

    let present_info = vk::PresentInfoKHR::default()
        .wait_semaphores(std::slice::from_ref(&frame.render_finished))
        .swapchains(std::slice::from_ref(&vk_state.swapchain.swapchain))
        .image_indices(std::slice::from_ref(&image_index));

    let present = unsafe {
        vk_state
            .swapchain
            .loader
            .queue_present(vk_state.ctx.queue, &present_info)
    };
    match present {
        Ok(_) => {}
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
            vk_state.swapchain_dirty = true;
        }
        Err(e) => return Err(Box::new(e)),
    }

    vk_state.frame_index = (vk_state.frame_index + 1) % vk_state.frames.len();
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    imgui_sdl3_backend::enable_native_ime_ui();

    let sdl = sdl3::init()?;
    let video = sdl.video()?;

    // Optional: ensure SDL loads Vulkan loader early (first Vulkan window would also load it).
    let _ = video.vulkan_load_library_default();

    // Place the app in a Box so the renderer's address stays stable for multi-viewport callbacks.
    let mut app = Box::new(App::new(&video)?);

    if app.enable_viewports {
        dear_imgui_ash::multi_viewport_sdl3::enable(
            &mut app.imgui.renderer,
            &mut app.imgui.context,
            app.vk.ctx.entry.clone(),
            app.vk.ctx.instance.clone(),
            app.vk.ctx.physical_device,
            app.vk.ctx.queue,
            app.vk.ctx.queue_family_index,
            app.vk.ctx.queue_family_index,
        );
    }

    app.run()
}
