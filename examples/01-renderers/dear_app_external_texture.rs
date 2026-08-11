//! Generation-aware external WGPU texture registration with `dear-app`.
//!
//! This example keeps the texture application-owned. `dear-app` only registers its `TextureView`
//! with the current renderer generation, resolves the opaque handle for each frame, and rebuilds
//! the GPU resource after device recovery.

use dear_app::{
    AppConfig, Application, ApplicationStage, ExternalTextureHandle, FrameContext, GpuApi,
    GpuContext, InitializedContext, RedrawMode, RunError, Theme, WgpuConfig, WgpuPreset, run,
};
use dear_imgui_rs::Condition;
use wgpu as wgpu_rs;

const TEXTURE_SIDE: u32 = 128;

#[derive(Clone, Copy)]
enum TextureCommand {
    Register,
    ReplaceView,
    Unregister,
}

struct RegisteredTexture {
    // Keep the application-owned resource and view alive while the renderer uses its cloned view.
    _texture: wgpu_rs::Texture,
    _view: wgpu_rs::TextureView,
    handle: ExternalTextureHandle,
}

#[derive(Default)]
struct ExternalTextureApp {
    texture: Option<RegisteredTexture>,
    pending_command: Option<TextureCommand>,
    revision: u32,
    gpu_recreations: u64,
}

