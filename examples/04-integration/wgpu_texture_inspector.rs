//! Inspect real depth, integer, and HDR textures through explicit GPU conversion.
//!
//! `--readback-test` requires an adapter and checks known output bytes without a window.
//! `--smoke-frames N` runs a bounded native scenario, including a stable-handle resize.

#[path = "../support/texture_preview.rs"]
mod texture_preview;

use dear_app::{
    AppConfig, Application, ApplicationStage, ExternalTextureHandle, FrameContext, GpuApi,
    GpuContext, InitializedContext, RedrawMode, RenderContext, RunError, ShutdownContext, Theme,
    run,
};
use dear_imgui_rs::Condition;
use texture_preview::{LABELS, Selection, TexturePreview};

struct Inspector {
    preview: Option<TexturePreview>,
    handles: Option<[ExternalTextureHandle; 3]>,
    selection: Selection,
    side: u32,
    pending_side: Option<u32>,
    replacement_count: u64,
    content_frame: u64,
    ui_frames: u64,
    smoke_frames: Option<u64>,
    observations: Vec<String>,
    #[cfg(feature = "texture-inspector-multi-viewport")]
    texture_ids: Option<[dear_imgui_rs::TextureId; 3]>,
    #[cfg(feature = "texture-inspector-multi-viewport")]
    saw_shared_replaced_hdr_targets: bool,
}

impl Inspector {
    fn new(smoke_frames: Option<u64>) -> Self {
        Self {
            preview: None,
            handles: None,
            selection: Selection::default(),
            side: 128,
            pending_side: None,
            replacement_count: 0,
            content_frame: 0,
            ui_frames: 0,
            smoke_frames,
            observations: Vec::new(),
            #[cfg(feature = "texture-inspector-multi-viewport")]
            texture_ids: None,
            #[cfg(feature = "texture-inspector-multi-viewport")]
            saw_shared_replaced_hdr_targets: false,
        }
    }

    fn create(&mut self, gpu: &mut GpuApi<'_>, stage: ApplicationStage) -> Result<(), RunError> {
        let preview = TexturePreview::new(gpu.device(), self.side);
        let mut handles = Vec::with_capacity(3);
        for view in preview.views() {
            match gpu.register_external_texture(view) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    for handle in handles {
                        let _ = gpu.unregister_external_texture(handle);
                    }
                    return Err(RunError::application(stage, error));
                }
            }
        }
        self.handles = Some(handles.try_into().expect("three preview views"));
        self.preview = Some(preview);
        Ok(())
    }

    fn release(&mut self, gpu: &mut GpuApi<'_>, stage: ApplicationStage) -> Result<(), RunError> {
        if let Some(handles) = self.handles.take() {
            for handle in handles {
                gpu.unregister_external_texture(handle)
                    .map_err(|error| RunError::application(stage, error))?;
            }
        }
        self.preview = None;
        Ok(())
    }

    fn resize(&mut self, gpu: &mut GpuApi<'_>, side: u32) -> Result<(), RunError> {
        let Some(handles) = self.handles else {
            return Ok(());
        };
        let replacement = TexturePreview::new(gpu.device(), side);
        // Each replacement advances the renderer's binding revision under the same registration.
        for (handle, view) in handles.into_iter().zip(replacement.views()) {
            gpu.update_external_texture(handle, view)
                .map_err(|error| RunError::application(ApplicationStage::Frame, error))?;
        }
        self.preview = Some(replacement);
        self.side = side;
        self.replacement_count += 1;
        assert_eq!(self.handles, Some(handles));
        println!("texture inspector: resized to {side}, stable handles {handles:?}");
        Ok(())
    }
}

impl Application for Inspector {
    fn initialized(
        &mut self,
        _: &mut InitializedContext<'_>,
        gpu: &mut GpuContext<'_>,
    ) -> Result<(), RunError> {
        self.create(gpu.gpu(), ApplicationStage::Initialized)
    }

    fn gpu_lost(&mut self, gpu: &mut GpuContext<'_>) -> Result<(), RunError> {
        self.release(gpu.gpu(), ApplicationStage::GpuLost)
    }

