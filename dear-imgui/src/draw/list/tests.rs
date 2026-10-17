use super::super::color::ImColor32;
use super::super::counts::{DrawFlags, DrawNgonSegmentCount, DrawSegmentCount};
use super::super::util::draw_list_counts;
use super::{DrawListMut, DrawListProvenance};
use crate::sys;

struct TestDrawList {
    shared: *mut sys::ImDrawListSharedData,
    raw: *mut sys::ImDrawList,
}

impl TestDrawList {
    fn new() -> Self {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw.is_null());
        Self { shared, raw }
    }

    fn draw_list(&self) -> DrawListMut<'static> {
        DrawListMut {
            draw_list: self.raw,
            ui: None,
            provenance: DrawListProvenance::Frame,
        }
    }

    fn borrowed_draw_list(&self) -> DrawListMut<'static> {
        DrawListMut::borrow_draw_list(self.raw);
        self.draw_list()
    }

    fn path_size(&self) -> i32 {
        unsafe { (*self.raw)._Path.Size }
    }

    fn clip_stack_size(&self) -> i32 {
        unsafe { (*self.raw)._ClipRectStack.Size }
    }
}

impl Drop for TestDrawList {
    fn drop(&mut self) {
        unsafe {
            sys::ImDrawList_destroy(self.raw);
            sys::ImDrawListSharedData_destroy(self.shared);
        }
    }
}

fn assert_panics_without_buffer_change(
    fixture: &TestDrawList,
    f: impl FnOnce(&DrawListMut<'static>),
) {
    let draw_list = fixture.draw_list();
    let before = draw_list_counts(fixture.raw);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&draw_list)));
    assert!(result.is_err());
    assert_eq!(draw_list_counts(fixture.raw), before);
}

#[test]
#[should_panic(
    expected = "A DrawListMut is already in use for this draw list; reuse the existing wrapper or drop it before acquiring another"
)]
fn draw_list_exclusive_use_guard_rejects_duplicate_wrapper() {
    let fixture = TestDrawList::new();
    let _first = fixture.borrowed_draw_list();
    let _duplicate = fixture.borrowed_draw_list();
}

#[test]
fn draw_list_exclusive_use_guard_allows_reacquisition_after_drop() {
    let fixture = TestDrawList::new();
    let first = fixture.borrowed_draw_list();
    drop(first);
    let reacquired = fixture.borrowed_draw_list();
    drop(reacquired);
}

#[test]
fn direct_draw_inputs_validate_before_ffi() {
    let fixture = TestDrawList::new();

    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_line_h(f32::NAN, 1.0, 0.0, ImColor32::WHITE, 1.0, DrawFlags::NONE);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_line_v(0.0, 0.0, 1.0, ImColor32::WHITE, 0.0, DrawFlags::NONE);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_quad(
            [f32::NAN, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            ImColor32::WHITE,
            1.0,
            DrawFlags::NONE,
        );
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_ellipse(
            [0.0, 0.0],
            [-1.0, 4.0],
            ImColor32::WHITE,
            0.0,
            DrawSegmentCount::AUTO,
            1.0,
            DrawFlags::NONE,
        );
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_bezier_quadratic(
            [0.0, 0.0],
            [1.0, 1.0],
            [2.0, 0.0],
            ImColor32::WHITE,
            1.0,
            DrawSegmentCount::count(0),
            DrawFlags::NONE,
        );
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_image_rounded(
            crate::texture::TextureId::new(1),
            [0.0, 0.0],
            [16.0, 16.0],
            [0.0, 0.0],
            [1.0, 1.0],
            ImColor32::WHITE,
            f32::INFINITY,
            DrawFlags::ROUND_ALL,
        );
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.path_rect(
            [0.0, 0.0],
            [1.0, 1.0],
            1.0,
            DrawFlags::from_bits_retain(sys::ImDrawFlags_Closed as u32),
        );
    });
}

