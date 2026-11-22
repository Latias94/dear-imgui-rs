#![cfg(not(target_arch = "wasm32"))]

use dear_imgui_rs::Context;
use dear_implot3d::{
    Line3DFlags, Mesh3D, Plot3D, Plot3DCond, Plot3DContext, Plot3DDataLayout, Plot3DDataOffset,
    Plot3DDataStride, Plot3DError, Plot3DUi, Quad3DFlags, Quads3D, Surface3D, Surface3DFlags,
    Triangle3DFlags, Triangles3D,
};
use dear_implot3d_sys::{ImDrawVert, surface_test_probe::dear_implot3d_probe_vertices};
use std::sync::Mutex;

fn appended_vertices<const N: usize>(
    plot: &Plot3DContext,
    submit: impl FnOnce(),
) -> [ImDrawVert; N] {
    // The plot is open, and the second call writes at most N vertices.
    let raw = unsafe { plot.raw() };
    let start = unsafe { dear_implot3d_probe_vertices(raw, 0, std::ptr::null_mut(), 0) };
    assert!(start >= 0);
    submit();
    let mut vertices = [ImDrawVert::default(); N];
    let count =
        unsafe { dear_implot3d_probe_vertices(raw, start, vertices.as_mut_ptr(), N as i32) };
    assert_eq!(count, N as i32);
    vertices
}

#[test]
fn strided_surface_uses_only_coordinate_fields_and_wraps_the_vertex_offset() {
    with_plot(|plot, ui| {
        let points = [
            [0.2, 0.2, 0.3, f32::NAN],
            [0.8, 0.2, 0.5, f32::NAN],
            [0.2, 0.8, 0.6, f32::NAN],
            [0.8, 0.8, 0.4, f32::NAN],
        ];
        let pixels = points.map(|[x, y, z, _]| ui.plot_to_pixels([x, y, z]));
        let data = points.as_flattened();
        let layout = Plot3DDataLayout::new(
            Plot3DDataOffset::samples(-1),
            Plot3DDataStride::for_type::<[f32; 4]>(),
        );
        let vertices = appended_vertices::<4>(plot, || unsafe {
            // Each slice covers its field in all four rows, including the last strided read.
            ui.surface_f32_raw(
                "strided",
                data,
                &data[1..],
                &data[2..],
                2,
                2,
                0.0,
                0.0,
                Surface3DFlags::NO_LINES | Surface3DFlags::NO_MARKERS,
                layout,
            )
            .unwrap();
        });
        // The quad visits grid indices 0, 1, 3, 2 after applying the circular offset.
        assert_eq!(
            vertices.map(|v| [v.pos.x, v.pos.y]),
            [pixels[3], pixels[0], pixels[2], pixels[1]]
        );
    });
}

