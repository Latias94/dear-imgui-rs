//! Vulkan (ash) texture demo (single file): create and update an ImGui-managed texture on the CPU
//! and show it via `Image`.

use ::image::ImageReader;
use ash::{
    Device, Entry, Instance,
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
#[cfg(feature = "ash-dynamic-rendering")]
use dear_imgui_ash::DynamicRendering;
use dear_imgui_ash::{
    AshRenderer, AshRendererConfig, Options as AshOptions, TextureRetirementBatch,
};
use dear_imgui_examples::animated_texture::animated_rgba_pixels;
use dear_imgui_rs::*;
use dear_imgui_winit::WinitPlatform;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    ffi::CString,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
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
        let entry = unsafe { Entry::load()? };

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
        })
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
        let replacement = match Self::new_with_old(
            ctx,
            window,
            render_target,
            self.surface_format,
            self.swapchain,
        ) {
            Ok(replacement) => replacement,
            Err(error) => {
                self.destroy(&ctx.device);
                return Err(error);
            }
        };
        let mut previous = std::mem::replace(self, replacement);
        previous.destroy(&ctx.device);
        Ok(())
    }

    fn destroy(&mut self, device: &Device) {
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
}

impl Drop for SwapchainState {
    fn drop(&mut self) {
        // `destroy()` requires a `Device`; handled by `VulkanState::drop()`.
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
        }
        destroy_frame_syncs(&self.ctx.device, self.ctx.command_pool, &mut self.frames);
        self.swapchain.destroy(&self.ctx.device);
        self.render_target.destroy(&self.ctx.device);
    }
}

struct ImguiState {
    context: Context,
    platform: WinitPlatform,
    renderer: AshRenderer,
    last_frame: Instant,
    animation_started: Instant,
    clear_color: [f32; 4],
    // Texture demo state (managed by ImGui modern texture system)
    img_tex: dear_imgui_rs::ManagedTextureId,
    photo_tex: Option<(dear_imgui_rs::ManagedTextureId, (u32, u32))>,
    tex_size: (u32, u32),
}

struct AppWindow {
    window: Arc<Window>,
    imgui: ImguiState,
    vk: VulkanState,
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        let _ = unsafe { self.vk.ctx.device.device_wait_idle() };
        let _ = self.imgui.renderer.shutdown(&mut self.imgui.context);
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
}

