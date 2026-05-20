#[cfg(test)]
mod tests {
    use crate::{OldColumnFlags, OldColumnIndex, OldColumnOffsetRef, OldColumnRef};

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        let _ = ctx.font_atlas_mut().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx
    }

    #[test]
    fn is_any_column_resizing_reads_current_columns_state() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_resize_test").build(|| {
            assert!(!ui.is_any_column_resizing());

            let _columns = ui.begin_columns_token("legacy_columns", 2, OldColumnFlags::NONE);
            let window = unsafe { crate::sys::igGetCurrentWindowRead() };
            assert!(!window.is_null());

            let columns = unsafe { (*window).DC.CurrentColumns };
            assert!(!columns.is_null());
            assert!(!ui.is_any_column_resizing());

            unsafe {
                (*columns).IsBeingResized = true;
            }

            assert!(ui.is_any_column_resizing());
        });
    }

    #[test]
    fn columns_reject_invalid_counts_and_nested_layouts() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_invalid_counts").build(|| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.columns(0, "bad_columns", true);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _columns = ui.begin_columns_token("bad_columns", 0, OldColumnFlags::NONE);
                }))
                .is_err()
            );

            let _columns = ui.begin_columns_token("outer_columns", 2, OldColumnFlags::NONE);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _nested = ui.begin_columns_token("nested_columns", 2, OldColumnFlags::NONE);
                }))
                .is_err()
            );
        });
    }

    #[test]
    fn columns_reject_out_of_range_indices_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_index_bounds").build(|| {
            let _columns = ui.begin_columns_token("legacy_columns", 2, OldColumnFlags::NONE);

            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.column_width(2);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_width(2, 10.0);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.column_offset(3);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.push_column_clip_rect(2);
                }))
                .is_err()
            );
        });
    }

    #[test]
    fn columns_reject_invalid_flags_and_numeric_inputs_before_ffi() {
        let mut ctx = setup_context();
        let ui = ctx.frame();

        ui.window("columns_numeric_bounds").build(|| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _columns = ui.begin_columns_token(
                        "bad_flags",
                        2,
                        OldColumnFlags::from_bits_retain(1 << 16),
                    );
                }))
                .is_err()
            );

            let _columns = ui.begin_columns_token("legacy_columns", 2, OldColumnFlags::NONE);
            assert_eq!(ui.column_count(), 2);
            assert_eq!(ui.current_column_index(), OldColumnIndex::ZERO);

            let _ = ui.current_column_width();
            let _ = ui.column_width(OldColumnRef::Current);
            let _ = ui.column_width(OldColumnIndex::new(1));
            let _ = ui.current_column_offset();
            let _ = ui.column_offset(OldColumnOffsetRef::Current);
            let _ = ui.column_offset(OldColumnOffsetRef::Trailing);

            ui.set_current_column_width(32.0);
            ui.set_current_column_offset(0.0);
            ui.set_column_width(OldColumnIndex::new(1), 16.0);
            ui.set_column_offset(OldColumnIndex::new(1), 8.0);
            ui.set_column_offset(OldColumnOffsetRef::Trailing, 96.0);
            ui.set_column_width_percentage(OldColumnIndex::new(1), 25.0);

            ui.set_current_column_width(0.0);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_width(1, f32::NAN);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_current_column_offset(-1.0);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_offset(1, f32::INFINITY);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.set_column_width_percentage(1, -1.0);
                }))
                .is_err()
            );
        });
    }
}
