//! Shared Ash/Winit multi-viewport lifecycle for the interactive example.
//!
//! The module owns the native order: Winit event forwarding, ImGui frame preparation,
//! secondary viewport submission, main-surface acquire/submit/present, and GPU-aware
//! renderer completion. UI composition is supplied through [`AshViewportScenario`].
//!
//! The private Vulkan validation executable enables a separate feature-gated adapter while
//! reusing this exact native lifecycle.

#[cfg(feature = "ash-validation-smoke")]
use ash::ext::debug_utils as ext_debug_utils;
use ash::{
    Device, Entry, Instance,
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
#[cfg(feature = "ash-dynamic-rendering")]
use dear_imgui_ash::DynamicRendering;
use dear_imgui_ash::{
    AshRenderer, AshRendererConfig, Options as AshOptions, multi_viewport as ash_mvp,
};
use dear_imgui_rs::{ConfigFlags, Context, FrameToken, Ui};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    ffi::{CStr, CString},
    mem::ManuallyDrop,
    sync::Arc,
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

#[path = "ash_frame_sync.rs"]
mod ash_frame_sync;
use ash_frame_sync::{
    FrameSync, clear_fence_references, create_frame_syncs, create_present_semaphores,
    destroy_frame_syncs, destroy_present_semaphores, replace_frame_sync,
};

#[cfg(feature = "ash-validation-smoke")]
#[path = "ash_multi_viewport_validation.rs"]
pub mod validation;

const FRAMES_IN_FLIGHT: usize = 2;

#[derive(Clone, Debug)]
pub struct VulkanAdapterInfo {
    pub name: String,
    pub driver: String,
    pub driver_info: String,
    pub device_type: &'static str,
    pub vendor: u32,
    pub device: u32,
}

pub type ExampleResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Renderer and platform values exposed while a scenario composes one ImGui frame.
// The package-level validation feature also compiles this interactive facade into the private
// validation binary, where it is intentionally not constructed.
#[cfg_attr(feature = "ash-validation-smoke", allow(dead_code))]
pub struct AshFrameUi<'a> {
    pub ui: &'a Ui,
    pub viewport_count: usize,
    pub surface_format: vk::Format,
    pub framebuffer_srgb: bool,
    pub clear_color: &'a mut [f32; 4],
    pub demo_open: &'a mut bool,
}

