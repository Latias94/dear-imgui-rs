//! Complete synchronous custom-renderer contract without a window or GPU.
//!
//! Run with:
//! `cargo run -j 1 -p dear-imgui-rs --example custom_renderer_headless`

use std::collections::{HashMap, HashSet};

use dear_imgui_rs::render::{
    DrawCmd, PendingFrame, RendererConsumerError, SnapshotTextureId, SynchronousRendererConsumer,
    TextureFeedbackError, TextureOp, TextureUploadIdentity,
};
use dear_imgui_rs::texture::{
    ManagedTextureId, OwnedTextureData, TextureFormat, TextureRegion, TextureSubresource,
    get_format_bytes_per_pixel,
};
use dear_imgui_rs::{BackendFlags, Condition, Context, FrameToken, TextureId};
use thiserror::Error;

#[derive(Debug, Error)]
enum CpuRendererError {
    #[error(transparent)]
    Consumer(#[from] RendererConsumerError),
    #[error(transparent)]
    Feedback(#[from] TextureFeedbackError),
    #[error("texture upload dimensions overflowed usize")]
    UploadSizeOverflow,
    #[error("texture upload row pitch is smaller than one tightly packed row")]
    InvalidRowPitch,
    #[error("a texture update did not match the renderer resource layout")]
    UpdateLayoutMismatch,
    #[error("the frame requires raw draw callbacks, which this CPU recorder does not support")]
    RawCallbacksUnsupported,
    #[error("draw command referenced unknown texture {0:?}")]
    UnknownTexture(TextureId),
}

struct CpuTexture {
    id: TextureId,
    format: TextureFormat,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    upload: TextureUploadIdentity,
}

#[derive(Default)]
struct DrawStats {
    draw_lists: usize,
    element_commands: usize,
    elements: usize,
    state_changes: usize,
}

struct CpuRenderer {
    consumer: Option<SynchronousRendererConsumer>,
    textures: HashMap<SnapshotTextureId, CpuTexture>,
    destroyed: HashSet<SnapshotTextureId>,
    next_texture_id: u64,
}

impl CpuRenderer {
    fn new(context: &mut Context) -> Result<Self, RendererConsumerError> {
        context
            .set_renderer_name(Some("headless-cpu-recorder"))
            .expect("the static renderer name contains no NUL bytes");
        let mut flags = context.io().backend_flags();
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        context.io_mut().set_backend_flags(flags);

        Ok(Self {
            consumer: Some(context.create_synchronous_renderer_consumer()?),
            textures: HashMap::new(),
            destroyed: HashSet::new(),
            next_texture_id: 1,
        })
    }

    fn consumer(&self) -> &SynchronousRendererConsumer {
        self.consumer.as_ref().expect("renderer is shut down")
    }

    fn render(&mut self, frame: PendingFrame<'_>) -> Result<DrawStats, CpuRendererError> {
        if frame.draw_requirements().requires_raw_callback_support() {
            return Err(CpuRendererError::RawCallbacksUnsupported);
        }

        let mut feedback = Vec::with_capacity(frame.texture_requests().len());
        for request in frame.texture_requests() {
            match request.operation() {
                TextureOp::Create {
                    format,
                    width,
                    height,
                    row_pitch,
                    pixels,
                } => {
                    if self.destroyed.contains(&request.texture()) {
                        feedback.push(request.superseded());
                        continue;
                    }
                    let upload = request
                        .upload_identity()
                        .expect("a create request has an upload identity");
                    if let Some(texture) = self.textures.get(&request.texture())
                        && texture.upload == upload
                    {
                        feedback.push(request.uploaded(texture.id)?);
                        continue;
                    }

                    let id = TextureId::new(self.next_texture_id);
                    self.next_texture_id = self
                        .next_texture_id
                        .checked_add(1)
                        .ok_or(CpuRendererError::UploadSizeOverflow)?;
                    let texture = CpuTexture {
                        id,
                        format: *format,
                        width: *width,
                        height: *height,
                        pixels: copy_tight_rows(*format, *width, *height, *row_pitch, pixels)?,
                        upload,
                    };
                    self.textures.insert(request.texture(), texture);
                    feedback.push(request.uploaded(id)?);
                }
                TextureOp::Update {
                    format,
                    width,
                    height,
                    rects,
                } => {
                    if self.destroyed.contains(&request.texture()) {
                        feedback.push(request.superseded());
                        continue;
                    }
                    let upload = request
                        .upload_identity()
                        .expect("an update request has an upload identity");
                    let Some(texture) = self.textures.get_mut(&request.texture()) else {
                        feedback.push(request.retry());
                        continue;
                    };
                    if texture.upload == upload {
                        feedback.push(request.uploaded(texture.id)?);
                        continue;
                    }
                    if texture.format != *format
                        || texture.width != *width
                        || texture.height != *height
                    {
                        return Err(CpuRendererError::UpdateLayoutMismatch);
                    }
                    for update in rects {
                        apply_update(texture, update)?;
                    }
                    texture.upload = upload;
                    feedback.push(request.uploaded(texture.id)?);
                }
                TextureOp::Destroy => {
                    self.destroyed.insert(request.texture());
                    self.textures.remove(&request.texture());
                    feedback.push(request.destroyed()?);
                }
            }
        }

        let frame = frame.reconcile_texture_feedback(feedback)?;
        let known_ids = self
            .textures
            .values()
            .map(|texture| texture.id)
            .collect::<HashSet<_>>();
        let mut stats = DrawStats::default();
        for draw_list in frame.draw_data().draw_lists() {
            stats.draw_lists += 1;
            let _vertices = draw_list.vtx_buffer();
            let _indices = draw_list.idx_buffer();
            for command in draw_list.commands() {
                match command {
                    DrawCmd::Elements { count, cmd_params } => {
                        if !known_ids.contains(&cmd_params.texture_id) {
                            return Err(CpuRendererError::UnknownTexture(cmd_params.texture_id));
                        }
                        stats.element_commands += 1;
                        stats.elements += count;
                    }
                    DrawCmd::ResetRenderState
                    | DrawCmd::SetSamplerLinear
                    | DrawCmd::SetSamplerNearest => stats.state_changes += 1,
                    DrawCmd::RawCallback(_) => unreachable!("raw callbacks were preflighted"),
                }
            }
        }
        Ok(stats)
    }

    fn shutdown(&mut self, context: &mut Context) -> Result<(), RendererConsumerError> {
        let Some(consumer) = self.consumer.as_ref() else {
            return Ok(());
        };

        let reset = context.prepare_renderer_texture_reset(consumer)?;
        self.textures.clear();
        reset.commit();
        self.destroyed.clear();
        drop(self.consumer.take());
        context.poll_snapshot_completions()?;
        Ok(())
    }
}

fn copy_tight_rows(
    format: TextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    source: &[u8],
) -> Result<Vec<u8>, CpuRendererError> {
    let tight_row = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(get_format_bytes_per_pixel(format)))
        .ok_or(CpuRendererError::UploadSizeOverflow)?;
    if row_pitch < tight_row {
        return Err(CpuRendererError::InvalidRowPitch);
    }
    let height = usize::try_from(height).map_err(|_| CpuRendererError::UploadSizeOverflow)?;
    let output_len = tight_row
        .checked_mul(height)
        .ok_or(CpuRendererError::UploadSizeOverflow)?;
    let mut output = Vec::with_capacity(output_len);
    for row in 0..height {
        let start = row
            .checked_mul(row_pitch)
            .ok_or(CpuRendererError::UploadSizeOverflow)?;
        let end = start
            .checked_add(tight_row)
            .ok_or(CpuRendererError::UploadSizeOverflow)?;
        let source_row = source
            .get(start..end)
            .ok_or(CpuRendererError::InvalidRowPitch)?;
        output.extend_from_slice(source_row);
    }
    Ok(output)
}

fn apply_update(
    texture: &mut CpuTexture,
    update: &dear_imgui_rs::render::TextureUploadRect,
) -> Result<(), CpuRendererError> {
    let bytes_per_pixel = get_format_bytes_per_pixel(texture.format);
    let rect = update.rect;
    let tight_row = usize::from(rect.w)
        .checked_mul(bytes_per_pixel)
        .ok_or(CpuRendererError::UploadSizeOverflow)?;
    if update.row_pitch < tight_row {
        return Err(CpuRendererError::InvalidRowPitch);
    }
    let texture_row = usize::try_from(texture.width)
        .ok()
        .and_then(|width| width.checked_mul(bytes_per_pixel))
        .ok_or(CpuRendererError::UploadSizeOverflow)?;

    for row in 0..usize::from(rect.h) {
        let source_start = row
            .checked_mul(update.row_pitch)
            .ok_or(CpuRendererError::UploadSizeOverflow)?;
        let source_end = source_start
            .checked_add(tight_row)
            .ok_or(CpuRendererError::UploadSizeOverflow)?;
        let source = update
            .data
            .get(source_start..source_end)
            .ok_or(CpuRendererError::InvalidRowPitch)?;

        let destination_y = usize::from(rect.y)
            .checked_add(row)
            .ok_or(CpuRendererError::UploadSizeOverflow)?;
        let destination_start = destination_y
            .checked_mul(texture_row)
            .and_then(|offset| {
                usize::from(rect.x)
                    .checked_mul(bytes_per_pixel)
                    .and_then(|x| offset.checked_add(x))
            })
            .ok_or(CpuRendererError::UploadSizeOverflow)?;
        let destination_end = destination_start
            .checked_add(tight_row)
            .ok_or(CpuRendererError::UploadSizeOverflow)?;
        let destination = texture
            .pixels
            .get_mut(destination_start..destination_end)
            .ok_or(CpuRendererError::UpdateLayoutMismatch)?;
        destination.copy_from_slice(source);
    }
    Ok(())
}

fn build_frame(frame: &FrameToken<'_>, texture: ManagedTextureId, frame_index: usize) {
    let ui = frame.ui();
    ui.window("CPU Renderer")
        .size([320.0, 180.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Frame {frame_index}"));
            if frame_index < 2 {
                ui.image(texture, [64.0, 64.0]);
            } else {
                ui.text("The managed texture is retiring.");
            }
        });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut context = Context::create();
    context.set_ini_filename(None::<String>)?;
    context.io_mut().set_display_size([800.0, 600.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);

    let texture = context.register_texture(OwnedTextureData::from_pixels(
        TextureFormat::RGBA32,
        2,
        2,
        &[
            255, 0, 0, 255, 0, 255, 0, 255, //
            0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )?);
    let mut renderer = CpuRenderer::new(&mut context)?;

    for frame_index in 0..3 {
        if frame_index == 1 {
            let region = TextureRegion::new(1, 0, 1, 2)?;
            context.try_with_texture_mut(texture, |mut texture| {
                texture.update_subresource(TextureSubresource::new(
                    region,
                    4,
                    &[255, 255, 0, 255, 0, 255, 255, 255],
                ))
            })?;
        } else if frame_index == 2 {
            context.remove_texture(texture)?;
        }

        let frame = context.begin_frame();
        build_frame(&frame, texture, frame_index);
        let pending = frame.render(renderer.consumer());
        let stats = renderer.render(pending)?;
        println!(
            "frame {frame_index}: lists={}, element_commands={}, elements={}, textures={}",
            stats.draw_lists,
            stats.element_commands,
            stats.elements,
            renderer.textures.len()
        );
    }

    renderer.shutdown(&mut context)?;
    Ok(())
}
