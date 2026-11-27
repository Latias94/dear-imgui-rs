use super::ScopeSnapshot;
use crate::{Condition, Context, ListClipper, Ui, sys};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn prepared_context() -> Context {
    let mut ctx = Context::create();
    ctx.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    ctx.io_mut().set_display_size([320.0, 240.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    ctx
}

fn in_window(test: impl FnOnce(&Ui)) {
    let _lock = crate::test_support::imgui_context_guard();
    let mut ctx = prepared_context();
    let ui: &Ui = ctx.frame();
    ui.window("Host scopes")
        .position([0.0, 0.0], Condition::Always)
        .size([300.0, 200.0], Condition::Always)
        .build(|| test(ui))
        .expect("visible test window");
    let _frame = ctx.render_legacy();
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HostState {
    scope: ScopeSnapshot,
    clips: i32,
    ids: i32,
}

#[test]
fn channel_split_merges_before_a_pending_tooltip_end() {
    in_window(|ui| {
        let host = host_state(ui);
        let tooltip = ui.begin_tooltip().expect("visible tooltip");
        let result = catch_unwind(AssertUnwindSafe(|| {
            ui.with_window_draw_list(|draw_list| {
                draw_list.channels_split(2, |channels| {
                    channels.set_current(1);
                    drop(tooltip);
                });
            });
        }));
        assert!(
            result.is_err(),
            "the split must outlive the attempted tooltip end"
        );
        assert_eq!(host_state(ui), host);
        ui.text("tooltip and draw channel scopes recovered");
    });
}

#[test]
fn channel_split_merges_on_its_owner_during_unwind() {
    in_window(|ui| {
        let owner = unsafe { sys::igGetCurrentContext() };
        let foreign = Context::create();
        let foreign_raw = foreign.as_raw();
        let draw_list = unsafe { (*sys::igGetCurrentWindowRead()).DrawList };
        let result = catch_unwind(AssertUnwindSafe(|| {
            ui.with_window_draw_list(|draw_list| {
                draw_list.channels_split(2, |channels| {
                    channels.set_current(1);
                    ui.text("channel content");
                    unsafe { sys::igSetCurrentContext(foreign_raw) };
                    panic!("unwind through channel split");
                });
            });
        }));
        let current_after_unwind = unsafe { sys::igGetCurrentContext() };
        unsafe { sys::igSetCurrentContext(owner) };
        assert!(result.is_err());
        assert_eq!(current_after_unwind, foreign_raw);
        assert_eq!(unsafe { (*draw_list)._Splitter._Count }, 1);
        assert_eq!(unsafe { (*draw_list)._Splitter._Current }, 0);
        ui.text("drawing after the split merged");
    });
}

#[test]
fn pushed_string_id_matches_get_id_for_embedded_nul() {
    in_window(|ui| {
        let label = "before\0after";
        let expected = ui.get_id(label);
        assert_ne!(expected, ui.get_id("before"));
        assert_ne!(expected, ui.get_id("before?after"));

        let _id = ui.push_id(label);
        let stack = unsafe { (*sys::igGetCurrentWindowRead()).IDStack };
        assert_eq!(
            unsafe { *stack.Data.add(stack.Size as usize - 1) },
            expected.raw()
        );
    });
}

#[test]
fn tab_builders_bind_the_owner_and_end_the_tab_bar_during_unwind() {
    in_window(|ui| {
        let owner = unsafe { sys::igGetCurrentContext() };
        let foreign = Context::create();
        let foreign_raw = foreign.as_raw();
        foreign.binding().with_bound_context(|| {
            let result = catch_unwind(AssertUnwindSafe(|| {
                ui.tab_bar_config("tabs").reorderable(true).build(|| {
                    assert_eq!(unsafe { sys::igGetCurrentContext() }, owner);
                    let mut opened = true;
                    let result = ui.tab_item_config("tab").opened(&mut opened).build(|| {
                        assert_eq!(unsafe { sys::igGetCurrentContext() }, owner);
                        ui.text("tab content");
                        42
                    });
                    assert_eq!(result, Some(42));
                    assert!(opened);
                    panic!("unwind through tab bar");
                });
            }));
            assert!(result.is_err());
            assert_eq!(unsafe { sys::igGetCurrentContext() }, foreign_raw);
        });
        assert!(unsafe { sys::igGetCurrentTabBar() }.is_null());
        assert_eq!(ui.tab_bar_config("next").build(|| 1), Some(1));
    });
}

fn host_state(ui: &Ui) -> HostState {
    ui.with_bound_context(|| {
        let window = unsafe { &*sys::igGetCurrentWindowRead() };
        HostState {
            scope: ui.current_native_scope(),
            clips: unsafe { (*window.DrawList)._ClipRectStack.Size },
            ids: window.IDStack.Size,
        }
    })
}

#[test]
fn native_window_scope_keeps_its_owner_current_with_a_live_foreign_frame() {
    let _lock = crate::test_support::imgui_context_guard();
    let mut ctx = prepared_context();
    let ui = ctx.frame();
    let host = host_state(ui);
    let (escaped, mut foreign, current) = ui.with_native_window_scope(|_| {
        let escaped = ui.push_id("escaped");
        let mut foreign = prepared_context();
        foreign.frame().text("another frame");
        (escaped, foreign, unsafe { sys::igGetCurrentContext() })
    });
    assert_eq!(current, ui.context_raw());
    assert_eq!(unsafe { sys::igGetCurrentContext() }, ui.context_raw());
    assert_eq!(host_state(ui), host);
    assert!(foreign.end_frame());
    drop(foreign);
    let later = ui.push_id("later");
    let with_later_id = host_state(ui);
    drop(escaped);
    assert_eq!(host_state(ui), with_later_id);
    drop(later);
    let _frame = ctx.render_legacy();
}

#[test]
fn native_window_scope_unwinds_after_the_foreign_frame_is_destroyed() {
    let _lock = crate::test_support::imgui_context_guard();
    let mut ctx = prepared_context();
    let ui = ctx.frame();
    let host = host_state(ui);
    let mut escaped = None;
    let mut current_after_destroy = std::ptr::null_mut();
    let result = catch_unwind(AssertUnwindSafe(|| {
        ui.with_native_window_scope(|scope| {
            scope.run(|| {
                escaped = Some(ui.push_id("escaped"));
                let mut foreign = prepared_context();
                foreign.frame().text("another frame");
                drop(foreign);
                current_after_destroy = unsafe { sys::igGetCurrentContext() };
                panic!("unwind through the native window scope");
            });
        });
    }));
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"unwind through the native window scope")
    );
    assert_eq!(current_after_destroy, ui.context_raw());
    assert_eq!(host_state(ui), host);
    drop(escaped);
    let _frame = ctx.render_legacy();
}

#[test]
fn native_window_scope_cleanup_rebinds_after_a_raw_context_switch() {
    in_window(|ui| {
        let foreign = prepared_context();
        let host = host_state(ui);
        ui.with_native_window_scope(|scope| {
            let mut escaped = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                scope.run(|| {
                    escaped = Some(ui.push_id("escaped"));
                    unsafe { sys::igSetCurrentContext(foreign.as_raw()) };
                    panic!("unwind after a raw context switch");
                });
            }));
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"unwind after a raw context switch")
            );
            assert_eq!(unsafe { sys::igGetCurrentContext() }, foreign.as_raw());
            assert_eq!(host_state(ui), host);
            drop(escaped);
        });
        assert_eq!(unsafe { sys::igGetCurrentContext() }, foreign.as_raw());
        unsafe { sys::igSetCurrentContext(ui.context_raw()) };
    });
}