#[test]
fn path_inputs_validate_before_path_mutation() {
    let fixture = TestDrawList::new();
    let draw_list = fixture.draw_list();

    draw_list.path_line_to([1.0, 1.0]);
    let path_size = fixture.path_size();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.path_line_to([f32::NAN, 2.0]);
    }));
    assert!(result.is_err());
    assert_eq!(fixture.path_size(), path_size);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.path_arc_to([0.0, 0.0], -1.0, 0.0, 1.0, DrawSegmentCount::AUTO);
    }));
    assert!(result.is_err());
    assert_eq!(fixture.path_size(), path_size);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.path_arc_to_fast([0.0, 0.0], 1.0, 0, 13);
    }));
    assert!(result.is_err());
    assert_eq!(fixture.path_size(), path_size);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.path_bezier_quadratic_curve_to(
            [2.0, 2.0],
            [3.0, 3.0],
            DrawSegmentCount::count(0),
        );
    }));
    assert!(result.is_err());
    assert_eq!(fixture.path_size(), path_size);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.path_stroke(ImColor32::WHITE, 1.0, DrawFlags::ROUND_TOP_LEFT);
    }));
    assert!(result.is_err());
    assert_eq!(fixture.path_size(), path_size);

    draw_list.path_clear();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.path_bezier_cubic_curve_to(
            [2.0, 2.0],
            [3.0, 3.0],
            [4.0, 4.0],
            DrawSegmentCount::AUTO,
        );
    }));
    assert!(result.is_err());
    assert_eq!(fixture.path_size(), 0);
}

#[test]
fn builder_inputs_validate_before_ffi() {
    let fixture = TestDrawList::new();

    assert_panics_without_buffer_change(&fixture, |draw_list| {
        let _ = draw_list
            .add_line([0.0, 0.0], [1.0, 1.0], ImColor32::WHITE)
            .thickness(0.0);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        let _ = draw_list.add_circle([0.0, 0.0], -1.0, ImColor32::WHITE);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        let _ = draw_list
            .add_rect([0.0, 0.0], [1.0, 1.0], ImColor32::WHITE)
            .rounding(f32::NAN);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        let _ = draw_list.add_polyline(vec![[0.0, 0.0], [f32::INFINITY, 1.0]], ImColor32::WHITE);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        let _ = draw_list
            .add_bezier_curve(
                [0.0, 0.0],
                [1.0, 1.0],
                [2.0, 1.0],
                [3.0, 0.0],
                ImColor32::WHITE,
            )
            .num_segments(DrawSegmentCount::count(i32::MAX as usize + 1));
    });
}

#[test]
fn draw_segment_counts_reject_invalid_values_before_ffi() {
    assert!(DrawNgonSegmentCount::new(2).is_none());
    assert!(DrawNgonSegmentCount::new(3).is_some());
    assert!(DrawNgonSegmentCount::new(i32::MAX as usize + 1).is_none());

    assert_eq!(DrawSegmentCount::AUTO.get(), None);
    assert_eq!(
        DrawSegmentCount::new(3).and_then(DrawSegmentCount::get),
        Some(3)
    );
    assert_eq!(DrawSegmentCount::new(0), None);
    assert_eq!(DrawSegmentCount::new(i32::MAX as usize + 1), None);

    assert!(std::panic::catch_unwind(|| DrawSegmentCount::count(0)).is_err());
    assert!(std::panic::catch_unwind(|| DrawSegmentCount::count(i32::MAX as usize + 1)).is_err());
}

