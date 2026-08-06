//! Minimal multi-viewport sample using winit + ash (Vulkan) backends.
//!
//! ⚠️ **EXPERIMENTAL TEST EXAMPLE ONLY** ⚠️
//!
//! Run with:
//! ```bash
//! cargo run -p dear-imgui-examples --bin multi_viewport_ash --features ash-winit-multi-viewport
//! cargo run -p dear-imgui-examples --bin multi_viewport_ash --features "ash-winit-multi-viewport,ash-dynamic-rendering"
//! ```
//!
//! Automated Linux validation smoke with Xvfb and Mesa Lavapipe:
//! ```text
//! python3 tools/ci/run_contract.py ash-vulkan-validation-smoke
//! ```
//!
//! Notes:
//! - This example targets desktop native (Windows/macOS/Linux).
//! - It uses Dear ImGui's multi-viewport system to create additional OS windows.
//! - Secondary viewports create their own Vulkan `SurfaceKHR` + swapchain.
//! - The ash renderer caches pipelines per swapchain format to handle per-viewport formats.

use ash::{
    Device, Entry, Instance,
    ext::debug_utils as ext_debug_utils,
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk::{self, Handle as _},
};
#[cfg(feature = "ash-dynamic-rendering")]
use dear_imgui_ash::DynamicRendering;
use dear_imgui_ash::{
    AshRenderState, AshRenderer, AshRendererConfig, Options as AshOptions,
    multi_viewport as ash_mvp,
};
use dear_imgui_rs::{
    Condition, ConfigFlags, Context, Id, ManagedTextureId, OwnedTextureData, TextureDataError,
    TextureFormat, TextureId, sys,
};
use dear_imgui_winit::{HiDpiMode, WinitPlatform, multi_viewport as winit_mvp};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    ffi::{CStr, CString, c_void},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::Instant,
};
use tracing::{error, info};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
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
const SMOKE_FRAME_BUDGET: u32 = 600;
const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

static RAW_CALLBACK_OBSERVED: AtomicBool = AtomicBool::new(false);
static CALLBACK_CONTRACT_FAILED: AtomicBool = AtomicBool::new(false);
static CALLBACK_ONLY_OBSERVED: AtomicBool = AtomicBool::new(false);
static NEAREST_SAMPLER_SET: AtomicU64 = AtomicU64::new(0);
static LINEAR_SAMPLER_SET: AtomicU64 = AtomicU64::new(0);
static RESET_AFTER_DRAW_OBSERVED: AtomicBool = AtomicBool::new(false);

fn smoke_texture_pixels(revision: u8) -> Vec<u8> {
    const SIDE: usize = 8;
    let mut pixels = Vec::with_capacity(SIDE * SIDE * 4);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let bright = ((x + y + usize::from(revision)) & 1) == 0;
            let (red, green, blue) = if bright {
                (240, 48u8.saturating_add(revision), 32)
            } else {
                (24, 72, 220u8.saturating_sub(revision))
            };
            pixels.extend_from_slice(&[red, green, blue, 255]);
        }
    }
    pixels
}

fn smoke_texture_data(revision: u8) -> Result<OwnedTextureData, TextureDataError> {
    OwnedTextureData::from_pixels(TextureFormat::RGBA32, 8, 8, &smoke_texture_pixels(revision))
}

#[derive(Debug, Default)]
struct ValidationState {
    warnings: AtomicU32,
    errors: AtomicU32,
    messages: Mutex<Vec<String>>,
}

impl ValidationState {
    fn record(&self, severity: vk::DebugUtilsMessageSeverityFlagsEXT, message: String) {
        if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
            self.errors.fetch_add(1, Ordering::Relaxed);
        } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
            self.warnings.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut messages) = self.messages.lock()
            && messages.len() < 32
        {
            messages.push(message);
        }
    }

    fn warning_count(&self) -> u32 {
        self.warnings.load(Ordering::Acquire)
    }

    fn error_count(&self) -> u32 {
        self.errors.load(Ordering::Acquire)
    }
}

unsafe extern "system" fn validation_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    user_data: *mut c_void,
) -> vk::Bool32 {
    if callback_data.is_null() || user_data.is_null() {
        return vk::FALSE;
    }
    let state = unsafe { &*user_data.cast::<ValidationState>() };
    let message = unsafe { CStr::from_ptr((*callback_data).p_message) }
        .to_string_lossy()
        .into_owned();
    state.record(severity, message);
    vk::FALSE
}

fn validation_messenger_info(
    state: &Arc<ValidationState>,
) -> vk::DebugUtilsMessengerCreateInfoEXT<'static> {
    vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(validation_callback))
        .user_data(Arc::as_ptr(state).cast_mut().cast())
}