#[test]
fn popup_stays_open_until_the_extension_ends_and_returned_tokens_are_inert() {
    in_window(|ui| {
        let parent = host_state(ui);
        ui.open_popup("editor host");
        let popup = ui.begin_popup("editor host").expect("open popup");
        let outer_clip = ui.push_clip_rect([0.0; 2], [320.0, 240.0], true);
        let original = host_state(ui);
        ui.with_native_window_scope(|host| {
            let children = host.run(|| {
                drop(popup);
                drop(outer_clip);
                assert_eq!(host_state(ui), original);
                let id = ui.push_id("canvas content");
                let clip = ui.push_clip_rect([0.0; 2], [320.0, 240.0], true);
                let table = ui.begin_table("content", 1).expect("visible table");
                ui.table_next_column();
                let tooltip = ui.begin_tooltip().expect("visible tooltip");
                assert!(!host.is_current());
                (id, clip, table, tooltip)
            });
            assert!(host.is_current());
            assert_eq!(host_state(ui), original);

            let _later = ui.push_id("later content");
            let later = host_state(ui);
            drop(children);
            assert_eq!(host_state(ui), later);
        });
        assert_eq!(host_state(ui), parent);
    });
}

#[test]
fn unwind_restores_the_host_before_the_extension_destructor() {
    struct RecordEnd<'a>(&'a Ui, &'a Cell<Option<HostState>>);
    impl Drop for RecordEnd<'_> {
        fn drop(&mut self) {
            self.1.set(Some(host_state(self.0)));
        }
    }

    in_window(|ui| {
        let parent = host_state(ui);
        ui.open_popup("unwind host");
        let popup = ui.begin_popup("unwind host").unwrap();
        let original = host_state(ui);
        let at_end = Cell::new(None);
        let mut late_tooltip = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            ui.with_native_window_scope(|host| {
                let _extension = RecordEnd(ui, &at_end);
                host.run(|| {
                    drop(popup);
                    std::mem::forget(ui.push_id("forgotten child"));
                    late_tooltip = ui.begin_tooltip();
                    assert!(late_tooltip.is_some());
                    panic!("callback unwind");
                });
            });
        }));
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"callback unwind")
        );
        assert_eq!(at_end.get(), Some(original));
        assert_eq!(host_state(ui), parent);
        drop(late_tooltip);
        ui.text("frame recovered");
    });
}

