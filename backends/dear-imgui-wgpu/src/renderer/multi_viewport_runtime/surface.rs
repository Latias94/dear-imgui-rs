//! Per-viewport WGPU surface state and recovery policy.

use super::platform_adapter;
use super::registry::GlobalHandles;
use super::runtime::WgpuViewportError;
use crate::WgpuViewportSurfaceConfig;
use dear_imgui_rs::ViewportFlags;
use dear_imgui_rs::platform_io::Viewport;

pub(super) struct ViewportWgpuData {
    #[cfg(feature = "wgpu-30")]
    pub(super) queue: wgpu::Queue,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) pending_frame: Option<wgpu::SurfaceTexture>,
    pub(super) pending_reconfigure: bool,
}

pub(super) fn resolve_present_mode(
    requested: wgpu::PresentMode,
    supported: &[wgpu::PresentMode],
) -> wgpu::PresentMode {
    match requested {
        wgpu::PresentMode::AutoVsync | wgpu::PresentMode::AutoNoVsync => requested,
        mode if supported.contains(&mode) => mode,
        wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed => wgpu::PresentMode::AutoVsync,
        wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox => wgpu::PresentMode::AutoNoVsync,
    }
}

pub(super) fn resolve_alpha_mode(
    requested: wgpu::CompositeAlphaMode,
    supported: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    if requested == wgpu::CompositeAlphaMode::Auto || supported.contains(&requested) {
        requested
    } else {
        wgpu::CompositeAlphaMode::Auto
    }
}

#[cfg(feature = "wgpu-30")]
pub(super) fn supports_surface_format(
    capabilities: &wgpu::SurfaceCapabilities,
    format: wgpu::TextureFormat,
) -> bool {
    capabilities
        .color_spaces(format)
        .contains(wgpu::SurfaceColorSpaces::SRGB)
}

#[cfg(not(feature = "wgpu-30"))]
pub(super) fn supports_surface_format(
    capabilities: &wgpu::SurfaceCapabilities,
    format: wgpu::TextureFormat,
) -> bool {
    capabilities.formats.contains(&format)
}

pub(super) fn surface_config_from_capabilities(
    render_target_format: wgpu::TextureFormat,
    viewport_surface_config: WgpuViewportSurfaceConfig,
    capabilities: &wgpu::SurfaceCapabilities,
    size: [u32; 2],
) -> Result<wgpu::SurfaceConfiguration, WgpuViewportError> {
    let format_supported = supports_surface_format(capabilities, render_target_format);
    if !format_supported {
        return Err(WgpuViewportError::UnsupportedSurfaceFormat {
            format: render_target_format,
            #[cfg(feature = "wgpu-30")]
            color_space: "the required sRGB color space",
            #[cfg(not(feature = "wgpu-30"))]
            color_space: "the surface's automatic color space",
        });
    }
    let present_mode = resolve_present_mode(
        viewport_surface_config.present_mode,
        &capabilities.present_modes,
    );
    let alpha_mode = resolve_alpha_mode(
        viewport_surface_config.alpha_mode,
        &capabilities.alpha_modes,
    );
    Ok(wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: render_target_format,
        #[cfg(feature = "wgpu-30")]
        color_space: wgpu::SurfaceColorSpace::Srgb,
        width: size[0].max(1),
        height: size[1].max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: viewport_surface_config.desired_maximum_frame_latency,
    })
}

fn surface_config(
    globals: &GlobalHandles,
    surface: &wgpu::Surface<'static>,
    size: [u32; 2],
) -> Result<wgpu::SurfaceConfiguration, WgpuViewportError> {
    let capabilities = surface.get_capabilities(&globals.adapter);
    surface_config_from_capabilities(
        globals.render_target_format,
        globals.viewport_surface_config,
        &capabilities,
        size,
    )
}

fn configure_surface(
    globals: &GlobalHandles,
    surface: &wgpu::Surface<'static>,
    size: [u32; 2],
) -> Result<wgpu::SurfaceConfiguration, WgpuViewportError> {
    let config = surface_config(globals, surface, size)?;
    surface.configure(&globals.device, &config);
    Ok(config)
}

pub(super) unsafe fn create_viewport_data(
    viewport: &Viewport,
    globals: &GlobalHandles,
) -> Result<ViewportWgpuData, WgpuViewportError> {
    let (surface, size) = unsafe { platform_adapter::create_surface(&globals.instance, viewport) }?;
    let config = configure_surface(globals, &surface, size)?;
    Ok(ViewportWgpuData {
        #[cfg(feature = "wgpu-30")]
        queue: globals.queue.clone(),
        surface,
        config,
        pending_frame: None,
        pending_reconfigure: false,
    })
}

pub(super) fn reconfigure_surface(
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
    size: [u32; 2],
) -> Result<(), WgpuViewportError> {
    let config = configure_surface(globals, &data.surface, size)?;
    data.config = config;
    Ok(())
}

unsafe fn recreate_surface(
    viewport: &Viewport,
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
) -> Result<(), WgpuViewportError> {
    let (surface, size) = unsafe { platform_adapter::create_surface(&globals.instance, viewport) }?;
    let config = configure_surface(globals, &surface, size)?;
    data.pending_frame = None;
    data.pending_reconfigure = false;
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
        SurfaceAction::Reconfigure => {
            let size = unsafe { platform_adapter::framebuffer_size(viewport) }?;
            reconfigure_surface(data, globals, size)
        }
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
