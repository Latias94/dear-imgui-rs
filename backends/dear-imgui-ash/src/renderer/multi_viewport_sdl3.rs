// Multi-viewport support (SDL3 platform backend + Ash renderer)
//
// This mirrors the winit multi-viewport renderer callbacks, but creates per-viewport Vulkan
// surfaces by calling the platform backend's `ImGuiPlatformIO::Platform_CreateVkSurface`
// callback (set by `imgui_impl_sdl3.cpp` when initialized for Vulkan).

use super::*;

use ash::vk::Handle;
use ash::{
    khr::{surface as khr_surface, swapchain as khr_swapchain},
    vk,
};
use dear_imgui_rs::Context;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::Viewport;
use dear_imgui_rs::sys;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};

type PlatformCreateVkSurfaceFn = unsafe extern "C" fn(
    vp: *mut sys::ImGuiViewport,
    vk_inst: sys::ImU64,
    vk_allocators: *const c_void,
    out_vk_surface: *mut sys::ImU64,
) -> std::os::raw::c_int;

struct ViewportAshData {
    surface: vk::SurfaceKHR,
    swapchain_loader: khr_swapchain::Device,
    swapchain: vk::SwapchainKHR,
    format: vk::Format,
    extent: vk::Extent2D,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    #[cfg(feature = "dynamic-rendering")]
    image_layouts: Vec<vk::ImageLayout>,
    #[cfg(not(feature = "dynamic-rendering"))]
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    frames: Vec<FrameSync>,
    images_in_flight: Vec<vk::Fence>,
    frame_index: usize,
    pending_present: Option<(usize, u32)>,
    mesh_frames: Frames,
}

struct FrameSync {
    fence: vk::Fence,
    command_buffer: vk::CommandBuffer,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
}

impl ViewportAshData {
    fn destroy(
        mut self,
        renderer: &mut AshRenderer,
        surface_loader: &khr_surface::Instance,
    ) -> RendererResult<()> {
        unsafe {
            let _ = renderer.device.device_wait_idle();
        }

        let _ = self
            .mesh_frames
            .destroy(&renderer.device, &mut renderer.allocator);

        unsafe {
            for f in self.frames.drain(..) {
                renderer.device.destroy_semaphore(f.image_available, None);
                renderer.device.destroy_semaphore(f.render_finished, None);
                renderer.device.destroy_fence(f.fence, None);
                renderer
                    .device
                    .free_command_buffers(self.command_pool, &[f.command_buffer]);
            }
            renderer
                .device
                .destroy_command_pool(self.command_pool, None);

            #[cfg(not(feature = "dynamic-rendering"))]
            for fb in self.framebuffers.drain(..) {
                renderer.device.destroy_framebuffer(fb, None);
            }

            for view in self.image_views.drain(..) {
                renderer.device.destroy_image_view(view, None);
            }

            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            surface_loader.destroy_surface(self.surface, None);
        }

        Ok(())
    }
}

static RENDERER_PTR: AtomicUsize = AtomicUsize::new(0);
static RENDERER_BORROWED: AtomicBool = AtomicBool::new(false);
static GLOBAL: Mutex<Option<GlobalHandles>> = Mutex::new(None);

#[derive(Clone)]
struct GlobalHandles {
    entry: ash::Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    present_queue: vk::Queue,
    graphics_queue_family_index: u32,
    present_queue_family_index: u32,
    in_flight_frames: usize,
    platform_create_vk_surface: PlatformCreateVkSurfaceFn,
}

