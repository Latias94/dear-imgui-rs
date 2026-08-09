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
use std::mem::ManuallyDrop;
use std::time::Instant;

use ash::khr::{surface as khr_surface, swapchain as khr_swapchain};
use ash::{Device, Entry, Instance, vk};
#[cfg(feature = "ash-dynamic-rendering")]
use dear_imgui_ash::DynamicRendering;
use dear_imgui_ash::multi_viewport_sdl3::{
    AshPreparedViewportFrame, AshViewportFrameCompletion, Sdl3ViewportRoute, VulkanViewportConfig,
};
use dear_imgui_ash::{
    AshRenderer, AshRendererConfig, Options as AshOptions, TextureRetirementBatch,
};
use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_rs::{Condition, ConfigFlags, Context, FrameToken, render::ReconciledFrame};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeviceTeardownState {
    Pending,
    Idle,
    DeviceLost,
}

impl DeviceTeardownState {
    const fn permits_native_destruction(self) -> bool {
        matches!(self, Self::Idle | Self::DeviceLost)
    }
}

fn classify_teardown_wait(
    result: Result<(), vk::Result>,
) -> Result<DeviceTeardownState, vk::Result> {
    match result {
        Ok(()) => Ok(DeviceTeardownState::Idle),
        Err(vk::Result::ERROR_DEVICE_LOST) => Ok(DeviceTeardownState::DeviceLost),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod teardown_state_tests {
    use super::*;

    #[test]
    fn only_idle_and_device_lost_are_terminal_drop_proofs() {
        assert!(!DeviceTeardownState::Pending.permits_native_destruction());
        assert!(DeviceTeardownState::Idle.permits_native_destruction());
        assert!(DeviceTeardownState::DeviceLost.permits_native_destruction());
    }

    #[test]
    fn retryable_wait_errors_do_not_publish_a_terminal_proof() {
        assert_eq!(
            classify_teardown_wait(Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY)),
            Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY)
        );
        assert_eq!(
            classify_teardown_wait(Err(vk::Result::ERROR_DEVICE_LOST)),
            Ok(DeviceTeardownState::DeviceLost)
        );
        assert_eq!(
            classify_teardown_wait(Ok(())),
            Ok(DeviceTeardownState::Idle)
        );
    }
}

struct VulkanContextInit {
    entry: Option<Entry>,
    instance: Option<Instance>,
    surface_loader: Option<khr_surface::Instance>,
    surface: Option<vk::SurfaceKHR>,
    device: Option<Device>,
    command_pool: Option<vk::CommandPool>,
}

impl VulkanContextInit {
    fn new(entry: Entry) -> Self {
        Self {
            entry: Some(entry),
            instance: None,
            surface_loader: None,
            surface: None,
            device: None,
            command_pool: None,
        }
    }

    fn finish(
        mut self,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
        queue: vk::Queue,
    ) -> VulkanContext {
        VulkanContext {
            entry: self.entry.take().expect("Vulkan entry was initialized"),
            instance: self
                .instance
                .take()
                .expect("Vulkan instance was initialized"),
            surface_loader: self
                .surface_loader
                .take()
                .expect("Vulkan surface loader was initialized"),
            surface: self.surface.take().expect("Vulkan surface was initialized"),
            physical_device,
            queue_family_index,
            device: self.device.take().expect("Vulkan device was initialized"),
            queue,
            command_pool: self
                .command_pool
                .take()
                .expect("Vulkan command pool was initialized"),
            teardown_state: DeviceTeardownState::Pending,
        }
    }
}

impl Drop for VulkanContextInit {
    fn drop(&mut self) {
        unsafe {
            if let Some(device) = self.device.take() {
                if let Some(command_pool) = self.command_pool.take() {
                    device.destroy_command_pool(command_pool, None);
                }
                device.destroy_device(None);
            }
            if let (Some(surface_loader), Some(surface)) =
                (self.surface_loader.as_ref(), self.surface.take())
            {
                surface_loader.destroy_surface(surface, None);
            }
            if let Some(instance) = self.instance.take() {
                instance.destroy_instance(None);
            }
        }
    }
}

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
    teardown_state: DeviceTeardownState,
}

