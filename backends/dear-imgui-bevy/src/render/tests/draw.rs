use super::*;
use crate::route::ImguiDiagnosticKind;
use pass::{imgui_target_windows, intersect_scissor_with_camera_viewport, scissor_for_render_pass};
use plugin::{
    imgui_bevy_draw_callback_linear, imgui_bevy_draw_callback_nearest,
    imgui_bevy_draw_callback_reset,
};
use prepare::{draw_indices_reference_out_of_bounds, prepare_draw_data};
use resources::pad_index_buffer_for_copy_alignment;

type RawDrawCallback =
    unsafe extern "C" fn(*const imgui::sys::ImDrawList, *const imgui::sys::ImDrawCmd);

unsafe extern "C" fn texture_sampler_callback(
    list: *const imgui::sys::ImDrawList,
    _command: *const imgui::sys::ImDrawCmd,
) {
    std::hint::black_box(list);
}

fn assert_fn_ptr_eq(actual: imgui::sys::ImDrawCallback, expected: RawDrawCallback) {
    assert_eq!(
        actual.map(|callback| std::ptr::fn_addr_eq(callback, expected) as u8),
        Some(1)
    );
}

#[test]
fn index_buffer_upload_pads_to_copy_alignment() {
    let mut indices = RawBufferVec::new(BufferUsages::INDEX);
    indices.push(1);
    indices.push(2);
    indices.push(3);

    pad_index_buffer_for_copy_alignment(&mut indices);

    assert_eq!(indices.len(), 4);
    assert_eq!(indices.values(), &vec![1, 2, 3, 0]);
}

#[test]
fn render_installation_exposes_standard_sampler_callbacks() {
    let mut app = crate::test_util::RenderTestApp::new();
    app.add_plugins(crate::test_util::render_extraction_plugin());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(crate::ImguiPlugin::default());

    let primary_id = app
        .world()
        .get_non_send::<crate::ImguiContexts>()
        .expect("ImguiPlugin should install the Context registry")
        .primary_id()
        .expect("Context registry must be active")
        .expect("ImguiPlugin should install the primary Context");
    app.world_mut()
        .get_non_send_mut::<crate::ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            let platform_io = context.platform_io();
            assert_fn_ptr_eq(
                platform_io.draw_callback_reset_render_state_raw(),
                imgui_bevy_draw_callback_reset,
            );
            assert_fn_ptr_eq(
                platform_io.draw_callback_set_sampler_linear_raw(),
                imgui_bevy_draw_callback_linear,
            );
            assert_fn_ptr_eq(
                platform_io.draw_callback_set_sampler_nearest_raw(),
                imgui_bevy_draw_callback_nearest,
            );
            assert!(
                platform_io
                    .draw_callback_set_sampler_from_texture_raw()
                    .is_none()
            );
        })
        .unwrap();
}

#[test]
fn unsupported_texture_sampler_slot_blocks_installation_and_preserves_foreign_ownership() {
    let mut context = managed_context();
    unsafe {
        context
            .platform_io_mut()
            .set_draw_callback_set_sampler_from_texture_raw(Some(texture_sampler_callback));
    }
    assert_eq!(
        standard_draw_callback_occupied(&context),
        Some("DrawCallback_SetSamplerFromTex")
    );
    assert_eq!(
        install_standard_draw_callbacks_for_context(&mut context),
        Err("DrawCallback_SetSamplerFromTex")
    );
    assert!(
        context
            .platform_io()
            .draw_callback_reset_render_state_raw()
            .is_none()
    );
    clear_standard_draw_callbacks_if_owned(&mut context);
    assert_fn_ptr_eq(
        context
            .platform_io()
            .draw_callback_set_sampler_from_texture_raw(),
        texture_sampler_callback,
    );

    unsafe {
        context
            .platform_io_mut()
            .set_draw_callback_set_sampler_from_texture_raw(None);
    }
    install_standard_draw_callbacks_for_context(&mut context).unwrap();
    assert_eq!(standard_draw_callback_contract(&context)[3], 0);
    unsafe {
        context
            .platform_io_mut()
            .set_draw_callback_set_sampler_from_texture_raw(Some(texture_sampler_callback));
    }
    assert_eq!(
        standard_draw_callback_conflict(&context),
        Some("DrawCallback_SetSamplerFromTex")
    );
    assert_ne!(standard_draw_callback_contract(&context)[3], 0);
    clear_standard_draw_callbacks_if_owned(&mut context);
    let platform_io = context.platform_io();
    assert!(platform_io.draw_callback_reset_render_state_raw().is_none());
    assert!(platform_io.draw_callback_set_sampler_linear_raw().is_none());
    assert!(
        platform_io
            .draw_callback_set_sampler_nearest_raw()
            .is_none()
    );
    assert_fn_ptr_eq(
        platform_io.draw_callback_set_sampler_from_texture_raw(),
        texture_sampler_callback,
    );
}