#[test]
fn primitive_reserve_counts_reject_overflow_before_ffi() {
    let fixture = TestDrawList::new();

    assert_panics_without_buffer_change(&fixture, |draw_list| unsafe {
        draw_list.prim_reserve(i32::MAX as usize + 1, 0);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| unsafe {
        draw_list.prim_reserve(0, i32::MAX as usize + 1);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| unsafe {
        draw_list.prim_unreserve(i32::MAX as usize + 1, 0);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| unsafe {
        draw_list.prim_unreserve(0, i32::MAX as usize + 1);
    });
}

#[test]
fn raw_callback_copy_inputs_validate_before_ffi() {
    unsafe extern "C" fn callback(
        _parent_list: *const sys::ImDrawList,
        _command: *const sys::ImDrawCmd,
    ) {
    }

    let fixture = TestDrawList::new();
    assert_panics_without_buffer_change(&fixture, |draw_list| unsafe {
        draw_list.add_callback(callback, std::ptr::null_mut(), 1);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| unsafe {
        draw_list.add_callback(callback, std::ptr::dangling_mut(), 1usize << 31);
    });
}

#[test]
fn raw_callback_data_modes_preserve_borrow_or_copy_semantics() {
    unsafe extern "C" fn callback(
        _parent_list: *const sys::ImDrawList,
        _command: *const sys::ImDrawCmd,
    ) {
    }

    let fixture = TestDrawList::new();
    let draw_list = fixture.draw_list();
    draw_list.add_draw_cmd();

    let mut borrowed = 41_u8;
    let borrowed_ptr = std::ptr::from_mut(&mut borrowed).cast();
    unsafe {
        draw_list.add_callback(callback, borrowed_ptr, 0);
    }

    let mut copied = [3_u8, 1, 4, 1, 5];
    unsafe {
        draw_list.add_callback(callback, copied.as_mut_ptr().cast(), copied.len());

        let commands = std::slice::from_raw_parts(
            (*fixture.raw).CmdBuffer.Data,
            usize::try_from((*fixture.raw).CmdBuffer.Size).unwrap(),
        );
        assert_eq!(commands[0].UserCallbackData, borrowed_ptr);
        assert_eq!(commands[0].UserCallbackDataSize, 0);
        assert_eq!(commands[0].UserCallbackDataOffset, -1);

        assert!(commands[1].UserCallbackData.is_null());
        assert_eq!(commands[1].UserCallbackDataSize, copied.len() as i32);
        assert_eq!(commands[1].UserCallbackDataOffset, 0);
        let copied_storage = std::slice::from_raw_parts(
            (*fixture.raw)._CallbacksDataBuf.Data,
            usize::try_from((*fixture.raw)._CallbacksDataBuf.Size).unwrap(),
        );
        assert_eq!(copied_storage, copied);
    }
}

#[test]
fn standard_sampler_commands_are_safe_and_preserve_order() {
    unsafe extern "C" fn linear(
        _parent_list: *const sys::ImDrawList,
        _command: *const sys::ImDrawCmd,
    ) {
    }
    unsafe extern "C" fn nearest(
        _parent_list: *const sys::ImDrawList,
        _command: *const sys::ImDrawCmd,
    ) {
    }

    unsafe extern "C" fn from_texture(
        _parent_list: *const sys::ImDrawList,
        _command: *const sys::ImDrawCmd,
    ) {
    }

    let mut context = crate::Context::create();
    let binding = context.binding();
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    unsafe {
        context
            .platform_io_mut()
            .set_draw_callback_set_sampler_linear_raw(Some(linear));
        context
            .platform_io_mut()
            .set_draw_callback_set_sampler_nearest_raw(Some(nearest));
        context
            .platform_io_mut()
            .set_draw_callback_set_sampler_from_texture_raw(Some(from_texture));
    }

    {
        let ui = context.frame();
        ui.window("standard sampler commands")
            .size([96.0, 96.0], crate::Condition::Always)
            .build(|| {
                let draw_list = ui.get_window_draw_list();
                draw_list.set_sampler_nearest();
                draw_list
                    .add_rect([8.0, 8.0], [24.0, 24.0], ImColor32::WHITE)
                    .filled(true)
                    .build();
                draw_list.set_sampler_linear();
                draw_list.set_sampler_from_texture();
                draw_list
                    .add_rect([32.0, 8.0], [48.0, 24.0], ImColor32::WHITE)
                    .filled(true)
                    .build();
            });
    }
    let frame = context.render_legacy();
    let commands = binding.with_bound_context(|| {
        frame
            .draw_data()
            .draw_lists()
            .flat_map(|list| list.commands())
            .filter(|command| {
                matches!(
                    command,
                    crate::render::DrawCmd::SetSamplerLinear
                        | crate::render::DrawCmd::SetSamplerNearest
                        | crate::render::DrawCmd::SetSamplerFromTexture
                )
            })
            .collect::<Vec<_>>()
    });

    assert_eq!(commands.len(), 3);
    assert!(matches!(
        commands[0],
        crate::render::DrawCmd::SetSamplerNearest
    ));
    assert!(matches!(
        commands[1],
        crate::render::DrawCmd::SetSamplerLinear
    ));
    assert!(matches!(
        commands[2],
        crate::render::DrawCmd::SetSamplerFromTexture
    ));
}

#[test]
fn standard_sampler_commands_validate_backend_support_before_ffi() {
    let mut context = crate::Context::create();
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();

    let ui = context.frame();
    let draw_list = ui.get_window_draw_list();
    let before = draw_list_counts(draw_list.draw_list);
    let commands_before = unsafe { (*draw_list.draw_list).CmdBuffer.Size };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.set_sampler_nearest();
    }));

    assert!(result.is_err());
    assert_eq!(draw_list_counts(draw_list.draw_list), before);
    assert_eq!(
        unsafe { (*draw_list.draw_list).CmdBuffer.Size },
        commands_before
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.set_sampler_from_texture();
    }));
    assert!(result.is_err());
    assert_eq!(draw_list_counts(draw_list.draw_list), before);
    assert_eq!(
        unsafe { (*draw_list.draw_list).CmdBuffer.Size },
        commands_before
    );
}

