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
    pub(super) surface: Option<wgpu::Surface<'static>>,
    pub(super) targets: Option<ViewportRenderTargets>,
    pub(super) size: [u32; 2],
    pub(super) pending_frame: Option<wgpu::SurfaceTexture>,
    pub(super) pending_reconfigure: bool,
}

pub(super) struct ViewportRenderTargets {
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) multisampled_color: Option<OwnedAttachment>,
    pub(super) depth_stencil: Option<OwnedAttachment>,
}

pub(super) struct OwnedAttachment {
    _texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
}

pub(super) fn release_surface_bundle_parts<Frame, Targets, Surface>(
    pending_frame: &mut Option<Frame>,
    targets: &mut Option<Targets>,
    surface: &mut Option<Surface>,
) {
    drop(pending_frame.take());
    drop(targets.take());
    drop(surface.take());
}

impl ViewportWgpuData {
    pub(super) fn release_surface_bundle(&mut self) {
        release_surface_bundle_parts(
            &mut self.pending_frame,
            &mut self.targets,
            &mut self.surface,
        );
        self.size = [0, 0];
        self.pending_reconfigure = false;
    }
}

impl Drop for ViewportWgpuData {
    fn drop(&mut self) {
        // SurfaceTexture discard needs the Surface registration to remain alive.
        self.release_surface_bundle();
    }
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
) -> Result<Option<wgpu::SurfaceConfiguration>, WgpuViewportError> {
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
    if size[0] == 0 || size[1] == 0 {
        return Ok(None);
    }
    Ok(Some(wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: render_target_format,
        #[cfg(feature = "wgpu-30")]
        color_space: wgpu::SurfaceColorSpace::Srgb,
        width: size[0],
        height: size[1],
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: viewport_surface_config.desired_maximum_frame_latency,
    }))
}

fn surface_config(
    globals: &GlobalHandles,
    surface: &wgpu::Surface<'static>,
    size: [u32; 2],
) -> Result<Option<wgpu::SurfaceConfiguration>, WgpuViewportError> {
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
) -> Result<Option<ViewportRenderTargets>, WgpuViewportError> {
    let Some(config) = surface_config(globals, surface, size)? else {
        return Ok(None);
    };
    surface.configure(&globals.device, &config);
    Ok(Some(create_render_targets(globals, config)))
}

fn create_render_targets(
    globals: &GlobalHandles,
    config: wgpu::SurfaceConfiguration,
) -> ViewportRenderTargets {
    let size = [config.width, config.height];
    let sample_count = globals.multisample_state.count;
    let multisampled_color = (sample_count > 1).then(|| {
        create_attachment(
            &globals.device,
            "dear-imgui-wgpu::viewport-msaa-color",
            size,
            sample_count,
            globals.render_target_format,
        )
    });
    let depth_stencil = globals.depth_stencil_format.map(|format| {
        create_attachment(
            &globals.device,
            "dear-imgui-wgpu::viewport-depth-stencil",
            size,
            sample_count,
            format,
        )
    });
    ViewportRenderTargets {
        config,
        multisampled_color,
        depth_stencil,
    }
}

fn create_attachment(
    device: &wgpu::Device,
    label: &'static str,
    size: [u32; 2],
    sample_count: u32,
    format: wgpu::TextureFormat,
) -> OwnedAttachment {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    OwnedAttachment {
        _texture: texture,
        view,
    }
}

pub(super) unsafe fn create_viewport_data(
    viewport: &Viewport,
    globals: &GlobalHandles,
) -> Result<ViewportWgpuData, WgpuViewportError> {
    let (surface, size) = unsafe { platform_adapter::create_surface(&globals.instance, viewport) }?;
    let targets = configure_surface(globals, &surface, size)?;
    Ok(ViewportWgpuData {
        #[cfg(feature = "wgpu-30")]
        queue: globals.queue.clone(),
        surface: Some(surface),
        targets,
        size,
        pending_frame: None,
        pending_reconfigure: false,
    })
}

pub(super) fn reconfigure_surface(
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
    size: [u32; 2],
) -> Result<(), WgpuViewportError> {
    drop(data.pending_frame.take());
    if size[0] == 0 || size[1] == 0 {
        drop(data.targets.take());
        data.size = size;
        data.pending_reconfigure = false;
        return Ok(());
    }
    let surface = data
        .surface
        .as_ref()
        .ok_or(WgpuViewportError::SurfaceOperationFailed {
            operation: "reconfigure a released viewport surface",
        })?;
    let targets = configure_surface(globals, surface, size)?;
    data.targets = targets;
    data.size = size;
    data.pending_reconfigure = false;
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
    Skip,
    Reject,
}

pub(super) const fn surface_action(event: SurfaceEvent) -> SurfaceAction {
    match event {
        SurfaceEvent::Success => SurfaceAction::Render,
        SurfaceEvent::Suboptimal => SurfaceAction::RenderThenReconfigure,
        SurfaceEvent::Outdated => SurfaceAction::Reconfigure,
        SurfaceEvent::Lost => SurfaceAction::Reject,
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
    viewport: &mut Viewport,
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
) -> Result<(), WgpuViewportError> {
    match surface_action(event) {
        SurfaceAction::Reconfigure => {
            let size = unsafe { platform_adapter::framebuffer_size(viewport) }?;
            reconfigure_surface(data, globals, size)
        }
        SurfaceAction::Skip => Ok(()),
        SurfaceAction::Reject => {
            if event == SurfaceEvent::Lost {
                data.release_surface_bundle();
                Err(WgpuViewportError::SurfaceLost)
            } else {
                Err(WgpuViewportError::SurfaceRejected {
                    event: event.name(),
                })
            }
        }
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
