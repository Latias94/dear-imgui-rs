//! Integration tests for the WGPU backend improvements
//!
//! This test verifies that our improvements maintain compatibility with the C++ implementation

use dear_imgui_rs::{
    BackendFlags, Condition, Context, ManagedTextureId, TextureId,
    texture::{OwnedTextureData, TextureFormat as ImGuiTextureFormat},
};
use dear_imgui_wgpu::wgpu::*;
use dear_imgui_wgpu::{
    RenderResources, RendererError, RendererResult, Uniforms, WgpuInitInfo, WgpuRenderer,
    WgpuTexture, WgpuTextureManager,
};
use static_assertions::assert_not_impl_any;

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

#[test]
fn renderer_is_bound_to_the_context_ui_thread() {
    assert_not_impl_any!(WgpuRenderer: Send, Sync);
}

fn render_test_frame(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
    device: &Device,
    queue: &Queue,
    managed_texture: Option<ManagedTextureId>,
    legacy_texture: Option<TextureId>,
) -> RendererResult<()> {
    context.io_mut().set_display_size([64.0, 64.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let ui = context.frame();
    ui.window("device lifecycle test")
        .position([0.0, 0.0], Condition::Always)
        .size([64.0, 64.0], Condition::Always)
        .build(|| ui.text("rebuild"));
    if let Some(texture) = managed_texture {
        ui.image(texture, [16.0, 16.0]);
    }
    if let Some(texture) = legacy_texture {
        ui.image(texture, [16.0, 16.0]);
    }

    let target = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu lifecycle test target"),
        size: Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("dear-imgui-wgpu lifecycle test encoder"),
    });
    {
        let color_attachments = [Some(RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::BLACK),
                store: StoreOp::Store,
            },
            depth_slice: None,
        })];
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("dear-imgui-wgpu lifecycle test pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
            multiview_mask: None,
            timestamp_writes: None,
        });
        renderer.new_frame()?;
        renderer.render_context(context, &mut render_pass)?;
    }
    queue.submit([encoder.finish()]);
    Ok(())
}

fn render_context_without_open_frame(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
    device: &Device,
) -> RendererResult<()> {
    let target = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu context identity test target"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("dear-imgui-wgpu context identity test encoder"),
    });
    let color_attachments = [Some(RenderPassColorAttachment {
        view: &view,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(Color::BLACK),
            store: StoreOp::Store,
        },
        depth_slice: None,
    })];
    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("dear-imgui-wgpu context identity test pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
        multiview_mask: None,
        timestamp_writes: None,
    });
    renderer.render_context(context, &mut render_pass)
}

/// Test that gamma correction values match the C++ implementation
#[test]
fn test_gamma_correction_formats() {
    // Test sRGB formats that should have gamma = 2.2
    let srgb_formats = vec![
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Bc1RgbaUnormSrgb,
        TextureFormat::Bc2RgbaUnormSrgb,
        TextureFormat::Bc3RgbaUnormSrgb,
        TextureFormat::Bc7RgbaUnormSrgb,
        TextureFormat::Etc2Rgb8UnormSrgb,
        TextureFormat::Etc2Rgb8A1UnormSrgb,
        TextureFormat::Etc2Rgba8UnormSrgb,
    ];

    for format in srgb_formats {
        let gamma = Uniforms::gamma_for_format(format);
        assert_eq!(gamma, 2.2, "sRGB format {:?} should have gamma 2.2", format);
    }

    // Test ASTC sRGB formats
    let astc_srgb_formats = vec![
        TextureFormat::Astc {
            block: AstcBlock::B4x4,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B5x4,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B5x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B6x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B6x6,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B8x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B8x6,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B8x8,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x5,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x6,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x8,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B10x10,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B12x10,
            channel: AstcChannel::UnormSrgb,
        },
        TextureFormat::Astc {
            block: AstcBlock::B12x12,
            channel: AstcChannel::UnormSrgb,
        },
    ];

    for format in astc_srgb_formats {
        let gamma = Uniforms::gamma_for_format(format);
        assert_eq!(
            gamma, 2.2,
            "ASTC sRGB format {:?} should have gamma 2.2",
            format
        );
    }

    // Test linear formats that should have gamma = 1.0
    let linear_formats = vec![
        TextureFormat::Rgba8Unorm,
        TextureFormat::Bgra8Unorm,
        TextureFormat::R8Unorm,
        TextureFormat::Rg8Unorm,
    ];

    for format in linear_formats {
        let gamma = Uniforms::gamma_for_format(format);
        assert_eq!(
            gamma, 1.0,
            "Linear format {:?} should have gamma 1.0",
            format
        );
    }
}

