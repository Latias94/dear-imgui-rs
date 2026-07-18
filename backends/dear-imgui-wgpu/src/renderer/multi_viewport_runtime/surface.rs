//! Per-viewport WGPU surface state and recovery policy.

use super::platform_adapter;
use super::registry::GlobalHandles;
use super::runtime::WgpuViewportError;
use dear_imgui_rs::ViewportFlags;
use dear_imgui_rs::platform_io::Viewport;

pub(super) struct ViewportWgpuData {
    pub(super) owner_context: usize,
    pub(super) device: wgpu::Device,
    #[cfg(feature = "wgpu-30")]
    pub(super) queue: wgpu::Queue,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) pending_frame: Option<wgpu::SurfaceTexture>,
    pub(super) pending_reconfigure: bool,
}

fn surface_config(
    globals: &GlobalHandles,
    surface: &wgpu::Surface<'static>,
    size: [u32; 2],
) -> Result<wgpu::SurfaceConfiguration, WgpuViewportError> {
    let capabilities = surface.get_capabilities(&globals.adapter);
    if !capabilities.formats.contains(&globals.render_target_format) {
        return Err(WgpuViewportError::SurfaceOperationFailed {
            operation: "negotiate renderer surface format",
        });
    }
    let present_mode = if capabilities
        .present_modes
        .contains(&wgpu::PresentMode::Fifo)
    {
        wgpu::PresentMode::Fifo
    } else {
        capabilities.present_modes.first().copied().ok_or(
            WgpuViewportError::SurfaceOperationFailed {
                operation: "negotiate surface present mode",
            },
        )?
    };
    let alpha_mode = if capabilities
        .alpha_modes
        .contains(&wgpu::CompositeAlphaMode::Opaque)
    {
        wgpu::CompositeAlphaMode::Opaque
    } else if capabilities
        .alpha_modes
        .contains(&wgpu::CompositeAlphaMode::Auto)
    {
        wgpu::CompositeAlphaMode::Auto
    } else {
        capabilities.alpha_modes.first().copied().ok_or(
            WgpuViewportError::SurfaceOperationFailed {
                operation: "negotiate surface alpha mode",
            },
        )?
    };
    Ok(wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: globals.render_target_format,
        #[cfg(feature = "wgpu-30")]
        color_space: wgpu::SurfaceColorSpace::Auto,
        width: size[0].max(1),
        height: size[1].max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![globals.render_target_format],
        desired_maximum_frame_latency: 1,
    })
}

pub(super) unsafe fn create_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: &Viewport,
    globals: &GlobalHandles,
) -> Result<ViewportWgpuData, WgpuViewportError> {
    let (surface, size) = unsafe { platform_adapter::create_surface(&globals.instance, viewport) }?;
    let config = surface_config(globals, &surface, size)?;
    surface.configure(&globals.device, &config);
    Ok(ViewportWgpuData {
        owner_context: context as usize,
        device: globals.device.clone(),
        #[cfg(feature = "wgpu-30")]
        queue: globals.queue.clone(),
        surface,
        config,
        pending_frame: None,
        pending_reconfigure: false,
    })
}

unsafe fn reconfigure_surface(
    viewport: &Viewport,
    data: &mut ViewportWgpuData,
) -> Result<(), WgpuViewportError> {
    let size = unsafe { platform_adapter::framebuffer_size(viewport) }?;
    data.config.width = size[0].max(1);
    data.config.height = size[1].max(1);
    data.surface.configure(&data.device, &data.config);
    Ok(())
}

unsafe fn recreate_surface(
    viewport: &Viewport,
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
) -> Result<(), WgpuViewportError> {
    let (surface, size) = unsafe { platform_adapter::create_surface(&globals.instance, viewport) }?;
    let config = surface_config(globals, &surface, size)?;
    surface.configure(&globals.device, &config);
    data.pending_frame = None;
    data.pending_reconfigure = false;
    data.device = globals.device.clone();
    #[cfg(feature = "wgpu-30")]
    {
        data.queue = globals.queue.clone();
    }
    data.surface = surface;
    data.config = config;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SurfaceEvent {
    Success,
    Suboptimal,
    Outdated,
    Lost,
    Timeout,
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
    Occluded,
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
    Validation,
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
    OutOfMemory,
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SurfaceAction {
    Render,
    RenderThenReconfigure,
    Reconfigure,
    Recreate,
    Skip,
    Reject,
}

pub(super) const fn surface_action(event: SurfaceEvent) -> SurfaceAction {
    match event {
        SurfaceEvent::Success => SurfaceAction::Render,
        SurfaceEvent::Suboptimal => SurfaceAction::RenderThenReconfigure,
        SurfaceEvent::Outdated => SurfaceAction::Reconfigure,
        SurfaceEvent::Lost => SurfaceAction::Recreate,
        SurfaceEvent::Timeout => SurfaceAction::Skip,
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
        SurfaceEvent::Occluded => SurfaceAction::Skip,
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
        SurfaceEvent::Validation => SurfaceAction::Reject,
        #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
        SurfaceEvent::OutOfMemory | SurfaceEvent::Other => SurfaceAction::Reject,
    }
}

impl SurfaceEvent {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Suboptimal => "suboptimal",
            Self::Outdated => "outdated",
            Self::Lost => "lost",
            Self::Timeout => "timeout",
            #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
            Self::Occluded => "occluded",
            #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
            Self::Validation => "validation",
            #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
            Self::OutOfMemory => "out of memory",
            #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
            Self::Other => "other",
        }
    }
}

pub(super) unsafe fn handle_non_renderable_surface_event(
    event: SurfaceEvent,
    viewport: &Viewport,
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
) -> Result<(), WgpuViewportError> {
    match surface_action(event) {
        SurfaceAction::Reconfigure => unsafe { reconfigure_surface(viewport, data) },
        SurfaceAction::Recreate => unsafe { recreate_surface(viewport, data, globals) },
        SurfaceAction::Skip => Ok(()),
        SurfaceAction::Reject => Err(WgpuViewportError::SurfaceRejected {
            event: event.name(),
        }),
        SurfaceAction::Render | SurfaceAction::RenderThenReconfigure => {
            debug_assert!(false, "renderable surface event reached recovery path");
            Ok(())
        }
    }
}

pub(super) fn request_close_after_surface_creation_failure(viewport: &mut Viewport) {
    viewport.set_platform_request_close(true);
}

pub(super) fn should_clear_viewport(flags: ViewportFlags) -> bool {
    !flags.contains(ViewportFlags::NO_RENDERER_CLEAR)
}