impl VulkanContext {
    fn new(window: &sdl3::video::Window, title: &str) -> Result<Self, Box<dyn Error>> {
        // Use runtime loader mode so CI/users don't need `vulkan-1.lib` at link time.
        let entry = unsafe { Entry::load()? };
        let mut init = VulkanContextInit::new(entry);

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
        let instance = unsafe {
            init.entry
                .as_ref()
                .expect("Vulkan entry was initialized")
                .create_instance(&instance_create_info, None)?
        };
        init.instance = Some(instance);

        init.surface_loader = Some(khr_surface::Instance::new(
            init.entry.as_ref().expect("Vulkan entry was initialized"),
            init.instance
                .as_ref()
                .expect("Vulkan instance was initialized"),
        ));
        let surface = unsafe {
            window.vulkan_create_surface(
                init.instance
                    .as_ref()
                    .expect("Vulkan instance was initialized")
                    .handle(),
            )?
        };
        init.surface = Some(surface);

        let (physical_device, queue_family_index) = pick_physical_device(
            init.instance
                .as_ref()
                .expect("Vulkan instance was initialized"),
            init.surface_loader
                .as_ref()
                .expect("Vulkan surface loader was initialized"),
            surface,
        )?;
        let (device, queue) = create_device(
            init.instance
                .as_ref()
                .expect("Vulkan instance was initialized"),
            physical_device,
            queue_family_index,
        )?;
        init.device = Some(device);

        let command_pool = unsafe {
            init.device
                .as_ref()
                .expect("Vulkan device was initialized")
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(queue_family_index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )?
        };
        init.command_pool = Some(command_pool);

        Ok(init.finish(physical_device, queue_family_index, queue))
    }

    fn wait_idle_for_teardown(&mut self) -> Result<(), vk::Result> {
        match self.teardown_state {
            DeviceTeardownState::Idle | DeviceTeardownState::DeviceLost => Ok(()),
            DeviceTeardownState::Pending => {
                self.teardown_state =
                    classify_teardown_wait(unsafe { self.device.device_wait_idle() })?;
                Ok(())
            }
        }
    }

    fn note_renderer_shutdown(&mut self) {
        // Ash renderer shutdown already waits for the complete device before destroying its
        // resources, so the native owner must not issue a redundant wait afterwards.
        if self.teardown_state == DeviceTeardownState::Pending {
            self.teardown_state = DeviceTeardownState::Idle;
        }
    }

    fn device_lost(&self) -> bool {
        self.teardown_state == DeviceTeardownState::DeviceLost
    }