/// Test that orthographic matrix calculation matches C++ implementation
#[test]
fn test_orthographic_matrix() {
    let display_pos = [10.0, 20.0];
    let display_size = [800.0, 600.0];

    let matrix = Uniforms::create_orthographic_matrix(display_pos, display_size);

    // Expected values based on C++ implementation:
    // L = 10.0, R = 810.0, T = 20.0, B = 620.0
    let l = display_pos[0];
    let r = display_pos[0] + display_size[0];
    let t = display_pos[1];
    let b = display_pos[1] + display_size[1];

    // Check matrix values match C++ calculation
    assert_eq!(matrix[0][0], 2.0 / (r - l));
    assert_eq!(matrix[1][1], 2.0 / (t - b));
    assert_eq!(matrix[2][2], 0.5);
    assert_eq!(matrix[3][0], (r + l) / (l - r));
    assert_eq!(matrix[3][1], (t + b) / (b - t));
    assert_eq!(matrix[3][2], 0.5);
    assert_eq!(matrix[3][3], 1.0);
}

/// Test that renderer can be created with default values
#[test]
fn test_renderer_creation() {
    let renderer = WgpuRenderer::empty();
    assert!(!renderer.is_initialized());

    // Test default creation
    let default_renderer = WgpuRenderer::default();
    assert!(!default_renderer.is_initialized());
}

