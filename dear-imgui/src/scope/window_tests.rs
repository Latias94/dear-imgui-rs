use super::ScopeSnapshot;
use crate::{Condition, Context, ListClipper, Ui, sys};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn in_window(test: impl FnOnce(&Ui)) {
    let _lock = crate::test_support::imgui_context_guard();
    let mut ctx = Context::create();
    ctx.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    ctx.io_mut().set_display_size([320.0, 240.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
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
        let draw_list = ui.get_window_draw_list();
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
}