    fn teardown_is_proven(&self) -> bool {
        self.teardown_state.permits_native_destruction()
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        if let Err(error) = self.wait_idle_for_teardown() {
            eprintln!(
                "Vulkan device-idle proof failed during fallback teardown; leaking the native context: {error}"
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

fn record_command_buffer<F, T>(
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
) -> Result<T, Box<dyn Error>>
where
    F: FnOnce(vk::CommandBuffer) -> Result<T, Box<dyn Error>>,
{
    let result;
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

        result = record(command_buffer)?;

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
    Ok(result)
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
    Viewports(Sdl3ViewportRoute),
}

enum RendererFrameCompletion {
    Single(Option<TextureRetirementBatch>),
    Viewports(AshViewportFrameCompletion),
}

enum PreparedRendererFrame<'frame> {
    Single {
        frame: ReconciledFrame<'frame>,
        retirement: Option<TextureRetirementBatch>,
    },
    Viewports(AshPreparedViewportFrame<'frame>),
}

impl PreparedRendererFrame<'_> {
    fn skip_main(self) -> RendererFrameCompletion {
        match self {
            Self::Single { frame, retirement } => {
                drop(frame);
                RendererFrameCompletion::Single(retirement)
            }
            Self::Viewports(frame) => RendererFrameCompletion::Viewports(frame.skip_main()),
        }
    }
}

impl RendererRuntime {
    unsafe fn cmd_draw_main(
        &mut self,
        command_buffer: vk::CommandBuffer,
        prepared: PreparedRendererFrame<'_>,
    ) -> Result<RendererFrameCompletion, Box<dyn Error>> {
        Ok(match (self, prepared) {
            (Self::Single(renderer), PreparedRendererFrame::Single { frame, retirement }) => {
                unsafe { renderer.cmd_draw(command_buffer, frame) }?;
                RendererFrameCompletion::Single(retirement)
            }
            (Self::Viewports(route), PreparedRendererFrame::Viewports(frame)) => unsafe {
                RendererFrameCompletion::Viewports(route.cmd_draw_main(command_buffer, frame)?)
            },
            _ => return Err("prepared frame does not belong to the active Ash route".into()),
        })
    }

    fn prepare<'ctx>(
        &mut self,
        frame: FrameToken<'ctx>,
    ) -> Result<PreparedRendererFrame<'ctx>, Box<dyn Error>> {
        Ok(match self {
            Self::Single(renderer) => {
                let pending_frame = frame.try_render(renderer.renderer_consumer()?)?;
                let (frame, retirement) = renderer.prepare_frame(pending_frame)?;
                PreparedRendererFrame::Single { frame, retirement }
            }
            Self::Viewports(route) => PreparedRendererFrame::Viewports(route.prepare(frame)?),
        })
    }

    fn wait_for_frame_completion(
        &mut self,
        completion: RendererFrameCompletion,
    ) -> Result<(), Box<dyn Error>> {
        match (self, completion) {
            (Self::Single(renderer), RendererFrameCompletion::Single(Some(batch))) => {
                renderer.wait_for_texture_retirements(batch)?;
            }
            (Self::Single(_), RendererFrameCompletion::Single(None)) => {}
            (Self::Viewports(route), RendererFrameCompletion::Viewports(completion)) => {
                route.wait_for_frame_completion(completion)?;
            }
            _ => return Err("frame completion does not belong to the active Ash route".into()),
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

    unsafe fn unregister_texture_unchecked(
        &mut self,
        texture: dear_imgui_rs::TextureId,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Single(renderer) => unsafe { renderer.unregister_texture_unchecked(texture)? },
            Self::Viewports(runtime) => unsafe { runtime.unregister_texture_unchecked(texture)? },
        }
        Ok(())
    }

    fn shutdown(&mut self, context: &mut Context) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Single(renderer) => renderer.shutdown(context)?,
            Self::Viewports(route) => route.shutdown(context)?,
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
}

struct VulkanState {
    // The device owner is released manually only after teardown has a terminal GPU proof.
    ctx: ManuallyDrop<VulkanContext>,
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
        if let Err(error) = self.ctx.wait_idle_for_teardown() {
            eprintln!(
                "Vulkan device-idle proof failed; leaking frame, swapchain, render-target, and device ownership: {error}"
            );
            return;
        }
        destroy_frame_syncs(&self.ctx.device, self.ctx.command_pool, &mut self.frames);
        self.swapchain.destroy_resources(&self.ctx.device);
        self.render_target.destroy(&self.ctx.device);

        // SAFETY: `ctx` is manually dropped exactly once, after every Vulkan child has been
        // destroyed and the wait outcome proved idle or terminal device loss.
        unsafe { ManuallyDrop::drop(&mut self.ctx) };
    }
}

struct App {
    // Keep the backend owner before ImguiState so its Drop runs while Context is alive.
    sdl3_backend: ManuallyDrop<Option<Sdl3PlatformBackend>>,
    imgui: ManuallyDrop<ImguiState>,
    vk: ManuallyDrop<VulkanState>,
    // Keep the platform window alive until renderer, swapchains, and surfaces have been dropped.
    window: ManuallyDrop<sdl3::video::Window>,
    gpu_idle_for_shutdown: bool,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
}

struct AppInit {
    renderer: Option<RendererRuntime>,
    sdl3_backend: Option<Sdl3PlatformBackend>,
    context: Option<Context>,
    frames: Vec<FrameSync>,
    swapchain: Option<SwapchainState>,
    render_target: Option<MainRenderTarget>,
    ctx: Option<VulkanContext>,
    // The primary SDL window must outlive the Vulkan surface even on initialization rollback.
    window: Option<sdl3::video::Window>,
}

impl AppInit {
    fn new(ctx: VulkanContext, window: sdl3::video::Window) -> Self {
        Self {
            renderer: None,
            sdl3_backend: None,
            context: None,
            frames: Vec::new(),
            swapchain: None,
            render_target: None,
            ctx: Some(ctx),
            window: Some(window),
        }
    }