#[test]
fn text_and_clip_inputs_validate_before_ffi() {
    let mut ctx = crate::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([128.0, 128.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    let ui = ctx.frame();
    let draw_list = ui.get_window_draw_list();
    let raw_draw_list = draw_list.draw_list;
    let font = ui.current_font();
    let before = draw_list_counts(raw_draw_list);
    let clip_stack_size = unsafe { (*raw_draw_list)._ClipRectStack.Size };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.add_text([f32::NAN, 0.0], ImColor32::WHITE, "hello");
    }));
    assert!(result.is_err());
    assert_eq!(draw_list_counts(raw_draw_list), before);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.add_text_with_font(font, -1.0, [0.0, 0.0], ImColor32::WHITE, "hello", 0.0, None);
    }));
    assert!(result.is_err());
    assert_eq!(draw_list_counts(raw_draw_list), before);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = draw_list.push_clip_rect([f32::NAN, 0.0], [1.0, 1.0], false);
    }));
    assert!(result.is_err());
    assert_eq!(
        unsafe { (*raw_draw_list)._ClipRectStack.Size },
        clip_stack_size
    );
}

#[test]
fn raw_draw_list_clip_helper_reads_stack_without_mutation() {
    let fixture = TestDrawList::new();
    let before = fixture.clip_stack_size();
    let draw_list = fixture.draw_list();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = draw_list.push_clip_rect([0.0, 0.0], [f32::INFINITY, 1.0], false);
    }));

    assert!(result.is_err());
    assert_eq!(fixture.clip_stack_size(), before);
}

#[test]
fn text_no_pixel_snap_scope_restores_previous_native_flags() {
    let fixture = TestDrawList::new();
    let draw_list = fixture.draw_list();
    unsafe {
        (*fixture.raw).Flags = DrawFlags::AA_FILL.bits() as sys::ImDrawFlags;
    }

    draw_list.with_text_no_pixel_snap(|| {
        let flags = DrawFlags::from_bits_retain(unsafe { (*fixture.raw).Flags } as u32);
        assert!(flags.contains(DrawFlags::TEXT_NO_PIXEL_SNAP));
        assert!(flags.contains(DrawFlags::AA_FILL));

        unsafe {
            (*fixture.raw).Flags |= DrawFlags::USE_VTX_OFFSET.bits() as sys::ImDrawFlags;
        }
    });

    let flags = DrawFlags::from_bits_retain(unsafe { (*fixture.raw).Flags } as u32);
    assert!(!flags.contains(DrawFlags::TEXT_NO_PIXEL_SNAP));
    assert!(flags.contains(DrawFlags::AA_FILL));
    assert!(!flags.contains(DrawFlags::USE_VTX_OFFSET));
}

