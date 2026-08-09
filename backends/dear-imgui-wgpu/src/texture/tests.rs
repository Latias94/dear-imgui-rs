use super::cleanup::ManagedRequestOutcome;
use super::*;
use dear_imgui_rs::{
    Context,
    render::SnapshotTextureId,
    texture::{OwnedTextureData, TextureFormat},
};

fn request_test_device() -> Option<(Device, Queue)> {
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let instance = Instance::new(&InstanceDescriptor::default());
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
    let request_adapter = |force_fallback_adapter| {
        pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::LowPower,
            force_fallback_adapter,
            ..Default::default()
        }))
    };
    let adapter = request_adapter(true)
        .or_else(|_| request_adapter(false))
        .ok()?;
    pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).ok()
}

fn managed_texture_id(context: &mut Context) -> SnapshotTextureId {
    let texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 2, 2, &[0; 16]).unwrap();
    SnapshotTextureId::User(context.register_texture(texture))
}

fn create_operation() -> TextureOp {
    TextureOp::Create {
        format: TextureFormat::RGBA32,
        width: 2,
        height: 2,
        row_pitch: 8,
        pixels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    }
}

#[test]
fn managed_requests_are_idempotent_and_retired_work_cannot_resurrect() -> RendererResult<()> {
    let Some((device, queue)) = request_test_device() else {
        eprintln!("skipping WGPU managed texture test because no headless adapter is available");
        return Ok(());
    };
    let mut context = Context::create();
    let id = managed_texture_id(&mut context);
    let mut manager = WgpuTextureManager::new();
    let mut render_resources = RenderResources::new();

    let first = manager.apply_managed_request(
        id,
        &create_operation(),
        &device,
        &queue,
        &mut render_resources,
    )?;
    let ManagedRequestOutcome::Uploaded(first_texture_id) = first else {
        panic!("create request must upload");
    };
    let duplicate = manager.apply_managed_request(
        id,
        &create_operation(),
        &device,
        &queue,
        &mut render_resources,
    )?;
    assert_eq!(duplicate, ManagedRequestOutcome::Uploaded(first_texture_id));
    assert_eq!(manager.managed_texture_count(), 1);

    let malformed_retry = TextureOp::Create {
        format: TextureFormat::RGBA32,
        width: 2,
        height: 2,
        row_pitch: 8,
        pixels: vec![0; 15],
    };
    assert!(
        manager
            .apply_managed_request(id, &malformed_retry, &device, &queue, &mut render_resources,)
            .is_err(),
        "a repeated create must validate and upload the request's current pixels"
    );
    assert_eq!(manager.managed_texture_count(), 1);

    let update = TextureOp::Update {
        format: TextureFormat::RGBA32,
        width: 2,
        height: 2,
        rects: vec![TextureUploadRect {
            rect: TextureRect {
                x: 1,
                y: 1,
                w: 1,
                h: 1,
            },
            row_pitch: 4,
            data: vec![21, 22, 23, 24],
        }],
    };
    assert_eq!(
        manager.apply_managed_request(id, &update, &device, &queue, &mut render_resources,)?,
        ManagedRequestOutcome::Uploaded(first_texture_id)
    );
    assert_eq!(
        manager.apply_managed_request(id, &update, &device, &queue, &mut render_resources,)?,
        ManagedRequestOutcome::Uploaded(first_texture_id)
    );

    assert_eq!(
        manager.apply_managed_request_at_epoch(
            id,
            &TextureOp::Destroy,
            4,
            &device,
            &queue,
            &mut render_resources,
        )?,
        ManagedRequestOutcome::Destroyed
    );
    assert_eq!(manager.managed_texture_count(), 0);
    assert_eq!(manager.destroyed_managed_texture_count(), 1);
    assert_eq!(
        manager.apply_managed_request_at_epoch(
            id,
            &TextureOp::Destroy,
            5,
            &device,
            &queue,
            &mut render_resources,
        )?,
        ManagedRequestOutcome::Destroyed
    );
    assert_eq!(
        manager.apply_managed_request_at_epoch(
            id,
            &create_operation(),
            3,
            &device,
            &queue,
            &mut render_resources,
        )?,
        ManagedRequestOutcome::Superseded
    );
    assert_eq!(manager.managed_texture_count(), 0);
    manager.clear_managed_textures();
    assert_eq!(manager.destroyed_managed_texture_count(), 1);
    assert_eq!(
        manager.apply_managed_request_at_epoch(
            id,
            &create_operation(),
            3,
            &device,
            &queue,
            &mut render_resources,
        )?,
        ManagedRequestOutcome::Superseded
    );
    manager.prune_destroyed_managed_textures(4);
    assert_eq!(manager.destroyed_managed_texture_count(), 1);
    manager.prune_destroyed_managed_textures(5);
    assert_eq!(manager.destroyed_managed_texture_count(), 0);
    Ok(())
}

#[test]
fn managed_destroy_and_renderer_invalidation_preserve_external_texture_handles()
-> RendererResult<()> {
    let Some((device, queue)) = request_test_device() else {
        eprintln!("skipping WGPU external texture test because no headless adapter is available");
        return Ok(());
    };
    let external_texture = device.create_texture(&TextureDescriptor {
        label: Some("external texture test"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let external_view = external_texture.create_view(&TextureViewDescriptor::default());

    let mut context = Context::create();
    let managed = managed_texture_id(&mut context);
    let mut manager = WgpuTextureManager::new();
    let external = manager.register_external_view(&external_view)?;
    let mut render_resources = RenderResources::new();
    manager.apply_managed_request(
        managed,
        &create_operation(),
        &device,
        &queue,
        &mut render_resources,
    )?;
    manager.apply_managed_request(
        managed,
        &TextureOp::Destroy,
        &device,
        &queue,
        &mut render_resources,
    )?;

    assert!(manager.contains_texture(external.texture_id()));
    assert!(manager.texture_view(external.texture_id()).is_some());
    assert_eq!(manager.texture_count(), 1);

    manager.clear_managed_textures();
    assert!(manager.contains_texture(external.texture_id()));
    assert_eq!(manager.texture_count(), 1);

    manager.clear_external_views();
    assert!(!manager.contains_texture(external.texture_id()));
    assert_eq!(manager.texture_count(), 0);
    Ok(())
}

#[test]
fn external_texture_handles_are_unique_and_renderer_scoped() -> RendererResult<()> {
    let Some((device, _queue)) = request_test_device() else {
        eprintln!("skipping WGPU external texture test because no headless adapter is available");
        return Ok(());
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("external handle scope test"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let first_view = texture.create_view(&TextureViewDescriptor::default());
    let second_view = texture.create_view(&TextureViewDescriptor::default());
    let mut first = WgpuTextureManager::new();
    let mut second = WgpuTextureManager::new();

    let first_handle = first.register_external_view(&first_view)?;
    let second_handle = second.register_external_view(&second_view)?;

    assert_ne!(first_handle, second_handle);
    assert!(matches!(
        second.update_external_view(first_handle, &second_view),
        Err(RendererError::ExternalTextureNotFound(id)) if id == first_handle.texture_id()
    ));
    first.remove_external_view(first_handle)?;
    assert!(matches!(
        first.remove_external_view(first_handle),
        Err(RendererError::ExternalTextureNotFound(id)) if id == first_handle.texture_id()
    ));
    Ok(())
}