    fn finish(mut self, img_tex: dear_imgui_rs::ManagedTextureId, tex_size: (u32, u32)) -> App {
        let image_count = self
            .swapchain
            .as_ref()
            .expect("Ash swapchain was initialized")
            .images
            .len();
        App {
            window: ManuallyDrop::new(self.window.take().expect("SDL3 window was initialized")),
            sdl3_backend: ManuallyDrop::new(self.sdl3_backend.take()),
            imgui: ManuallyDrop::new(ImguiState {
                context: self.context.take().expect("ImGui context was initialized"),
                renderer: self.renderer.take().expect("Ash renderer was initialized"),
                last_frame: Instant::now(),
                clear_color: [0.1, 0.12, 0.15, 1.0],
                img_tex,
                tex_size,
                frame: 0,
                show_demo: true,
                external: None,
            }),
            vk: ManuallyDrop::new(VulkanState {
                ctx: ManuallyDrop::new(self.ctx.take().expect("Vulkan context was initialized")),
                render_target: self
                    .render_target
                    .take()
                    .expect("Ash render target was initialized"),
                swapchain: self
                    .swapchain
                    .take()
                    .expect("Ash swapchain was initialized"),
                frames: std::mem::take(&mut self.frames),
                images_in_flight: vec![vk::Fence::null(); image_count],
                frame_index: 0,
                swapchain_dirty: false,
            }),
            gpu_idle_for_shutdown: false,
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
        }
    }

    fn leak_ownership_tree(&mut self) {
        if let Some(renderer) = self.renderer.take() {
            std::mem::forget(renderer);
        }
        if let Some(sdl3_backend) = self.sdl3_backend.take() {
            std::mem::forget(sdl3_backend);
        }
        if let Some(context) = self.context.take() {
            std::mem::forget(context);
        }
        std::mem::forget(std::mem::take(&mut self.frames));
        if let Some(swapchain) = self.swapchain.take() {
            std::mem::forget(swapchain);
        }
        // MainRenderTarget is a Copy handle wrapper with no Drop; taking it is sufficient to
        // suppress the explicit Vulkan destroy path.
        let _ = self.render_target.take();
        if let Some(ctx) = self.ctx.take() {
            std::mem::forget(ctx);
        }
        if let Some(window) = self.window.take() {
            std::mem::forget(window);
        }
    }
}

impl Drop for AppInit {
    fn drop(&mut self) {
        let wait_error = self
            .ctx
            .as_mut()
            .and_then(|ctx| ctx.wait_idle_for_teardown().err());
        if let Some(error) = wait_error {
            eprintln!(
                "Vulkan initialization rollback lacks a terminal GPU proof; leaking the complete ownership tree: {error}"
            );
            self.leak_ownership_tree();
            return;
        }

        if let Some(context) = self.context.as_mut() {
            context.end_frame();
            if let Some(renderer) = self.renderer.as_mut() {
                if renderer.shutdown(context).is_ok()
                    && let Some(ctx) = self.ctx.as_mut()
                {
                    ctx.note_renderer_shutdown();
                }
            }
            if let Some(sdl3_backend) = self.sdl3_backend.as_mut() {
                let _ = sdl3_backend.shutdown(context);
            }
        }

        if let Some(ctx) = self.ctx.as_mut() {
            destroy_frame_syncs(&ctx.device, ctx.command_pool, &mut self.frames);
            if let Some(swapchain) = self.swapchain.as_mut() {
                swapchain.destroy_resources(&ctx.device);
            }
            if let Some(render_target) = self.render_target.take() {
                render_target.destroy(&ctx.device);
            }
        }
    }
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
        if !self.vk.ctx.teardown_is_proven() {
            eprintln!(
                "SDL3 + Ash fallback teardown lacks a terminal GPU proof; leaking the platform, ImGui, Vulkan, and window ownership tree"
            );
            return;
        }