/// Enable Vulkan multi-viewport (SDL3): installs renderer callbacks.
pub fn enable(
    renderer: &mut AshRenderer,
    imgui_context: &mut Context,
    entry: ash::Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    present_queue: vk::Queue,
    graphics_queue_family_index: u32,
    present_queue_family_index: u32,
) {
    let platform_create_vk_surface = unsafe {
        let platform_io = imgui_context.platform_io_mut();
        let cb = platform_io.platform_create_vk_surface_raw();
        if cb.is_none() {
            eprintln!(
                "[ash-mv-sdl3] Platform_CreateVkSurface is not set. \
                 Ensure the SDL3 platform backend is initialized for Vulkan multi-viewport."
            );
            return;
        }

        platform_io.set_renderer_create_window(Some(
            renderer_create_window as unsafe extern "C" fn(*mut Viewport),
        ));
        platform_io.set_renderer_destroy_window(Some(
            renderer_destroy_window as unsafe extern "C" fn(*mut Viewport),
        ));
        platform_io.set_renderer_set_window_size(Some(
            renderer_set_window_size
                as unsafe extern "C" fn(*mut Viewport, dear_imgui_rs::sys::ImVec2),
        ));
        platform_io.set_platform_render_window_raw(Some(platform_render_window_sys));
        platform_io.set_platform_swap_buffers_raw(Some(platform_swap_buffers_sys));
        cb.unwrap()
    };

    RENDERER_PTR.store(renderer as *mut _ as usize, Ordering::SeqCst);
    let mut g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
    *g = Some(GlobalHandles {
        entry,
        instance,
        physical_device,
        present_queue,
        graphics_queue_family_index,
        present_queue_family_index,
        in_flight_frames: renderer.options.in_flight_frames.max(1),
        platform_create_vk_surface,
    });
}

pub(crate) fn clear_for_drop(renderer: *mut AshRenderer) {
    if RENDERER_PTR
        .compare_exchange(renderer as usize, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let mut g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
        *g = None;
    }
}

/// Disable multi-viewport callbacks and clear stored globals.
pub fn disable(imgui_context: &mut Context) {
    unsafe {
        let platform_io = imgui_context.platform_io_mut();
        platform_io.set_renderer_create_window(None);
        platform_io.set_renderer_destroy_window(None);
        platform_io.set_renderer_set_window_size(None);
        platform_io.set_platform_render_window_raw(None);
        platform_io.set_platform_swap_buffers_raw(None);
    }
    RENDERER_PTR.store(0, Ordering::SeqCst);
    let mut g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
    *g = None;
}

/// Convenience helper that disables callbacks and destroys all platform windows.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    disable(context);
    context.destroy_platform_windows();
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let ptr = RENDERER_PTR.load(Ordering::SeqCst) as *mut AshRenderer;
    if ptr.is_null() {
        return None;
    }
    if RENDERER_BORROWED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        eprintln!("[ash-mv] renderer already mutably borrowed; skipping callback");
        return None;
    }
    Some(RendererBorrowGuard { renderer: ptr })
}

fn global_handles() -> Option<GlobalHandles> {
    let g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
    g.as_ref().cloned()
}

struct RendererBorrowGuard {
    renderer: *mut AshRenderer,
}

impl Drop for RendererBorrowGuard {
    fn drop(&mut self) {
        RENDERER_BORROWED.store(false, Ordering::SeqCst);
    }
}

impl std::ops::Deref for RendererBorrowGuard {
    type Target = AshRenderer;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.renderer }
    }
}

impl std::ops::DerefMut for RendererBorrowGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.renderer }
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn viewport_user_data_mut<'a>(vpm: &'a mut Viewport) -> Option<&'a mut ViewportAshData> {
    let data = vpm.renderer_user_data();
    if data.is_null() {
        None
    } else {
        Some(&mut *(data as *mut ViewportAshData))
    }
}

fn pick_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
        return vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_SRGB,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        };
    }

    let preferred = [
        vk::Format::B8G8R8A8_SRGB,
        vk::Format::R8G8B8A8_SRGB,
        vk::Format::B8G8R8A8_UNORM,
        vk::Format::R8G8B8A8_UNORM,
    ];
    for p in preferred {
        if let Some(f) = formats.iter().find(|f| f.format == p) {
            return *f;
        }
    }
    *formats.first().unwrap_or(&vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_UNORM,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    })
}

fn pick_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    if modes.contains(&vk::PresentModeKHR::MAILBOX) {
        vk::PresentModeKHR::MAILBOX
    } else {
        vk::PresentModeKHR::FIFO
    }
}