fn with_plot(test: impl FnOnce(&Plot3DContext, &Plot3DUi<'_>)) {
    static GUARD: Mutex<()> = Mutex::new(());
    let _guard = GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut imgui = Context::create();
    imgui.io_mut().set_display_size([800.0, 600.0]);
    imgui.io_mut().set_delta_time(1.0 / 60.0);
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let plot = Plot3DContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = plot.get_plot_ui(frame.ui());
    let _token = ui.begin_plot("primitive submission").build().unwrap();
    ui.setup_axes_limits(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, Plot3DCond::Always);
    test(&plot, &ui);
}

#[test]
fn native_strip_joints_and_packed_face_offsets_match_projected_geometry() {
    with_plot(|plot, ui| {
        let points = [
            [0.1, 0.2, 0.3],
            [0.4, 0.3, 0.4],
            [0.8, 0.4, 0.6],
            [0.1, 0.8, 0.3],
        ];
        let pixels = points.map(|p| ui.plot_to_pixels(p));
        assert_ne!(pixels[0], pixels[1]);
        let xs = points.map(|p| p[0]);
        let ys = points.map(|p| p[1]);
        let zs = points.map(|p| p[2]);
        let strip = appended_vertices::<8>(plot, || {
            ui.set_next_line_style([1.0; 4], 4.0);
            ui.plot_line_f32("strip", &xs[..3], &ys[..3], &zs[..3], Line3DFlags::NONE);
        });
        assert_eq!(strip[1].pos, strip[4].pos);
        assert_eq!(strip[2].pos, strip[7].pos);
        assert_ne!(strip[1].pos, strip[2].pos);

        let doubles = points.map(|p| p.map(f64::from));
        let offset = Plot3DDataOffset::samples(-1);
        let triangle = Triangle3DFlags::NO_LINES | Triangle3DFlags::NO_MARKERS;
        for shape in [
            Triangles3D::points_f32("t32", &points[..3]),
            Triangles3D::points_f64("t64", &doubles[..3]),
        ] {
            let vertices = appended_vertices::<3>(plot, || {
                shape.flags(triangle).offset(offset).try_plot(ui).unwrap()
            })
            .map(|vertex| [vertex.pos.x, vertex.pos.y]);
            assert_eq!(vertices, [pixels[2], pixels[0], pixels[1]]);
        }
        let quad = Quad3DFlags::NO_LINES | Quad3DFlags::NO_MARKERS;
        for shape in [
            Quads3D::points_f32("q32", &points),
            Quads3D::points_f64("q64", &doubles),
        ] {
            let vertices = appended_vertices::<4>(plot, || {
                shape.flags(quad).offset(offset).try_plot(ui).unwrap()
            })
            .map(|vertex| [vertex.pos.x, vertex.pos.y]);
            assert_eq!(vertices, [pixels[3], pixels[0], pixels[1], pixels[2]]);
        }

        let short = &points[..2];
        let indices = [0, 1, 2];
        let expected = Err(Plot3DError::MeshIndexOutOfBounds {
            index: 2,
            vertex_count: 2,
        });
        assert_eq!(Mesh3D::new("mesh", short, &indices).try_plot(ui), expected);
        assert_eq!(ui.mesh("builder", short, &indices).try_plot(), expected);
        assert!(matches!(
            ui.mesh("incomplete", &points, &[0, 1]).try_plot(),
            Err(Plot3DError::NotMultipleOf { .. })
        ));
    });
}

#[test]
fn automatic_surface_scale_maps_heights_to_vertex_colors() {
    with_plot(|plot, ui| {
        let axis = [0.2, 0.8];
        let heights = [0.2, 0.8, 0.8, 0.2];
        let flags = Surface3DFlags::NO_LINES | Surface3DFlags::NO_MARKERS;
        let surface = Surface3D::new("surface", &axis, &axis, &heights).flags(flags);
        let colors = appended_vertices::<4>(plot, || surface.try_plot(ui).unwrap()).map(|v| v.col);
        let [low, high, _, _] = colors;
        assert_ne!(low, high);
        assert_eq!(colors, [low, high, low, high]);

        let builder = ui
            .surface_f32("builder", &axis, &axis, &heights)
            .flags(flags);
        let built = appended_vertices::<4>(plot, || builder.plot().unwrap());
        assert_eq!(built.map(|v| v.col), colors);

        let flat = appended_vertices::<4>(plot, || {
            ui.surface_f32_flat(
                "flat",
                &[0.2, 0.8, 0.2, 0.8],
                &[0.2, 0.2, 0.8, 0.8],
                &heights,
                2,
                2,
                f64::NAN,
                f64::NAN,
                flags,
            )
            .unwrap();
        });
        assert_eq!(flat.map(|v| v.col), colors);

        let reversed = appended_vertices::<4>(plot, || {
            surface
                .scale(f64::from(axis[1]), f64::from(axis[0]))
                .try_plot(ui)
                .unwrap();
        });
        assert_eq!(reversed.map(|v| v.col), [high, low, high, low]);

        assert_eq!(
            ui.surface_f32("invalid scale", &axis, &axis, &heights)
                .scale(0.0, f64::INFINITY)
                .plot(),
            Err(Plot3DError::InvalidSurfaceScale)
        );
    });
}
