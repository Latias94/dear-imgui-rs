use super::*;
use crate::draw::ImColor32;
use crate::widget::TableColumnFlags;
use crate::widget::table::sort::copy_table_sort_specs;
use std::any::Any;

fn setup_context() -> crate::Context {
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
    ctx
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_owned(),
            Err(_) => "non-string panic payload".to_owned(),
        },
    }
}

fn assert_panics_with(expected: &str, f: impl FnOnce()) {
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .expect_err("operation should panic");
    let message = panic_message(panic);
    assert!(
        message.contains(expected),
        "panic `{message}` did not contain `{expected}`"
    );
}

#[test]
fn table_column_user_data_preserves_every_raw_value() {
    for raw in [0, 1, u32::MAX] {
        let user_data = TableColumnUserData::new(raw);
        assert_eq!(user_data.get(), raw);
        assert_eq!(TableColumnUserData::from(raw), user_data);
        assert_eq!(u32::from(user_data), raw);

        let mut raw_column = sys::ImGuiTableColumnSortSpecs {
            ColumnUserID: raw,
            ColumnIndex: 0,
            SortOrder: 0,
            SortDirection: sys::ImGuiSortDirection_Ascending,
        };
        let mut raw_specs = sys::ImGuiTableSortSpecs {
            Specs: &mut raw_column,
            SpecsCount: 1,
            SpecsDirty: true,
        };
        let (dirty, specs) = unsafe { copy_table_sort_specs(&mut raw_specs) };
        assert!(dirty);
        let spec = specs.first().expect("one sort specification");
        assert_eq!(spec.column_user_data, user_data);
    }
}

#[test]
fn table_user_data_builders_accept_integer_values() {
    let setup_data = 7;
    let setup = TableColumnSetup::new("column").user_data(setup_data);
    assert_eq!(setup.user_data, TableColumnUserData::from(7));

    let mut ctx = setup_context();
    let ui = ctx.frame();
    let setup_data = 0;
    let builder_data = 9;
    let _ = ui.window("table_integer_user_data").build(|| {
        {
            let _table = ui.begin_table("setup", 1).unwrap();
            ui.table_setup_column_with_user_data(
                "column",
                TableColumnFlags::NONE,
                None,
                setup_data,
            );
            // New tables apply column metadata during the first layout reconciliation.
            ui.table_next_row();
            assert_eq!(unsafe { current_table_column_user_data(0) }, setup_data);
        }

        ui.table("table")
            .column("column")
            .user_data(builder_data)
            .done()
            .build(|_| {
                // New tables apply column metadata during the first layout reconciliation.
                ui.table_next_row();
                assert_eq!(unsafe { current_table_column_user_data(0) }, builder_data);
            });
    });
}

unsafe fn current_table_column_user_data(column: usize) -> u32 {
    let table = unsafe { sys::igGetCurrentTable() };
    assert!(!table.is_null());
    let columns = unsafe { (*table).Columns.Data };
    let columns_end = unsafe { (*table).Columns.DataEnd };
    assert!(!columns.is_null());
    assert!(!columns_end.is_null());
    let column_count = unsafe { columns_end.offset_from(columns) as usize };
    assert!(column < column_count);
    unsafe { (*columns.add(column)).UserData }
}

unsafe fn current_table_draw_channel() -> i32 {
    let table = assert_current_table("current_table_draw_channel()");
    let draw_list = unsafe { (*(*table).InnerWindow).DrawList };
    unsafe { (*draw_list)._Splitter._Current }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeTableStacks {
    id: i32,
    item_width: i32,
    clip_rect: i32,
    style_color: i32,
}

unsafe fn current_native_table_stacks() -> NativeTableStacks {
    let context = unsafe { sys::igGetCurrentContext() };
    assert!(!context.is_null());
    let window = unsafe { (*context).CurrentWindow };
    assert!(!window.is_null());
    let draw_list = unsafe { (*window).DrawList };
    assert!(!draw_list.is_null());
    NativeTableStacks {
        id: unsafe { (*window).IDStack.Size },
        item_width: unsafe { (*window).DC.ItemWidthStack.Size },
        clip_rect: unsafe { (*draw_list)._ClipRectStack.Size },
        style_color: unsafe { (*context).ColorStack.Size },
    }
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
fn table_end_before_channel_panics_before_ffi_and_recovers_in_native_order() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_channel_lifo").build(|| {
        let mut table = Some(ui.begin_table("table", 1).unwrap());
        ui.table_next_row();
        assert!(ui.table_set_column_index(0));
        ui.with_table_column_channel(0, || {
            assert_panics_with("native scope order violation", || {
                table.take().expect("table token").end();
            });
            assert_panics_with("native scope order was violated", || ui.text("blocked"));
        });
        assert!(current_table_if_any().is_none());
        ui.text("scope tracker recovered");
    });
}