fn extent_from_size_and_scale(size: [f32; 2], framebuffer_scale: [f32; 2]) -> vk::Extent2D {
    let sx = if framebuffer_scale[0].is_finite() && framebuffer_scale[0] > 0.0 {
        framebuffer_scale[0]
    } else {
        1.0
    };
    let sy = if framebuffer_scale[1].is_finite() && framebuffer_scale[1] > 0.0 {
        framebuffer_scale[1]
    } else {
        1.0
    };
    let w = (size[0] * sx).max(1.0).round() as u32;
    let h = (size[1] * sy).max(1.0).round() as u32;
    vk::Extent2D {
        width: w.max(1),
        height: h.max(1),
    }
}

fn extent_from_viewport(vpm: &Viewport) -> vk::Extent2D {
    extent_from_size_and_scale(vpm.size(), vpm.framebuffer_scale())
}

fn extent_from_imvec2(size: sys::ImVec2, framebuffer_scale: [f32; 2]) -> vk::Extent2D {
    extent_from_size_and_scale([size.x, size.y], framebuffer_scale)
}

fn create_command_pool(
    device: &Device,
    queue_family_index: u32,
) -> RendererResult<vk::CommandPool> {
    let info = vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family_index)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    unsafe { Ok(device.create_command_pool(&info, None)?) }
}

fn create_frame_sync(device: &Device, command_pool: vk::CommandPool) -> RendererResult<FrameSync> {
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
        fence,
        command_buffer,
        image_available,
        render_finished,
    })
}

