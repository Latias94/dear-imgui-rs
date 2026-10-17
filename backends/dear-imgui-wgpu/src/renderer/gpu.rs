//! GPU resources shared by live and detached execution; no Context or UI capability.

use crate::{
    GammaMode, RendererError, RendererResult, ShaderManager, WgpuBackendData, WgpuInitInfo,
    WgpuTextureManager, wgpu,
};
use wgpu::*;

pub(super) struct GpuState {
    /// Backend data
    pub(super) backend_data: Option<WgpuBackendData>,
    /// Shader manager
    pub(super) shader_manager: ShaderManager,
    /// Texture manager
    pub(super) texture_manager: WgpuTextureManager,
    /// Default texture for fallback
    pub(super) default_texture: Option<TextureView>,
    /// Gamma mode: automatic (by format), force linear (1.0), or force 2.2
    pub(super) gamma_mode: GammaMode,
}

impl GpuState {
    pub(super) fn empty() -> Self {
        Self {
            backend_data: None,
            shader_manager: ShaderManager::new(),
            texture_manager: WgpuTextureManager::new(),
            default_texture: None,
            gamma_mode: GammaMode::Auto,
        }
    }

    pub(super) fn initialize(&mut self, init_info: WgpuInitInfo) -> RendererResult<()> {
        // Create backend data
        let mut backend_data = WgpuBackendData::new(init_info);

        // Preflight: ensure the render target format is render-attachable and blendable.
        // The ImGui pipeline always uses alpha blending; non-blendable formats will
        // fail validation later with less actionable errors.
        let fmt = backend_data.render_target_format;
        if let Some(adapter) = backend_data.init_info.adapter.as_ref() {
            let fmt_features = adapter.get_texture_format_features(fmt);
            if !fmt_features
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                || !fmt_features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
            {
                return Err(RendererError::InvalidRenderState(format!(
                    "Render target format {:?} is not suitable for ImGui WGPU renderer (requires RENDER_ATTACHMENT + BLENDABLE). allowed_usages={:?} flags={:?}",
                    fmt, fmt_features.allowed_usages, fmt_features.flags
                )));
            }
        }

        if let Err(error) = self.create_device_objects(&mut backend_data) {
            self.shader_manager = ShaderManager::new();
            self.default_texture = None;
            return Err(error);
        }

        self.backend_data = Some(backend_data);
        Ok(())
    }

    /// Recreate every resource discarded by `invalidate_device_objects()`.
    pub(super) fn create_device_objects(
        &mut self,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        backend_data
            .render_resources
            .initialize(&backend_data.device)?;
        self.shader_manager.initialize(&backend_data.device)?;
        self.default_texture =
            Some(self.create_default_texture(&backend_data.device, &backend_data.queue)?);
        self.create_render_pipeline(backend_data)
    }

    /// Create a default 1x1 white texture
    fn create_default_texture(
        &self,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<TextureView> {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Dear ImGui Default Texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload white pixel
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255u8, 255u8, 255u8], // RGBA white
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        Ok(texture.create_view(&TextureViewDescriptor::default()))
    }
}