#[test]
fn text_no_pixel_snap_scope_is_nested_and_panic_safe() {
    let fixture = TestDrawList::new();
    let draw_list = fixture.draw_list();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.with_text_no_pixel_snap(|| {
            assert!(
                DrawFlags::from_bits_retain(unsafe { (*fixture.raw).Flags } as u32)
                    .contains(DrawFlags::TEXT_NO_PIXEL_SNAP)
            );
            draw_list.with_text_no_pixel_snap(|| {
                assert!(
                    DrawFlags::from_bits_retain(unsafe { (*fixture.raw).Flags } as u32)
                        .contains(DrawFlags::TEXT_NO_PIXEL_SNAP)
                );
                panic!("forced panic inside text flag scope");
            });
        });
    }));

    assert!(result.is_err());
    assert!(
        !DrawFlags::from_bits_retain(unsafe { (*fixture.raw).Flags } as u32)
            .contains(DrawFlags::TEXT_NO_PIXEL_SNAP)
    );
}

#[test]
fn draw_flag_scopes_restore_nested_defaults_and_reject_overflow() {
    let fixture = TestDrawList::new();
    let draw_list = fixture.draw_list();
    let initial = draw_list.flags();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.with_draw_flags(DrawFlags::AA_LINES, true, || {
            assert!(draw_list.flags().contains(DrawFlags::AA_LINES));
            draw_list.with_draw_flags(DrawFlags::AA_LINES, false, || {
                assert!(!draw_list.flags().contains(DrawFlags::AA_LINES));
                draw_list.with_draw_flags(DrawFlags::AA_FILL, true, || {
                    assert_eq!(unsafe { (*fixture.raw)._DrawFlagsStack.Size }, 3);
                    draw_list.with_draw_flags(DrawFlags::TEXT_NO_PIXEL_SNAP, true, || {});
                });
            });
        });
    }));
    assert!(result.is_err());
    assert_eq!(draw_list.flags(), initial);
    assert_eq!(unsafe { (*fixture.raw)._DrawFlagsStack.Size }, 0);
}

#[test]
fn draw_flag_scopes_reject_inherited_rounding_conflicts_without_mutation() {
    let fixture = TestDrawList::new();
    let draw_list = fixture.draw_list();
    let initial = draw_list.flags();
    for (outer, inner) in [
        (DrawFlags::ROUND_TOP, DrawFlags::ROUND_NONE),
        (DrawFlags::ROUND_NONE, DrawFlags::ROUND_TOP),
    ] {
        draw_list.with_draw_flags(outer, true, || {
            let outer_flags = draw_list.flags();
            let stack_depth = unsafe { (*fixture.raw)._DrawFlagsStack.Size };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                draw_list.with_draw_flags(inner, true, || {});
            }));
            assert!(result.is_err());
            assert_eq!(draw_list.flags(), outer_flags);
            assert_eq!(unsafe { (*fixture.raw)._DrawFlagsStack.Size }, stack_depth);

            draw_list.with_draw_flags(outer, false, || {
                draw_list.with_draw_flags(inner, true, || {
                    assert!(draw_list.flags().contains(inner));
                    assert!(!draw_list.flags().intersects(outer));
                });
            });
            assert_eq!(draw_list.flags(), outer_flags);
        });
        assert_eq!(draw_list.flags(), initial);
        assert_eq!(unsafe { (*fixture.raw)._DrawFlagsStack.Size }, 0);
    }
}

#[test]
fn draw_flags_reject_invalid_modes_before_native_mutation() {
    let fixture = TestDrawList::new();
    let draw_list = fixture.draw_list();
    for flags in [
        DrawFlags::CLOSED,
        DrawFlags::STROKE_INSIDE,
        DrawFlags::ROUND_NONE | DrawFlags::ROUND_TOP_LEFT,
        DrawFlags::from_bits_retain(1),
    ] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.with_draw_flags(flags, true, || {});
        }));
        assert!(result.is_err());
        assert_eq!(unsafe { (*fixture.raw)._DrawFlagsStack.Size }, 0);
    }
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        let _ = draw_list
            .add_line([0.0, 0.0], [1.0, 1.0], ImColor32::WHITE)
            .flags(DrawFlags::STROKE_INSIDE | DrawFlags::STROKE_OUTSIDE);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        let _ = draw_list
            .add_circle([10.0, 10.0], 4.0, ImColor32::WHITE)
            .flags(DrawFlags::AA_LINES);
    });
}