fn recreate_swapchain(
    renderer: &mut AshRenderer,
    global: &GlobalHandles,
    surface_loader: &khr_surface::Instance,
    data: &mut ViewportAshData,
    desired_extent: vk::Extent2D,
) -> RendererResult<()> {
    unsafe {
        let _ = renderer.device.device_wait_idle();
    }

    #[cfg(not(feature = "dynamic-rendering"))]
    for fb in data.framebuffers.drain(..) {
        unsafe { renderer.device.destroy_framebuffer(fb, None) };
    }

    for view in data.image_views.drain(..) {
        unsafe { renderer.device.destroy_image_view(view, None) };
    }
    unsafe {
        data.swapchain_loader
            .destroy_swapchain(data.swapchain, None);
    }

    let caps = unsafe {
        surface_loader
            .get_physical_device_surface_capabilities(global.physical_device, data.surface)?
    };
    let formats = unsafe {
        surface_loader.get_physical_device_surface_formats(global.physical_device, data.surface)?
    };
    let present_modes = unsafe {
        surface_loader
            .get_physical_device_surface_present_modes(global.physical_device, data.surface)?
    };

    let surface_format = pick_surface_format(&formats);
    let present_mode = pick_present_mode(&present_modes);

    let mut extent = desired_extent;
    if caps.current_extent.width != u32::MAX && caps.current_extent.height != u32::MAX {
        extent = caps.current_extent;
    }

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

    let mut sci = vk::SwapchainCreateInfoKHR::default()
        .surface(data.surface)
        .min_image_count(min_image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .pre_transform(caps.current_transform)
        .composite_alpha(composite_alpha)
        .present_mode(present_mode)
        .clipped(true);

    let queue_family_indices = [
        global.graphics_queue_family_index,
        global.present_queue_family_index,
    ];
    if global.graphics_queue_family_index != global.present_queue_family_index {
        sci = sci
            .image_sharing_mode(vk::SharingMode::CONCURRENT)
            .queue_family_indices(&queue_family_indices);
    } else {
        sci = sci.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
    }

    let swapchain = unsafe { data.swapchain_loader.create_swapchain(&sci, None)? };
    let images = unsafe { data.swapchain_loader.get_swapchain_images(swapchain)? };

    // Ensure pipeline exists for this format (per-format support).
    let _ = renderer.viewport_pipeline(surface_format.format)?;

    let mut image_views = Vec::with_capacity(images.len());
    for &image in &images {
        let create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(surface_format.format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = unsafe { renderer.device.create_image_view(&create_info, None)? };
        image_views.push(view);
    }

    #[cfg(not(feature = "dynamic-rendering"))]
    {
        let rp = renderer
            .viewport_pipeline(surface_format.format)?
            .render_pass;
        let mut framebuffers = Vec::with_capacity(image_views.len());
        for &view in &image_views {
            let fb = unsafe {
                renderer.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(rp)
                        .attachments(std::slice::from_ref(&view))
                        .width(extent.width)
                        .height(extent.height)
                        .layers(1),
                    None,
                )?
            };
            framebuffers.push(fb);
        }
        data.framebuffers = framebuffers;
    }

    data.swapchain = swapchain;
    data.images = images;
    data.image_views = image_views;
    #[cfg(feature = "dynamic-rendering")]
    {
        data.image_layouts = vec![vk::ImageLayout::UNDEFINED; data.images.len()];
    }
    data.format = surface_format.format;
    data.extent = extent;
    data.images_in_flight = vec![vk::Fence::null(); data.images.len()];
    data.pending_present = None;
    Ok(())
}

/// Renderer: create per-viewport Vulkan resources (surface + swapchain).
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_create_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };

        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);
        let vpm = &mut *vp;

        let mut out_surface: sys::ImU64 = 0;
        let err = unsafe {
            (global.platform_create_vk_surface)(
                vpm.as_raw_mut(),
                global.instance.handle().as_raw(),
                std::ptr::null(),
                &mut out_surface,
            )
        };
        if err != 0 || out_surface == 0 {
            eprintln!(
                "[ash-mv-sdl3] Platform_CreateVkSurface failed (err={err}, surface={out_surface})"
            );
            return;
        }
        let surface = vk::SurfaceKHR::from_raw(out_surface as u64);

        let swapchain_loader = khr_swapchain::Device::new(&global.instance, &renderer.device);

        let present_supported = match unsafe {
            surface_loader.get_physical_device_surface_support(
                global.physical_device,
                global.present_queue_family_index,
                surface,
            )
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_surface_support error: {e:?}");
                unsafe { surface_loader.destroy_surface(surface, None) };
                return;
            }
        };
        if !present_supported {
            eprintln!("[ash-mv-sdl3] surface has no present support for the selected queue family");
            unsafe { surface_loader.destroy_surface(surface, None) };
            return;
        }

        let caps = match unsafe {
            surface_loader.get_physical_device_surface_capabilities(global.physical_device, surface)
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_surface_capabilities error: {e:?}");
                unsafe { surface_loader.destroy_surface(surface, None) };
                return;
            }
        };
        let formats = match unsafe {
            surface_loader.get_physical_device_surface_formats(global.physical_device, surface)
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_surface_formats error: {e:?}");
                unsafe { surface_loader.destroy_surface(surface, None) };
                return;
            }
        };
        let present_modes = match unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(global.physical_device, surface)
        } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv-sdl3] get_present_modes error: {e:?}");
                unsafe { surface_loader.destroy_surface(surface, None) };
                return;
            }
        };

        let surface_format = pick_surface_format(&formats);
        let present_mode = pick_present_mode(&present_modes);

        let mut extent = extent_from_viewport(vpm);
        if caps.current_extent.width != u32::MAX && caps.current_extent.height != u32::MAX {
            extent = caps.current_extent;
        }

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

        let mut sci = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(min_image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .pre_transform(caps.current_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .clipped(true);

        let queue_family_indices = [
            global.graphics_queue_family_index,
            global.present_queue_family_index,
        ];
        if global.graphics_queue_family_index != global.present_queue_family_index {
            sci = sci
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&queue_family_indices);
        } else {
            sci = sci.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
        }

        let swapchain = match unsafe { swapchain_loader.create_swapchain(&sci, None) } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ash-mv] create_swapchain error: {e:?}");
                unsafe { surface_loader.destroy_surface(surface, None) };
                return;
            }
        };
        let images = match unsafe { swapchain_loader.get_swapchain_images(swapchain) } {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ash-mv] get_swapchain_images error: {e:?}");
                unsafe {
                    swapchain_loader.destroy_swapchain(swapchain, None);
                    surface_loader.destroy_surface(surface, None);
                }
                return;
            }
        };

        let _ = renderer.viewport_pipeline(surface_format.format);

        let mut image_views = Vec::with_capacity(images.len());
        for &image in &images {
            let create_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface_format.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            let view = match unsafe { renderer.device.create_image_view(&create_info, None) } {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[ash-mv] create_image_view error: {e:?}");
                    unsafe {
                        for v in image_views.drain(..) {
                            renderer.device.destroy_image_view(v, None);
                        }
                        swapchain_loader.destroy_swapchain(swapchain, None);
                        surface_loader.destroy_surface(surface, None);
                    }
                    return;
                }
            };
            image_views.push(view);
        }

        #[cfg(not(feature = "dynamic-rendering"))]
        let framebuffers: Vec<vk::Framebuffer> = {
            let rp = match renderer.viewport_pipeline(surface_format.format) {
                Ok(vp) => vp.render_pass,
                Err(e) => {
                    eprintln!("[ash-mv] create viewport pipeline error: {e:?}");
                    unsafe {
                        for v in image_views.drain(..) {
                            renderer.device.destroy_image_view(v, None);
                        }
                        swapchain_loader.destroy_swapchain(swapchain, None);
                        surface_loader.destroy_surface(surface, None);
                    }
                    return;
                }
            };
            let mut framebuffers = Vec::with_capacity(image_views.len());
            for &view in &image_views {
                let fb = unsafe {
                    renderer.device.create_framebuffer(
                        &vk::FramebufferCreateInfo::default()
                            .render_pass(rp)
                            .attachments(std::slice::from_ref(&view))
                            .width(extent.width)
                            .height(extent.height)
                            .layers(1),
                        None,
                    )
                };
                match fb {
                    Ok(fb) => framebuffers.push(fb),
                    Err(e) => {
                        eprintln!("[ash-mv] create_framebuffer error: {e:?}");
                        unsafe {
                            for fb in framebuffers.drain(..) {
                                renderer.device.destroy_framebuffer(fb, None);
                            }
                            for v in image_views.drain(..) {
                                renderer.device.destroy_image_view(v, None);
                            }
                            swapchain_loader.destroy_swapchain(swapchain, None);
                            surface_loader.destroy_surface(surface, None);
                        }
                        return;
                    }
                }
            }
            framebuffers
        };

        let command_pool =
            match create_command_pool(&renderer.device, global.graphics_queue_family_index) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[ash-mv] create_command_pool error: {e:?}");
                    unsafe {
                        #[cfg(not(feature = "dynamic-rendering"))]
                        for fb in framebuffers.iter().copied() {
                            renderer.device.destroy_framebuffer(fb, None);
                        }
                        for v in image_views.drain(..) {
                            renderer.device.destroy_image_view(v, None);
                        }
                        swapchain_loader.destroy_swapchain(swapchain, None);
                        surface_loader.destroy_surface(surface, None);
                    }
                    return;
                }
            };
        let frames = (0..global.in_flight_frames)
            .map(|_| create_frame_sync(&renderer.device, command_pool))
            .collect::<RendererResult<Vec<_>>>();
        let frames = match frames {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[ash-mv] create frame sync error: {e:?}");
                unsafe {
                    #[cfg(not(feature = "dynamic-rendering"))]
                    for fb in framebuffers.iter().copied() {
                        renderer.device.destroy_framebuffer(fb, None);
                    }
                    for v in image_views.drain(..) {
                        renderer.device.destroy_image_view(v, None);
                    }
                    renderer.device.destroy_command_pool(command_pool, None);
                    swapchain_loader.destroy_swapchain(swapchain, None);
                    surface_loader.destroy_surface(surface, None);
                }
                return;
            }
        };

        let image_count = images.len();
        let data = ViewportAshData {
            surface,
            swapchain_loader,
            swapchain,
            format: surface_format.format,
            extent,
            images_in_flight: vec![vk::Fence::null(); image_count],
            images,
            image_views,
            #[cfg(feature = "dynamic-rendering")]
            image_layouts: vec![vk::ImageLayout::UNDEFINED; image_count],
            #[cfg(not(feature = "dynamic-rendering"))]
            framebuffers,
            command_pool,
            frames,
            frame_index: 0,
            pending_present: None,
            mesh_frames: Frames::new(global.in_flight_frames),
        };

        let boxed = Box::new(data);
        vpm.set_renderer_user_data(Box::into_raw(boxed) as *mut c_void);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Renderer_CreateWindow");
        std::process::abort();
    }
}