#[test]
fn preparation_rejects_texture_samplers_in_unrouted_viewports_before_drawing() {
    let mut list = draw_list_for_test();
    list.commands.push(DrawCmdSnapshot::SetSamplerFromTexture);
    let main = imgui::render::DrawDataSnapshot {
        frame_count: 0,
        display_pos: [0.0; 2],
        display_size: [32.0; 2],
        framebuffer_scale: [1.0; 2],
        draw_lists: vec![draw_list_for_test()],
    };
    let viewports = [imgui::render::ViewportDrawDataSnapshot::new(
        imgui::Id::from(0xBAD),
        false,
        imgui::render::DrawDataSnapshot {
            frame_count: 0,
            display_pos: [0.0; 2],
            display_size: [32.0; 2],
            framebuffer_scale: [1.0; 2],
            draw_lists: vec![list],
        },
    )];
    let camera = Entity::from_raw_u32(7).unwrap();
    let targets = [camera_target_for_test(camera, None)];
    assert!(matches!(
        prepare_draw_data(&main, &viewports, &targets),
        Err(ImguiDiagnosticKind::TextureSamplerUnsupported)
    ));
}

#[test]
fn native_texture_sampler_snapshot_is_abandoned_before_upload_without_blocking_other_contexts() {
    use bevy_ecs::system::RunSystemOnce as _;

    let mut context_a = managed_context();
    install_standard_draw_callbacks_for_context(&mut context_a).unwrap();
    let consumer_a = context_a.create_detached_renderer_consumer().unwrap();
    let texture_a = register_test_texture(&mut context_a);
    unsafe {
        context_a
            .platform_io_mut()
            .set_draw_callback_set_sampler_from_texture_raw(Some(texture_sampler_callback));
    }
    let frame = context_a.begin_frame();
    frame.ui().image(texture_a, [16.0; 2]);
    unsafe {
        frame.ui().get_background_draw_list().add_callback(
            texture_sampler_callback,
            std::ptr::null_mut(),
            0,
        );
    }
    let snapshot_a = frame.render_snapshot(&consumer_a).unwrap();
    assert!(
        snapshot_a
            .draw_data()
            .draw_lists
            .iter()
            .flat_map(|list| &list.commands)
            .any(|command| matches!(command, DrawCmdSnapshot::SetSamplerFromTexture))
    );
    assert!(!snapshot_a.texture_requests().is_empty());
    let context_a_id = context_a.id();

    let mut context_b = managed_context();
    let consumer_b = context_b.create_detached_renderer_consumer().unwrap();
    let texture_b = register_test_texture(&mut context_b);
    let snapshot_b = managed_snapshot(&mut context_b, &consumer_b, Some(texture_b));
    let feedback_b = retry_texture_feedback(&snapshot_b);
    let context_b_id = context_b.id();

    let camera = Entity::from_raw_u32(7).unwrap();
    let second_camera = Entity::from_raw_u32(8).unwrap();
    let target = camera_target_for_test(camera, None);
    let mut extracted = ImguiExtractedRenderFrame::default();
    extracted.replace(context_a_id, pending_frame(1, snapshot_a), Vec::new());
    extracted.replace_camera_targets(
        context_a_id,
        vec![
            ImguiCameraTarget {
                context_id: context_a_id,
                ..target.clone()
            },
            ImguiCameraTarget {
                context_id: context_a_id,
                camera: second_camera,
                view: RetainedViewEntity::new(second_camera.into(), None, 0),
                ..target.clone()
            },
        ],
    );
    extracted.replace(context_b_id, pending_frame(1, snapshot_b), Vec::new());
    extracted.replace_camera_targets(
        context_b_id,
        vec![ImguiCameraTarget {
            context_id: context_b_id,
            ..target
        }],
    );
    extracted.extend_texture_feedback(context_b_id, feedback_b);
    let diagnostics = crate::route::ImguiDiagnostics::default();
    let mut world = World::new();
    world.insert_resource(extracted);
    world.insert_resource(ImguiPreparedRenderFrame::default());
    world.insert_resource(diagnostics.clone());
    world
        .run_system_once(prepare::prepare_imgui_render_frame)
        .unwrap();

    let extracted = world.resource::<ImguiExtractedRenderFrame>();
    assert!(extracted.snapshot(context_a_id).is_none());
    assert!(extracted.snapshot(context_b_id).is_some());
    let prepared = world.resource::<ImguiPreparedRenderFrame>();
    assert!(prepared.frame_index(context_a_id).is_none());
    assert_eq!(prepared.frame_index(context_b_id), Some(1));
    let entries = diagnostics
        .entries_for(crate::route::ImguiDiagnosticOrigin::RenderPreparation)
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].kind(),
        &crate::route::ImguiDiagnosticKind::TextureSamplerUnsupported
    );
    assert_eq!(entries[0].context_id(), Some(context_a_id));
    assert!(
        world
            .resource_mut::<ImguiExtractedRenderFrame>()
            .commit_all()
            .is_empty()
    );

    let progress_a = context_a.poll_snapshot_completions().unwrap();
    assert_eq!(progress_a.abandoned(), 1);
    assert_eq!(progress_a.committed(), 0);
    assert_eq!(progress_a.feedback_applied(), 0);
    let progress_b = context_b.poll_snapshot_completions().unwrap();
    assert_eq!(progress_b.abandoned(), 0);
    assert_eq!(progress_b.committed(), 1);
}