fn appended_vertex_bounds(draw_list: &DrawListMut<'_>, draw: impl FnOnce()) -> [f32; 4] {
    let start = unsafe { (*draw_list.draw_list).VtxBuffer.Size } as usize;
    draw();
    let vertices = unsafe {
        let buffer = &(*draw_list.draw_list).VtxBuffer;
        assert!(buffer.Size as usize > start);
        std::slice::from_raw_parts(buffer.Data.add(start), buffer.Size as usize - start)
    };
    vertices.iter().fold(
        [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        |bounds, vertex| {
            [
                bounds[0].min(vertex.pos.x),
                bounds[1].min(vertex.pos.y),
                bounds[2].max(vertex.pos.x),
                bounds[3].max(vertex.pos.y),
            ]
        },
    )
}

#[test]
fn native_stroke_geometry_uses_193_positions_and_explicit_legacy_mode() {
    let mut context = crate::Context::create();
    context.io_mut().set_display_size([256.0, 256.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = context.frame();
    let draw_list = ui.get_background_draw_list();
    draw_list.with_draw_flags(DrawFlags::AA_LINES, false, || {
        let line_bounds = appended_vertex_bounds(&draw_list, || {
            draw_list
                .add_line([20.0, 20.0], [80.0, 20.0], ImColor32::WHITE)
                .thickness(4.0)
                .build();
        });
        assert_eq!(line_bounds, [20.0, 18.0, 80.0, 22.0]);
        assert_eq!(
            appended_vertex_bounds(&draw_list, || {
                draw_list
                    .add_line([20.0, 20.0], [80.0, 20.0], ImColor32::WHITE)
                    .thickness(4.0)
                    .flags(DrawFlags::CAP_SQUARE)
                    .build();
            }),
            [18.0, 18.0, 82.0, 22.0]
        );
        let legacy_bounds = appended_vertex_bounds(&draw_list, || {
            draw_list
                .add_line([20.0, 20.0], [80.0, 20.0], ImColor32::WHITE)
                .thickness(4.0)
                .flags(DrawFlags::STROKE_LEGACY)
                .build();
        });
        assert_eq!(
            legacy_bounds,
            line_bounds.map(|coordinate| coordinate + 0.5)
        );

        for (flags, bounds) in [
            (DrawFlags::NONE, [20.0, 20.0, 80.0, 80.0]),
            (DrawFlags::STROKE_CENTER, [18.0, 18.0, 82.0, 82.0]),
            (DrawFlags::STROKE_OUTSIDE, [16.0, 16.0, 84.0, 84.0]),
        ] {
            assert_eq!(
                appended_vertex_bounds(&draw_list, || {
                    draw_list
                        .add_rect([20.0, 20.0], [80.0, 80.0], ImColor32::WHITE)
                        .thickness(4.0)
                        .flags(flags)
                        .build();
                }),
                bounds
            );
        }
        assert_eq!(
            appended_vertex_bounds(&draw_list, || {
                draw_list.add_line_h(20.0, 80.0, 20.0, ImColor32::WHITE, 4.0, DrawFlags::NONE);
            }),
            [20.0, 20.0, 80.0, 24.0]
        );
        assert_eq!(
            appended_vertex_bounds(&draw_list, || {
                draw_list
                    .add_circle([50.0, 50.0], 20.0, ImColor32::WHITE)
                    .num_segments(DrawSegmentCount::count(4))
                    .thickness(4.0)
                    .build();
            }),
            [30.0, 30.0, 70.0, 70.0]
        );
    });
}

#[test]
fn native_line_end_antialiasing_adds_geometry_only_for_the_scoped_draws() {
    let mut context = crate::Context::create();
    context.io_mut().set_display_size([256.0, 256.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = context.frame();
    let draw_list = ui.get_background_draw_list();
    let draw = || {
        let before = draw_list_counts(draw_list.draw_list);
        draw_list
            .add_line([20.0, 20.0], [80.0, 20.0], ImColor32::WHITE)
            .thickness(4.0)
            .build();
        let after = draw_list_counts(draw_list.draw_list);
        (after.0 - before.0, after.1 - before.1)
    };
    let without_ends = draw();
    let with_ends = draw_list.with_draw_flags(DrawFlags::AA_LINE_ENDS, true, draw);
    assert!(with_ends.0 > without_ends.0);
    assert!(with_ends.1 > without_ends.1);
    assert_eq!(draw(), without_ends);
}

#[test]
fn draw_flag_scopes_reject_stale_window_views_before_mutation() {
    let mut context = crate::Context::create();
    context.io_mut().set_display_size([256.0, 256.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui: &crate::Ui = context.frame();
    let draw_list = ui
        .window("draw flag stale provenance")
        .build(|| ui.get_window_draw_list())
        .unwrap();
    let raw = draw_list.draw_list;
    let initial_flags = unsafe { (*raw).Flags };
    let initial_depth = unsafe { (*raw)._DrawFlagsStack.Size };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = draw_list.flags();
    }));
    assert!(result.is_err());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.with_draw_flags(DrawFlags::AA_LINE_ENDS, true, || {});
    }));
    assert!(result.is_err());
    assert_eq!(unsafe { (*raw).Flags }, initial_flags);
    assert_eq!(unsafe { (*raw)._DrawFlagsStack.Size }, initial_depth);
}

#[test]
fn draw_flag_scope_window_order_violations_recover_without_double_panic() {
    let mut context = crate::Context::create();
    context.io_mut().set_display_size([256.0, 256.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = context.frame();

    let tooltip = ui.begin_tooltip().unwrap();
    let draw_list = ui.get_window_draw_list();
    let raw = draw_list.draw_list;
    let initial_flags = unsafe { (*raw).Flags };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_list.with_draw_flags(DrawFlags::AA_LINE_ENDS, true, || drop(tooltip));
    }));
    assert!(result.is_err());
    assert_eq!(unsafe { (*raw).Flags }, initial_flags);
    assert_eq!(unsafe { (*raw)._DrawFlagsStack.Size }, 0);
    drop(draw_list);
    ui.text("window end recovered after the flag scope");

    let draw_list = ui.get_window_draw_list();
    let raw = draw_list.draw_list;
    let initial_flags = unsafe { (*raw).Flags };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = draw_list.with_draw_flags(DrawFlags::AA_LINE_ENDS, true, || {
            ui.begin_tooltip().unwrap()
        });
    }));
    assert!(result.is_err());
    assert_eq!(unsafe { (*raw).Flags }, initial_flags);
    assert_eq!(unsafe { (*raw)._DrawFlagsStack.Size }, 0);
    ui.text("flag scope recovered after the inner tooltip ended");
}

#[test]
fn draw_flag_scopes_bind_the_owner_context_and_restore_the_previous_context() {
    let mut context = crate::Context::create();
    let owner = unsafe { sys::igGetCurrentContext() };
    context.io_mut().set_display_size([256.0, 256.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = context.frame();
    let draw_list = ui.get_background_draw_list();
    let initial = draw_list.flags();
    let foreign = unsafe { sys::igCreateContext(std::ptr::null_mut()) };
    assert!(!foreign.is_null());
    unsafe { sys::igSetCurrentContext(foreign) };

    assert_eq!(draw_list.flags(), initial);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, foreign);
    draw_list.with_draw_flags(DrawFlags::AA_LINE_ENDS, true, || {
        assert!(draw_list.flags().contains(DrawFlags::AA_LINE_ENDS));
        assert_eq!(unsafe { sys::igGetCurrentContext() }, foreign);
    });
    assert_eq!(draw_list.flags(), initial);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, foreign);
    assert_eq!(unsafe { (*draw_list.draw_list)._DrawFlagsStack.Size }, 0);

    unsafe {
        sys::igSetCurrentContext(owner);
        sys::igDestroyContext(foreign);
    }
}