/// Renderer: destroy per-viewport Vulkan resources.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_destroy_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;
        let data_ptr = vpm.renderer_user_data();
        if data_ptr.is_null() {
            return;
        }
        vpm.set_renderer_user_data(std::ptr::null_mut());
        let boxed: Box<ViewportAshData> = Box::from_raw(data_ptr as *mut ViewportAshData);
        let _ = boxed.destroy(&mut renderer, &surface_loader);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Renderer_DestroyWindow");
        std::process::abort();
    }
}

/// Renderer: resize/recreate per-viewport swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_set_window_size(
    vp: *mut Viewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;

        // Convert to physical pixels using framebuffer scale (fallback to 1.0).
        let scale = vpm.framebuffer_scale();
        let extent = extent_from_imvec2(size, scale);

        let Some(data) = viewport_user_data_mut(vpm) else {
            return;
        };

        if data.extent != extent {
            let _ = recreate_swapchain(&mut renderer, &global, &surface_loader, data, extent);
        }
    }));
    if res.is_err() {
        eprintln!("[ash-mv-sdl3] panic in Renderer_SetWindowSize");
        std::process::abort();
    }
}

#[cfg(feature = "dynamic-rendering")]
fn transition_swapchain_image(
    device: &Device,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    old: vk::ImageLayout,
    new: vk::ImageLayout,
) {
    let barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old)
        .new_layout(new)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        })
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
    unsafe {
        device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
    }
}