#[test]
fn device_objects_rebind_after_invalidation_and_shutdown() -> RendererResult<()> {
    let Some((device, queue)) = request_test_device() else {
        eprintln!("skipping WGPU lifecycle test because no headless adapter is available");
        return Ok(());
    };
    let format = TextureFormat::Rgba8Unorm;
    let mut context = Context::create();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), format),
        &mut context,
    )?;

    render_test_frame(&mut renderer, &mut context, &device, &queue, None, None)?;
    let first_texture_id = context.font_atlas().texture_id();
    assert!(!first_texture_id.is_null());
    assert!(
        renderer
            .texture_manager()
            .contains_texture(first_texture_id)
    );

    renderer.invalidate_device_objects(&mut context)?;
    assert!(!renderer.is_initialized());
    assert!(context.font_atlas().texture_id().is_null());
    assert_eq!(renderer.texture_manager().texture_count(), 0);

    render_test_frame(&mut renderer, &mut context, &device, &queue, None, None)?;
    assert!(renderer.is_initialized());
    let recreated_texture_id = context.font_atlas().texture_id();
    assert!(!recreated_texture_id.is_null());
    assert!(
        renderer
            .texture_manager()
            .contains_texture(recreated_texture_id)
    );

    let suspended_owner = context.suspend();
    let mut foreign_context = Context::create();
    let foreign_flags = foreign_context.io().backend_flags();
    assert!(matches!(
        renderer.invalidate_device_objects(&mut foreign_context),
        Err(RendererError::ContextMismatch)
    ));
    assert!(matches!(
        renderer.shutdown(&mut foreign_context),
        Err(RendererError::ContextMismatch)
    ));
    assert!(matches!(
        render_context_without_open_frame(&mut renderer, &mut foreign_context, &device),
        Err(RendererError::ContextMismatch)
    ));
    #[cfg(feature = "multi-viewport-winit")]
    assert_eq!(
        // SAFETY: context identity validation rejects this call before callbacks retain renderer.
        unsafe { dear_imgui_wgpu::multi_viewport::enable(&mut renderer, &mut foreign_context) },
        Err(dear_imgui_wgpu::multi_viewport::CallbackOwnershipError::RendererContextMismatch)
    );
    #[cfg(feature = "multi-viewport-sdl3")]
    assert_eq!(
        // SAFETY: context identity validation rejects this call before callbacks retain renderer.
        unsafe {
            dear_imgui_wgpu::multi_viewport_sdl3::enable(&mut renderer, &mut foreign_context)
        },
        Err(dear_imgui_wgpu::multi_viewport_sdl3::CallbackOwnershipError::RendererContextMismatch)
    );
    assert!(renderer.is_initialized());
    assert_eq!(foreign_context.io().backend_flags(), foreign_flags);

    let suspended_foreign = foreign_context.suspend();
    let mut context = suspended_owner
        .activate()
        .expect("renderer owner context should reactivate");
    render_test_frame(&mut renderer, &mut context, &device, &queue, None, None)?;

    renderer.shutdown(&mut context)?;
    assert!(!renderer.is_initialized());
    assert!(
        !context.io().backend_flags().intersects(
            BackendFlags::RENDERER_HAS_TEXTURES | BackendFlags::RENDERER_HAS_VTX_OFFSET
        )
    );
    assert!(context.font_atlas().texture_id().is_null());
    assert_eq!(renderer.texture_manager().texture_count(), 0);

    renderer.init_with_context(
        WgpuInitInfo::new(device.clone(), queue.clone(), format),
        &mut context,
    )?;
    render_test_frame(&mut renderer, &mut context, &device, &queue, None, None)?;
    assert!(renderer.is_initialized());
    renderer.shutdown(&mut context)?;

    let suspended_owner = context.suspend();
    let mut context = suspended_foreign
        .activate()
        .expect("replacement context should activate after owner shutdown");
    drop(suspended_owner);

    renderer.init_with_context(
        WgpuInitInfo::new(device.clone(), queue.clone(), format),
        &mut context,
    )?;
    render_test_frame(&mut renderer, &mut context, &device, &queue, None, None)?;
    let reinitialized_texture_id = context.font_atlas().texture_id();
    assert!(!reinitialized_texture_id.is_null());
    assert!(
        renderer
            .texture_manager()
            .contains_texture(reinitialized_texture_id)
    );
    renderer.shutdown(&mut context)?;

    Ok(())
}

#[test]
fn rendered_frame_reconciles_managed_lifecycle_and_preserves_legacy_ids() -> RendererResult<()> {
    let Some((device, queue)) = request_test_device() else {
        eprintln!("skipping WGPU rendered-frame test because no headless adapter is available");
        return Ok(());
    };
    let mut context = Context::create();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), TextureFormat::Rgba8Unorm),
        &mut context,
    )?;

    let mut managed_data = OwnedTextureData::new();
    managed_data.create(ImGuiTextureFormat::RGBA32, 2, 2);
    managed_data.set_data(&[7; 16]);
    let managed = context.register_texture(managed_data);

    let legacy_texture = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu legacy render test"),
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
    let legacy_view = legacy_texture.create_view(&TextureViewDescriptor::default());
    let legacy = renderer.register_external_texture(&legacy_texture, &legacy_view);

    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        Some(managed),
        Some(legacy),
    )?;
    let first_managed_id = context
        .with_texture(managed, |texture| texture.texture_id())
        .expect("managed texture should remain active");
    assert!(!first_managed_id.is_null());
    assert!(
        renderer
            .texture_manager()
            .contains_texture(first_managed_id)
    );
    assert!(renderer.texture_manager().contains_texture(legacy));

    context
        .with_texture_mut(managed, |mut texture| texture.set_data(&[11; 16]))
        .expect("managed texture update should be accepted");
    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        Some(managed),
        Some(legacy),
    )?;
    assert_eq!(
        context
            .with_texture(managed, |texture| texture.texture_id())
            .expect("updated texture should remain active"),
        first_managed_id,
        "updates must keep the renderer-facing texture ID stable"
    );

    context
        .remove_texture(managed)
        .expect("managed texture should begin retirement");
    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        None,
        Some(legacy),
    )?;
    assert!(
        !renderer
            .texture_manager()
            .contains_texture(first_managed_id)
    );
    assert!(renderer.texture_manager().contains_texture(legacy));

    renderer.shutdown(&mut context)?;
    Ok(())
}