        // SAFETY: the terminal GPU proof permits ordered destruction. The platform backend and
        // ImGui attachments are released while Context, Vulkan, and the SDL window remain live;
        // Vulkan children and their device are then destroyed before the window.
        unsafe {
            ManuallyDrop::drop(&mut self.sdl3_backend);
            ManuallyDrop::drop(&mut self.imgui);
            ManuallyDrop::drop(&mut self.vk);
            ManuallyDrop::drop(&mut self.window);
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
        let main_scale = if main_scale.is_finite() && main_scale > 0.0 {
            main_scale
        } else {
            1.0
        };

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
        let mut init = AppInit::new(ctx, window);
        let surface_format =
            pick_surface_format(init.ctx.as_ref().expect("Vulkan context was initialized"))?;
        let render_target = MainRenderTarget::new(
            &init
                .ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .device,
            surface_format.format,
        )?;
        init.render_target = Some(render_target);
        let swapchain = SwapchainState::new(
            init.ctx.as_ref().expect("Vulkan context was initialized"),
            init.window.as_ref().expect("SDL3 window was initialized"),
            render_target,
            surface_format,
        )?;
        init.swapchain = Some(swapchain);

        // Dear ImGui context.
        let mut context = Context::create();
        context.set_ini_filename(None::<String>)?;

        {
            let io = context.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);
            io.set_config_dpi_scale_fonts(true);
            io.set_config_dpi_scale_viewports(true);
        }
        {
            let style = context.style_mut();
            style.scale_all_sizes(main_scale);
            style.set_font_scale_dpi(main_scale);
        }

        if ENABLE_VIEWPORTS {
            context.enable_multi_viewport();
        }
        init.context = Some(context);

        // SDL3 platform backend for Vulkan (sets Platform_CreateVkSurface for multi-viewport).
        // SAFETY: `window` outlives ordered shutdown and Context teardown through App ownership.
        let sdl3_backend = unsafe {
            let AppInit {
                context, window, ..
            } = &mut init;
            Sdl3PlatformBackend::init_for_vulkan(
                context.as_mut().expect("ImGui context was initialized"),
                window.as_ref().expect("SDL3 window was initialized"),
            )?
        };
        init.sdl3_backend = Some(sdl3_backend);
        {
            let AppInit {
                sdl3_backend,
                context,
                ..
            } = &mut init;
            sdl3_backend
                .as_mut()
                .expect("SDL3 platform was initialized")
                .set_gamepad_mode(
                    context.as_mut().expect("ImGui context was initialized"),
                    GamepadMode::AutoAll,
                )?;
        }