impl AppWindow {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        let size = LogicalSize::new(1280.0, 720.0);
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Dear ImGui Ash - Texture Demo")
                    .with_inner_size(size),
            )?,
        );

        let ctx = VulkanContext::new(&window, "dear-imgui-ash-textures")?;
        let surface_format = pick_surface_format(&ctx)?;
        let render_target = MainRenderTarget::new(&ctx.device, surface_format.format)?;
        let swapchain = SwapchainState::new(&ctx, &window, render_target, surface_format)?;

        // ImGui context
        let mut context = Context::create();
        context.set_ini_filename(None::<String>).unwrap();
        let mut platform = WinitPlatform::new(&mut context)?;
        platform.attach_window(
            Arc::clone(&window),
            dear_imgui_winit::HiDpiMode::Default,
            &mut context,
        )?;

        // Renderer
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
        // SAFETY: all handles were created from ctx's device lineage; the graphics queue and
        // command pool are compatible, and the render target matches the swapchain format.
        let renderer = unsafe {
            AshRenderer::with_default_allocator(
                &ctx.instance,
                ctx.physical_device,
                renderer_config,
                &mut context,
            )?
        };

        // Create a managed ImGui texture (CPU-side pixels; backend will create GPU texture).
        let tex_w: u32 = 128;
        let tex_h: u32 = 128;
        let mut img_tex = dear_imgui_rs::texture::OwnedTextureData::new();
        img_tex.create(dear_imgui_rs::texture::TextureFormat::RGBA32, tex_w, tex_h);
        let pixels = animated_rgba_pixels(tex_w, tex_h, Duration::ZERO);
        img_tex.set_data(&pixels);

        let photo_tex = Self::maybe_load_photo_texture();

        let img_tex = context.register_texture(img_tex);
        let photo_tex = photo_tex.map(|photo| {
            let size = (photo.width(), photo.height());
            (context.register_texture(photo), size)
        });

        // Frame sync objects
        let frames = create_frame_syncs(&ctx.device, ctx.command_pool, FRAMES_IN_FLIGHT)?;
        let images_in_flight = vec![vk::Fence::null(); swapchain.images.len()];
        let now = Instant::now();

        Ok(Self {
            window,
            imgui: ImguiState {
                context,
                platform,
                renderer,
                last_frame: now,
                animation_started: now,
                clear_color: [0.1, 0.2, 0.3, 1.0],
                img_tex,
                photo_tex,
                tex_size: (tex_w, tex_h),
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
        })
    }

    fn maybe_load_photo_texture() -> Option<dear_imgui_rs::texture::OwnedTextureData> {
        let asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let candidates = [
            asset_dir.join("texture_clean.ppm"),
            asset_dir.join("texture.jpg"),
        ];
        let path = candidates.iter().find(|p| p.exists())?.clone();

        let reader = ImageReader::open(&path).ok()?.with_guessed_format().ok()?;
        let img = reader.decode().ok()?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();

        let mut tex = dear_imgui_rs::texture::OwnedTextureData::new();
        tex.create(dear_imgui_rs::texture::TextureFormat::RGBA32, w, h);
        tex.set_data(&rgba);
        Some(tex)
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.vk.swapchain_dirty = true;
    }

    fn recover_aborted_acquire(
        &mut self,
        frame_slot: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.vk.swapchain_dirty = true;
        unsafe { self.vk.ctx.device.device_wait_idle()? };

        let frame = self
            .vk
            .frames
            .get_mut(frame_slot)
            .ok_or("Ash frame slot disappeared during acquire recovery")?;
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

    fn update_texture(&mut self) {
        let (w, h) = self.imgui.tex_size;
        let pixels = animated_rgba_pixels(w, h, self.imgui.animation_started.elapsed());
        self.imgui
            .context
            .with_texture_mut(self.imgui.img_tex, |mut texture| texture.set_data(&pixels))
            .expect("animated texture should remain active");
    }

    fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.vk.swapchain_dirty {
            self.vk
                .swapchain
                .recreate(&self.vk.ctx, &self.window, self.vk.render_target)?;
            self.vk.images_in_flight = vec![vk::Fence::null(); self.vk.swapchain.images.len()];
            self.vk.swapchain_dirty = false;
        }

        let now = Instant::now();
        let delta_time = now - self.imgui.last_frame;
        self.imgui
            .context
            .io_mut()
            .set_delta_time(delta_time.as_secs_f32());
        self.imgui.last_frame = now;

        // Update animated texture (marks WantUpdates).
        self.update_texture();

        self.imgui
            .platform
            .prepare_frame(&self.window, &mut self.imgui.context)?;
        let ui = self.imgui.context.frame();

        ui.window("Ash Texture Demo (ImGui-managed)")
            .size([560.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Wall-clock animation; pixels upload every rendered frame");
                ui.separator();

                ui.color_edit4("Clear color", &mut self.imgui.clear_color);
                ui.separator();

                ui.text("Animated texture:");
                ui.image(self.imgui.img_tex, [256.0, 256.0]);

                if let Some((photo, (width, height))) = self.imgui.photo_tex {
                    ui.separator();
                    ui.text("Loaded image (1:1):");
                    ui.image(photo, [width as f32, height as f32]);
                } else {
                    ui.separator();
                    ui.text_wrapped("Place examples/assets/texture_clean.ppm or texture.jpg to show a loaded image.");
                }
            });

        ui.show_demo_window(&mut true);

        // Finalize inputs on platform and build draw data.
        self.imgui
            .platform
            .prepare_render_with_ui(&ui, &self.window)?;
        let rendered_frame = self.imgui.context.render();

        let frame_slot = self.vk.frame_index % self.vk.frames.len();
        let frame_fence = self.vk.frames[frame_slot].fence;
        let image_available = self.vk.frames[frame_slot].image_available;
        let command_buffer = self.vk.frames[frame_slot].command_buffer;
        unsafe {
            self.vk
                .ctx
                .device
                .wait_for_fences(&[frame_fence], true, u64::MAX)?;
        }
        if let Some(retirement) = self.vk.frames[frame_slot].texture_retirement.take() {
            // SAFETY: this frame fence covers its draw submission and every earlier upload on the
            // same renderer queue.
            unsafe {
                self.imgui
                    .renderer
                    .complete_texture_retirements_with_fences(retirement, &[frame_fence])?;
            }
        }

        let acquire = unsafe {
            self.vk.swapchain.loader.acquire_next_image(
                self.vk.swapchain.swapchain,
                u64::MAX,
                image_available,
                vk::Fence::null(),
            )
        };

        let (image_index, acquire_suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                self.vk.swapchain_dirty = true;
                return Ok(());
            }
            Err(e) => return Err(Box::new(e)),
        };
        let image_index_usize = image_index as usize;
        let submission = (|| -> Result<(Option<TextureRetirementBatch>, vk::Semaphore), Box<dyn std::error::Error>> {
            let image_fence = self
                .vk
                .images_in_flight
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no in-flight fence slot")?;
            let present_semaphore = self
                .vk
                .swapchain
                .present_semaphores
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no present semaphore")?;
            #[cfg(not(feature = "ash-dynamic-rendering"))]
            let framebuffer = self
                .vk
                .swapchain
                .framebuffers
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no framebuffer")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image = self
                .vk
                .swapchain
                .images
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image is missing")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let image_view = self
                .vk
                .swapchain
                .image_views
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no image view")?;
            #[cfg(feature = "ash-dynamic-rendering")]
            let old_layout = self
                .vk
                .swapchain
                .image_layouts
                .get(image_index_usize)
                .copied()
                .ok_or("acquired Ash image has no tracked layout")?;

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

            let texture_retirement = record_command_buffer(
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
                // SAFETY: cmd is recording inside the compatible render pass and is submitted
                // before any renderer resource can be retired or destroyed.
                |cmd| unsafe { self.imgui.renderer.cmd_draw(cmd, rendered_frame) },
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
                if let Err(recovery_error) = self.recover_aborted_acquire(frame_slot) {
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
            self.vk.swapchain.image_layouts[image_index_usize] = vk::ImageLayout::PRESENT_SRC_KHR;
        }
        self.vk.images_in_flight[image_index_usize] = frame_fence;
        self.vk.frames[frame_slot].texture_retirement = texture_retirement;
        self.vk.swapchain_dirty |= acquire_suboptimal;

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
        if let Err(error) =
            imgui
                .platform
                .handle_window_event(&mut imgui.context, &window.window, &event)
        {
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
}

fn main() {
    dear_imgui_examples::init_tracing_with_filter("dear_imgui=debug,ash_textures=info");
    info!("Starting Dear ImGui Ash Texture Example");

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
    _render_target: MainRenderTarget,
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