/// Test uniforms structure size and alignment
#[test]
fn test_uniforms_layout() {
    use std::mem::{align_of, size_of};

    // Ensure uniforms structure has proper size and alignment for GPU usage
    assert_eq!(size_of::<Uniforms>(), 80); // 4x4 f32 matrix (64 bytes) + f32 gamma (4 bytes) + padding (12 bytes)
    assert_eq!(align_of::<Uniforms>(), 4); // f32 alignment

    let uniforms = Uniforms::new();

    // Test default values
    assert_eq!(uniforms.gamma, 1.0);
    assert_eq!(uniforms.mvp[0][0], 1.0); // Identity matrix
    assert_eq!(uniforms.mvp[1][1], 1.0);
    assert_eq!(uniforms.mvp[2][2], 1.0);
    assert_eq!(uniforms.mvp[3][3], 1.0);
}

/// Public texture-management APIs should expose semantic TextureId handles, not raw integers.
#[test]
fn texture_id_api_boundaries_are_typed() {
    let _: fn(&mut WgpuTextureManager, WgpuTexture) -> TextureId =
        WgpuTextureManager::register_texture;
    let _: for<'a> fn(&'a WgpuTextureManager, TextureId) -> Option<&'a WgpuTexture> =
        WgpuTextureManager::get_texture;
    let _: fn(&mut WgpuTextureManager, TextureId) -> Option<WgpuTexture> =
        WgpuTextureManager::remove_texture;
    let _: fn(&WgpuTextureManager, TextureId) -> bool = WgpuTextureManager::contains_texture;
    let _: fn(&mut WgpuTextureManager, TextureId, WgpuTexture) =
        WgpuTextureManager::insert_texture_with_id;
    let _: fn(&mut WgpuTextureManager, TextureId) = WgpuTextureManager::destroy_texture_by_id;
    let _: for<'a, 'b> fn(
        &'a mut RenderResources,
        &'b Device,
        TextureId,
        &TextureView,
    ) -> RendererResult<&'a BindGroup> = RenderResources::get_or_create_image_bind_group;
    let _: fn(&mut RenderResources, TextureId) = RenderResources::remove_image_bind_group;

    let _: for<'a, 'b, 'c> fn(&'a mut WgpuRenderer, &'b Texture, &'c TextureView) -> TextureId =
        WgpuRenderer::register_external_texture;
    let _: for<'a, 'b, 'c, 'd> fn(
        &'a mut WgpuRenderer,
        &'b Texture,
        &'c TextureView,
        &'d Sampler,
    ) -> TextureId = WgpuRenderer::register_external_texture_with_sampler;
    let _: for<'a, 'b> fn(&'a mut WgpuRenderer, TextureId, &'b TextureView) -> bool =
        WgpuRenderer::update_external_texture_view;
    let _: for<'a, 'b> fn(&'a mut WgpuRenderer, TextureId, &'b Sampler) -> bool =
        WgpuRenderer::update_external_texture_sampler;
    let _: fn(&mut WgpuRenderer, TextureId) = WgpuRenderer::unregister_texture;
}