        // Create a managed ImGui texture (CPU-side pixels; backend will create GPU texture).
        let tex_w: u32 = 128;
        let tex_h: u32 = 128;
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
        let img_tex = dear_imgui_rs::texture::OwnedTextureData::from_pixels(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            tex_w,
            tex_h,
            &pixels,
        )?;

        let img_tex = init
            .context
            .as_mut()
            .expect("ImGui context was initialized")
            .register_texture(img_tex);

        // Renderer.
        let framebuffer_srgb = is_srgb_format(
            init.swapchain
                .as_ref()
                .expect("Ash swapchain was initialized")
                .surface_format
                .format,
        );
        #[cfg(not(feature = "ash-dynamic-rendering"))]
        let renderer_config = AshRendererConfig::with_render_pass(
            init.ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .device
                .clone(),
            init.ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .queue,
            init.ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .command_pool,
            render_target.render_pass,
        );
        #[cfg(feature = "ash-dynamic-rendering")]
        let renderer_config = AshRendererConfig::with_dynamic_rendering(
            init.ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .device
                .clone(),
            init.ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .queue,
            init.ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .command_pool,
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
        let mut renderer = {
            let AppInit { ctx, context, .. } = &mut init;
            let ctx = ctx.as_ref().expect("Vulkan context was initialized");
            let context = context.as_mut().expect("ImGui context was initialized");
            unsafe {
                AshRenderer::with_default_allocator(
                    &ctx.instance,
                    ctx.physical_device,
                    renderer_config,
                    context,
                )?
            }
        };
        renderer.set_viewport_clear_color([0.1, 0.12, 0.15, 1.0]);
        init.renderer = Some(RendererRuntime::Single(renderer));

        let renderer = match init.renderer.take() {
            Some(RendererRuntime::Single(renderer)) => renderer,
            _ => unreachable!("initial Ash renderer must be in single-viewport state"),
        };
        let renderer = if ENABLE_VIEWPORTS {
            // SAFETY: the application serializes all host access to ctx.queue and ctx.device while
            // this runtime can submit, present, rebuild swapchains, or wait for device idle.
            let viewport_config = {
                let ctx = init.ctx.as_ref().expect("Vulkan context was initialized");
                let swapchain = init
                    .swapchain
                    .as_ref()
                    .expect("Ash swapchain was initialized");
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
                }
            };
            let attach_result = {
                let AppInit {
                    sdl3_backend,
                    context,
                    ..
                } = &mut init;
                unsafe {
                    Sdl3ViewportRoute::attach(
                        context.as_mut().expect("ImGui context was initialized"),
                        sdl3_backend
                            .as_ref()
                            .expect("SDL3 platform was initialized"),
                        renderer,
                        viewport_config,
                    )
                }
            };
            match attach_result {
                Ok(route) => RendererRuntime::Viewports(route),
                Err(error) => {
                    let (error, renderer) = error.into_parts();
                    init.renderer = Some(RendererRuntime::Single(renderer));
                    return Err(error.into());
                }
            }
        } else {
            RendererRuntime::Single(renderer)
        };
        init.renderer = Some(renderer);

        // Frame sync objects.
        init.frames = create_frame_syncs(
            &init
                .ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .device,
            init.ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .command_pool,
            FRAMES_IN_FLIGHT,
        )?;

        Ok(init.finish(img_tex, (tex_w, tex_h)))
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        let mut errors = Vec::new();
        if !self.gpu_idle_for_shutdown {
            self.vk.ctx.wait_idle_for_teardown()?;
            self.gpu_idle_for_shutdown = true;
        }

        self.destroy_external_texture()?;

        if !self.renderer_shutdown_complete {
            let ImguiState {
                renderer, context, ..
            } = &mut *self.imgui;
            match renderer.shutdown(context) {
                Ok(()) => self.renderer_shutdown_complete = true,
                Err(error) => {
                    errors.push(format!("Ash renderer shutdown failed: {error}"));
                    if self.vk.ctx.device_lost() {
                        self.renderer_shutdown_complete = true;
                    }
                }
            }
        }
        if !self.renderer_shutdown_complete {
            return Err(errors.join("; ").into());
        }

