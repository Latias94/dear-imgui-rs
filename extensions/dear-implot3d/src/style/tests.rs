#[cfg(test)]
mod tests {
    use crate::style::{
        Colormap, ColormapColorIndex, ColormapIndex, Plot3DItemArrayStyle, colormap,
        with_scoped_next_plot3d_item_array_style,
    };

    #[test]
    fn colormap_indices_reject_negative_values() {
        assert_eq!(ColormapIndex::from_raw(-1), None);
        assert_eq!(ColormapIndex::new(0).map(ColormapIndex::raw), Some(0));
        assert_eq!(ColormapIndex::new(0).map(ColormapIndex::get), Some(0));
        assert_eq!(ColormapIndex::new(i32::MAX as usize + 1), None);
        assert_eq!(
            ColormapIndex::from(Colormap::Viridis).raw(),
            crate::sys::ImPlot3DColormap_Viridis
        );

        assert_eq!(
            ColormapColorIndex::new(0).map(ColormapColorIndex::get),
            Some(0)
        );
        assert_eq!(
            ColormapColorIndex::from_usize(i32::MAX as usize).map(ColormapColorIndex::raw),
            Some(i32::MAX)
        );
        assert_eq!(ColormapColorIndex::from_usize(i32::MAX as usize + 1), None);
    }

    #[test]
    #[should_panic(expected = "test returned a negative colormap count")]
    fn colormap_count_conversion_rejects_negative_ffi_values() {
        let _ = colormap::colormap_count_from_i32(-1, "test");
    }

    #[test]
    fn next_plot3d_item_array_style_is_consumed_by_next_spec() {
        let line_colors = [0x01020304u32, 0x05060708];
        let marker_sizes = [1.5f32, 2.5];
        let marker_fill_colors = [0x11223344u32];

        with_scoped_next_plot3d_item_array_style(
            Plot3DItemArrayStyle::new()
                .with_line_colors(&line_colors)
                .with_marker_sizes(&marker_sizes)
                .with_marker_fill_colors(&marker_fill_colors),
            || {
                let spec = crate::plot3d_spec_from(
                    9,
                    crate::Plot3DDataLayout::new(
                        crate::Plot3DDataOffset::samples(2),
                        crate::Plot3DDataStride::bytes(24),
                    ),
                );
                assert_eq!(spec.Flags, 9);
                assert_eq!(spec.Offset, 2);
                assert_eq!(spec.Stride, 24);
                assert_eq!(spec.LineColors, line_colors.as_ptr() as *mut _);
                assert_eq!(spec.MarkerSizes, marker_sizes.as_ptr() as *mut _);
                assert_eq!(spec.MarkerFillColors, marker_fill_colors.as_ptr() as *mut _);
            },
        );

        let spec = crate::plot3d_spec_from(0, crate::Plot3DDataLayout::DEFAULT);
        assert!(spec.LineColors.is_null());
        assert!(spec.MarkerSizes.is_null());
        assert!(spec.MarkerFillColors.is_null());
    }

    #[test]
    fn next_plot3d_item_array_style_is_restored_if_unused() {
        let fill_colors = [0xAABBCCDDu32];

        with_scoped_next_plot3d_item_array_style(
            Plot3DItemArrayStyle::new().with_fill_colors(&fill_colors),
            || {},
        );

        let spec = crate::plot3d_spec_from(0, crate::Plot3DDataLayout::DEFAULT);
        assert!(spec.FillColors.is_null());
    }

    #[test]
    fn next_plot3d_item_array_style_is_restored_if_closure_panics() {
        crate::set_next_plot3d_spec(None);
        let line_colors = [0xAABBCCDDu32];

        let result = std::panic::catch_unwind(|| {
            with_scoped_next_plot3d_item_array_style(
                Plot3DItemArrayStyle::new().with_line_colors(&line_colors),
                || panic!("boom"),
            );
        });

        assert!(result.is_err());
        assert!(crate::take_next_plot3d_spec().is_none());
    }
}