impl ExternalTextureApp {
    fn pixels(revision: u32) -> Vec<u8> {
        let palettes = [
            ([34, 211, 238], [15, 23, 42]),
            ([250, 204, 21], [88, 28, 135]),
            ([74, 222, 128], [30, 41, 59]),
        ];
        let (light, dark) = palettes[revision as usize % palettes.len()];
        let mut pixels = Vec::with_capacity((TEXTURE_SIDE * TEXTURE_SIDE * 4) as usize);

        for y in 0..TEXTURE_SIDE {
            for x in 0..TEXTURE_SIDE {
                let checker = ((x / 16) + (y / 16)) % 2 == 0;
                let color = if checker { light } else { dark };
                pixels.extend_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }

        pixels
    }

    fn create_resource(
        gpu: &GpuApi<'_>,
        revision: u32,
    ) -> (wgpu_rs::Texture, wgpu_rs::TextureView) {
        let extent = wgpu_rs::Extent3d {
            width: TEXTURE_SIDE,
            height: TEXTURE_SIDE,
            depth_or_array_layers: 1,
        };
        let texture = gpu.device().create_texture(&wgpu_rs::TextureDescriptor {
            label: Some("dear-app external texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu_rs::TextureDimension::D2,
            format: wgpu_rs::TextureFormat::Rgba8Unorm,
            usage: wgpu_rs::TextureUsages::TEXTURE_BINDING | wgpu_rs::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue().write_texture(
            wgpu_rs::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu_rs::Origin3d::ZERO,
                aspect: wgpu_rs::TextureAspect::All,
            },
            &Self::pixels(revision),
            wgpu_rs::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(TEXTURE_SIDE * 4),
                rows_per_image: Some(TEXTURE_SIDE),
            },
            extent,
        );
        let view = texture.create_view(&wgpu_rs::TextureViewDescriptor::default());
        (texture, view)
    }

    fn register(&mut self, gpu: &mut GpuApi<'_>, stage: ApplicationStage) -> Result<(), RunError> {
        if self.texture.is_some() {
            return Ok(());
        }

        let revision = self.revision.wrapping_add(1);
        let (texture, view) = Self::create_resource(gpu, revision);
        let handle = gpu
            .register_external_texture(&view)
            .map_err(|error| RunError::application(stage, error))?;
        self.revision = revision;
        self.texture = Some(RegisteredTexture {
            _texture: texture,
            _view: view,
            handle,
        });
        Ok(())
    }

    fn replace_view(
        &mut self,
        gpu: &mut GpuApi<'_>,
        stage: ApplicationStage,
    ) -> Result<(), RunError> {
        let Some(current) = self.texture.as_mut() else {
            return Ok(());
        };

        let revision = self.revision.wrapping_add(1);
        let (texture, view) = Self::create_resource(gpu, revision);
        gpu.update_external_texture(current.handle, &view)
            .map_err(|error| RunError::application(stage, error))?;
        current._texture = texture;
        current._view = view;
        self.revision = revision;
        Ok(())
    }

    fn unregister(
        &mut self,
        gpu: &mut GpuApi<'_>,
        stage: ApplicationStage,
    ) -> Result<(), RunError> {
        let Some(texture) = self.texture.take() else {
            return Ok(());
        };

        if let Err(error) = gpu.unregister_external_texture(texture.handle) {
            self.texture = Some(texture);
            return Err(RunError::application(stage, error));
        }
        Ok(())
    }

    fn apply_pending_command(&mut self, gpu: &mut GpuApi<'_>) -> Result<(), RunError> {
        match self.pending_command.take() {
            Some(TextureCommand::Register) => self.register(gpu, ApplicationStage::Frame),
            Some(TextureCommand::ReplaceView) => self.replace_view(gpu, ApplicationStage::Frame),
            Some(TextureCommand::Unregister) => self.unregister(gpu, ApplicationStage::Frame),
            None => Ok(()),
        }
    }
}

impl Application for ExternalTextureApp {
    fn initialized(
        &mut self,
        _context: &mut InitializedContext<'_>,
        context: &mut GpuContext<'_>,
    ) -> Result<(), RunError> {
        self.register(context.gpu(), ApplicationStage::Initialized)
    }

    fn gpu_lost(&mut self, context: &mut GpuContext<'_>) -> Result<(), RunError> {
        self.unregister(context.gpu(), ApplicationStage::GpuLost)
    }

    fn gpu_recreated(&mut self, context: &mut GpuContext<'_>) -> Result<(), RunError> {
        self.gpu_recreations += 1;
        self.register(context.gpu(), ApplicationStage::GpuRecreated)
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let ui = context.ui();
        let gpu = context.gpu();
        self.apply_pending_command(gpu)?;

        let generation = gpu.generation().get();
        let texture_id = self
            .texture
            .as_ref()
            .map(|texture| gpu.resolve_external_texture(texture.handle))
            .transpose()
            .map_err(|error| RunError::application(ApplicationStage::Frame, error))?;
        let handle_generation = self
            .texture
            .as_ref()
            .map(|texture| texture.handle.generation().get());

        ui.window("External WGPU Texture")
            .size([520.0, 510.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Application-owned TextureView registered with dear-app");
                ui.separator();
                ui.text(format!("Current GPU generation: {generation}"));
                ui.text(format!(
                    "GPU recreations observed: {}",
                    self.gpu_recreations
                ));
                match handle_generation {
                    Some(handle_generation) => {
                        ui.text(format!("Handle generation: {handle_generation}"));
                        ui.text(format!("Texture revision: {}", self.revision));
                    }
                    None => ui.text("No external texture is registered"),
                }

                ui.separator();
                if let Some(texture_id) = texture_id {
                    ui.image(texture_id, [256.0, 256.0]);
                    if ui.button("Replace registered TextureView") {
                        self.pending_command = Some(TextureCommand::ReplaceView);
                    }
                    ui.same_line();
                    if ui.button("Unregister") {
                        self.pending_command = Some(TextureCommand::Unregister);
                    }
                } else if ui.button("Register external TextureView") {
                    self.pending_command = Some(TextureCommand::Register);
                }

                ui.separator();
                ui.text_wrapped(
                    "The opaque handle is resolved through the current frame's GpuApi. After a \
                     device recovery, gpu_recreated builds a new texture and registers a handle \
                     for the replacement generation.",
                );
                ui.text_wrapped(
                    "Button commands run at the start of the next frame, so unregistering never \
                     invalidates a texture referenced by the current frame's draw data.",
                );
            });
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    dear_imgui_examples::init_tracing_with_filter(
        "dear_imgui=info,dear_app_external_texture=info,wgpu=warn",
    );

    let config = AppConfig {
        window_title: "Dear App - External WGPU Texture".to_owned(),
        window_size: (840.0, 620.0),
        wgpu: WgpuConfig::from_preset(WgpuPreset::HighPerformance),
        redraw: RedrawMode::Poll,
        theme: Some(Theme::Dark),
        ..Default::default()
    };
    run(config, ExternalTextureApp::default())
}
