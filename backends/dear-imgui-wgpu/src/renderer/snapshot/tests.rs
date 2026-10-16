use super::*;
use crate::texture::request_test_device;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn detached_thread_contracts_are_derived_from_owned_fields() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WgpuSnapshotExecutor>();
    assert_send_sync::<WgpuSnapshotSubmission>();
    assert_send_sync::<WgpuSnapshotTarget>();
    assert_send_sync::<WgpuSnapshotReport>();
    static_assertions::assert_not_impl_any!(WgpuSnapshotRenderer: Send, Sync);
}

fn snapshot(context: &mut Context, owner: &WgpuSnapshotRenderer) -> FrameSnapshot {
    context.io_mut().set_display_size([16.0, 16.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .begin_frame()
        .render_snapshot(owner.renderer_consumer().unwrap())
        .unwrap()
}

#[test]
fn stale_target_retries_requests_without_mutating_resources() -> RendererResult<()> {
    let Some((device, queue)) = request_test_device() else {
        return Ok(());
    };
    let mut context = Context::create();
    let (mut owner, mut executor) = WgpuSnapshotRenderer::new(
        WgpuInitInfo::new(device, queue, wgpu::TextureFormat::Rgba8Unorm),
        &mut context,
    )?;
    let viewport = context.main_viewport().id();
    let managed = context.register_texture(
        dear_imgui_rs::texture::OwnedTextureData::from_pixels(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            1,
            1,
            &[3, 5, 7, 255],
        )
        .unwrap(),
    );
    let native_before = NativeWindowTarget {
        instance_id: dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap(),
        viewport_id: viewport,
        is_main: true,
    };
    let native_after = NativeWindowTarget {
        instance_id: dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap(),
        ..native_before
    };
    let old = executor.set_target(viewport, Some(native_before))?;
    let frame = snapshot(&mut context, &owner);
    assert!(frame.texture_requests().iter().any(
        |request| request.texture() == dear_imgui_rs::render::SnapshotTextureId::User(managed)
    ));
    let new = executor.set_target(viewport, Some(native_after))?;
    let failure = match executor.prepare(WgpuSnapshotSubmission::new(frame, [old])) {
        Ok(_) => panic!("old target accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        *failure.error,
        RendererError::StaleSnapshotTarget { .. }
    ));
    assert_eq!(failure.report.targets[0].target, old);
    assert_eq!(
        failure.report.targets[0].target.native_target(),
        Some(native_before)
    );
    assert!(!failure.report.targets[0].draw.recorded);
    assert!(failure.completion_error.is_none());
    assert!(
        executor
            .gpu
            .texture_manager
            .managed_texture_id(dear_imgui_rs::render::SnapshotTextureId::User(managed))
            .is_none()
    );
    context.poll_snapshot_completions()?;
    let retry = snapshot(&mut context, &owner);
    assert!(retry.texture_requests().iter().any(
        |request| request.texture() == dear_imgui_rs::render::SnapshotTextureId::User(managed)
    ));
    let report = executor
        .prepare(WgpuSnapshotSubmission::new(retry, [new]))
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(report.targets[0].target.native_target(), Some(native_after));
    context.poll_snapshot_completions()?;
    owner.shutdown(&mut context, &mut executor)
}

#[test]
fn prepared_drop_completes_managed_create_update_and_destroy_feedback() -> RendererResult<()> {
    use dear_imgui_rs::render::{SnapshotTextureId, TextureRequestKind};
    let Some((device, queue)) = request_test_device() else {
        return Ok(());
    };
    let mut context = Context::create();
    let (mut owner, mut executor) = WgpuSnapshotRenderer::new(
        WgpuInitInfo::new(device, queue, wgpu::TextureFormat::Rgba8Unorm),
        &mut context,
    )?;
    let viewport = context.main_viewport().id();
    let target = executor.set_target(viewport, None)?;
    let texture = context.register_texture(
        dear_imgui_rs::texture::OwnedTextureData::from_pixels(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            1,
            1,
            &[1, 2, 3, 255],
        )
        .unwrap(),
    );
    let identity = SnapshotTextureId::User(texture);
    let frame = snapshot(&mut context, &owner);
    let prepared = executor
        .prepare(WgpuSnapshotSubmission::new(frame, [target]))
        .unwrap();
    assert!(!prepared.report().targets[0].draw.recorded);
    drop(prepared);
    context.poll_snapshot_completions()?;
    let original = context
        .with_texture(texture, |texture| texture.texture_id())
        .unwrap();
    assert!(!original.is_null());

    context
        .try_with_texture_mut(texture, |mut texture| {
            texture.replace_pixels(&[4, 5, 6, 255])
        })
        .unwrap();
    let frame = snapshot(&mut context, &owner);
    assert!(frame.texture_requests().iter().any(
        |request| request.texture() == identity && request.kind() == TextureRequestKind::Update
    ));
    drop(
        executor
            .prepare(WgpuSnapshotSubmission::new(frame, [target]))
            .unwrap(),
    );
    context.poll_snapshot_completions()?;
    assert_eq!(
        context
            .with_texture(texture, |texture| texture.texture_id())
            .unwrap(),
        original
    );

    context.remove_texture(texture).unwrap();
    let frame = snapshot(&mut context, &owner);
    assert!(
        frame
            .texture_requests()
            .iter()
            .any(|request| request.texture() == identity
                && request.kind() == TextureRequestKind::Destroy)
    );
    drop(
        executor
            .prepare(WgpuSnapshotSubmission::new(frame, [target]))
            .unwrap(),
    );
    assert!(
        executor
            .gpu
            .texture_manager
            .managed_texture_id(identity)
            .is_none()
    );
    context.poll_snapshot_completions()?;
    owner.shutdown(&mut context, &mut executor)
}

#[test]
fn late_texture_failure_preserves_earlier_uploaded_feedback_and_retries_rest() -> RendererResult<()>
{
    use dear_imgui_rs::render::{SnapshotTextureId, TextureOp};
    let Some((device, queue)) = request_test_device() else {
        return Ok(());
    };
    let mut context = Context::create();
    let (mut owner, mut executor) = WgpuSnapshotRenderer::new(
        WgpuInitInfo::new(device, queue, wgpu::TextureFormat::Rgba8Unorm),
        &mut context,
    )?;
    for _ in 0..2 {
        context.register_texture(
            dear_imgui_rs::texture::OwnedTextureData::from_pixels(
                dear_imgui_rs::texture::TextureFormat::RGBA32,
                1,
                1,
                &[9, 8, 7, 255],
            )
            .unwrap(),
        );
    }
    let viewport = context.main_viewport().id();
    let target = executor.set_target(viewport, None)?;
    let frame = snapshot(&mut context, &owner);
    let users = frame
        .texture_requests()
        .iter()
        .filter_map(|request| match request.texture() {
            SnapshotTextureId::User(id) => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(users.len(), 2);
    let backend = executor.gpu.backend_data.as_mut().unwrap();
    executor
        .gpu
        .texture_manager
        .apply_managed_request_at_epoch(
            SnapshotTextureId::User(users[1]),
            &TextureOp::Create {
                format: dear_imgui_rs::texture::TextureFormat::RGBA32,
                width: 2,
                height: 1,
                row_pitch: 8,
                pixels: vec![255; 8],
            },
            frame.epoch().sequence(),
            &backend.device,
            &backend.queue,
            &mut backend.render_resources,
        )?;
    let failure = match executor.prepare(WgpuSnapshotSubmission::new(frame, [target])) {
        Ok(_) => panic!("incompatible resource layout accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        *failure.error,
        RendererError::ManagedTextureLayoutMismatch { .. }
    ));
    assert!(failure.completion_error.is_none());
    context.poll_snapshot_completions()?;
    assert!(
        !context
            .with_texture(users[0], |texture| texture.texture_id())
            .unwrap()
            .is_null()
    );
    assert!(
        context
            .with_texture(users[1], |texture| texture.texture_id())
            .unwrap()
            .is_null()
    );
    let retry = snapshot(&mut context, &owner);
    assert!(
        !retry
            .texture_requests()
            .iter()
            .any(|request| request.texture() == SnapshotTextureId::User(users[0]))
    );
    assert!(
        retry
            .texture_requests()
            .iter()
            .any(|request| request.texture() == SnapshotTextureId::User(users[1]))
    );
    drop(retry);
    context.poll_snapshot_completions()?;
    owner.shutdown(&mut context, &mut executor)
}

#[test]
fn delayed_snapshot_is_superseded_without_replaying_texture_operations() -> RendererResult<()> {
    let Some((device, queue)) = request_test_device() else {
        return Ok(());
    };
    let mut context = Context::create();
    let (mut owner, mut executor) = WgpuSnapshotRenderer::new(
        WgpuInitInfo::new(device, queue, wgpu::TextureFormat::Rgba8Unorm),
        &mut context,
    )?;
    let viewport = context.main_viewport().id();
    let target = executor.set_target(viewport, None)?;
    let older = snapshot(&mut context, &owner);
    let newer = snapshot(&mut context, &owner);
    let newer_epoch = newer.epoch();
    executor
        .prepare(WgpuSnapshotSubmission::new(newer, [target]))
        .unwrap()
        .finish()
        .unwrap();
    let failure = match executor.prepare(WgpuSnapshotSubmission::new(older, [target])) {
        Ok(_) => panic!("older epoch accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        *failure.error,
        RendererError::FrameEpochOutOfOrder { .. }
    ));
    assert!(failure.completion_error.is_none());
    assert_eq!(
        context.poll_snapshot_completions()?.watermark(),
        newer_epoch.sequence()
    );
    owner.shutdown(&mut context, &mut executor)
}

#[test]
fn ordered_texture_churn_reclaims_tombstones_and_never_replays_delayed_creates()
-> RendererResult<()> {
    use dear_imgui_rs::render::SnapshotTextureId;
    let Some((device, queue)) = request_test_device() else {
        return Ok(());
    };
    let mut context = Context::create();
    let (mut owner, mut executor) = WgpuSnapshotRenderer::new(
        WgpuInitInfo::new(device, queue, wgpu::TextureFormat::Rgba8Unorm),
        &mut context,
    )?;
    let viewport = context.main_viewport().id();
    let target = executor.set_target(viewport, None)?;
    for _ in 0..8 {
        let texture = context.register_texture(
            dear_imgui_rs::texture::OwnedTextureData::from_pixels(
                dear_imgui_rs::texture::TextureFormat::RGBA32,
                1,
                1,
                &[7, 9, 11, 255],
            )
            .unwrap(),
        );
        let identity = SnapshotTextureId::User(texture);
        let delayed = snapshot(&mut context, &owner);
        assert!(
            delayed
                .texture_requests()
                .iter()
                .any(|request| request.texture() == identity)
        );
        let newer = snapshot(&mut context, &owner);
        executor
            .prepare(WgpuSnapshotSubmission::new(newer, [target]))
            .unwrap()
            .finish()
            .unwrap();
        context.remove_texture(texture).unwrap();
        let destroy = snapshot(&mut context, &owner);
        executor
            .prepare(WgpuSnapshotSubmission::new(destroy, [target]))
            .unwrap()
            .finish()
            .unwrap();
        assert!(
            executor
                .gpu
                .texture_manager
                .managed_texture_id(identity)
                .is_none()
        );
        assert_eq!(
            executor
                .gpu
                .texture_manager
                .destroyed_managed_texture_count(),
            1
        );

        let failure = match executor.prepare(WgpuSnapshotSubmission::new(delayed, [target])) {
            Ok(_) => panic!("delayed create resurrected a destroyed texture"),
            Err(error) => error,
        };
        assert!(matches!(
            *failure.error,
            RendererError::FrameEpochOutOfOrder { .. }
        ));
        assert!(
            executor
                .gpu
                .texture_manager
                .managed_texture_id(identity)
                .is_none()
        );
        context.poll_snapshot_completions()?;
        let next = snapshot(&mut context, &owner);
        executor
            .prepare(WgpuSnapshotSubmission::new(next, [target]))
            .unwrap()
            .finish()
            .unwrap();
        assert_eq!(
            executor
                .gpu
                .texture_manager
                .destroyed_managed_texture_count(),
            0,
            "completed texture churn must not retain historical destroy identities"
        );
        context.poll_snapshot_completions()?;
    }
    owner.shutdown(&mut context, &mut executor)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn detached_executor_records_and_reads_back_on_a_worker_thread() -> RendererResult<()> {
    use crate::wgpu::*;
    use std::sync::mpsc;
    use std::thread;

    let Some((device, queue)) = request_test_device() else {
        return Ok(());
    };
    let scene = device.create_texture(&TextureDescriptor {
        label: Some("detached engine producer texture"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let scene_view = scene.create_view(&TextureViewDescriptor::default());
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: &scene,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &[0, 0, 255, 255],
        TexelCopyBufferLayout {
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
    let mut context = Context::create();
    let (mut owner, mut executor) = WgpuSnapshotRenderer::new(
        WgpuInitInfo::new(device, queue, TextureFormat::Rgba8Unorm),
        &mut context,
    )?;
    executor.set_gamma_mode(GammaMode::Linear);
    let external = executor.register_external_texture(&scene_view)?;
    let original = executor.external_texture_binding(external)?;
    let managed = context.register_texture(
        dear_imgui_rs::texture::OwnedTextureData::from_pixels(
            dear_imgui_rs::texture::TextureFormat::RGBA32,
            1,
            1,
            &[0, 255, 0, 255],
        )
        .unwrap(),
    );
    let viewport = context.main_viewport().id();
    let target = executor.set_target(viewport, None)?;
    context.io_mut().set_display_size([16.0, 16.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let frame = context.begin_frame();
    {
        let ui = frame.ui();
        let draw = ui.get_foreground_draw_list();
        draw.add_image(
            external.texture_id(),
            [0.0, 0.0],
            [8.0, 16.0],
            [0.0, 0.0],
            [1.0, 1.0],
            dear_imgui_rs::Color::WHITE,
        );
        draw.add_image(
            managed,
            [8.0, 0.0],
            [16.0, 16.0],
            [0.0, 0.0],
            [1.0, 1.0],
            dear_imgui_rs::Color::WHITE,
        );
    }
    let snapshot = frame.render_snapshot(owner.renderer_consumer()?)?;
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let mut executor = executor;
        let result = (|| -> Result<[[u8; 4]; 2], String> {
            let mut prepared = executor
                .prepare(WgpuSnapshotSubmission::new(snapshot, [target]))
                .map_err(|error| format!("prepare failed: {error:?}"))?;
            let (device, queue) = {
                let backend = prepared
                    .executor
                    .gpu
                    .backend_data
                    .as_ref()
                    .ok_or_else(|| "missing backend".to_owned())?;
                (backend.device.clone(), backend.queue.clone())
            };
            let output = device.create_texture(&TextureDescriptor {
                label: Some("detached worker readback target"),
                size: Extent3d {
                    width: 16,
                    height: 16,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let output_view = output.create_view(&TextureViewDescriptor::default());
            let readback = device.create_buffer(&BufferDescriptor {
                label: Some("detached worker readback"),
                size: 256 * 16,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
            {
                let attachments = [Some(RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })];
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    color_attachments: &attachments,
                    #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                    multiview_mask: None,
                    ..Default::default()
                });
                prepared
                    .record(target, &mut pass, FramebufferExtent::new(16, 16))
                    .map_err(|error| format!("record failed: {error:?}"))?;
            }
            encoder.copy_texture_to_buffer(
                TexelCopyTextureInfo {
                    texture: &output,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(16),
                    },
                },
                Extent3d {
                    width: 16,
                    height: 16,
                    depth_or_array_layers: 1,
                },
            );
            // The UI was recorded first; the queue must execute the engine producer first.
            let mut producer = device.create_command_encoder(&CommandEncoderDescriptor::default());
            {
                let attachments = [Some(RenderPassColorAttachment {
                    view: &scene_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::RED),
                        store: StoreOp::Store,
                    },
                })];
                let _pass = producer.begin_render_pass(&RenderPassDescriptor {
                    color_attachments: &attachments,
                    #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                    multiview_mask: None,
                    ..Default::default()
                });
            }
            let submission = queue.submit([producer.finish(), encoder.finish()]);
            let report = prepared
                .finish()
                .map_err(|error| format!("finish failed: {error:?}"))?;
            assert_eq!(
                report.targets[0].draw.dependencies.external_bindings,
                [original]
            );
            assert!(report.targets[0].draw.recorded);
            assert!(
                !report.targets[0]
                    .draw
                    .dependencies
                    .contains_unknown_callback_dependencies
            );
            let (mapped_tx, mapped_rx) = mpsc::sync_channel(1);
            readback.slice(..).map_async(MapMode::Read, move |result| {
                let _ = mapped_tx.send(result);
            });
            device
                .poll(PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .map_err(|error| format!("poll failed: {error:?}"))?;
            mapped_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .map_err(|error| error.to_string())?
                .map_err(|error| format!("map failed: {error:?}"))?;
            #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", feature = "wgpu-29"))]
            let bytes = readback.slice(..).get_mapped_range();
            #[cfg(feature = "wgpu-30")]
            let bytes = readback
                .slice(..)
                .get_mapped_range()
                .map_err(|error| format!("mapped range failed: {error:?}"))?;
            let pixel = |x: usize| {
                let offset = 8 * 256 + x * 4;
                [
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]
            };
            let pixels = [pixel(4), pixel(12)];
            drop(bytes);
            readback.unmap();
            Ok(pixels)
        })();
        result_tx.send(result).ok();
        executor
    });
    let pixel = result_rx
        .recv()
        .expect("worker should return readback")
        .expect("worker render should succeed");
    assert_eq!(
        pixel,
        [[255, 0, 0, 255], [0, 255, 0, 255]],
        "worker must sample the newly produced scene and the managed upload"
    );
    let mut executor = worker.join().expect("worker should not panic");
    context.poll_snapshot_completions()?;
    owner.shutdown(&mut context, &mut executor)
}