    fn gpu_recreated(&mut self, gpu: &mut GpuContext<'_>) -> Result<(), RunError> {
        self.create(gpu.gpu(), ApplicationStage::GpuRecreated)
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        self.ui_frames += 1;
        if self.smoke_frames.is_some() {
            if self.ui_frames == 2 {
                self.pending_side = Some(256);
            }
            if self.ui_frames == 3 {
                self.selection = Selection {
                    layer: 1,
                    mip: 2,
                    exposure: 1.0,
                    ..Selection::default()
                };
            }
        }
        let ui = context.ui();
        let gpu = context.gpu();
        if let Some(side) = self.pending_side.take() {
            self.resize(gpu, side)?;
        }
        let Some(handles) = self.handles else {
            return Ok(());
        };
        let mut ids = Vec::with_capacity(3);
        for handle in handles {
            ids.push(
                gpu.resolve_external_texture(handle)
                    .map_err(|error| RunError::application(ApplicationStage::Frame, error))?,
            );
        }
        #[cfg(feature = "texture-inspector-multi-viewport")]
        {
            self.texture_ids = Some([ids[0], ids[1], ids[2]]);
        }
        let origin = ui.main_viewport().pos();
        let main_size = ui.main_viewport().size();
        let placement = if self.smoke_frames.is_some() {
            Condition::Always
        } else {
            Condition::FirstUseEver
        };
        ui.window("Texture inspector controls")
            .position([origin[0] + 16.0, origin[1] + 16.0], placement)
            .size([550.0, 330.0], Condition::FirstUseEver)
            .build(|| {
                let mut layer = self.selection.layer as i32;
                let mut mip = self.selection.mip as i32;
                ui.slider_i32("Array layer", &mut layer, 0, texture_preview::LAYERS as i32 - 1);
                ui.slider_i32("Mip level", &mut mip, 0, texture_preview::MIPS as i32 - 1);
                self.selection.layer = layer as u32;
                self.selection.mip = mip as u32;
                ui.slider("HDR exposure (stops)", -4.0, 4.0, &mut self.selection.exposure);
                ui.slider("Depth minimum", 0.0, 0.9, &mut self.selection.depth_min);
                ui.slider("Depth maximum", self.selection.depth_min + 0.01, 1.0, &mut self.selection.depth_max);
                self.selection.depth_max = self.selection.depth_max.max(self.selection.depth_min + 0.01);
                if ui.button("Resize sources + previews") {
                    self.pending_side = Some(if self.side == 128 { 256 } else { 128 });
                }
                ui.text(format!("Extent: {} x {}; replacements: {}", self.side, self.side, self.replacement_count));
                ui.text(format!("Host content frame: {} (independent of binding revisions)", self.content_frame));
                ui.text_wrapped("record_render orders depth/ID/HDR producers and conversion before every UI consumer on the same queue. The three RGBA8 views keep stable registrations across resize.");
                ui.text_wrapped("HDR uses exposure and Reinhard mapping into linear RGBA8. Object IDs use an explicit 3-bit RGB palette. Drag a preview outside the main window with the optional multi-viewport feature.");
            });
        for (index, (&id, label)) in ids.iter().zip(LABELS).enumerate() {
            ui.window(label)
                .position(
                    [origin[0] + 16.0 + index as f32 * 300.0, origin[1] + 365.0],
                    placement,
                )
                .size([290.0, 340.0], Condition::FirstUseEver)
                .build(|| {
                    ui.text(format!(
                        "Layer {}, mip {}",
                        self.selection.layer, self.selection.mip
                    ));
                    ui.image(id, [256.0, 256.0]);
                    ui.text(format!("Registration: {:?}", handles[index]));
                });
        }
        // This additional window draws the SAME HDR registration, including after detach/resize.
        ui.window("Shared HDR mirror")
            .position(
                [
                    origin[0]
                        + if cfg!(feature = "texture-inspector-multi-viewport") {
                            main_size[0] + 32.0
                        } else {
                            590.0
                        },
                    origin[1] + 32.0,
                ],
                placement,
            )
            .size([320.0, 310.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Same HDR registration as the HDR panel");
                ui.image(ids[2], [256.0, 230.0]);
            });
        ui.window("Observed native work")
            .position([origin[0] + 580.0, origin[1] + 16.0], placement)
            .size([440.0, 320.0], Condition::FirstUseEver)
            .build(|| {
                ui.text_wrapped("Stages describe recorded commands, queue submission, and presentation API requests. They do not prove GPU completion or visible pixels.");
                if self.observations.is_empty() {
                    ui.text("Enable texture-inspector-multi-viewport for native target reports.");
                }
                for observation in &self.observations { ui.text_wrapped(observation); }
            });
        if self
            .smoke_frames
            .is_some_and(|limit| self.ui_frames >= limit)
        {
            context.request_exit();
        }
        Ok(())
    }