        if !self.platform_shutdown_complete {
            match self.shutdown_platform_backend() {
                Ok(()) => self.platform_shutdown_complete = true,
                Err(error) => errors.push(format!("SDL3 platform shutdown failed: {error}")),
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; ").into())
        }
    }

    fn init_external_texture(&mut self) -> Result<(), Box<dyn Error>> {
        let vk = &*self.vk;
        let imgui = &mut *self.imgui;
        if imgui.external.is_some() {
            return Ok(());
        }

        let external = create_external_rgba_texture(
            &vk.ctx.instance,
            vk.ctx.physical_device,
            &vk.ctx.device,
            vk.ctx.queue,
            vk.ctx.command_pool,
            &mut imgui.renderer,
        )?;
        imgui.external = Some(external);
        Ok(())
    }

    fn destroy_external_texture(&mut self) -> Result<(), Box<dyn Error>> {
        let vk = &*self.vk;
        let imgui = &mut *self.imgui;
        let Some(external) = imgui.external.take() else {
            return Ok(());
        };

        // SAFETY: shutdown has waited for device idle and no recorded frame command buffer will be
        // submitted again, so the external descriptor has no remaining submitted or future users.
        if let Err(error) = unsafe { imgui.renderer.unregister_texture_unchecked(external.tex_id) }
        {
            imgui.external = Some(external);
            return Err(error);
        }

        unsafe {
            vk.ctx.device.destroy_image_view(external.image_view, None);
            vk.ctx.device.destroy_image(external.image, None);
            vk.ctx.device.free_memory(external.image_mem, None);
        }
        Ok(())
    }

    fn shutdown_platform_backend(&mut self) -> Result<(), imgui_sdl3_backend::Sdl3BackendError> {
        if let Some(mut backend) = self.sdl3_backend.take() {
            let imgui = &mut *self.imgui;
            if let Err(error) = backend.shutdown(&mut imgui.context) {
                *self.sdl3_backend = Some(backend);
                return Err(error);
            }
        }
        Ok(())
    }

    fn update_texture(&mut self) -> Result<(), Box<dyn Error>> {
        let imgui = &mut *self.imgui;
        let (w, h) = imgui.tex_size;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let t = imgui.frame as f32 * 0.08;
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
        imgui
            .context
            .try_with_texture_mut(imgui.img_tex, |mut texture| texture.replace_pixels(&pixels))?;
        imgui.frame = imgui.frame.wrapping_add(1);
        Ok(())
    }

    fn process_event(
        &mut self,
        event: &dear_imgui_examples::sdl3_callbacks::Sdl3CallbackEvent,
    ) -> Result<AppResult, Box<dyn Error>> {
        let window = &*self.window;
        let imgui = &mut *self.imgui;
        let backend = self
            .sdl3_backend
            .as_mut()
            .expect("SDL3 backend must be active while the app is running");
        let _ = backend.process_callback_event(&mut imgui.context, event)?;

        if requests_exit(event, window.id()) {
            return Ok(AppResult::Success);
        }
        if event.is_pixel_size_changed_for(window.id()) {
            let (width, height) = window.size_in_pixels();
            if width > 0 && height > 0 {
                self.vk.swapchain_dirty = true;
            }
        }
        Ok(AppResult::Continue)
    }