unsafe extern "C" fn smoke_raw_callback(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let valid = unsafe {
        AshRenderState::with_current(|state| {
            let valid = state.command_buffer() != vk::CommandBuffer::null()
                && state.pipeline() != vk::Pipeline::null()
                && state.pipeline_layout() != vk::PipelineLayout::null()
                && state.device().handle() != vk::Device::null();
            if valid {
                state.device().cmd_set_viewport(
                    state.command_buffer(),
                    0,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
            }
            valid
        })
    }
    .unwrap_or(false);
    RAW_CALLBACK_OBSERVED.fetch_or(valid, Ordering::AcqRel);
    if !valid {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_callback_only_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let valid = unsafe {
        AshRenderState::with_current(|state| {
            state.command_buffer() != vk::CommandBuffer::null()
                && state.sampler_descriptor_set() != vk::DescriptorSet::null()
        })
    }
    .unwrap_or(false);
    CALLBACK_ONLY_OBSERVED.fetch_or(valid, Ordering::AcqRel);
    RAW_CALLBACK_OBSERVED.fetch_or(valid, Ordering::AcqRel);
    if !valid {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_nearest_sampler_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let observed =
        unsafe { AshRenderState::with_current(|state| state.sampler_descriptor_set().as_raw()) }
            .unwrap_or(0);
    if observed == 0 {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    } else {
        NEAREST_SAMPLER_SET.store(observed, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_linear_sampler_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let observed =
        unsafe { AshRenderState::with_current(|state| state.sampler_descriptor_set().as_raw()) }
            .unwrap_or(0);
    if observed == 0 {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    } else {
        LINEAR_SAMPLER_SET.store(observed, Ordering::Release);
    }
}

unsafe extern "C" fn smoke_reset_probe(
    _parent_list: *const sys::ImDrawList,
    _command: *const sys::ImDrawCmd,
) {
    let expected_linear = LINEAR_SAMPLER_SET.load(Ordering::Acquire);
    let (state_valid, draw_recovered) = unsafe {
        AshRenderState::with_current(|state| {
            let state_valid = expected_linear != 0
                && state.sampler_descriptor_set().as_raw() == expected_linear
                && state.reset_count() > 0;
            (
                state_valid,
                state_valid && state.draw_commands_since_reset() > 0,
            )
        })
    }
    .unwrap_or((false, false));
    RESET_AFTER_DRAW_OBSERVED.fetch_or(draw_recovered, Ordering::AcqRel);
    if !state_valid {
        CALLBACK_CONTRACT_FAILED.store(true, Ordering::Release);
    }
}

#[derive(Clone, Debug)]
struct VulkanAdapterInfo {
    name: String,
    driver: String,
    driver_info: String,
    device_type: &'static str,
    vendor: u32,
    device: u32,
}

struct VulkanContext {
    entry: Entry,
    instance: Instance,
    debug_loader: Option<ext_debug_utils::Instance>,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    validation: Arc<ValidationState>,
    validation_enabled: bool,
    surface_loader: khr_surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    device: Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    adapter: VulkanAdapterInfo,
}

impl VulkanContext {
    fn new(window: &Window, title: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { Entry::load()? };
        let validation_enabled =
            std::env::var("DEAR_IMGUI_REQUIRE_VULKAN_VALIDATION").is_ok_and(|value| value == "1");
        let validation = Arc::new(ValidationState::default());

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

        let mut extensions =
            ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())?.to_vec();
        let layer_names = if validation_enabled {
            let available_layers = unsafe { entry.enumerate_instance_layer_properties()? };
            let available = available_layers.iter().any(|layer| unsafe {
                CStr::from_ptr(layer.layer_name.as_ptr()) == VALIDATION_LAYER
            });
            if !available {
                return Err("VK_LAYER_KHRONOS_validation is required but unavailable".into());
            }
            extensions.push(ext_debug_utils::NAME.as_ptr());
            vec![VALIDATION_LAYER.as_ptr()]
        } else {
            Vec::new()
        };

        let mut debug_create_info = validation_messenger_info(&validation);
        let mut instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layer_names);
        if validation_enabled {
            instance_create_info = instance_create_info.push_next(&mut debug_create_info);
        }
        let instance = unsafe { entry.create_instance(&instance_create_info, None)? };

        let (debug_loader, debug_messenger) = if validation_enabled {
            let loader = ext_debug_utils::Instance::new(&entry, &instance);
            let messenger =
                match unsafe { loader.create_debug_utils_messenger(&debug_create_info, None) } {
                    Ok(messenger) => messenger,
                    Err(error) => {
                        unsafe { instance.destroy_instance(None) };
                        return Err(Box::new(error));
                    }
                };
            (Some(loader), messenger)
        } else {
            (None, vk::DebugUtilsMessengerEXT::null())
        };

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
        let adapter = describe_physical_device(&instance, physical_device);
        if std::env::var("DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN").is_ok_and(|value| value == "1") {
            validate_software_adapter(&adapter)?;
        }

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
            debug_loader,
            debug_messenger,
            validation,
            validation_enabled,
            surface_loader,
            surface,
            physical_device,
            queue_family_index,
            device,
            queue,
            command_pool,
            adapter,
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
            if let Some(loader) = self.debug_loader.as_ref() {
                loader.destroy_debug_utils_messenger(self.debug_messenger, None);
            }
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
    fn new(_device: &Device, format: vk::Format) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            #[cfg(feature = "ash-dynamic-rendering")]
            format,
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            render_pass: create_render_pass(_device, format)?,
        })
    }

    fn destroy(self, device: &Device) {
        #[cfg(not(feature = "ash-dynamic-rendering"))]
        unsafe {
            device.destroy_render_pass(self.render_pass, None);
        }
        let _ = device;
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
        window: &Window,
        render_target: MainRenderTarget,
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
        _render_target: MainRenderTarget,
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
        window: &Window,
        render_target: MainRenderTarget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { ctx.device.device_wait_idle()? };
        self.recreate_after_device_idle(ctx, window, render_target)
    }

    fn recreate_after_device_idle(
        &mut self,
        ctx: &VulkanContext,
        window: &Window,
        render_target: MainRenderTarget,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

enum RendererRuntime {
    Single(AshRenderer),
    Viewports(ash_mvp::WinitViewportRuntime),
}

impl RendererRuntime {
    fn begin_frame_trace(
        &self,
    ) -> Result<Option<ash_mvp::AshViewportFrameTrace<'_>>, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Single(_) => None,
            Self::Viewports(runtime) => Some(runtime.begin_frame_trace()?),
        })
    }

    fn poll_fault(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Self::Viewports(runtime) = self {
            runtime.poll_fault()?;
        }
        Ok(())
    }

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

    fn cmd_draw_reconciled(
        &mut self,
        command_buffer: vk::CommandBuffer,
        frame: dear_imgui_rs::render::ReconciledFrame<'_>,
    ) -> Result<Option<dear_imgui_ash::TextureRetirementBatch>, Box<dyn std::error::Error>> {
        // `record_command_buffer` supplies a live command buffer inside the renderer-compatible
        // render scope, and the example submits it only to the renderer's configured queue.
        Ok(match self {
            Self::Single(renderer) => unsafe {
                renderer.cmd_draw_reconciled(command_buffer, frame)?
            },
            Self::Viewports(runtime) => unsafe {
                runtime.cmd_draw_reconciled(command_buffer, frame)?
            },
        })
    }

    fn prepare_context<'ctx>(
        &mut self,
        context: &'ctx mut Context,
    ) -> Result<
        (
            dear_imgui_rs::render::ReconciledFrame<'ctx>,
            Option<dear_imgui_ash::TextureRetirementBatch>,
        ),
        Box<dyn std::error::Error>,
    > {
        Ok(match self {
            Self::Single(renderer) => {
                let pending_frame = context.render(renderer.renderer_consumer()?);
                renderer.prepare_frame(pending_frame)?
            }
            Self::Viewports(runtime) => runtime.prepare_context(context)?,
        })
    }

    fn pending_texture_retirement(
        &self,
    ) -> Result<Option<dear_imgui_ash::TextureRetirementBatch>, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Single(renderer) => renderer.pending_texture_retirement()?,
            Self::Viewports(runtime) => runtime.pending_texture_retirement()?,
        })
    }

    fn expect_null_retirement_fence_rejected(
        &mut self,
        batch: dear_imgui_ash::TextureRetirementBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => {
                match unsafe {
                    renderer.complete_texture_retirements_with_fences(batch, &[vk::Fence::null()])
                } {
                    Err(dear_imgui_ash::RendererError::TextureRetirementFenceNull { index: 0 }) => {
                        Ok(())
                    }
                    Err(error) => Err(format!(
                        "null retirement fence returned an unexpected renderer error: {error}"
                    )
                    .into()),
                    Ok(count) => Err(format!(
                        "null retirement fence unexpectedly released {count} texture(s)"
                    )
                    .into()),
                }
            }
            Self::Viewports(runtime) => {
                match unsafe {
                    runtime.complete_texture_retirements_with_fences(batch, &[vk::Fence::null()])
                } {
                    Err(ash_mvp::AshViewportError::Renderer(
                        dear_imgui_ash::RendererError::TextureRetirementFenceNull { index: 0 },
                    )) => Ok(()),
                    Err(error) => Err(format!(
                        "null retirement fence returned an unexpected viewport error: {error}"
                    )
                    .into()),
                    Ok(count) => Err(format!(
                        "null retirement fence unexpectedly released {count} texture(s)"
                    )
                    .into()),
                }
            }
        }
    }

    unsafe fn complete_texture_retirements_with_fences(
        &mut self,
        batch: dear_imgui_ash::TextureRetirementBatch,
        fences: &[vk::Fence],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Single(renderer) => unsafe {
                renderer.complete_texture_retirements_with_fences(batch, fences)?
            },
            Self::Viewports(runtime) => unsafe {
                runtime.complete_texture_retirements_with_fences(batch, fences)?
            },
        })
    }

    fn wait_for_texture_retirements(
        &mut self,
        batch: dear_imgui_ash::TextureRetirementBatch,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Single(renderer) => renderer.wait_for_texture_retirements(batch)?,
            Self::Viewports(runtime) => runtime.wait_for_texture_retirements(batch)?,
        })
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
    render_target: MainRenderTarget,
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
        }
        self.render_target.destroy(&self.ctx.device);
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
    font_texture: TextureId,
    sampler_linear_callback: unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
    sampler_nearest_callback: unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
    reset_render_state_callback:
        unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd),
}