#[test]
fn standard_draw_callback_markers_keep_distinct_addresses_and_snapshot_classes() {
    let callbacks = [
        imgui_bevy_draw_callback_reset as RawDrawCallback,
        imgui_bevy_draw_callback_linear as RawDrawCallback,
        imgui_bevy_draw_callback_nearest as RawDrawCallback,
    ];
    for left in 0..callbacks.len() {
        for right in left + 1..callbacks.len() {
            assert!(
                !std::ptr::fn_addr_eq(callbacks[left], callbacks[right]),
                "standard callback markers must remain pairwise distinct after optimization"
            );
        }
    }

    let mut context = managed_context();
    install_standard_draw_callbacks_for_context(&mut context).unwrap();
    let consumer = context.create_detached_renderer_consumer().unwrap();
    context.prepare_frame(
        imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
    );
    let frame = context.begin_frame();
    {
        let draw_list = frame.ui().get_background_draw_list();
        draw_list.add_draw_cmd();
        for callback in callbacks {
            unsafe { draw_list.add_callback(callback, std::ptr::null_mut(), 0) };
        }
    }
    let snapshot = frame.render_snapshot(&consumer).unwrap();
    let classes = snapshot
        .draw_data()
        .draw_lists
        .iter()
        .flat_map(|list| list.commands.iter())
        .filter_map(|command| match command {
            DrawCmdSnapshot::ResetRenderState => Some("reset"),
            DrawCmdSnapshot::SetSamplerLinear => Some("linear"),
            DrawCmdSnapshot::SetSamplerNearest => Some("nearest"),
            DrawCmdSnapshot::SetSamplerFromTexture => Some("texture"),
            DrawCmdSnapshot::Elements { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(classes, ["reset", "linear", "nearest"]);
    drop(snapshot);
    context.poll_snapshot_completions().unwrap();
    context
        .prepare_renderer_texture_reset(&consumer)
        .unwrap()
        .commit();
}

#[test]
fn prepared_draws_preserve_standard_sampler_callback_state() {
    let camera = Entity::from_raw_u32(7).expect("test entity index should be valid");
    let draw = imgui::render::DrawDataSnapshot {
        frame_count: 0,
        display_pos: [0.0, 0.0],
        display_size: [32.0, 32.0],
        framebuffer_scale: [1.0, 1.0],
        draw_lists: vec![imgui::render::DrawListSnapshot {
            vtx: vec![
                imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
            ],
            idx: vec![0, 1, 2],
            commands: vec![
                DrawCmdSnapshot::SetSamplerNearest,
                DrawCmdSnapshot::Elements {
                    count: 3,
                    clip_rect: [0.0, 0.0, 16.0, 16.0],
                    texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                    vtx_offset: 0,
                    idx_offset: 0,
                },
                DrawCmdSnapshot::ResetRenderState,
                DrawCmdSnapshot::Elements {
                    count: 3,
                    clip_rect: [0.0, 0.0, 16.0, 16.0],
                    texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                    vtx_offset: 0,
                    idx_offset: 0,
                },
            ],
        }],
    };
    let targets = [camera_target_for_test(camera, None)];

    let draws = prepare_draw_data(&draw, &[], &targets).unwrap().draws;

    assert_eq!(draws.len(), 2);
    assert_eq!(draws[0].sampler, ImguiSampler::Nearest);
    assert_eq!(draws[1].sampler, ImguiSampler::Linear);
}

#[test]
fn prepared_draws_preserve_sampler_state_across_draw_lists() {
    let camera = Entity::from_raw_u32(8).expect("test entity index should be valid");
    let draw = imgui::render::DrawDataSnapshot {
        frame_count: 0,
        display_pos: [0.0, 0.0],
        display_size: [32.0, 32.0],
        framebuffer_scale: [1.0, 1.0],
        draw_lists: vec![
            imgui::render::DrawListSnapshot {
                vtx: vec![
                    imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
                ],
                idx: vec![0, 1, 2],
                commands: vec![
                    DrawCmdSnapshot::SetSamplerNearest,
                    DrawCmdSnapshot::Elements {
                        count: 3,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 0,
                        idx_offset: 0,
                    },
                ],
            },
            imgui::render::DrawListSnapshot {
                vtx: vec![
                    imgui::render::DrawVert::new([2.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([3.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([2.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
                ],
                idx: vec![0, 1, 2],
                commands: vec![DrawCmdSnapshot::Elements {
                    count: 3,
                    clip_rect: [0.0, 0.0, 16.0, 16.0],
                    texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                    vtx_offset: 0,
                    idx_offset: 0,
                }],
            },
        ],
    };
    let targets = [camera_target_for_test(camera, None)];

    let draws = prepare_draw_data(&draw, &[], &targets).unwrap().draws;

    assert_eq!(draws.len(), 2);
    assert_eq!(draws[0].sampler, ImguiSampler::Nearest);
    assert_eq!(draws[1].sampler, ImguiSampler::Nearest);
}

#[test]
fn prepared_draws_skip_commands_with_out_of_range_index_or_vertex_offsets() {
    let camera = Entity::from_raw_u32(9).expect("test entity index should be valid");
    let draw = imgui::render::DrawDataSnapshot {
        frame_count: 0,
        display_pos: [0.0, 0.0],
        display_size: [32.0, 32.0],
        framebuffer_scale: [1.0, 1.0],
        draw_lists: vec![imgui::render::DrawListSnapshot {
            vtx: vec![
                imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
            ],
            idx: vec![0, 1, 2, 3, 1, 2],
            commands: vec![
                DrawCmdSnapshot::Elements {
                    count: 1,
                    clip_rect: [0.0, 0.0, 16.0, 16.0],
                    texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                    vtx_offset: 0,
                    idx_offset: 6,
                },
                DrawCmdSnapshot::Elements {
                    count: 1,
                    clip_rect: [0.0, 0.0, 16.0, 16.0],
                    texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                    vtx_offset: 0,
                    idx_offset: 3,
                },
                DrawCmdSnapshot::Elements {
                    count: 3,
                    clip_rect: [0.0, 0.0, 16.0, 16.0],
                    texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                    vtx_offset: 4,
                    idx_offset: 3,
                },
                DrawCmdSnapshot::Elements {
                    count: 3,
                    clip_rect: [0.0, 0.0, 16.0, 16.0],
                    texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                    vtx_offset: 0,
                    idx_offset: 0,
                },
            ],
        }],
    };
    let targets = [camera_target_for_test(camera, None)];

    let draws = prepare_draw_data(&draw, &[], &targets).unwrap().draws;

    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].index_range, 0..3);
    assert_eq!(draws[0].vertex_offset, 0);
}

#[test]
fn draw_index_validation_rejects_absolute_indices_outside_uploaded_vertices() {
    assert!(!draw_indices_reference_out_of_bounds(&[0, 1, 2], 0, 3));
    assert!(!draw_indices_reference_out_of_bounds(&[0, 1, 2], 3, 6));
    assert!(draw_indices_reference_out_of_bounds(&[3], 0, 3));
    assert!(draw_indices_reference_out_of_bounds(&[0], 3, 3));
}

#[test]
fn prepared_scissor_projects_fractional_clips_and_rejects_non_finite_coordinates() {
    let targets = [camera_target_for_test(
        Entity::from_raw_u32(7).unwrap(),
        None,
    )];
    let mut draw = imgui::render::DrawDataSnapshot {
        frame_count: 0,
        display_pos: [10.0, 20.0],
        display_size: [2.0, 0.75],
        framebuffer_scale: [2.0, 4.0],
        draw_lists: vec![draw_list_for_test()],
    };
    let prepare = |draw: &mut imgui::render::DrawDataSnapshot, rect| {
        let DrawCmdSnapshot::Elements { clip_rect, .. } = &mut draw.draw_lists[0].commands[0]
        else {
            panic!("the test draw must contain an indexed command");
        };
        *clip_rect = rect;
        prepare_draw_data(draw, &[], &targets).unwrap().draws
    };
    let scissor = prepare(&mut draw, [9.9, 20.25, 12.1, 21.01])[0].scissor;
    assert_eq!(
        scissor,
        ImguiScissorRect {
            x: 0,
            y: 1,
            width: 4,
            height: 2
        }
    );
    assert!(prepare(&mut draw, [f32::NAN, 20.0, 12.0, 21.0]).is_empty());
    assert!(prepare(&mut draw, [11.0, 20.0, 11.0, 21.0]).is_empty());
    assert!(prepare(&mut draw, [10.0, 20.0, f32::MAX, 21.0]).is_empty());

    draw.display_size[0] = 0.0;
    assert!(prepare(&mut draw, [10.0, 20.0, 12.0, 21.0]).is_empty());
    draw.display_size[0] = 2.0;
    for scale in [f32::INFINITY, -1.0] {
        draw.framebuffer_scale[0] = scale;
        assert!(prepare(&mut draw, [10.0, 20.0, 12.0, 21.0]).is_empty());
    }
}

#[test]
fn render_pass_scissor_intersects_draws_with_camera_viewport_without_scaling() {
    let scissor = intersect_scissor_with_camera_viewport(
        ImguiScissorRect {
            x: 320,
            y: 180,
            width: 640,
            height: 360,
        },
        ImguiCameraViewport {
            physical_position: [640, 0],
            physical_size: [640, 360],
        },
    )
    .expect("valid scissor should map into a valid camera viewport");

    assert_eq!(
        scissor,
        ImguiScissorRect {
            x: 640,
            y: 180,
            width: 320,
            height: 180,
        }
    );
}

#[test]
fn render_pass_scissor_is_clamped_to_real_render_target_size() {
    let scissor = scissor_for_render_pass(
        &ImguiPreparedDraw {
            context_id: test_context_id(),
            route_epoch: 0,
            camera: Entity::from_raw_u32(13).expect("test entity index should be valid"),
            view: RetainedViewEntity::new(
                Entity::from_raw_u32(13)
                    .expect("test entity index should be valid")
                    .into(),
                None,
                0,
            ),
            order: 0,
            camera_order: 0,
            camera_schedule: Core2d.intern(),
            target: NormalizedRenderTarget::Window(
                bevy_window::WindowRef::Entity(
                    Entity::from_raw_u32(14).expect("test entity index should be valid"),
                )
                .normalize(None)
                .expect("entity window target should normalize"),
            ),
            target_format: TextureFormat::Rgba8UnormSrgb,
            texture_usages: TextureUsages::RENDER_ATTACHMENT,
            msaa: Msaa::Off,
            physical_target_size: [570, 390],
            viewport_id: Some(imgui::Id::from(0x570)),
            texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
            sampler: ImguiSampler::Linear,
            scissor: ImguiScissorRect {
                x: 0,
                y: 0,
                width: 570,
                height: 392,
            },
            framebuffer_size: [570, 392],
            camera_viewport: None,
            index_range: 0..3,
            vertex_offset: 0,
        },
        Some([570, 390]),
    )
    .expect("overlapping scissor should be clipped instead of rejected");

    assert_eq!(
        scissor,
        ImguiScissorRect {
            x: 0,
            y: 0,
            width: 570,
            height: 390,
        },
        "render pass scissors must never exceed the real WGPU target extent"
    );
}

#[test]
fn camera_viewport_uniforms_use_logical_viewport_rect_without_scaling_imgui_coordinates() {
    let camera = Entity::from_raw_u32(12).expect("test entity index should be valid");
    let draw = imgui::render::DrawDataSnapshot {
        frame_count: 0,
        display_pos: [0.0, 0.0],
        display_size: [640.0, 360.0],
        framebuffer_scale: [2.0, 2.0],
        draw_lists: vec![draw_list_for_test()],
    };
    let target = ImguiCameraTarget {
        camera_viewport: Some(ImguiCameraViewport {
            physical_position: [640, 0],
            physical_size: [640, 720],
        }),
        ..camera_target_for_test(camera, None)
    };

    let prepare::PreparedImguiDrawData {
        draws,
        uniforms_by_view,
        ..
    } = prepare_draw_data(&draw, &[], &[target]).unwrap();

    assert_eq!(draws.len(), 1);
    assert_eq!(
        draws[0].scissor,
        ImguiScissorRect {
            x: 0,
            y: 0,
            width: 32,
            height: 32,
        },
        "prepared draw scissors stay in source framebuffer coordinates"
    );
    assert_eq!(
        scissor_for_render_pass(&draws[0], None),
        None,
        "commands outside the camera viewport are clipped instead of scaled into it"
    );
    assert_eq!(
        uniforms_by_view
            .get(&RetainedViewEntity::new(camera.into(), None, 0))
            .copied(),
        Some(ImguiUniforms::from_display_rect(
            [320.0, 0.0],
            [320.0, 360.0]
        ))
    );
}

#[test]
fn prepared_draws_skip_only_viewports_with_invalid_display_rects() {
    let primary_camera = Entity::from_raw_u32(10).expect("test entity index should be valid");
    let secondary_camera = Entity::from_raw_u32(11).expect("test entity index should be valid");
    let secondary_viewport = imgui::Id::from(0xBEEF);
    let draw = imgui::render::DrawDataSnapshot {
        frame_count: 0,
        display_pos: [0.0, 0.0],
        display_size: [f32::NAN, 32.0],
        framebuffer_scale: [1.0, 1.0],
        draw_lists: vec![draw_list_for_test()],
    };
    let viewports = [imgui::render::ViewportDrawDataSnapshot::new(
        secondary_viewport,
        false,
        imgui::render::DrawDataSnapshot {
            frame_count: 0,
            display_pos: [0.0, 0.0],
            display_size: [32.0, 32.0],
            framebuffer_scale: [1.0, 1.0],
            draw_lists: vec![draw_list_for_test()],
        },
    )];
    let targets = [
        camera_target_for_test(primary_camera, None),
        camera_target_for_test(secondary_camera, Some(secondary_viewport)),
    ];

    let prepare::PreparedImguiDrawData {
        draws,
        uniforms_by_view,
        ..
    } = prepare_draw_data(&draw, &viewports, &targets).unwrap();

    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].camera, secondary_camera);
    assert!(!uniforms_by_view.contains_key(&RetainedViewEntity::new(
        primary_camera.into(),
        None,
        0
    )));
    assert!(uniforms_by_view.contains_key(&RetainedViewEntity::new(
        secondary_camera.into(),
        None,
        0
    )));
}

fn draw_list_for_test() -> imgui::render::DrawListSnapshot {
    imgui::render::DrawListSnapshot {
        vtx: vec![
            imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
            imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
            imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
        ],
        idx: vec![0, 1, 2],
        commands: vec![DrawCmdSnapshot::Elements {
            count: 3,
            clip_rect: [0.0, 0.0, 16.0, 16.0],
            texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
            vtx_offset: 0,
            idx_offset: 0,
        }],
    }
}

fn camera_target_for_test(camera: Entity, viewport_id: Option<imgui::Id>) -> ImguiCameraTarget {
    ImguiCameraTarget {
        context_id: test_context_id(),
        route_epoch: 0,
        camera,
        view: RetainedViewEntity::new(camera.into(), None, 0),
        order: 0,
        camera_order: 0,
        camera_schedule: Core2d.intern(),
        target: NormalizedRenderTarget::Window(
            bevy_window::WindowRef::Entity(camera)
                .normalize(None)
                .expect("entity window target should normalize"),
        ),
        target_format: TextureFormat::Rgba8UnormSrgb,
        texture_usages: TextureUsages::RENDER_ATTACHMENT,
        msaa: Msaa::Off,
        physical_target_size: [64, 64],
        viewport_id,
        camera_viewport: None,
    }
}

#[test]
fn presentable_output_scope_includes_every_imgui_window_target() {
    let application_window = Entity::from_raw_u32(41).expect("test entity index should be valid");
    let viewport_window = Entity::from_raw_u32(42).expect("test entity index should be valid");
    let targets = [
        camera_target_for_test(application_window, None),
        camera_target_for_test(viewport_window, Some(imgui::Id::from(7))),
    ];

    let outputs = imgui_target_windows(targets.iter());

    assert_eq!(
        outputs,
        HashSet::from([application_window, viewport_window])
    );
}