/// Renderer: render viewport draw data into its swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_render_window(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;
        let desired_extent = extent_from_viewport(vpm);

        let raw_dd = vpm.draw_data();
        if raw_dd.is_null() {
            return;
        }
        let draw_data: &dear_imgui_rs::render::DrawData =
            dear_imgui_rs::render::DrawData::from_raw(&*raw_dd);

        let Some(data) = viewport_user_data_mut(vpm) else {
            return;
        };

        let frame_i = data.frame_index % data.frames.len();
        data.frame_index = (data.frame_index + 1) % data.frames.len();
        let frame = &data.frames[frame_i];

        if unsafe {
            renderer
                .device
                .wait_for_fences(&[frame.fence], true, u64::MAX)
        }
        .is_err()
        {
            return;
        }

        let acquire = unsafe {
            data.swapchain_loader.acquire_next_image(
                data.swapchain,
                u64::MAX,
                frame.image_available,
                vk::Fence::null(),
            )
        };

        let (image_index, _suboptimal) = match acquire {
            Ok(v) => v,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                let _ = recreate_swapchain(
                    &mut renderer,
                    &global,
                    &surface_loader,
                    data,
                    desired_extent,
                );
                return;
            }
            Err(e) => {
                eprintln!("[ash-mv] acquire_next_image error: {e:?}");
                return;
            }
        };

        if data.images_in_flight[image_index as usize] != vk::Fence::null() {
            if unsafe {
                renderer.device.wait_for_fences(
                    &[data.images_in_flight[image_index as usize]],
                    true,
                    u64::MAX,
                )
            }
            .is_err()
            {
                return;
            }
        }
        data.images_in_flight[image_index as usize] = frame.fence;

        if unsafe { renderer.device.reset_fences(&[frame.fence]) }.is_err() {
            return;
        }
        if unsafe {
            renderer
                .device
                .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())
        }
        .is_err()
        {
            return;
        }

        let mesh = match data.mesh_frames.next() {
            Some(m) => m,
            None => return,
        };

        let pipeline = match renderer.viewport_pipeline(data.format) {
            Ok(p) => p.pipeline,
            Err(e) => {
                eprintln!("[ash-mv] viewport_pipeline error: {e:?}");
                return;
            }
        };
        let gamma = renderer.gamma_for_format(data.format);

        if unsafe {
            renderer.device.begin_command_buffer(
                frame.command_buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
        }
        .is_err()
        {
            return;
        }

        #[cfg(not(feature = "dynamic-rendering"))]
        unsafe {
            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: renderer.viewport_clear_color(),
                },
            }];

            let rp = match renderer.viewport_pipeline(data.format) {
                Ok(p) => p.render_pass,
                Err(e) => {
                    eprintln!("[ash-mv] viewport_pipeline error: {e:?}");
                    return;
                }
            };
            renderer.device.cmd_begin_render_pass(
                frame.command_buffer,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(rp)
                    .framebuffer(data.framebuffers[image_index as usize])
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: data.extent,
                    })
                    .clear_values(&clear_values),
                vk::SubpassContents::INLINE,
            );

            if let Err(e) =
                renderer.cmd_draw_with_mesh(frame.command_buffer, draw_data, pipeline, gamma, mesh)
            {
                eprintln!("[ash-mv] cmd_draw error: {e:?}");
                renderer.device.cmd_end_render_pass(frame.command_buffer);
                return;
            }
            renderer.device.cmd_end_render_pass(frame.command_buffer);
        }

        #[cfg(feature = "dynamic-rendering")]
        unsafe {
            let img_i = image_index as usize;
            let old_layout = data
                .image_layouts
                .get(img_i)
                .copied()
                .unwrap_or(vk::ImageLayout::UNDEFINED);
            transition_swapchain_image(
                &renderer.device,
                frame.command_buffer,
                data.images[img_i],
                old_layout,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            if let Some(slot) = data.image_layouts.get_mut(img_i) {
                *slot = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
            }

            let clear = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: renderer.viewport_clear_color(),
                },
            };
            let color_attachment = vk::RenderingAttachmentInfo::default()
                .image_view(data.image_views[image_index as usize])
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear);

            renderer.device.cmd_begin_rendering(
                frame.command_buffer,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: data.extent,
                    })
                    .layer_count(1)
                    .color_attachments(std::slice::from_ref(&color_attachment)),
            );

            if let Err(e) =
                renderer.cmd_draw_with_mesh(frame.command_buffer, draw_data, pipeline, gamma, mesh)
            {
                eprintln!("[ash-mv] cmd_draw error: {e:?}");
                renderer.device.cmd_end_rendering(frame.command_buffer);
                return;
            }
            renderer.device.cmd_end_rendering(frame.command_buffer);

            transition_swapchain_image(
                &renderer.device,
                frame.command_buffer,
                data.images[img_i],
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );
            if let Some(slot) = data.image_layouts.get_mut(img_i) {
                *slot = vk::ImageLayout::PRESENT_SRC_KHR;
            }
        }

        if unsafe { renderer.device.end_command_buffer(frame.command_buffer) }.is_err() {
            return;
        }

        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&frame.image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&frame.command_buffer))
            .signal_semaphores(std::slice::from_ref(&frame.render_finished));

        if unsafe {
            renderer.device.queue_submit(
                renderer.queue,
                std::slice::from_ref(&submit_info),
                frame.fence,
            )
        }
        .is_err()
        {
            return;
        }

        data.pending_present = Some((frame_i, image_index));
    }));
    if res.is_err() {
        eprintln!("[ash-mv-sdl3] panic in Renderer_RenderWindow");
        std::process::abort();
    }
}

