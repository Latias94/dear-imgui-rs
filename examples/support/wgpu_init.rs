//! Minimal WGPU initialization helpers for examples.
//!
//! Intent: de-duplicate the repetitive graphics setup while keeping
//! the raw `wgpu` types visible to example code. These helpers do not
//! wrap rendering or ImGui — they only cover common WGPU boilerplate.
//!
//! Usage pattern (inside an example):
//! - call `init_wgpu_for_window(&window)` to get `(device, queue, surface, config)`
//! - on resize, call `reconfigure_surface(...)`
//! - per frame, acquire a frame with `surface.get_current_texture()` as usual
//!
//! Keeping this module small helps preserve explicit API usage in examples.

use winit::{dpi::PhysicalSize, window::Window};

/// Create a WGPU device/queue/surface configured for the given window.
/// Returns `(device, queue, surface, surface_config)`.
pub fn init_wgpu_for_window(
    window: &std::sync::Arc<Window>,
) -> Result<
    (
        wgpu::Device,
        wgpu::Queue,
        wgpu::Surface<'static>,
        wgpu::SurfaceConfiguration,
    ),
    Box<dyn std::error::Error>,
> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

    // SAFETY: winit guarantees the window outlives the surface; using Arc<Window> gives 'static.
    let surface = instance.create_surface(window.clone())?;

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .map_err(|_| "No suitable GPU adapter found")?;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("dear-imgui-examples (wgpu device)"),
        ..Default::default()
    }))?;

    let caps = surface.get_capabilities(&adapter);
    let format = preferred_srgb_format(&caps);

    let PhysicalSize { width, height } = window.inner_size();
    let mut config = default_surface_config(format, width.max(1), height.max(1));
    surface.configure(&device, &config);

    Ok((device, queue, surface, config))
}

/// Choose a commonly supported sRGB surface format.
pub fn preferred_srgb_format(caps: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    // Prefer BGRA/RGBA sRGB — match common swapchain formats across platforms.
    let preferred = [
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ];
    preferred
        .into_iter()
        .find(|f| caps.formats.contains(f))
        .unwrap_or(caps.formats[0])
}

/// Construct a reasonable default SurfaceConfiguration.
pub fn default_surface_config(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

/// Update and reconfigure the surface when the window size changes.
pub fn reconfigure_surface(
    surface: &wgpu::Surface<'static>,
    device: &wgpu::Device,
    config: &mut wgpu::SurfaceConfiguration,
    new_size: PhysicalSize<u32>,
) {
    if new_size.width == 0 || new_size.height == 0 {
        return;
    }
    config.width = new_size.width;
    config.height = new_size.height;
    surface.configure(device, config);
}