struct AppWindow {
    enable_viewports: bool,
    imgui: ImguiState,
    vk: VulkanState,
    // Keep the platform window alive until renderer, swapchains, and surfaces have been dropped.
    window: Arc<Window>,
    renderer_shutdown_complete: bool,
    viewport_runtime_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    gpu_idle_before_teardown: bool,
    viewport_smoke: Option<ViewportSmokeState>,
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            error!("Ash example fallback shutdown failed: {error}");
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<Box<AppWindow>>,
    error: Option<String>,
}

impl AppWindow {
    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.context.end_frame();
        if !self.renderer_shutdown_complete {
            if let Err(error) = self.imgui.renderer.shutdown(&mut self.imgui.context) {
                return Err(format!("Ash renderer shutdown failed: {error}").into());
            }
            self.renderer_shutdown_complete = true;
        }

        let runtime_error = if !self.viewport_runtime_shutdown_complete {
            let ImguiState {
                viewport_runtime,
                context,
                ..
            } = &mut self.imgui;
            viewport_runtime
                .as_mut()
                .and_then(|runtime| runtime.shutdown(context).err())
        } else {
            None
        };

        let platform_error = if !self.platform_shutdown_complete {
            let ImguiState {
                platform, context, ..
            } = &mut self.imgui;
            platform.shutdown(context).err()
        } else {
            None
        };
        if platform_error.is_none() && !self.platform_shutdown_complete {
            // The base platform owns the final attachment and can complete a runtime that has
            // already released native state while reporting a deferred callback fault.
            self.viewport_runtime_shutdown_complete = true;
            self.platform_shutdown_complete = true;
        }