#[test]
fn table_channel_rejects_cell_transitions_and_nested_channels_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_channel_cell_lock").build(|| {
        let _table = ui.begin_table("table", 2).unwrap();
        ui.table_next_row();
        assert!(ui.table_set_column_index(0));
        let table = assert_current_table("table_channel_rejects_cell_transitions");
        let initial_row = unsafe { (*table).CurrentRow };
        let initial_column = unsafe { (*table).CurrentColumn };

        ui.with_table_background_channel(|| {
            let pushed_channel = unsafe { current_table_draw_channel() };
            assert_panics_with("cannot change the current table cell", || {
                let _ = ui.table_next_column();
            });
            assert_panics_with("cannot change the current table cell", || {
                let _ = ui.table_set_column_index(1);
            });
            assert_panics_with("cannot change the current table cell", || {
                ui.table_next_row();
            });
            assert_panics_with("cannot change the current table cell", || {
                ui.with_table_column_channel(1, || {});
            });

            assert_eq!(unsafe { (*table).CurrentRow }, initial_row);
            assert_eq!(unsafe { (*table).CurrentColumn }, initial_column);
            assert_eq!(unsafe { current_table_draw_channel() }, pushed_channel);
        });

        assert_eq!(unsafe { (*table).CurrentRow }, initial_row);
        assert_eq!(unsafe { (*table).CurrentColumn }, initial_column);
        ui.table_next_column();
    });
}

#[test]
fn table_phase_methods_reject_missing_table_before_ffi() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    assert_panics_with("BeginTable/EndTable scope", || ui.table_next_row());
    assert_panics_with("BeginTable/EndTable scope", || {
        let _ = ui.table_get_header_row_height();
    });
    assert_panics_with("BeginTable/EndTable scope", || {
        let _ = ui.table_get_header_angled_max_label_width();
    });
    assert_panics_with("BeginTable/EndTable scope", || {
        ui.table_angled_headers_row();
    });
    assert_panics_with("BeginTable/EndTable scope", || {
        ui.table_angled_headers_row_ex_with_data(0, 0.0, 0.0, &[]);
    });
    assert_panics_with("BeginTable/EndTable scope", || {
        ui.table_open_context_menu(TableContextMenuTarget::CurrentColumn);
    });
}

#[test]
fn table_end_waits_for_every_scope_created_inside_the_table() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    ui.window("table_scope_barriers").build(|| {
        let baseline = unsafe { current_native_table_stacks() };
        let table = ui.begin_table("item_width", 1).unwrap();
        let item_width = ui.push_item_width(80.0);
        let before_failure = unsafe { current_native_table_stacks() };
        assert_panics_with("table-local scope", || drop(table));
        assert_eq!(unsafe { current_native_table_stacks() }, before_failure);
        drop(item_width);
        assert!(unsafe { sys::igGetCurrentTable() }.is_null());
        assert_eq!(unsafe { current_native_table_stacks() }, baseline);

        let baseline = unsafe { current_native_table_stacks() };
        let table = ui.begin_table("id", 1).unwrap();
        let id = ui.push_id("inside-table");
        let before_failure = unsafe { current_native_table_stacks() };
        assert_panics_with("table-local scope", || drop(table));
        assert_eq!(unsafe { current_native_table_stacks() }, before_failure);
        drop(id);
        assert!(unsafe { sys::igGetCurrentTable() }.is_null());
        assert_eq!(unsafe { current_native_table_stacks() }, baseline);

        let baseline = unsafe { current_native_table_stacks() };
        let table = ui.begin_table("ui_clip", 1).unwrap();
        let clip = ui.push_clip_rect([0.0, 0.0], [64.0, 64.0], true);
        let before_failure = unsafe { current_native_table_stacks() };
        assert_panics_with("table-local scope", || drop(table));
        assert_eq!(unsafe { current_native_table_stacks() }, before_failure);
        drop(clip);
        assert!(unsafe { sys::igGetCurrentTable() }.is_null());
        assert_eq!(unsafe { current_native_table_stacks() }, baseline);

        let baseline = unsafe { current_native_table_stacks() };
        let table = ui
            .begin_table_with_sizing(
                "scrolling_style",
                1,
                TableFlags::SCROLL_Y,
                [100.0, 40.0],
                0.0,
            )
            .unwrap();
        let color = ui.push_style_color(crate::StyleColor::Text, [0.8, 0.7, 0.6, 1.0]);
        let before_failure = unsafe { current_native_table_stacks() };
        assert_panics_with("table-local scope", || drop(table));
        assert_eq!(unsafe { current_native_table_stacks() }, before_failure);
        drop(color);
        assert!(unsafe { sys::igGetCurrentTable() }.is_null());
        assert_eq!(unsafe { current_native_table_stacks() }, baseline);
    });
}