    fn record_render(&mut self, context: &mut RenderContext<'_>) -> Result<(), RunError> {
        if let Some(preview) = &self.preview {
            let device = context.device().clone();
            preview.record(&device, context.encoder(), self.selection);
            self.content_frame = context.frame_index();
        }
        Ok(())
    }

    #[cfg(feature = "texture-inspector-multi-viewport")]
    fn viewport_frame(
        &mut self,
        context: &dear_app::ViewportFrameContext<'_>,
    ) -> Result<(), RunError> {
        self.observations.clear();
        let hdr_id = self.texture_ids.map(|ids| ids[2]);
        let mut hdr_targets = Vec::new();
        for observation in context
            .main()
            .into_iter()
            .chain(context.secondary().observations())
        {
            if let Some(binding) = observation
                .dependencies
                .external_bindings
                .iter()
                .find(|binding| Some(binding.texture.texture_id()) == hdr_id)
            {
                if observation.stage >= dear_imgui_wgpu::WgpuFrameStage::Submitted
                    && binding.revision.get() >= 2
                {
                    if !hdr_targets.contains(&observation.target.instance_id) {
                        hdr_targets.push(observation.target.instance_id);
                    }
                }
            }
            self.observations.push(format!(
                "instance {:?}; viewport {:?}; main {}; stage {:?}; bindings {:?}; outcome {:?}",
                observation.target.instance_id,
                observation.target.viewport_id,
                observation.target.is_main,
                observation.stage,
                observation.dependencies.external_bindings,
                observation.outcome,
            ));
        }
        self.saw_shared_replaced_hdr_targets |= hdr_targets.len() >= 2;
        Ok(())
    }

    fn shutdown(&mut self, _: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        if self.smoke_frames.is_some() {
            if self.replacement_count != 1 || self.content_frame == 0 {
                return Err(RunError::application(
                    ApplicationStage::Shutdown,
                    std::io::Error::other(
                        "native smoke did not record producers and replace the preview views",
                    ),
                ));
            }
            #[cfg(feature = "texture-inspector-multi-viewport")]
            if !self.saw_shared_replaced_hdr_targets {
                return Err(RunError::application(
                    ApplicationStage::Shutdown,
                    std::io::Error::other(
                        "native smoke did not observe shared HDR submission to distinct native targets with a replaced binding",
                    ),
                ));
            }
            println!(
                "texture inspector: bounded native run ended after {} UI frames; replacements {}; last host content frame {}",
                self.ui_frames, self.replacement_count, self.content_frame
            );
            for observation in &self.observations {
                println!("texture inspector: {observation}");
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let mut smoke_frames = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--readback-test" => return texture_preview::run_readback_tests(),
            "--smoke-frames" => {
                let count: u64 = arguments
                    .next()
                    .ok_or("--smoke-frames requires a frame count")?
                    .parse()?;
                if !(4..=600).contains(&count) {
                    return Err("smoke frame count must be 4..=600".into());
                }
                smoke_frames = Some(count);
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    dear_imgui_examples::init_tracing_with_filter(
        "dear_app=info,wgpu_texture_inspector=info,wgpu=warn",
    );
    run(
        AppConfig {
            window_title: "WGPU texture inspector".to_owned(),
            window_size: (1040.0, 780.0),
            multi_viewport: cfg!(feature = "texture-inspector-multi-viewport"),
            redraw: RedrawMode::Poll,
            theme: Some(Theme::Dark),
            ..Default::default()
        },
        Inspector::new(smoke_frames),
    )?;
    Ok(())
}