        let shutdown_error = match (runtime_error, platform_error) {
            (None, None) => None,
            (Some(error), None) => Some(format!("Winit multi-viewport shutdown failed: {error}")),
            (None, Some(error)) => Some(format!("Winit platform shutdown failed: {error}")),
            (Some(runtime), Some(platform)) => Some(format!(
                "Winit multi-viewport shutdown failed: {runtime}; Winit platform shutdown failed: {platform}"
            )),
        };
        if let Some(error) = shutdown_error {
            return Err(error.into());
        }
        if !self.gpu_idle_before_teardown {
            unsafe { self.vk.ctx.device.device_wait_idle()? };
            self.gpu_idle_before_teardown = true;
        }
        Ok(())
    }

    fn teardown_evidence(&self) -> TeardownEvidence {
        TeardownEvidence {
            renderer_shutdown_complete: self.renderer_shutdown_complete,
            viewport_runtime_shutdown_complete: self.viewport_runtime_shutdown_complete,
            platform_shutdown_complete: self.platform_shutdown_complete,
            gpu_idle_before_teardown: self.gpu_idle_before_teardown,
        }
    }

    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let enable_viewports = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));
        let run_viewport_smoke =
            std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1");
        if run_viewport_smoke && !cfg!(feature = "ash-dynamic-rendering") {
            return Err("Ash validation smoke requires feature `ash-dynamic-rendering`".into());
        }

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
        if run_viewport_smoke && !ctx.validation_enabled {
            return Err("Ash validation smoke requires Vulkan validation to be enabled".into());
        }
        if run_viewport_smoke {
            println!(
                "Ash Vulkan adapter: name='{}', type={}, driver='{}', info='{}'",
                ctx.adapter.name,
                ctx.adapter.device_type,
                ctx.adapter.driver,
                ctx.adapter.driver_info,
            );
            RAW_CALLBACK_OBSERVED.store(false, Ordering::Release);
            CALLBACK_CONTRACT_FAILED.store(false, Ordering::Release);
            CALLBACK_ONLY_OBSERVED.store(false, Ordering::Release);
            NEAREST_SAMPLER_SET.store(0, Ordering::Release);
            LINEAR_SAMPLER_SET.store(0, Ordering::Release);
            RESET_AFTER_DRAW_OBSERVED.store(false, Ordering::Release);
        }
        let surface_format = pick_surface_format(&ctx, &window)?;
        let render_target = MainRenderTarget::new(&ctx.device, surface_format.format)?;
        let swapchain = SwapchainState::new(&ctx, &window, render_target, surface_format)?;

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

        let mut platform = WinitPlatform::new(&mut imgui)?;
        platform.attach_window(Arc::clone(&window), HiDpiMode::Default, &mut imgui)?;

        let viewport_runtime = enable_viewports
            .then(|| winit_mvp::WinitPlatformRuntime::new(&mut imgui, &platform))
            .transpose()?;

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
        let mut renderer = unsafe {
            AshRenderer::with_default_allocator(
                &ctx.instance,
                ctx.physical_device,
                renderer_config,
                &mut imgui,
            )?
        };
        renderer.set_viewport_clear_color([0.1, 0.12, 0.15, 1.0]);
        let font_texture = imgui.font_atlas().texture_id();
        let sampler_linear_callback = imgui
            .platform_io()
            .draw_callback_set_sampler_linear_raw()
            .ok_or("Ash did not publish its linear sampler callback")?;
        let sampler_nearest_callback = imgui
            .platform_io()
            .draw_callback_set_sampler_nearest_raw()
            .ok_or("Ash did not publish its nearest sampler callback")?;
        let reset_render_state_callback = imgui
            .platform_io()
            .draw_callback_reset_render_state_raw()
            .ok_or("Ash did not publish its reset-render-state callback")?;
        let smoke_managed_texture = if run_viewport_smoke {
            Some(imgui.register_texture(smoke_texture_data(0)?))
        } else {
            None
        };
        let renderer = if enable_viewports {
            RendererRuntime::Viewports(unsafe {
                ash_mvp::WinitViewportRuntime::attach(
                    &mut imgui,
                    viewport_runtime
                        .as_ref()
                        .expect("Winit viewport owner must be initialized before Ash"),
                    renderer,
                    ash_mvp::VulkanViewportConfig {
                        entry: ctx.entry.clone(),
                        instance: ctx.instance.clone(),
                        physical_device: ctx.physical_device,
                        validation_surface: ctx.surface,
                        present_queue: ctx.queue,
                        graphics_queue_family_index: ctx.queue_family_index,
                        present_queue_family_index: ctx.queue_family_index,
                        swapchain_policy: ash_mvp::ViewportSwapchainPolicy::from_main_surface(
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

        let frames = create_frame_syncs(&ctx.device, ctx.command_pool, FRAMES_IN_FLIGHT)?;
        let images_in_flight = vec![vk::Fence::null(); swapchain.images.len()];
        let viewport_smoke = run_viewport_smoke.then(|| ViewportSmokeState {
            result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            adapter: ctx.adapter.clone(),
            validation: Arc::clone(&ctx.validation),
            frame_count: 0,
            phase: SmokePhase::CallbackOnly,
            secondary_id: None,
            initial_secondary_size: None,
            secondary_created: false,
            secondary_resized: false,
            merge_observed: false,
            render_submitted_ids: Vec::new(),
            present_submitted_ids: Vec::new(),
            callback_only_frame_executed: false,
            raw_callback_typed_state_observed: false,
            nearest_sampler_descriptor_set_observed: false,
            linear_sampler_descriptor_set_observed: false,
            sampler_descriptor_sets_distinct: false,
            reset_render_state_recovered: false,
            render_state_cleared_after_callback: false,
            managed_texture: smoke_managed_texture,
            managed_texture_updated: false,
            managed_texture_removed: false,
            texture_retirement_null_fence_rejected: false,
            texture_retirement_fence_completion_count: 0,
            texture_retirement_queue_drained: false,
            main_present_completed: false,
        });
        Ok(Self {
            window,
            enable_viewports,
            imgui: ImguiState {
                renderer,
                viewport_runtime,
                platform,
                context: imgui,
                clear_color: [0.1, 0.12, 0.15, 1.0],
                demo_open: !run_viewport_smoke,
                last_frame: Instant::now(),
                font_texture,
                sampler_linear_callback,
                sampler_nearest_callback,
                reset_render_state_callback,
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
            renderer_shutdown_complete: false,
            viewport_runtime_shutdown_complete: false,
            platform_shutdown_complete: false,
            gpu_idle_before_teardown: false,
            viewport_smoke,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.vk.swapchain_dirty = true;
    }

    fn recover_aborted_main_acquire(
        &mut self,
        frame_index: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.vk.swapchain_dirty = true;
        unsafe { self.vk.ctx.device.device_wait_idle()? };

        let frame = self
            .vk
            .frames
            .get_mut(frame_index)
            .ok_or("Ash main frame disappeared during acquire recovery")?;
        let abandoned_fence = frame.fence;
        clear_fence_references(&mut self.vk.images_in_flight, abandoned_fence);
        let abandoned_retirement =
            replace_frame_sync(&self.vk.ctx.device, self.vk.ctx.command_pool, frame)?;

        self.vk.swapchain.recreate_after_device_idle(
            &self.vk.ctx,
            &self.window,
            self.vk.render_target,
        )?;
        self.vk.images_in_flight = vec![vk::Fence::null(); self.vk.swapchain.images.len()];
        self.vk.swapchain_dirty = false;

        if let Some(retirement) = abandoned_retirement {
            self.imgui
                .renderer
                .wait_for_texture_retirements(retirement)?;
        }
        if let Some(retirement) = self.imgui.renderer.pending_texture_retirement()? {
            self.imgui
                .renderer
                .wait_for_texture_retirements(retirement)?;
        }
        Ok(())
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let window_size = self.window.inner_size();
        if window_size.width == 0 || window_size.height == 0 {
            return Ok(());
        }
        if self.vk.swapchain_dirty {
            self.vk
                .swapchain
                .recreate(&self.vk.ctx, &self.window, self.vk.render_target)?;
            self.vk.images_in_flight = vec![vk::Fence::null(); self.vk.swapchain.images.len()];
            self.vk.swapchain_dirty = false;
        }
        if let Some(smoke) = self.viewport_smoke.as_mut() {
            smoke.prepare_managed_texture(&mut self.imgui.context)?;
        }

        let now = Instant::now();
        let dt = (now - self.imgui.last_frame).as_secs_f32();
        self.imgui.context.io_mut().set_delta_time(dt);
        self.imgui.last_frame = now;

        self.imgui
            .platform
            .prepare_frame(&mut self.imgui.context, &self.window)?;
        if let Some(smoke) = self.viewport_smoke.as_mut() {
            smoke.begin_frame()?;
        }
        let viewport_count = self.imgui.context.platform_io().viewports_iter().count();
        let ui = self.imgui.context.frame();
        let smoke_phase = self.viewport_smoke.as_ref().map(|smoke| smoke.phase);
        let callback_only_frame = smoke_phase == Some(SmokePhase::CallbackOnly);

        if !callback_only_frame {
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
                ui.show_demo_window(&mut self.imgui.demo_open);
            }
        }

        if callback_only_frame {
            let draw_list = ui.get_background_draw_list();
            unsafe {
                draw_list.add_callback(
                    self.imgui.sampler_nearest_callback,
                    std::ptr::null_mut(),
                    0,
                );
                draw_list.add_callback(smoke_nearest_sampler_probe, std::ptr::null_mut(), 0);
                draw_list.add_callback(smoke_callback_only_probe, std::ptr::null_mut(), 0);
                draw_list.add_callback(self.imgui.sampler_linear_callback, std::ptr::null_mut(), 0);
                draw_list.add_callback(smoke_linear_sampler_probe, std::ptr::null_mut(), 0);
            }
        } else if let Some(phase) = smoke_phase {
            let main_viewport_id = ui.main_viewport().id();
            let (position, size) = match phase {
                SmokePhase::CallbackOnly => unreachable!("handled above"),
                SmokePhase::Spawn => ([1500.0, 120.0], [360.0, 240.0]),
                SmokePhase::Resize => ([1500.0, 120.0], [620.0, 420.0]),
                SmokePhase::Merge | SmokePhase::Complete => {
                    ui.set_next_window_viewport(main_viewport_id);
                    ([720.0, 120.0], [420.0, 280.0])
                }
            };
            let mut observed_viewport_id = main_viewport_id;
            let mut observed_viewport_size = [0.0, 0.0];
            let font_texture = self.imgui.font_texture;
            let sampler_nearest = self.imgui.sampler_nearest_callback;
            let sampler_linear = self.imgui.sampler_linear_callback;
            let reset_render_state = self.imgui.reset_render_state_callback;
            let managed_texture = self
                .viewport_smoke
                .as_ref()
                .and_then(|smoke| smoke.managed_texture);
            ui.window("Ash Vulkan validation smoke")
                .position(position, Condition::Always)
                .size(size, Condition::Always)
                .build(|| {
                    let viewport = ui.window_viewport();
                    observed_viewport_id = viewport.id();
                    observed_viewport_size = viewport.size();
                    ui.text("Ash dynamic rendering validation surface");
                    {
                        let draw_list = ui.get_window_draw_list();
                        unsafe {
                            draw_list.add_callback(sampler_nearest, std::ptr::null_mut(), 0);
                            draw_list.add_callback(
                                smoke_nearest_sampler_probe,
                                std::ptr::null_mut(),
                                0,
                            );
                        }
                    }
                    if let Some(texture) = managed_texture {
                        ui.image(texture, [64.0, 64.0]);
                    } else {
                        ui.image(font_texture, [64.0, 64.0]);
                    }
                    {
                        let draw_list = ui.get_window_draw_list();
                        unsafe {
                            draw_list.add_callback(sampler_linear, std::ptr::null_mut(), 0);
                            draw_list.add_callback(
                                smoke_linear_sampler_probe,
                                std::ptr::null_mut(),
                                0,
                            );
                            draw_list.add_callback(smoke_raw_callback, std::ptr::null_mut(), 0);
                            draw_list.add_callback(reset_render_state, std::ptr::null_mut(), 0);
                        }
                    }
                    ui.text("Draw after reset-render-state callback");
                    {
                        let draw_list = ui.get_window_draw_list();
                        unsafe {
                            draw_list.add_callback(smoke_reset_probe, std::ptr::null_mut(), 0);
                        }
                    }
                });
            if let Some(smoke) = self.viewport_smoke.as_mut() {
                smoke.observe_window(
                    observed_viewport_id,
                    observed_viewport_size,
                    main_viewport_id,
                    viewport_count,
                );
            }
        }

        self.imgui.platform.prepare_render(&ui, &self.window)?;
        let (mut reconciled_frame, prepared_texture_retirement) = self
            .imgui
            .renderer
            .prepare_context(&mut self.imgui.context)?;
        let callback_only_zero_geometry = callback_only_frame
            && reconciled_frame.draw_data().total_vtx_count() == 0
            && reconciled_frame.draw_data().total_idx_count() == 0;

        // Secondary swapchains submit and present before the main swapchain is acquired. This
        // avoids overlapping WSI acquisition semaphores across independently owned surfaces. The
        // no-surface preparation above makes managed texture updates visible to these draws.
        let secondary_report = {
            let secondary_trace = if self.viewport_smoke.is_some() {
                self.imgui.renderer.begin_frame_trace()?
            } else {
                None
            };
            if self.enable_viewports {
                reconciled_frame.update_and_render_platform_windows_default();
            }
            secondary_trace.map(ash_mvp::AshViewportFrameTrace::finish)
        };
        if let Some(report) = secondary_report {
            if let Some(smoke) = self.viewport_smoke.as_mut() {
                smoke.observe_submissions(
                    report.render_submitted_viewport_ids(),
                    report.present_submitted_viewport_ids(),
                );
            }
        }
        self.imgui.renderer.poll_fault()?;

        let frame_index = self.vk.frame_index % self.vk.frames.len();
        let frame_fence = self.vk.frames[frame_index].fence;
        let image_available = self.vk.frames[frame_index].image_available;
        let command_buffer = self.vk.frames[frame_index].command_buffer;

        unsafe {
            self.vk
                .ctx
                .device
                .wait_for_fences(&[frame_fence], true, u64::MAX)?;
        }

        let acquire = unsafe {
            self.vk.swapchain.loader.acquire_next_image(
                self.vk.swapchain.swapchain,
                u64::MAX,
                image_available,
                vk::Fence::null(),
            )
        };

        let (image_index, suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                self.vk.swapchain_dirty = true;
                if let Some(batch) = prepared_texture_retirement {
                    let null_fence_rejected = if self.viewport_smoke.is_some() {
                        self.imgui
                            .renderer
                            .expect_null_retirement_fence_rejected(batch)?;
                        true
                    } else {
                        false
                    };
                    self.imgui.renderer.wait_for_texture_retirements(batch)?;
                    let queue_drained = self.imgui.renderer.pending_texture_retirement()?.is_none();
                    if let Some(smoke) = self.viewport_smoke.as_mut() {
                        smoke.record_texture_retirement(null_fence_rejected, 0, queue_drained);
                    }
                }
                return Ok(());
            }
            Err(error) => {
                self.vk.swapchain_dirty = true;
                return Err(Box::new(error));
            }
        };
        let image_index_usize = image_index as usize;
        let submission = (|| -> Result<(Option<dear_imgui_ash::TextureRetirementBatch>, vk::Semaphore), Box<dyn std::error::Error>> {
            let image_fence = self
                .vk
                .images_in_flight
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no in-flight fence slot")?;
            let present_semaphore = self
                .vk
                .swapchain
                .present_semaphores
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no present semaphore")?;
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            let framebuffer = self
                .vk
                .swapchain
                .framebuffers
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no framebuffer")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image = self
                .vk
                .swapchain
                .images
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image is missing")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image_view = self
                .vk
                .swapchain
                .image_views
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no image view")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let old_layout = self
                .vk
                .swapchain
                .image_layouts
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash main image has no tracked layout")?;

            if image_fence != vk::Fence::null() {
                unsafe {
                    self.vk
                        .ctx
                        .device
                        .wait_for_fences(&[image_fence], true, u64::MAX)?;
                }
            }
            unsafe {
                self.vk.ctx.device.reset_command_buffer(
                    command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )?;
            }

            let recorded_texture_retirement = record_command_buffer(
                &self.vk.ctx.device,
                command_buffer,
                self.vk.render_target,
                #[cfg(not(feature = "ash-dynamic-rendering"))]
                framebuffer,
                #[cfg(feature = "ash-dynamic-rendering")]
                image,
                #[cfg(feature = "ash-dynamic-rendering")]
                image_view,
                #[cfg(feature = "ash-dynamic-rendering")]
                old_layout,
                self.vk.swapchain.extent,
                self.imgui.clear_color,
                |cmd| {
                    self.imgui
                        .renderer
                        .cmd_draw_reconciled(cmd, reconciled_frame)
                },
            )?;
            let texture_retirement = ash_frame_sync::merge_texture_retirement_batches(
                prepared_texture_retirement,
                recorded_texture_retirement,
            )?;
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(std::slice::from_ref(&image_available))
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(std::slice::from_ref(&command_buffer))
                .signal_semaphores(std::slice::from_ref(&present_semaphore));
            unsafe {
                self.vk.ctx.device.reset_fences(&[frame_fence])?;
                self.vk.ctx.device.queue_submit(
                    self.vk.ctx.queue,
                    std::slice::from_ref(&submit_info),
                    frame_fence,
                )?;
            }
            Ok((texture_retirement, present_semaphore))
        })();
        let (texture_retirement, present_semaphore) = match submission {
            Ok(submission) => submission,
            Err(error) => {
                if let Err(recovery_error) = self.recover_aborted_main_acquire(frame_index) {
                    return Err(format!(
                        "Ash main acquire failed before submit: {error}; recovery also failed: {recovery_error}"
                    )
                    .into());
                }
                return Err(error);
            }
        };

        let submitted_fence = frame_fence;
        #[cfg(feature = "ash-dynamic-rendering")]
        {
            self.vk.swapchain.image_layouts[image_index_usize] = vk::ImageLayout::PRESENT_SRC_KHR;
        }
        self.vk.images_in_flight[image_index_usize] = submitted_fence;
        self.vk.swapchain_dirty |= suboptimal;

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
            Err(error) => {
                self.vk.swapchain_dirty = true;
                return Err(Box::new(error));
            }
        }

        self.vk.frame_index = (self.vk.frame_index + 1) % self.vk.frames.len();

        let mut null_fence_rejected = false;
        let mut fence_completion_count = 0;
        if let Some(batch) = texture_retirement {
            if self.viewport_smoke.is_some() {
                self.imgui
                    .renderer
                    .expect_null_retirement_fence_rejected(batch)?;
                null_fence_rejected = true;
                unsafe {
                    self.vk
                        .ctx
                        .device
                        .wait_for_fences(&[submitted_fence], true, u64::MAX)?;
                    fence_completion_count = self
                        .imgui
                        .renderer
                        .complete_texture_retirements_with_fences(batch, &[submitted_fence])?;
                }
            } else {
                self.imgui.renderer.wait_for_texture_retirements(batch)?;
            }
        }
        let texture_retirement_queue_drained =
            self.imgui.renderer.pending_texture_retirement()?.is_none();

        if let Some(smoke) = self.viewport_smoke.as_mut() {
            let render_state_cleared = unsafe {
                self.imgui
                    .context
                    .platform_io()
                    .renderer_render_state()
                    .is_null()
            };
            smoke.update_callback_evidence(callback_only_zero_geometry, render_state_cleared);
            smoke.record_texture_retirement(
                null_fence_rejected,
                fence_completion_count,
                texture_retirement_queue_drained,
            );
            smoke.mark_main_presented();
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
                self.error = Some(e.to_string());
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
                self.error = Some(error.to_string());
                event_loop.exit();
                return;
            }
        } else {
            if let Err(error) =
                app.imgui
                    .platform
                    .handle_event(&mut app.imgui.context, &app.window, &full)
            {
                error!("Winit platform event error: {error}");
                self.error = Some(error.to_string());
                event_loop.exit();
                return;
            }
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
                    match app.render_with_event_loop(event_loop) {
                        Ok(()) => {
                            if app
                                .viewport_smoke
                                .as_ref()
                                .is_some_and(|smoke| smoke.phase == SmokePhase::Complete)
                            {
                                event_loop.exit();
                            } else {
                                app.window.request_redraw();
                            }
                        }
                        Err(error) => {
                            error!("Render error: {error}");
                            self.error = Some(error.to_string());
                            event_loop.exit();
                        }
                    }
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
    let event_loop_result = event_loop.run_app(&mut app);
    let app_error = app.error.take();
    let smoke_result = app
        .window
        .as_ref()
        .and_then(|window| window.viewport_smoke.as_ref())
        .and_then(ViewportSmokeState::completed_result);
    let shutdown_result = app
        .window
        .as_mut()
        .map_or(Ok(()), |window| window.shutdown());
    let teardown_evidence = app.window.as_ref().map(|window| window.teardown_evidence());
    drop(app);

    let mut errors = Vec::new();
    if let Err(error) = event_loop_result {
        errors.push(format!("event loop failed: {error}"));
    }
    if let Some(error) = app_error {
        errors.push(error);
    }
    if let Err(error) = shutdown_result {
        errors.push(error.to_string());
    }
    if let (Some(smoke), Some(teardown)) = (smoke_result, teardown_evidence) {
        smoke.write_after_teardown(teardown)?;
        if smoke.validation.warning_count() != 0 || smoke.validation.error_count() != 0 {
            let diagnostics = smoke
                .validation
                .messages
                .lock()
                .map(|messages| messages.join(" | "))
                .unwrap_or_else(|_| "validation diagnostics lock was poisoned".to_owned());
            errors.push(format!(
                "Vulkan validation reported {} warning(s) and {} error(s): {diagnostics}",
                smoke.validation.warning_count(),
                smoke.validation.error_count()
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokePhase {
    CallbackOnly,
    Spawn,
    Resize,
    Merge,
    Complete,
}

struct ViewportSmokeState {
    result_path: Option<PathBuf>,
    adapter: VulkanAdapterInfo,
    validation: Arc<ValidationState>,
    frame_count: u32,
    phase: SmokePhase,
    secondary_id: Option<Id>,
    initial_secondary_size: Option<[f32; 2]>,
    secondary_created: bool,
    secondary_resized: bool,
    merge_observed: bool,
    render_submitted_ids: Vec<u32>,
    present_submitted_ids: Vec<u32>,
    callback_only_frame_executed: bool,
    raw_callback_typed_state_observed: bool,
    nearest_sampler_descriptor_set_observed: bool,
    linear_sampler_descriptor_set_observed: bool,
    sampler_descriptor_sets_distinct: bool,
    reset_render_state_recovered: bool,
    render_state_cleared_after_callback: bool,
    managed_texture: Option<ManagedTextureId>,
    managed_texture_updated: bool,
    managed_texture_removed: bool,
    texture_retirement_null_fence_rejected: bool,
    texture_retirement_fence_completion_count: usize,
    texture_retirement_queue_drained: bool,
    main_present_completed: bool,
}

#[derive(Clone)]
struct CompletedViewportSmoke {
    result_path: Option<PathBuf>,
    adapter: VulkanAdapterInfo,
    validation: Arc<ValidationState>,
    secondary_created: bool,
    secondary_resized: bool,
    merge_observed: bool,
    render_submitted_ids: Vec<u32>,
    present_submitted_ids: Vec<u32>,
    callback_only_frame_executed: bool,
    raw_callback_typed_state_observed: bool,
    nearest_sampler_descriptor_set_observed: bool,
    linear_sampler_descriptor_set_observed: bool,
    sampler_descriptor_sets_distinct: bool,
    reset_render_state_recovered: bool,
    render_state_cleared_after_callback: bool,
    managed_texture_updated: bool,
    managed_texture_removed: bool,
    texture_retirement_null_fence_rejected: bool,
    texture_retirement_fence_completion_count: usize,
    texture_retirement_queue_drained: bool,
    main_present_completed: bool,
}

#[derive(Clone, Copy)]
struct TeardownEvidence {
    renderer_shutdown_complete: bool,
    viewport_runtime_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    gpu_idle_before_teardown: bool,
}

impl ViewportSmokeState {
    fn prepare_managed_texture(
        &mut self,
        context: &mut Context,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.phase {
            SmokePhase::Spawn if !self.managed_texture_updated => {
                let texture = self
                    .managed_texture
                    .ok_or("Ash smoke managed texture disappeared before its update")?;
                let pixels = smoke_texture_pixels(37);
                context
                    .try_with_texture_mut(texture, |mut texture| texture.replace_pixels(&pixels))?;
                self.managed_texture_updated = true;
            }
            SmokePhase::Merge if !self.managed_texture_removed => {
                let texture = self
                    .managed_texture
                    .ok_or("Ash smoke managed texture disappeared before its removal")?;
                context.remove_texture(texture)?;
                self.managed_texture = None;
                self.managed_texture_removed = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn begin_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.frame_count = self
            .frame_count
            .checked_add(1)
            .ok_or("Ash validation smoke frame counter overflowed")?;
        if self.frame_count > SMOKE_FRAME_BUDGET {
            return Err(format!(
                "Ash validation smoke exceeded {SMOKE_FRAME_BUDGET} frames in phase {:?}; \
                 callback_only={}, raw_callback={}, nearest_sampler={}, linear_sampler={}, \
                 distinct_samplers={}, reset_after_draw={}, render_state_cleared={}, \
                 callback_contract_failed={}",
                self.phase,
                self.callback_only_frame_executed,
                self.raw_callback_typed_state_observed,
                self.nearest_sampler_descriptor_set_observed,
                self.linear_sampler_descriptor_set_observed,
                self.sampler_descriptor_sets_distinct,
                self.reset_render_state_recovered,
                self.render_state_cleared_after_callback,
                CALLBACK_CONTRACT_FAILED.load(Ordering::Acquire),
            )
            .into());
        }
        Ok(())
    }

    fn observe_window(
        &mut self,
        viewport_id: Id,
        viewport_size: [f32; 2],
        main_viewport_id: Id,
        viewport_count: usize,
    ) {
        match self.phase {
            SmokePhase::Spawn if viewport_id != main_viewport_id && viewport_count > 1 => {
                self.secondary_created = true;
                self.secondary_id = Some(viewport_id);
                self.initial_secondary_size = Some(viewport_size);
            }
            SmokePhase::Resize if Some(viewport_id) == self.secondary_id => {
                if self.initial_secondary_size.is_some_and(|initial| {
                    (initial[0] - viewport_size[0]).abs() > 64.0
                        || (initial[1] - viewport_size[1]).abs() > 64.0
                }) {
                    self.secondary_resized = true;
                }
            }
            SmokePhase::Merge if viewport_id == main_viewport_id && viewport_count == 1 => {
                self.merge_observed = true;
            }
            _ => {}
        }
    }

    fn observe_submissions(&mut self, rendered: &[Id], presented: &[Id]) {
        self.render_submitted_ids
            .extend(rendered.iter().map(|id| id.raw()));
        self.render_submitted_ids.sort_unstable();
        self.render_submitted_ids.dedup();
        self.present_submitted_ids
            .extend(presented.iter().map(|id| id.raw()));
        self.present_submitted_ids.sort_unstable();
        self.present_submitted_ids.dedup();
        let secondary_presented = self.secondary_id.is_some_and(|secondary| {
            rendered.contains(&secondary) && presented.contains(&secondary)
        });
        match self.phase {
            SmokePhase::Spawn if self.secondary_created && secondary_presented => {
                self.phase = SmokePhase::Resize;
            }
            SmokePhase::Resize if self.secondary_resized && secondary_presented => {
                self.phase = SmokePhase::Merge;
            }
            _ => {}
        }
    }

    fn update_callback_evidence(
        &mut self,
        callback_only_zero_geometry: bool,
        render_state_cleared: bool,
    ) {
        let callback_failed = CALLBACK_CONTRACT_FAILED.load(Ordering::Acquire);
        self.raw_callback_typed_state_observed =
            RAW_CALLBACK_OBSERVED.load(Ordering::Acquire) && !callback_failed;
        self.callback_only_frame_executed |= callback_only_zero_geometry
            && CALLBACK_ONLY_OBSERVED.load(Ordering::Acquire)
            && !callback_failed;
        let nearest = NEAREST_SAMPLER_SET.load(Ordering::Acquire);
        let linear = LINEAR_SAMPLER_SET.load(Ordering::Acquire);
        self.nearest_sampler_descriptor_set_observed |= nearest != 0 && !callback_failed;
        self.linear_sampler_descriptor_set_observed |= linear != 0 && !callback_failed;
        self.sampler_descriptor_sets_distinct |= nearest != 0 && linear != 0 && nearest != linear;
        self.reset_render_state_recovered |=
            RESET_AFTER_DRAW_OBSERVED.load(Ordering::Acquire) && !callback_failed;
        self.render_state_cleared_after_callback |= render_state_cleared;

        if self.phase == SmokePhase::CallbackOnly
            && self.callback_only_frame_executed
            && self.raw_callback_typed_state_observed
            && self.nearest_sampler_descriptor_set_observed
            && self.linear_sampler_descriptor_set_observed
            && self.sampler_descriptor_sets_distinct
            && self.render_state_cleared_after_callback
        {
            self.phase = SmokePhase::Spawn;
        }
    }

    fn record_texture_retirement(
        &mut self,
        null_fence_rejected: bool,
        fence_completion_count: usize,
        queue_drained: bool,
    ) {
        self.texture_retirement_null_fence_rejected |= null_fence_rejected;
        self.texture_retirement_fence_completion_count = self
            .texture_retirement_fence_completion_count
            .saturating_add(fence_completion_count);
        self.texture_retirement_queue_drained = queue_drained;
    }

    fn mark_main_presented(&mut self) {
        self.main_present_completed = true;
        if self.phase == SmokePhase::Merge
            && self.merge_observed
            && self.secondary_created
            && self.secondary_resized
            && self.callback_only_frame_executed
            && self.raw_callback_typed_state_observed
            && self.nearest_sampler_descriptor_set_observed
            && self.linear_sampler_descriptor_set_observed
            && self.sampler_descriptor_sets_distinct
            && self.reset_render_state_recovered
            && self.render_state_cleared_after_callback
            && self.managed_texture_updated
            && self.managed_texture_removed
            && self.texture_retirement_null_fence_rejected
            && self.texture_retirement_fence_completion_count >= 2
            && self.texture_retirement_queue_drained
        {
            self.phase = SmokePhase::Complete;
        }
    }

    fn completed_result(&self) -> Option<CompletedViewportSmoke> {
        (self.phase == SmokePhase::Complete).then(|| CompletedViewportSmoke {
            result_path: self.result_path.clone(),
            adapter: self.adapter.clone(),
            validation: Arc::clone(&self.validation),
            secondary_created: self.secondary_created,
            secondary_resized: self.secondary_resized,
            merge_observed: self.merge_observed,
            render_submitted_ids: self.render_submitted_ids.clone(),
            present_submitted_ids: self.present_submitted_ids.clone(),
            callback_only_frame_executed: self.callback_only_frame_executed,
            raw_callback_typed_state_observed: self.raw_callback_typed_state_observed,
            nearest_sampler_descriptor_set_observed: self.nearest_sampler_descriptor_set_observed,
            linear_sampler_descriptor_set_observed: self.linear_sampler_descriptor_set_observed,
            sampler_descriptor_sets_distinct: self.sampler_descriptor_sets_distinct,
            reset_render_state_recovered: self.reset_render_state_recovered,
            render_state_cleared_after_callback: self.render_state_cleared_after_callback,
            managed_texture_updated: self.managed_texture_updated,
            managed_texture_removed: self.managed_texture_removed,
            texture_retirement_null_fence_rejected: self.texture_retirement_null_fence_rejected,
            texture_retirement_fence_completion_count: self
                .texture_retirement_fence_completion_count,
            texture_retirement_queue_drained: self.texture_retirement_queue_drained,
            main_present_completed: self.main_present_completed,
        })
    }
}

impl CompletedViewportSmoke {
    fn write_after_teardown(
        &self,
        teardown: TeardownEvidence,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = self.result_path.as_ref() else {
            return Ok(());
        };
        let payload = serde_json::json!({
            "schema_version": 2,
            "adapter": {
                "name": self.adapter.name,
                "backend": "Vulkan",
                "device_type": self.adapter.device_type,
                "driver": self.adapter.driver,
                "driver_info": self.adapter.driver_info,
                "vendor": self.adapter.vendor,
                "device": self.adapter.device,
            },
            "dynamic_rendering_enabled": cfg!(feature = "ash-dynamic-rendering"),
            "validation_layer_enabled": true,
            "secondary_viewport_created": self.secondary_created,
            "secondary_viewport_resized": self.secondary_resized,
            "merge_observed": self.merge_observed,
            "secondary_render_submitted_viewport_ids": self.render_submitted_ids,
            "secondary_present_submitted_viewport_ids": self.present_submitted_ids,
            "callback_only_frame_executed": self.callback_only_frame_executed,
            "raw_callback_typed_state_observed": self.raw_callback_typed_state_observed,
            "nearest_sampler_descriptor_set_observed":
                self.nearest_sampler_descriptor_set_observed,
            "linear_sampler_descriptor_set_observed":
                self.linear_sampler_descriptor_set_observed,
            "sampler_descriptor_sets_distinct": self.sampler_descriptor_sets_distinct,
            "reset_render_state_recovered": self.reset_render_state_recovered,
            "render_state_cleared_after_callback": self.render_state_cleared_after_callback,
            "managed_texture_updated": self.managed_texture_updated,
            "managed_texture_removed": self.managed_texture_removed,
            "texture_retirement_null_fence_rejected":
                self.texture_retirement_null_fence_rejected,
            "texture_retirement_fence_completion_count":
                self.texture_retirement_fence_completion_count,
            "texture_retirement_queue_drained": self.texture_retirement_queue_drained,
            "main_present_completed": self.main_present_completed,
            "renderer_shutdown_complete": teardown.renderer_shutdown_complete,
            "viewport_runtime_shutdown_complete": teardown.viewport_runtime_shutdown_complete,
            "platform_shutdown_complete": teardown.platform_shutdown_complete,
            "gpu_idle_before_teardown": teardown.gpu_idle_before_teardown,
            "vulkan_resources_dropped": true,
            "validation_warning_count": self.validation.warning_count(),
            "validation_error_count": self.validation.error_count(),
        });
        let json = serde_json::to_string(&payload)?;
        write_json_atomic(path, &json)
    }
}

fn describe_physical_device(instance: &Instance, device: vk::PhysicalDevice) -> VulkanAdapterInfo {
    let properties = unsafe { instance.get_physical_device_properties(device) };
    let name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    let device_type = match properties.device_type {
        vk::PhysicalDeviceType::CPU => "Cpu",
        vk::PhysicalDeviceType::DISCRETE_GPU => "DiscreteGpu",
        vk::PhysicalDeviceType::INTEGRATED_GPU => "IntegratedGpu",
        vk::PhysicalDeviceType::VIRTUAL_GPU => "VirtualGpu",
        _ => "Other",
    };
    VulkanAdapterInfo {
        name,
        driver: format!("0x{:08x}", properties.driver_version),
        driver_info: format!(
            "Vulkan API {}.{}.{}",
            vk::api_version_major(properties.api_version),
            vk::api_version_minor(properties.api_version),
            vk::api_version_patch(properties.api_version)
        ),
        device_type,
        vendor: properties.vendor_id,
        device: properties.device_id,
    }
}

fn validate_software_adapter(
    adapter: &VulkanAdapterInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    if adapter.device_type != "Cpu" {
        return Err(format!(
            "Ash validation smoke requires a CPU Vulkan adapter, selected '{}' ({})",
            adapter.name, adapter.device_type
        )
        .into());
    }
    let identity = format!(
        "{} {} {}",
        adapter.name, adapter.driver, adapter.driver_info
    )
    .to_lowercase();
    if !identity.contains("lavapipe") && !identity.contains("llvmpipe") {
        return Err(format!(
            "Ash validation smoke requires Lavapipe/llvmpipe, selected '{}'",
            adapter.name
        )
        .into());
    }
    Ok(())
}

fn write_json_atomic(path: &Path, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("DEAR_IMGUI_VIEWPORT_SMOKE_JSON must name a file")?;
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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
        if formats[0].color_space != vk::ColorSpaceKHR::SRGB_NONLINEAR {
            return Err("main surface does not expose an SRGB_NONLINEAR color space".into());
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
    Err("main surface has no supported 8-bit sRGB/SRGB_NONLINEAR pair".into())
}

fn surface_supports_format(
    ctx: &VulkanContext,
    requested: vk::SurfaceFormatKHR,
) -> Result<bool, Box<dyn std::error::Error>> {
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

fn record_command_buffer<F, T>(
    device: &Device,
    cmd: vk::CommandBuffer,
    _render_target: MainRenderTarget,
    #[cfg(not(feature = "ash-dynamic-rendering"))] framebuffer: vk::Framebuffer,
    #[cfg(feature = "ash-dynamic-rendering")] image: vk::Image,
    #[cfg(feature = "ash-dynamic-rendering")] image_view: vk::ImageView,
    #[cfg(feature = "ash-dynamic-rendering")] old_layout: vk::ImageLayout,
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

        let result = record_draws(cmd)?;

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