#[test]
fn table_sort_specs_are_owned_but_native_acknowledgement_stays_table_scoped() {
    let mut ctx = setup_context();
    let ui = ctx.frame();

    let _ = ui.window("owned_sort_specs").build(|| {
        let table = ui
            .begin_table_with_flags("source", 1, TableFlags::SORTABLE)
            .unwrap();
        ui.table_setup_column("value", TableColumnFlags::NONE, None);
        let mut specs = ui
            .table_get_sort_specs()
            .expect("sortable table should expose sort specs");
        assert!(!specs.is_empty());
        let copied = specs.iter().copied().collect::<Vec<_>>();
        table.end();

        assert_eq!(specs.len(), copied.len());
        assert_panics_with("source table is no longer current", || {
            specs.clear_dirty(ui);
        });

        let _other = ui
            .begin_table_with_flags("other", 1, TableFlags::SORTABLE)
            .unwrap();
        ui.table_setup_column("other", TableColumnFlags::NONE, None);
        assert_panics_with("source table is no longer current", || {
            specs.clear_dirty(ui);
        });
        assert_eq!(specs.iter().copied().collect::<Vec<_>>(), copied);
        drop(_other);

        let _reopened = ui
            .begin_table_with_flags("source", 1, TableFlags::SORTABLE)
            .unwrap();
        ui.table_setup_column("value", TableColumnFlags::NONE, None);
        assert_panics_with("source table is no longer current", || {
            specs.clear_dirty(ui);
        });
        assert_eq!(specs.iter().copied().collect::<Vec<_>>(), copied);
    });
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
                ui.with_table_column_channel(2, || {});
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
                ui.with_table_background_channel(|| {});
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.with_table_column_channel(0, || {});
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
        let _table = ui
            .begin_table_with_flags("table", 2, TableFlags::HIDEABLE | TableFlags::SORTABLE)
            .unwrap();
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
fn table_column_mutations_validate_required_flags_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_required_flags").build(|| {
        let _table = ui.begin_table("table", 1).unwrap();
        assert_panics_with("HIDEABLE", || {
            ui.table_set_column_enabled(0, false);
        });
        assert_panics_with("SORTABLE", || {
            ui.table_set_column_sort_direction(0, SortDirection::Ascending, false);
        });
    });
}

#[test]
fn table_sort_none_requires_tristate_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_sort_tristate").build(|| {
        {
            let _table = ui
                .begin_table_with_flags("single", 1, TableFlags::SORTABLE)
                .unwrap();
            assert_panics_with("SORT_TRISTATE", || {
                ui.table_set_column_sort_direction(0, SortDirection::None, false);
            });
        }

        let _table = ui
            .begin_table_with_flags(
                "tristate",
                1,
                TableFlags::SORTABLE | TableFlags::SORT_TRISTATE,
            )
            .unwrap();
        ui.table_set_column_sort_direction(0, SortDirection::None, false);
    });
}

#[test]
fn table_setup_methods_reject_late_or_excess_calls_before_ffi() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("table_setup_preconditions").build(|| {
        let _table = ui.begin_table("table", 1).unwrap();
        ui.table_setup_column("one", TableColumnFlags::NONE, None);

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ui.table_setup_column("two", TableColumnFlags::NONE, None);
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
    let _ = ctx.render_legacy();

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
        ui.table_setup_column("one", TableColumnFlags::ANGLED_HEADER, None);
        ui.table_setup_column("two", TableColumnFlags::ANGLED_HEADER, None);

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