#[test]
fn nested_extensions_in_one_window_have_distinct_hosts() {
    in_window(|ui| {
        ui.with_native_window_scope(|outer| {
            outer.run(|| {
                assert!(outer.is_current());
                ui.with_native_window_scope(|inner| {
                    inner.run(|| {
                        assert!(inner.is_current());
                        assert!(!outer.is_current());
                        assert!(catch_unwind(AssertUnwindSafe(|| outer.run(|| {}))).is_err());
                    });
                });
                assert!(outer.is_current());
            });
        });
    });
}

#[test]
fn escaped_clippers_release_their_native_stack_before_the_extension_continues() {
    in_window(|ui| {
        let depth = || unsafe { (*ui.context_raw()).ClipperTempDataStacked };
        let mut outer = ListClipper::new(2).items_height(10.0).begin(ui);
        assert_eq!(depth(), 1);
        ui.with_native_window_scope(|host| {
            let mut escaped = host.run(|| {
                assert!(catch_unwind(AssertUnwindSafe(|| outer.step())).is_err());
                let inner = ListClipper::unknown_count().items_height(10.0).begin(ui);
                assert_eq!(depth(), 2);
                inner
            });
            assert_eq!(depth(), 1);
            assert!(catch_unwind(AssertUnwindSafe(|| escaped.next_range())).is_err());
            drop(escaped);
        });
        assert!(outer.step());
        outer.end();
        assert_eq!(depth(), 0);
    });
}

#[test]
fn unwind_releases_forgotten_clippers_before_a_deferred_parent() {
    in_window(|ui| {
        let depth = || unsafe { (*ui.context_raw()).ClipperTempDataStacked };
        let outer = ListClipper::new(2).items_height(10.0).begin(ui);
        let result = catch_unwind(AssertUnwindSafe(|| {
            ui.with_native_window_scope(|host| {
                host.run(|| {
                    drop(outer);
                    assert_eq!(depth(), 1);
                    std::mem::forget(ListClipper::new(1).items_height(10.0).begin(ui));
                    assert_eq!(depth(), 2);
                    panic!("leave the clipper callback");
                });
            })
        }));
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"leave the clipper callback")
        );
        assert_eq!(depth(), 0);
        ListClipper::new(1).items_height(10.0).begin(ui).end();
    });
}

#[test]
fn a_host_table_keeps_its_cell_while_nested_tables_can_advance() {
    in_window(|ui| {
        let table = ui.begin_table("host table", 2).unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| ui.with_native_window_scope(|_| ()))).is_err());
        let mut clipper = ListClipper::new(10).items_height(12.0).begin(ui);
        assert!(clipper.step());
        let visible_items = clipper.display_range();
        ui.table_next_column();
        let original = host_state(ui);
        ui.with_native_window_scope(|host| {
            let nested = host.run(|| {
                drop(table);
                assert!(catch_unwind(AssertUnwindSafe(|| ui.table_next_row())).is_err());
                assert!(
                    catch_unwind(AssertUnwindSafe(|| ui.with_table_column_channel(1, || {})))
                        .is_err()
                );
                assert!(catch_unwind(AssertUnwindSafe(|| ListClipper::new(1).begin(ui))).is_err());
                assert_eq!(clipper.display_range(), visible_items);
                assert!(catch_unwind(AssertUnwindSafe(|| clipper.step())).is_err());
                drop(clipper);
                assert_eq!(host_state(ui), original);
                let id = ui.push_id("nested table host");
                let nested = ui.begin_table("inner table", 1).unwrap();
                ui.table_next_column();
                assert!(!host.is_current());
                let cell = host_state(ui);
                drop(id);
                assert_eq!(host_state(ui), cell);
                nested
            });
            assert_eq!(host_state(ui), original);
            drop(nested);
        });
        assert!(ui.current_native_scope().table().is_none());
    });
}

#[test]
fn an_outer_split_can_select_its_channel_again_after_the_extension() {
    in_window(|ui| {
        ui.with_window_draw_list(|draw_list| {
            ui.with_native_window_scope(|_| {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| draw_list.channels_split(2, |_| {}))).is_err()
                );
            });
            draw_list.channels_split(2, |outer| {
                outer.set_current(1);
                ui.with_native_window_scope(|_| {
                    assert!(catch_unwind(AssertUnwindSafe(|| outer.set_current(0))).is_err());
                    let window = unsafe { &*sys::igGetCurrentWindowRead() };
                    assert_eq!(unsafe { (*window.DrawList)._Splitter._Current }, 1);
                    let foreground = ui.get_foreground_draw_list();
                    foreground.channels_split(2, |independent| independent.set_current(1));
                });
                outer.set_current(0);
            });
        });
    });
}
