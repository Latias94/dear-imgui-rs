use super::super::color::ImColor32;
use super::super::counts::{
    DrawCornerFlags, DrawNgonSegmentCount, DrawSegmentCount, PolylineFlags,
};
use super::super::util::draw_list_counts;
use super::DrawListMut;
use crate::sys;
use std::marker::PhantomData;

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
            _phantom: PhantomData,
        }
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
fn direct_draw_inputs_validate_before_ffi() {
    let fixture = TestDrawList::new();

    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_line_h(f32::NAN, 1.0, 0.0, ImColor32::WHITE, 1.0);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_line_v(0.0, 0.0, 1.0, ImColor32::WHITE, 0.0);
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.add_quad(
            [f32::NAN, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            ImColor32::WHITE,
            1.0,
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
            DrawCornerFlags::ALL,
        );
    });
    assert_panics_without_buffer_change(&fixture, |draw_list| {
        draw_list.path_rect(
            [0.0, 0.0],
            [1.0, 1.0],
            1.0,
            DrawCornerFlags::from_bits_retain(sys::ImDrawFlags_Closed as u32),
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
        draw_list.path_stroke(
            ImColor32::WHITE,
            PolylineFlags::from_bits_retain(sys::ImDrawFlags_RoundCornersTopLeft as u32),
            1.0,
        );
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
fn text_and_clip_inputs_validate_before_ffi() {
    let mut ctx = crate::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([128.0, 128.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
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