/// Renderer: present frame for viewport swapchain.
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        let Some(global) = global_handles() else {
            return;
        };
        let surface_loader = khr_surface::Instance::new(&global.entry, &global.instance);

        let vpm = &mut *vp;
        let desired_extent = extent_from_viewport(vpm);

        let Some(data) = viewport_user_data_mut(vpm) else {
            return;
        };
        let Some((frame_i, image_index)) = data.pending_present.take() else {
            return;
        };

        let frame = &data.frames[frame_i];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&frame.render_finished))
            .swapchains(std::slice::from_ref(&data.swapchain))
            .image_indices(std::slice::from_ref(&image_index));

        let present = unsafe {
            data.swapchain_loader
                .queue_present(global.present_queue, &present_info)
        };
        match present {
            Ok(_) => {}
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                let _ = recreate_swapchain(
                    &mut renderer,
                    &global,
                    &surface_loader,
                    data,
                    desired_extent,
                );
            }
            Err(e) => {
                eprintln!("[ash-mv] queue_present error: {e:?}");
            }
        }
    }));
    if res.is_err() {
        eprintln!("[ash-mv-sdl3] panic in Renderer_SwapBuffers");
        std::process::abort();
    }
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn platform_render_window_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_render_window(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Platform_RenderWindow");
        std::process::abort();
    }
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn platform_swap_buffers_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_swap_buffers(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[ash-mv] panic in Platform_SwapBuffers");
        std::process::abort();
    }
}