    fn iterate(&mut self) -> Result<(), Box<dyn Error>> {
        self.init_external_texture()?;
        // Update the managed texture before opening a frame, which marks it for reconciliation.
        self.update_texture()?;

        let window = &*self.window;
        let vk = &mut *self.vk;
        let backend = self
            .sdl3_backend
            .as_mut()
            .expect("SDL3 backend must be active while the app is running");
        let ImguiState {
            context,
            renderer,
            last_frame,
            clear_color,
            img_tex,
            show_demo,
            external,
            ..
        } = &mut *self.imgui;

        let now = Instant::now();
        let dt = (now - *last_frame).as_secs_f32();
        *last_frame = now;
        context.io_mut().set_delta_time(dt);

        backend.new_frame(context)?;
        let frame = context.begin_frame();
        let ui = frame.ui();

        ui.dockspace().build()?;

        ui.window("SDL3 + Ash (multi-viewport)")
            .size([460.0, 280.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Drag ImGui windows outside to spawn OS windows.");
                ui.separator();
                ui.checkbox("Show demo window", show_demo);
                ui.color_edit4("Clear color", clear_color);
                ui.separator();
                ui.text("Animated ImGui-managed texture:");
                ui.image(*img_tex, [256.0, 256.0]);

                if let Some(external) = external.as_mut() {
                    ui.separator();
                    ui.text("External Vulkan texture (legacy TextureId):");

                    let mut use_linear = external.use_linear_sampler;
                    ui.checkbox("Use linear sampler", &mut use_linear);
                    external.use_linear_sampler = use_linear;

                    let draw_list = ui.get_window_draw_list();
                    if use_linear {
                        draw_list.set_sampler_linear();
                    } else {
                        draw_list.set_sampler_nearest();
                    }
                    ui.image(external.tex_id, [256.0, 256.0]);
                    draw_list.set_sampler_linear();
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

        if *show_demo {
            ui.show_demo_window(show_demo);
        }

        renderer.set_viewport_clear_color(*clear_color)?;

        // Route preparation reconciles managed textures and completes secondary swapchains before
        // main-surface acquisition, including platform and renderer fault aggregation.
        let prepared = renderer.prepare(frame)?;

        let completion = render_main_window(vk, renderer, window, *clear_color, prepared)?;
        renderer.wait_for_frame_completion(completion)?;
        Ok(())
    }
}

fn recover_aborted_main_acquire(
    vk_state: &mut VulkanState,
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
    let _ = replace_frame_sync(&vk_state.ctx.device, vk_state.ctx.command_pool, frame)?;

    vk_state
        .swapchain
        .recreate_after_device_idle(&vk_state.ctx, window, vk_state.render_target)?;
    vk_state.images_in_flight = vec![vk::Fence::null(); vk_state.swapchain.images.len()];
    vk_state.swapchain_dirty = false;

    Ok(())
}

fn render_main_window(
    vk_state: &mut VulkanState,
    renderer: &mut RendererRuntime,
    window: &sdl3::video::Window,
    clear_color: [f32; 4],
    prepared: PreparedRendererFrame<'_>,
) -> Result<RendererFrameCompletion, Box<dyn Error>> {
    let (width, height) = window.size_in_pixels();
    if width == 0 || height == 0 {
        return Ok(prepared.skip_main());
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
            return Ok(prepared.skip_main());
        }
        Err(e) => return Err(Box::new(e)),
    };
    let image_index_usize = image_index as usize;
    let submission = (|| -> Result<(RendererFrameCompletion, vk::Semaphore), Box<dyn Error>> {
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

        let completion = record_command_buffer(
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
            |cmd| unsafe { renderer.cmd_draw_main(cmd, prepared) },
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
        Ok((completion, present_semaphore))
    })();
    let (completion, present_semaphore) = match submission {
        Ok(submission) => submission,
        Err(error) => {
            if let Err(recovery_error) = recover_aborted_main_acquire(vk_state, window, frame_index)
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
    Ok(completion)
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
        let mut events = match self.events.try_drain() {
            Ok(events) => events,
            Err(error) => {
                eprintln!("SDL3 callback event handoff failed: {error}");
                return AppResult::Failure;
            }
        };
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
        // SAFETY: SDL supplies a valid event whose transient payload remains live for this call.
        unsafe { self.events.push_from_callback(raw) };
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