/// Interactive UI policy layered over the shared native Ash lifecycle.
#[cfg_attr(feature = "ash-validation-smoke", allow(dead_code))]
pub trait AshViewportScenario: 'static {
    fn requires_dynamic_rendering(&self) -> bool {
        false
    }

    fn initialize(
        &mut self,
        _context: &mut Context,
        _adapter: &VulkanAdapterInfo,
    ) -> ExampleResult {
        Ok(())
    }

    fn prepare_frame(&mut self, _context: &mut Context) -> ExampleResult {
        Ok(())
    }

    fn begin_frame(&mut self) -> ExampleResult {
        Ok(())
    }

    fn draw_ui(&mut self, frame: AshFrameUi<'_>) -> ExampleResult;

    fn is_complete(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct RuntimeInstancePolicy {
    #[cfg(feature = "ash-validation-smoke")]
    validation_enabled: bool,
    #[cfg(feature = "ash-validation-smoke")]
    require_software_vulkan: bool,
}

#[derive(Default)]
struct RuntimeValidation {
    #[cfg(feature = "ash-validation-smoke")]
    state: Arc<validation::ValidationState>,
}

#[derive(Default)]
struct RuntimeCallbacks {
    #[cfg(feature = "ash-validation-smoke")]
    validation: Option<validation::RuntimeDrawCallbacks>,
}

#[cfg(feature = "ash-validation-smoke")]
impl RuntimeCallbacks {
    fn load(_context: &Context, required: bool) -> ExampleResult<Self> {
        if !required {
            return Ok(Self::default());
        }

        Ok(Self {
            validation: Some(validation::load_renderer_callbacks(_context)?),
        })
    }
}

#[cfg_attr(feature = "ash-validation-smoke", allow(dead_code))]
struct RuntimeFrameUi<'a> {
    ui: &'a Ui,
    viewport_count: usize,
    surface_format: vk::Format,
    framebuffer_srgb: bool,
    clear_color: &'a mut [f32; 4],
    demo_open: &'a mut bool,
    _callbacks: &'a RuntimeCallbacks,
}

#[cfg_attr(feature = "ash-validation-smoke", allow(dead_code))]
impl<'a> RuntimeFrameUi<'a> {
    fn into_interactive(self) -> AshFrameUi<'a> {
        AshFrameUi {
            ui: self.ui,
            viewport_count: self.viewport_count,
            surface_format: self.surface_format,
            framebuffer_srgb: self.framebuffer_srgb,
            clear_color: self.clear_color,
            demo_open: self.demo_open,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct RuntimeFrameDirective {
    #[cfg(feature = "ash-validation-smoke")]
    callback_only: bool,
}

trait RuntimeScenario: 'static {
    #[cfg(feature = "ash-validation-smoke")]
    type Evidence;

    #[cfg(feature = "ash-validation-smoke")]
    fn instance_policy(&self) -> RuntimeInstancePolicy {
        RuntimeInstancePolicy::default()
    }

    fn requires_dynamic_rendering(&self) -> bool {
        false
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn requires_validation(&self) -> bool {
        false
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn requires_renderer_callbacks(&self) -> bool {
        false
    }

    fn initialize(
        &mut self,
        _context: &mut Context,
        _adapter: &VulkanAdapterInfo,
        _validation: &RuntimeValidation,
    ) -> ExampleResult {
        Ok(())
    }

    fn prepare_frame(&mut self, _context: &mut Context) -> ExampleResult {
        Ok(())
    }

    fn begin_frame(&mut self) -> ExampleResult {
        Ok(())
    }

    fn draw_ui(&mut self, frame: RuntimeFrameUi<'_>) -> ExampleResult<RuntimeFrameDirective>;

    #[cfg(feature = "ash-validation-smoke")]
    fn observe_secondary_submissions(
        &mut self,
        _report: validation::RuntimeSecondarySubmissions<'_>,
    ) {
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn completion_request(&self) -> validation::RuntimeCompletionRequest {
        validation::RuntimeCompletionRequest::default()
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn observe_frame_outcome(&mut self, _outcome: validation::RuntimeFrameOutcome) {}

    fn is_complete(&self) -> bool {
        false
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn completed_evidence(&self) -> Option<Self::Evidence> {
        None
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn finalize(
        _evidence: Self::Evidence,
        _teardown: validation::RuntimeTeardownEvidence,
    ) -> ExampleResult {
        Ok(())
    }
}

#[cfg_attr(feature = "ash-validation-smoke", allow(dead_code))]
struct InteractiveScenarioAdapter<S>(S);

impl<S: AshViewportScenario> RuntimeScenario for InteractiveScenarioAdapter<S> {
    #[cfg(feature = "ash-validation-smoke")]
    type Evidence = ();

    fn requires_dynamic_rendering(&self) -> bool {
        self.0.requires_dynamic_rendering()
    }

    fn initialize(
        &mut self,
        context: &mut Context,
        adapter: &VulkanAdapterInfo,
        _validation: &RuntimeValidation,
    ) -> ExampleResult {
        self.0.initialize(context, adapter)
    }

    fn prepare_frame(&mut self, context: &mut Context) -> ExampleResult {
        self.0.prepare_frame(context)
    }

    fn begin_frame(&mut self) -> ExampleResult {
        self.0.begin_frame()
    }

    fn draw_ui(&mut self, frame: RuntimeFrameUi<'_>) -> ExampleResult<RuntimeFrameDirective> {
        self.0.draw_ui(frame.into_interactive())?;
        Ok(RuntimeFrameDirective::default())
    }

    fn is_complete(&self) -> bool {
        self.0.is_complete()
    }
}

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
    #[cfg(feature = "ash-validation-smoke")]
    debug_loader: Option<ext_debug_utils::Instance>,
    #[cfg(feature = "ash-validation-smoke")]
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    surface_loader: Option<khr_surface::Instance>,
    surface: Option<vk::SurfaceKHR>,
    device: Option<Device>,
    command_pool: Option<vk::CommandPool>,
    window_keepalive: Option<Arc<Window>>,
}

impl VulkanContextInit {
    fn new(entry: Entry, window_keepalive: Arc<Window>) -> Self {
        Self {
            entry: Some(entry),
            instance: None,
            #[cfg(feature = "ash-validation-smoke")]
            debug_loader: None,
            #[cfg(feature = "ash-validation-smoke")]
            debug_messenger: None,
            surface_loader: None,
            surface: None,
            device: None,
            command_pool: None,
            window_keepalive: Some(window_keepalive),
        }
    }

    fn finish(
        mut self,
        validation: RuntimeValidation,
        _policy: RuntimeInstancePolicy,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
        queue: vk::Queue,
        adapter: VulkanAdapterInfo,
    ) -> VulkanContext {
        VulkanContext {
            entry: self.entry.take().expect("Vulkan entry was initialized"),
            instance: self
                .instance
                .take()
                .expect("Vulkan instance was initialized"),
            #[cfg(feature = "ash-validation-smoke")]
            debug_loader: self.debug_loader.take(),
            #[cfg(feature = "ash-validation-smoke")]
            debug_messenger: self
                .debug_messenger
                .take()
                .unwrap_or(vk::DebugUtilsMessengerEXT::null()),
            validation,
            #[cfg(feature = "ash-validation-smoke")]
            validation_enabled: _policy.validation_enabled,
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
            adapter,
            teardown_state: DeviceTeardownState::Pending,
            _window_keepalive: self
                .window_keepalive
                .take()
                .expect("Vulkan window keepalive was initialized"),
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
            #[cfg(feature = "ash-validation-smoke")]
            if let (Some(debug_loader), Some(debug_messenger)) =
                (self.debug_loader.as_ref(), self.debug_messenger.take())
            {
                debug_loader.destroy_debug_utils_messenger(debug_messenger, None);
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
    #[cfg(feature = "ash-validation-smoke")]
    debug_loader: Option<ext_debug_utils::Instance>,
    #[cfg(feature = "ash-validation-smoke")]
    debug_messenger: vk::DebugUtilsMessengerEXT,
    validation: RuntimeValidation,
    #[cfg(feature = "ash-validation-smoke")]
    validation_enabled: bool,
    surface_loader: khr_surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    queue_family_index: u32,
    device: Device,
    queue: vk::Queue,
    command_pool: vk::CommandPool,
    adapter: VulkanAdapterInfo,
    teardown_state: DeviceTeardownState,
    // A leaked surface must not outlive the native window it was created from.
    _window_keepalive: Arc<Window>,
}

impl VulkanContext {
    fn new(
        window: &Arc<Window>,
        title: &str,
        policy: RuntimeInstancePolicy,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let validation = RuntimeValidation::default();
        let entry = unsafe { Entry::load()? };
        let mut init = VulkanContextInit::new(entry, Arc::clone(window));

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

        let extensions =
            ash_window::enumerate_required_extensions(window.display_handle()?.as_raw())?.to_vec();
        #[cfg(feature = "ash-validation-smoke")]
        let mut extensions = extensions;
        #[cfg(feature = "ash-validation-smoke")]
        let layer_names = if policy.validation_enabled {
            let available_layers = unsafe {
                init.entry
                    .as_ref()
                    .expect("Vulkan entry was initialized")
                    .enumerate_instance_layer_properties()?
            };
            let available = available_layers.iter().any(|layer| unsafe {
                CStr::from_ptr(layer.layer_name.as_ptr()) == validation::VALIDATION_LAYER
            });
            if !available {
                return Err("VK_LAYER_KHRONOS_validation is required but unavailable".into());
            }
            extensions.push(ext_debug_utils::NAME.as_ptr());
            vec![validation::VALIDATION_LAYER.as_ptr()]
        } else {
            Vec::new()
        };
        #[cfg(not(feature = "ash-validation-smoke"))]
        let layer_names: Vec<*const i8> = Vec::new();

        let instance_create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layer_names);
        #[cfg(feature = "ash-validation-smoke")]
        let mut debug_create_info = validation::validation_messenger_info(&validation.state);
        #[cfg(feature = "ash-validation-smoke")]
        let instance_create_info = if policy.validation_enabled {
            instance_create_info.push_next(&mut debug_create_info)
        } else {
            instance_create_info
        };
        let instance = unsafe {
            init.entry
                .as_ref()
                .expect("Vulkan entry was initialized")
                .create_instance(&instance_create_info, None)?
        };
        init.instance = Some(instance);

        #[cfg(feature = "ash-validation-smoke")]
        if policy.validation_enabled {
            let loader = ext_debug_utils::Instance::new(
                init.entry.as_ref().expect("Vulkan entry was initialized"),
                init.instance
                    .as_ref()
                    .expect("Vulkan instance was initialized"),
            );
            let messenger =
                unsafe { loader.create_debug_utils_messenger(&debug_create_info, None)? };
            init.debug_loader = Some(loader);
            init.debug_messenger = Some(messenger);
        }

        init.surface_loader = Some(khr_surface::Instance::new(
            init.entry.as_ref().expect("Vulkan entry was initialized"),
            init.instance
                .as_ref()
                .expect("Vulkan instance was initialized"),
        ));
        let surface = unsafe {
            ash_window::create_surface(
                init.entry.as_ref().expect("Vulkan entry was initialized"),
                init.instance
                    .as_ref()
                    .expect("Vulkan instance was initialized"),
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
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
        let adapter = describe_physical_device(
            init.instance
                .as_ref()
                .expect("Vulkan instance was initialized"),
            physical_device,
        );
        #[cfg(feature = "ash-validation-smoke")]
        if policy.require_software_vulkan {
            validation::validate_software_adapter(&adapter)?;
        }

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

        Ok(init.finish(
            validation,
            policy,
            physical_device,
            queue_family_index,
            queue,
            adapter,
        ))
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
            #[cfg(feature = "ash-validation-smoke")]
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
    Single(Box<AshRenderer>),
    Viewports(ash_mvp::WinitViewportRoute),
}

enum RendererFrameCompletion {
    Single(Option<dear_imgui_ash::TextureRetirementBatch>),
    Viewports(ash_mvp::AshViewportFrameCompletion),
}

enum PreparedRendererFrame<'frame> {
    Single {
        frame: dear_imgui_rs::render::ReconciledFrame<'frame>,
        retirement: Option<dear_imgui_ash::TextureRetirementBatch>,
    },
    Viewports(ash_mvp::AshPreparedViewportFrame<'frame>),
}

impl PreparedRendererFrame<'_> {
    #[cfg(feature = "ash-validation-smoke")]
    fn draw_data(&self) -> &dear_imgui_rs::render::DrawData {
        match self {
            Self::Single { frame, .. } => frame.draw_data(),
            Self::Viewports(frame) => frame.draw_data(),
        }
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn secondary_report(&self) -> Option<&ash_mvp::AshViewportFrameReport> {
        match self {
            Self::Single { .. } => None,
            Self::Viewports(frame) => Some(frame.secondary_report()),
        }
    }

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

    fn cmd_draw_main(
        &mut self,
        command_buffer: vk::CommandBuffer,
        prepared: PreparedRendererFrame<'_>,
    ) -> Result<RendererFrameCompletion, Box<dyn std::error::Error>> {
        // `record_command_buffer` supplies a live command buffer inside the renderer-compatible
        // render scope, and the example submits it only to the renderer's configured queue.
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
        event_loop: &ActiveEventLoop,
        frame: FrameToken<'ctx>,
    ) -> Result<PreparedRendererFrame<'ctx>, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Single(renderer) => {
                let pending_frame = frame.try_render(renderer.renderer_consumer()?)?;
                let (frame, retirement) = renderer.prepare_frame(pending_frame)?;
                PreparedRendererFrame::Single { frame, retirement }
            }
            Self::Viewports(route) => {
                PreparedRendererFrame::Viewports(route.prepare(event_loop, frame)?)
            }
        })
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn expect_null_completion_fence_rejected(
        &mut self,
        completion: RendererFrameCompletion,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match (self, completion) {
            (Self::Single(renderer), RendererFrameCompletion::Single(Some(batch))) => {
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
            (Self::Viewports(route), RendererFrameCompletion::Viewports(completion)) => {
                match unsafe { route.complete_frame_with_fences(completion, &[vk::Fence::null()]) }
                {
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
            (Self::Single(_), RendererFrameCompletion::Single(None)) => {
                Err("null-fence probe reached a frame without texture retirement".into())
            }
            _ => Err("frame completion does not belong to the active Ash route".into()),
        }
    }

    #[cfg(feature = "ash-validation-smoke")]
    unsafe fn complete_frame_with_fences(
        &mut self,
        completion: RendererFrameCompletion,
        fences: &[vk::Fence],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(match (self, completion) {
            (Self::Single(renderer), RendererFrameCompletion::Single(Some(batch))) => unsafe {
                renderer.complete_texture_retirements_with_fences(batch, fences)?
            },
            (Self::Single(_), RendererFrameCompletion::Single(None)) => 0,
            (Self::Viewports(route), RendererFrameCompletion::Viewports(completion)) => unsafe {
                route.complete_frame_with_fences(completion, fences)?
            },
            _ => return Err("frame completion does not belong to the active Ash route".into()),
        })
    }

    fn wait_for_frame_completion(
        &mut self,
        completion: RendererFrameCompletion,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(match (self, completion) {
            (Self::Single(renderer), RendererFrameCompletion::Single(Some(batch))) => {
                renderer.wait_for_texture_retirements(batch)?
            }
            (Self::Single(_), RendererFrameCompletion::Single(None)) => 0,
            (Self::Viewports(route), RendererFrameCompletion::Viewports(completion)) => {
                route.wait_for_frame_completion(completion)?
            }
            _ => return Err("frame completion does not belong to the active Ash route".into()),
        })
    }

    fn shutdown(&mut self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => renderer.shutdown(context)?,
            Self::Viewports(route) => route.shutdown(context)?,
        }
        Ok(())
    }
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

impl Drop for VulkanState {
    fn drop(&mut self) {
        if let Err(error) = self.ctx.wait_idle_for_teardown() {
            error!(
                ?error,
                "Vulkan device-idle proof failed; leaking frame, swapchain, render-target, and device ownership"
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

struct ImguiState {
    renderer: RendererRuntime,
    platform: WinitPlatform,
    context: Context,
    clear_color: [f32; 4],
    demo_open: bool,
    last_frame: Instant,
    callbacks: RuntimeCallbacks,
}

struct AppWindow<S: RuntimeScenario> {
    imgui: ManuallyDrop<ImguiState>,
    vk: ManuallyDrop<VulkanState>,
    // A scenario may retain device-lineage resources, so it belongs to the same terminal-proof
    // boundary as the renderer and native Vulkan owners.
    scenario: ManuallyDrop<S>,
    // Keep the platform window alive until renderer, swapchains, and surfaces have been dropped.
    window: ManuallyDrop<Arc<Window>>,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    gpu_idle_before_teardown: bool,
}

struct AppWindowInit {
    renderer: Option<RendererRuntime>,
    platform: Option<WinitPlatform>,
    context: Option<Context>,
    frames: Vec<FrameSync>,
    swapchain: Option<SwapchainState>,
    render_target: Option<MainRenderTarget>,
    ctx: Option<VulkanContext>,
}

impl AppWindowInit {
    fn new(ctx: VulkanContext) -> Self {
        Self {
            renderer: None,
            platform: None,
            context: None,
            frames: Vec::new(),
            swapchain: None,
            render_target: None,
            ctx: Some(ctx),
        }
    }

    fn finish<S: RuntimeScenario>(
        mut self,
        window: Arc<Window>,
        scenario: S,
        callbacks: RuntimeCallbacks,
    ) -> AppWindow<S> {
        let image_count = self
            .swapchain
            .as_ref()
            .expect("Ash swapchain was initialized")
            .images
            .len();
        AppWindow {
            imgui: ManuallyDrop::new(ImguiState {
                renderer: self.renderer.take().expect("Ash renderer was initialized"),
                platform: self
                    .platform
                    .take()
                    .expect("Winit platform was initialized"),
                context: self.context.take().expect("ImGui context was initialized"),
                clear_color: [0.1, 0.12, 0.15, 1.0],
                demo_open: true,
                last_frame: Instant::now(),
                callbacks,
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
            scenario: ManuallyDrop::new(scenario),
            window: ManuallyDrop::new(window),
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
            gpu_idle_before_teardown: false,
        }
    }

    fn leak_ownership_tree(&mut self) {
        if let Some(renderer) = self.renderer.take() {
            std::mem::forget(renderer);
        }
        if let Some(platform) = self.platform.take() {
            std::mem::forget(platform);
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
    }
}

impl Drop for AppWindowInit {
    fn drop(&mut self) {
        let wait_error = self
            .ctx
            .as_mut()
            .and_then(|ctx| ctx.wait_idle_for_teardown().err());
        if let Some(error) = wait_error {
            error!(
                ?error,
                "Vulkan initialization rollback lacks a terminal GPU proof; leaking the complete ownership tree"
            );
            self.leak_ownership_tree();
            return;
        }

        if let Some(context) = self.context.as_mut() {
            context.end_frame();
            if let Some(renderer) = self.renderer.as_mut()
                && renderer.shutdown(context).is_ok()
                && let Some(ctx) = self.ctx.as_mut()
            {
                ctx.note_renderer_shutdown();
            }
            if let Some(platform) = self.platform.as_mut() {
                if platform.viewports_enabled() {
                    let _ = platform.disable_viewports(context);
                }
                if !platform.viewports_enabled() {
                    let _ = platform.shutdown(context);
                }
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

impl<S: RuntimeScenario> Drop for AppWindow<S> {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            error!("Ash example fallback shutdown failed: {error}");
        }
        if !self.vk.ctx.teardown_is_proven() {
            error!(
                "Ash fallback teardown lacks a terminal GPU proof; leaking the ImGui, Vulkan, and window ownership tree"
            );
            return;
        }

        // SAFETY: the terminal GPU proof permits ordered destruction. ImGui attachments are
        // released before Vulkan children and their device, while the platform window remains live.
        unsafe {
            ManuallyDrop::drop(&mut self.scenario);
            ManuallyDrop::drop(&mut self.imgui);
            ManuallyDrop::drop(&mut self.vk);
            ManuallyDrop::drop(&mut self.window);
        }
    }
}

struct App<S: RuntimeScenario> {
    pending_scenario: Option<S>,
    window: Option<Box<AppWindow<S>>>,
    error: Option<String>,
}

impl<S: RuntimeScenario> App<S> {
    fn new(scenario: S) -> Self {
        Self {
            pending_scenario: Some(scenario),
            window: None,
            error: None,
        }
    }
}

impl<S: RuntimeScenario> AppWindow<S> {
    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let ImguiState {
            renderer,
            platform,
            context,
            ..
        } = &mut *self.imgui;
        let mut errors = Vec::new();
        context.end_frame();
        if !self.renderer_shutdown_complete {
            match renderer.shutdown(context) {
                Ok(()) => {
                    self.renderer_shutdown_complete = true;
                    self.vk.ctx.note_renderer_shutdown();
                    self.gpu_idle_before_teardown = true;
                }
                Err(error) => {
                    errors.push(format!("Ash renderer shutdown failed: {error}"));
                    match self.vk.ctx.wait_idle_for_teardown() {
                        Ok(()) => {
                            self.gpu_idle_before_teardown = true;
                            if self.vk.ctx.device_lost() {
                                self.renderer_shutdown_complete = true;
                            }
                        }
                        Err(error) => {
                            errors.push(format!("Ash device-idle wait failed: {error}"));
                            return Err(errors.join("; ").into());
                        }
                    }
                    if !self.renderer_shutdown_complete {
                        return Err(errors.join("; ").into());
                    }
                }
            }
        }

        if !self.gpu_idle_before_teardown {
            match self.vk.ctx.wait_idle_for_teardown() {
                Ok(()) => self.gpu_idle_before_teardown = true,
                Err(error) => {
                    errors.push(format!("Ash device-idle wait failed: {error}"));
                    return Err(errors.join("; ").into());
                }
            }
        }

        let runtime_error = if platform.viewports_enabled() {
            platform.disable_viewports(context).err()
        } else {
            None
        };

        let platform_error = if !self.platform_shutdown_complete && !platform.viewports_enabled() {
            platform.shutdown(context).err()
        } else {
            None
        };
        if platform_error.is_none() && !self.platform_shutdown_complete {
            self.platform_shutdown_complete = !platform.viewports_enabled();
        }

        if let Some(error) = runtime_error {
            errors.push(format!("Winit multi-viewport shutdown failed: {error}"));
        }
        if let Some(error) = platform_error {
            errors.push(format!("Winit platform shutdown failed: {error}"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; ").into())
        }
    }

    #[cfg(feature = "ash-validation-smoke")]
    fn teardown_evidence(&self) -> validation::RuntimeTeardownEvidence {
        validation::RuntimeTeardownEvidence {
            renderer_shutdown_complete: self.renderer_shutdown_complete,
            viewport_runtime_shutdown_complete: !self.imgui.platform.viewports_enabled(),
            platform_shutdown_complete: self.platform_shutdown_complete,
            gpu_idle_before_teardown: self.gpu_idle_before_teardown,
        }
    }

    fn new(event_loop: &ActiveEventLoop, mut scenario: S) -> ExampleResult<Self> {
        let enable_viewports = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));
        #[cfg(feature = "ash-validation-smoke")]
        let instance_policy = scenario.instance_policy();
        #[cfg(not(feature = "ash-validation-smoke"))]
        let instance_policy = RuntimeInstancePolicy::default();
        if scenario.requires_dynamic_rendering() && !cfg!(feature = "ash-dynamic-rendering") {
            return Err("this Ash scenario requires feature `ash-dynamic-rendering`".into());
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

        let ctx = VulkanContext::new(&window, "dear-imgui-multi-viewport-ash", instance_policy)?;
        let mut init = AppWindowInit::new(ctx);
        #[cfg(feature = "ash-validation-smoke")]
        if scenario.requires_validation()
            && !init
                .ctx
                .as_ref()
                .expect("Vulkan context was initialized")
                .validation_enabled
        {
            return Err("this Ash scenario requires Vulkan validation to be enabled".into());
        }
        let surface_format = pick_surface_format(
            init.ctx.as_ref().expect("Vulkan context was initialized"),
            &window,
        )?;
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
            &window,
            render_target,
            surface_format,
        )?;
        init.swapchain = Some(swapchain);

        let mut imgui = Context::create();
        imgui.set_ini_filename(None::<String>)?;

        if enable_viewports {
            imgui.enable_multi_viewport();
        }
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);
        }
        init.context = Some(imgui);

        let platform = WinitPlatform::new(
            init.context
                .as_mut()
                .expect("ImGui context was initialized"),
        )?;
        init.platform = Some(platform);
        {
            let AppWindowInit {
                platform, context, ..
            } = &mut init;
            let platform = platform.as_mut().expect("Winit platform was initialized");
            let context = context.as_mut().expect("ImGui context was initialized");
            platform.attach_window(Arc::clone(&window), HiDpiMode::Default, context)?;
            if enable_viewports {
                platform.enable_viewports(context)?;
            }
        }

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
        let mut renderer = {
            let AppWindowInit { ctx, context, .. } = &mut init;
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
        init.renderer = Some(RendererRuntime::Single(Box::new(renderer)));
        #[cfg(feature = "ash-validation-smoke")]
        let callbacks = RuntimeCallbacks::load(
            init.context
                .as_ref()
                .expect("ImGui context was initialized"),
            scenario.requires_renderer_callbacks(),
        )?;
        #[cfg(not(feature = "ash-validation-smoke"))]
        let callbacks = RuntimeCallbacks::default();
        {
            let AppWindowInit { ctx, context, .. } = &mut init;
            let ctx = ctx.as_ref().expect("Vulkan context was initialized");
            scenario.initialize(
                context.as_mut().expect("ImGui context was initialized"),
                &ctx.adapter,
                &ctx.validation,
            )?;
        }

        let renderer = match init.renderer.take() {
            Some(RendererRuntime::Single(renderer)) => *renderer,
            _ => unreachable!("initial Ash renderer must be in single-viewport state"),
        };
        let renderer = if enable_viewports {
            let viewport_config = {
                let ctx = init.ctx.as_ref().expect("Vulkan context was initialized");
                let swapchain = init
                    .swapchain
                    .as_ref()
                    .expect("Ash swapchain was initialized");
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
                }
            };
            let attach_result = {
                let AppWindowInit {
                    platform, context, ..
                } = &mut init;
                unsafe {
                    ash_mvp::WinitViewportRoute::attach(
                        context.as_mut().expect("ImGui context was initialized"),
                        platform.as_ref().expect("Winit platform was initialized"),
                        renderer,
                        viewport_config,
                    )
                }
            };
            match attach_result {
                Ok(route) => RendererRuntime::Viewports(route),
                Err(error) => {
                    let (error, renderer) = error.into_parts();
                    init.renderer = Some(RendererRuntime::Single(Box::new(renderer)));
                    return Err(error.into());
                }
            }
        } else {
            RendererRuntime::Single(Box::new(renderer))
        };
        init.renderer = Some(renderer);

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
        Ok(init.finish(window, scenario, callbacks))
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.vk.swapchain_dirty = true;
    }

    fn recover_aborted_main_acquire(
        vk: &mut VulkanState,
        window: &Arc<Window>,
        frame_index: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        vk.swapchain_dirty = true;
        unsafe { vk.ctx.device.device_wait_idle()? };

        let frame = vk
            .frames
            .get_mut(frame_index)
            .ok_or("Ash main frame disappeared during acquire recovery")?;
        let abandoned_fence = frame.fence;
        clear_fence_references(&mut vk.images_in_flight, abandoned_fence);
        let _ = replace_frame_sync(&vk.ctx.device, vk.ctx.command_pool, frame)?;

        vk.swapchain
            .recreate_after_device_idle(&vk.ctx, window, vk.render_target)?;
        vk.images_in_flight = vec![vk::Fence::null(); vk.swapchain.images.len()];
        vk.swapchain_dirty = false;

        Ok(())
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn std::error::Error>> {
        let window = &*self.window;
        let vk = &mut *self.vk;
        let scenario = &mut *self.scenario;
        let ImguiState {
            renderer,
            platform,
            context,
            clear_color,
            demo_open,
            last_frame,
            callbacks,
        } = &mut *self.imgui;

        let window_size = window.inner_size();
        if window_size.width == 0 || window_size.height == 0 {
            return Ok(());
        }
        if vk.swapchain_dirty {
            vk.swapchain.recreate(&vk.ctx, window, vk.render_target)?;
            vk.images_in_flight = vec![vk::Fence::null(); vk.swapchain.images.len()];
            vk.swapchain_dirty = false;
        }
        scenario.prepare_frame(context)?;

        let now = Instant::now();
        let dt = (now - *last_frame).as_secs_f32();
        context.io_mut().set_delta_time(dt);
        *last_frame = now;

        platform.prepare_frame(context, window)?;
        scenario.begin_frame()?;
        let viewport_count = context.platform_io().viewports_iter().count();
        let frame = context.begin_frame();
        let ui = frame.ui();
        let directive = scenario.draw_ui(RuntimeFrameUi {
            ui,
            viewport_count,
            surface_format: vk.swapchain.surface_format.format,
            framebuffer_srgb: is_srgb_format(vk.swapchain.surface_format.format),
            clear_color,
            demo_open,
            _callbacks: callbacks,
        })?;
        renderer.set_viewport_clear_color(*clear_color)?;

        platform.prepare_render(ui, window)?;
        let clear_color = *clear_color;
        let prepared = renderer.prepare(event_loop, frame)?;
        #[cfg(feature = "ash-validation-smoke")]
        let callback_only_zero_geometry = directive.callback_only
            && prepared.draw_data().total_vtx_count() == 0
            && prepared.draw_data().total_idx_count() == 0;
        #[cfg(not(feature = "ash-validation-smoke"))]
        let _ = directive;

        // Secondary swapchains submit and present before the main swapchain is acquired. This
        // avoids overlapping WSI acquisition semaphores across independently owned surfaces. The
        // prepared transaction makes managed texture updates visible to those draws before the
        // main command buffer consumes it.
        #[cfg(feature = "ash-validation-smoke")]
        if let Some(report) = prepared.secondary_report() {
            scenario.observe_secondary_submissions(validation::RuntimeSecondarySubmissions {
                rendered: report.render_submitted_viewport_ids(),
                presented: report.present_submitted_viewport_ids(),
            });
        }

        let frame_index = vk.frame_index % vk.frames.len();
        let frame_fence = vk.frames[frame_index].fence;
        let image_available = vk.frames[frame_index].image_available;
        let command_buffer = vk.frames[frame_index].command_buffer;

        unsafe {
            vk.ctx
                .device
                .wait_for_fences(&[frame_fence], true, u64::MAX)?;
        }

        let acquire = unsafe {
            vk.swapchain.loader.acquire_next_image(
                vk.swapchain.swapchain,
                u64::MAX,
                image_available,
                vk::Fence::null(),
            )
        };

        let (image_index, suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                vk.swapchain_dirty = true;
                let completion = prepared.skip_main();
                #[cfg(feature = "ash-validation-smoke")]
                {
                    let completion_request = scenario.completion_request();
                    let (null_fence_rejected, completion_count, queue_drained) =
                        if completion_request.reject_null_fence {
                            renderer.expect_null_completion_fence_rejected(completion)?;
                            (true, 0, false)
                        } else {
                            let completion_count =
                                renderer.wait_for_frame_completion(completion)?;
                            (false, completion_count, true)
                        };
                    scenario.observe_frame_outcome(validation::RuntimeFrameOutcome {
                        main_presented: false,
                        callback_only_zero_geometry: None,
                        render_state_cleared: None,
                        null_fence_rejected,
                        fence_completion_count: completion_count,
                        texture_retirement_queue_drained: queue_drained,
                    });
                }
                #[cfg(not(feature = "ash-validation-smoke"))]
                renderer.wait_for_frame_completion(completion)?;
                return Ok(());
            }
            Err(error) => {
                vk.swapchain_dirty = true;
                return Err(Box::new(error));
            }
        };
        let image_index_usize = image_index as usize;
        let submission =
            (|| -> Result<(RendererFrameCompletion, vk::Semaphore), Box<dyn std::error::Error>> {
                let image_fence = vk
                    .images_in_flight
                    .get(image_index_usize)
                    .copied()
                    .ok_or("acquired Ash main image has no in-flight fence slot")?;
                let present_semaphore = vk
                    .swapchain
                    .present_semaphores
                    .get(image_index_usize)
                    .copied()
                    .ok_or("acquired Ash main image has no present semaphore")?;
                #[cfg(not(feature = "ash-dynamic-rendering"))]
                let framebuffer = vk
                    .swapchain
                    .framebuffers
                    .get(image_index_usize)
                    .copied()
                    .ok_or("acquired Ash main image has no framebuffer")?;
                #[cfg(feature = "ash-dynamic-rendering")]
                let image = vk
                    .swapchain
                    .images
                    .get(image_index_usize)
                    .copied()
                    .ok_or("acquired Ash main image is missing")?;
                #[cfg(feature = "ash-dynamic-rendering")]
                let image_view = vk
                    .swapchain
                    .image_views
                    .get(image_index_usize)
                    .copied()
                    .ok_or("acquired Ash main image has no image view")?;
                #[cfg(feature = "ash-dynamic-rendering")]
                let old_layout = vk
                    .swapchain
                    .image_layouts
                    .get(image_index_usize)
                    .copied()
                    .ok_or("acquired Ash main image has no tracked layout")?;

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

                let target = MainCommandTarget {
                    #[cfg(not(feature = "ash-dynamic-rendering"))]
                    framebuffer,
                    #[cfg(not(feature = "ash-dynamic-rendering"))]
                    render_pass: vk.render_target.render_pass,
                    #[cfg(feature = "ash-dynamic-rendering")]
                    image,
                    #[cfg(feature = "ash-dynamic-rendering")]
                    image_view,
                    #[cfg(feature = "ash-dynamic-rendering")]
                    old_layout,
                    extent: vk.swapchain.extent,
                    clear_color,
                };
                let completion =
                    record_command_buffer(&vk.ctx.device, command_buffer, target, |cmd| {
                        renderer.cmd_draw_main(cmd, prepared)
                    })?;
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
                Ok((completion, present_semaphore))
            })();
        let (completion, present_semaphore) = match submission {
            Ok(submission) => submission,
            Err(error) => {
                if let Err(recovery_error) =
                    Self::recover_aborted_main_acquire(vk, window, frame_index)
                {
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
            vk.swapchain.image_layouts[image_index_usize] = vk::ImageLayout::PRESENT_SRC_KHR;
        }
        vk.images_in_flight[image_index_usize] = submitted_fence;
        vk.swapchain_dirty |= suboptimal;

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

        #[cfg(feature = "ash-validation-smoke")]
        {
            let completion_request = scenario.completion_request();
            let (null_fence_rejected, fence_completion_count, texture_retirement_queue_drained) =
                if completion_request.reject_null_fence {
                    renderer.expect_null_completion_fence_rejected(completion)?;
                    (true, 0, false)
                } else if completion_request.complete_with_submitted_fence {
                    unsafe {
                        vk.ctx
                            .device
                            .wait_for_fences(&[submitted_fence], true, u64::MAX)?;
                        let completion_count =
                            renderer.complete_frame_with_fences(completion, &[submitted_fence])?;
                        (false, completion_count, true)
                    }
                } else {
                    renderer.wait_for_frame_completion(completion)?;
                    (false, 0, true)
                };

            let render_state_cleared =
                unsafe { context.platform_io().renderer_render_state().is_null() };
            scenario.observe_frame_outcome(validation::RuntimeFrameOutcome {
                main_presented: true,
                callback_only_zero_geometry: Some(callback_only_zero_geometry),
                render_state_cleared: Some(render_state_cleared),
                null_fence_rejected,
                fence_completion_count,
                texture_retirement_queue_drained,
            });
        }
        #[cfg(not(feature = "ash-validation-smoke"))]
        renderer.wait_for_frame_completion(completion)?;

        Ok(())
    }
}

impl<S: RuntimeScenario> ApplicationHandler for App<S> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.window.request_redraw();
            return;
        }
        let Some(scenario) = self.pending_scenario.take() else {
            self.error = Some("Ash scenario was already consumed".to_owned());
            event_loop.exit();
            return;
        };
        match AppWindow::new(event_loop, scenario) {
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

        let ImguiState {
            platform, context, ..
        } = &mut *app.imgui;
        if let Err(error) = platform.handle_event(context, &app.window, &full) {
            error!("Winit platform event error: {error}");
            self.error = Some(error.to_string());
            event_loop.exit();
            return;
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
            WindowEvent::RedrawRequested if is_main_window => {
                // We drive rendering from the main window. Secondary viewport windows are
                // rendered via ImGui's platform callbacks during `app.render()`.
                match app.render(event_loop) {
                    Ok(()) => {
                        if app.scenario.is_complete() {
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
            _ => {}
        }
    }
}

fn run_runtime<S: RuntimeScenario>(scenario: S) -> ExampleResult {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(scenario);
    let event_loop_result = event_loop.run_app(&mut app);
    let app_error = app.error.take();
    #[cfg(feature = "ash-validation-smoke")]
    let scenario_evidence = app
        .window
        .as_ref()
        .and_then(|window| window.scenario.completed_evidence());
    let shutdown_result = app
        .window
        .as_mut()
        .map_or(Ok(()), |window| window.shutdown());
    #[cfg(feature = "ash-validation-smoke")]
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
    #[cfg(feature = "ash-validation-smoke")]
    if let (Some(evidence), Some(teardown)) = (scenario_evidence, teardown_evidence)
        && let Err(error) = S::finalize(evidence, teardown)
    {
        errors.push(error.to_string());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; ").into())
    }
}

#[cfg_attr(feature = "ash-validation-smoke", allow(dead_code))]
pub fn run<S: AshViewportScenario>(scenario: S) -> ExampleResult {
    dear_imgui_examples::init_tracing_with_filter("dear_imgui=debug,multi_viewport_ash=info");
    info!("Starting Dear ImGui Multi-Viewport (ash) Example");
    run_runtime(InteractiveScenarioAdapter(scenario))
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

struct MainCommandTarget {
    #[cfg(not(feature = "ash-dynamic-rendering"))]
    render_pass: vk::RenderPass,
    #[cfg(not(feature = "ash-dynamic-rendering"))]
    framebuffer: vk::Framebuffer,
    #[cfg(feature = "ash-dynamic-rendering")]
    image: vk::Image,
    #[cfg(feature = "ash-dynamic-rendering")]
    image_view: vk::ImageView,
    #[cfg(feature = "ash-dynamic-rendering")]
    old_layout: vk::ImageLayout,
    extent: vk::Extent2D,
    clear_color: [f32; 4],
}

fn record_command_buffer<F, T>(
    device: &Device,
    cmd: vk::CommandBuffer,
    target: MainCommandTarget,
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
                float32: target.clear_color,
            },
        };

        #[cfg(not(feature = "ash-dynamic-rendering"))]
        device.cmd_begin_render_pass(
            cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(target.render_pass)
                .framebuffer(target.framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: target.extent,
                })
                .clear_values(std::slice::from_ref(&clear_value)),
            vk::SubpassContents::INLINE,
        );

        #[cfg(feature = "ash-dynamic-rendering")]
        {
            ash_frame_sync::transition_swapchain_image(
                device,
                cmd,
                target.image,
                target.old_layout,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            let color_attachment = vk::RenderingAttachmentInfo::default()
                .image_view(target.image_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear_value);
            device.cmd_begin_rendering(
                cmd,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: target.extent,
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
                target.image,
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
