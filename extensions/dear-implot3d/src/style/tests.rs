#[cfg(test)]
mod tests {
    use crate::style::{
        Colormap, ColormapColorIndex, ColormapIndex, Plot3DItemArrayStyle, colormap,
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
    fn borrowed_array_style_restores_scalar_settings_on_unwind_and_stays_in_its_plot() {
        use crate::{Plot3DDataLayout, layout::NextItemStyle};
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let next = NextItemStyle::default();
        let other_plot = NextItemStyle::default();
        let layout = Plot3DDataLayout::DEFAULT;
        let colors = [0x01020304u32, 0x05060708];
        let arrays = Plot3DItemArrayStyle::new()
            .with_line_colors(&colors)
            .with_marker_sizes(&[]);
        next.update(|s| s.LineWeight = 2.0);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                next.scoped(
                    |s| arrays.apply_to_spec(s),
                    || {
                        assert!(other_plot.spec(0, layout).LineColors.is_null());
                        panic!("stop before submission");
                    },
                );
            }))
            .is_err()
        );
        let restored = next.spec(0, layout);
        assert_eq!(restored.LineWeight, 2.0);
        assert!(restored.LineColors.is_null());
        next.scoped(
            |s| arrays.apply_to_spec(s),
            || {
                let spec = next.spec(0, layout);
                assert_eq!(spec.LineColors.cast_const(), colors.as_ptr());
                assert!(spec.MarkerSizes.is_null());
            },
        );
        assert!(next.spec(0, layout).LineColors.is_null());
    }
}
