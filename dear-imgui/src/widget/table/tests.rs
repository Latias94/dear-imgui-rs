use super::*;
use crate::draw::ImColor32;
use crate::widget::TableColumnFlags;

fn setup_context() -> crate::Context {
    let mut ctx = crate::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([128.0, 128.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx
}

#[test]
fn table_user_ids_hide_zero_sentinel() {
    let id = crate::Id::from(7u32);
    assert_eq!(optional_user_id_raw(None, "test"), 0);
    assert_eq!(optional_user_id_raw(Some(id), "test"), id.raw());
    assert_eq!(optional_user_id_from_raw(0), None);
    assert_eq!(optional_user_id_from_raw(id.raw()), Some(id));

    assert!(
        std::panic::catch_unwind(|| optional_user_id_raw(Some(crate::Id::default()), "test"))
            .is_err(),
        "explicit zero user ids must not cross the safe API boundary"
    );
}

unsafe fn current_table_draw_channel() -> i32 {
    let table = assert_current_table("current_table_draw_channel()");
    let draw_list = unsafe { (*(*table).InnerWindow).DrawList };
    unsafe { (*draw_list)._Splitter._Current }
}

#[test]
fn table_column_channel_is_popped_after_panic() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.window("table_channel_panic").build(|| {
            let _table = ui.begin_table("table", 2).unwrap();
            ui.table_next_row();
            assert!(ui.table_set_column_index(0));
            let initial_channel = unsafe { current_table_draw_channel() };

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.with_table_column_channel(1, || {
                    let pushed_channel = unsafe { current_table_draw_channel() };
                    assert_ne!(pushed_channel, initial_channel);
                    panic!("forced panic while table column channel is pushed");
                });
            }));

            assert!(result.is_err());
            assert_eq!(unsafe { current_table_draw_channel() }, initial_channel);
        });
    }));

    assert!(result.is_ok());
}

#[test]
fn begin_table_rejects_invalid_column_counts_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.begin_table("zero_columns", 0);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.begin_table("too_many_columns", TABLE_MAX_COLUMNS);
        }))
        .is_err()
    );
}

#[test]
fn table_index_queries_return_none_without_current_table_or_cell() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    assert_eq!(ui.table_get_column_index(), None);
    assert_eq!(ui.table_get_row_index(), None);
    assert_eq!(ui.table_get_hovered_row(), TableHoveredRow::None);

    let _ = ui.window("table_index_queries").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();
        assert_eq!(ui.table_get_column_index(), None);
        assert_eq!(ui.table_get_row_index(), None);

        ui.table_next_row();
        assert_eq!(ui.table_get_row_index(), Some(TableRowIndex::ZERO));
        assert_eq!(ui.table_get_column_index(), None);

        assert!(ui.table_set_column_index(0));
        assert_eq!(ui.table_get_column_index(), Some(TableColumnIndex::ZERO));
    });
}

#[test]
fn table_column_channel_rejects_out_of_range_column_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_channel_oob").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();
        ui.table_next_row();
        assert!(ui.table_set_column_index(0));

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _token = ui.table_column_channel(2);
            }))
            .is_err()
        );
    });
}

#[test]
fn table_channels_require_current_cell_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_channel_cell_required").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _token = ui.table_background_channel();
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _token = ui.table_column_channel(0);
            }))
            .is_err()
        );
    });
}

#[test]
fn table_accessors_reject_invalid_columns_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_accessors_oob").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();
        ui.table_next_row();
        assert!(ui.table_set_column_index(0));

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_set_column_index(2);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.table_get_column_name(2);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_set_column_enabled(2, true);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_set_column_sort_direction(2, SortDirection::Ascending, false);
            }))
            .is_err()
        );
    });
}

#[test]
fn table_setup_methods_reject_late_or_excess_calls_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_setup_preconditions").build(|| {
        let _table = ui.begin_table("table", 1).unwrap();
        ui.table_setup_column("one", TableColumnFlags::NONE, None, None);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_column("two", TableColumnFlags::NONE, None, None);
            }))
            .is_err()
        );

        ui.table_next_row();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_scroll_freeze(1, 0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_set_column_width(0, 32.0);
            }))
            .is_err()
        );
    });
}

#[test]
fn table_freeze_counts_reject_out_of_range_values_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_freeze_bounds").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_scroll_freeze(TABLE_MAX_COLUMNS, 0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_scroll_freeze(1, 128);
            }))
            .is_err()
        );
    });
}

#[test]
fn table_set_column_width_rejects_invalid_widths_before_ffi() {
    let mut ctx = setup_context();

    {
        let ui = ctx.frame();
        let _ = ui.window("table_width_bounds").build(|| {
            let _table = ui.begin_table("table", 1).unwrap();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.table_set_column_width(0, 32.0);
                }))
                .is_err()
            );
            ui.table_next_row();
        });
    }
    ctx.render();

    let ui = ctx.frame();
    let _ = ui.window("table_width_bounds").build(|| {
        let _table = ui.begin_table("table", 1).unwrap();
        ui.table_set_column_width(0, 0.0);
        ui.table_set_column_width(0, 32.0);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_set_column_width(0, -1.0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_set_column_width(0, f32::NAN);
            }))
            .is_err()
        );
    });
}

#[test]
fn table_bg_color_helpers_validate_column_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_bg_preconditions").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();
        ui.table_next_row();
        assert!(ui.table_set_column_index(0));

        ui.table_set_cell_bg_color_u32(0, TableColumnRef::Current);
        ui.table_set_row_bg0_color_u32(0);
        ui.table_set_row_bg1_color_u32(0);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_set_cell_bg_color_u32(0, 2);
            }))
            .is_err()
        );
    });
}

#[test]
fn table_angled_headers_validate_indices_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_angled_header_invalid").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();
        ui.table_setup_column("one", TableColumnFlags::ANGLED_HEADER, None, None);
        ui.table_setup_column("two", TableColumnFlags::ANGLED_HEADER, None, None);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let invalid = [TableHeaderData::new(
                    2,
                    ImColor32::WHITE,
                    ImColor32::BLACK,
                    ImColor32::BLACK,
                )];
                ui.table_angled_headers_row_ex_with_data(0, 0.0, 0.0, &invalid);
            }))
            .is_err()
        );
    });
}
