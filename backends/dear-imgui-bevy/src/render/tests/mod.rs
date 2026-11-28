use super::*;
use bevy_render::{renderer::initialize_renderer, settings::WgpuSettings};

mod draw;
mod lifecycle;
mod texture_bindings;

fn managed_context() -> imgui::Context {
    let mut context = imgui::Context::create();
    context.io_mut().set_config_input_trickle_event_queue(false);
    context.prepare_frame(
        imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
    );
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
    context
}

fn managed_snapshot(
    context: &mut imgui::Context,
    consumer: &imgui::render::DetachedRendererConsumer,
    texture: Option<imgui::ManagedTextureId>,
) -> imgui::render::FrameSnapshot {
    context.prepare_frame(
        imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
    );
    let frame = context.begin_frame();
    if let Some(texture) = texture {
        frame.ui().image(texture, [16.0, 16.0]);
    }
    frame
        .render_snapshot(consumer)
        .expect("managed frame should produce a Context-owned snapshot")
}

fn user_texture_request(
    snapshot: &imgui::render::FrameSnapshot,
    texture: imgui::ManagedTextureId,
) -> &imgui::render::snapshot::TextureRequest {
    snapshot
        .texture_requests()
        .iter()
        .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture))
        .expect("snapshot should contain the user texture request")
}

fn successful_texture_feedback(
    snapshot: &imgui::render::FrameSnapshot,
    user_texture: Option<(imgui::ManagedTextureId, imgui::TextureId)>,
) -> Vec<imgui::render::snapshot::TextureFeedback> {
    snapshot
        .texture_requests()
        .iter()
        .enumerate()
        .map(|(index, request)| match request.operation() {
            imgui::render::TextureOp::Create { .. } | imgui::render::TextureOp::Update { .. } => {
                let texture_id = match (request.texture(), user_texture) {
                    (imgui::render::SnapshotTextureId::User(actual), Some((expected, id)))
                        if actual == expected =>
                    {
                        id
                    }
                    _ => imgui::TextureId::new(
                        0xF07A
                            + u64::try_from(index).expect("test texture request index fits in u64"),
                    ),
                };
                request.uploaded(texture_id).unwrap()
            }
            imgui::render::TextureOp::Destroy => request.destroyed().unwrap(),
        })
        .collect()
}

fn retry_texture_feedback(
    snapshot: &imgui::render::FrameSnapshot,
) -> Vec<imgui::render::snapshot::TextureFeedback> {
    snapshot
        .texture_requests()
        .iter()
        .map(imgui::render::snapshot::TextureRequest::retry)
        .collect()
}

fn register_test_texture(context: &mut imgui::Context) -> imgui::ManagedTextureId {
    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::TextureFormat::RGBA32,
        1,
        1,
        &[255, 255, 255, 255],
    )
    .unwrap();
    context.register_texture(texture)
}

fn pending_frame(
    frame_index: u64,
    snapshot: imgui::render::FrameSnapshot,
) -> crate::context::PendingFrame {
    crate::context::PendingFrame {
        frame_index,
        include_platform_viewports: false,
        render_routes: crate::route::ImguiRenderRouteEpoch::default(),
        snapshot,
    }
}

struct RenderHarnessResources {
    render_device: RenderDevice,
    render_queue: RenderQueue,
    pipeline_cache: PipelineCache,
}

fn initialize_render_harness_resources() -> RenderHarnessResources {
    let settings = WgpuSettings::default();

    let resources = bevy_platform::future::block_on(initialize_renderer(
        settings
            .backends
            .expect("render harness should configure an explicit backend"),
        None,
        &settings,
    ));
    let render_device = resources.0.clone();
    let render_queue = resources.1.clone();
    RenderHarnessResources {
        render_device: render_device.clone(),
        render_queue,
        pipeline_cache: PipelineCache::new(render_device, true),
    }
}

fn test_context_id() -> imgui::ContextId {
    imgui::Context::create().id()
}
