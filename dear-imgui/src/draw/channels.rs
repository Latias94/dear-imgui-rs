use crate::sys;

use super::list::DrawListMut;
#[cfg(test)]
use super::list::DrawListProvenance;
use super::util::count_to_i32;

/// Represent the drawing interface within a call to `channels_split`.
pub struct ChannelsSplit<'ui> {
    pub(super) draw_list: &'ui DrawListMut<'ui>,
    pub(super) channels_count: usize,
}

impl<'ui> ChannelsSplit<'ui> {
    pub(super) fn new(draw_list: &'ui DrawListMut<'ui>, channels_count: usize) -> Self {
        Self {
            draw_list,
            channels_count,
        }
    }

    /// Change current channel. Panics if `channel_index >= channels_count`.
    #[doc(alias = "ChannelsSetCurrent")]
    pub fn set_current(&self, channel_index: usize) {
        assert!(
            channel_index < self.channels_count,
            "Channel index {} out of range {}",
            channel_index,
            self.channels_count
        );
        let channel_index_i32 = count_to_i32(
            "ChannelsSplit::set_current()",
            "channel_index",
            channel_index,
        );
        unsafe { sys::ImDrawList_ChannelsSetCurrent(self.draw_list.draw_list, channel_index_i32) };
    }
}

#[cfg(test)]
mod channels_tests {
    use super::*;
    use crate::internal::len_i32;

    #[test]
    fn with_clip_rect_pops_after_panic() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let draw_list = ui.get_window_draw_list();
        let raw_draw_list = draw_list.draw_list;
        let initial_stack_size = unsafe { (*raw_draw_list)._ClipRectStack.Size };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.with_clip_rect([0.0, 0.0], [8.0, 8.0], || {
                assert_eq!(
                    unsafe { (*raw_draw_list)._ClipRectStack.Size },
                    initial_stack_size + 1
                );
                panic!("forced panic while draw-list clip rect is pushed");
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            unsafe { (*raw_draw_list)._ClipRectStack.Size },
            initial_stack_size
        );
    }

    #[test]
    fn with_texture_pops_after_panic() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let draw_list = ui.get_window_draw_list();
        let raw_draw_list = draw_list.draw_list;
        let initial_stack_size = unsafe { (*raw_draw_list)._TextureStack.Size };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.with_texture(crate::texture::TextureId::new(1), || {
                assert_eq!(
                    unsafe { (*raw_draw_list)._TextureStack.Size },
                    initial_stack_size + 1
                );
                panic!("forced panic while draw-list texture is pushed");
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            unsafe { (*raw_draw_list)._TextureStack.Size },
            initial_stack_size
        );
    }

    #[test]
    fn draw_list_clip_tokens_reject_out_of_order_drop_and_recover() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.font_atlas().build();

        let ui = ctx.frame();
        let draw_list = ui.get_window_draw_list();
        let raw = draw_list.draw_list;
        let initial = unsafe { (*raw)._ClipRectStack.Size };
        let outer = draw_list.push_clip_rect([0.0, 0.0], [64.0, 64.0], false);
        let inner = draw_list.push_clip_rect([8.0, 8.0], [32.0, 32.0], true);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(outer)))
            .expect_err("out-of-order clip token should panic");
        let message = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        assert!(message.contains("native scope order violation"));
        assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, initial + 2);

        drop(inner);
        assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, initial);
        ui.text("draw-list clip stack recovered");
    }

    #[test]
    fn draw_list_texture_tokens_reject_out_of_order_drop_and_recover() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.font_atlas().build();

        let ui = ctx.frame();
        let draw_list = ui.get_window_draw_list();
        let raw = draw_list.draw_list;
        let initial = unsafe { (*raw)._TextureStack.Size };
        let outer = draw_list.push_texture(crate::texture::TextureId::new(1));
        let inner = draw_list.push_texture(crate::texture::TextureId::new(2));

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(outer)))
            .expect_err("out-of-order texture token should panic");
        let message = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        assert!(message.contains("native scope order violation"));
        assert_eq!(unsafe { (*raw)._TextureStack.Size }, initial + 2);

        drop(inner);
        assert_eq!(unsafe { (*raw)._TextureStack.Size }, initial);
        ui.text("draw-list texture stack recovered");
    }

    #[test]
    fn window_draw_list_rejects_use_after_its_begin_scope_before_ffi() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let ui_ref: &crate::Ui = ui;
        let draw_list = ui_ref
            .window("window draw-list provenance")
            .build(|| ui_ref.get_window_draw_list())
            .expect("the source window should submit");
        let raw = draw_list.draw_list;
        let initial = unsafe { (*raw)._ClipRectStack.Size };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = draw_list.push_clip_rect([0.0, 0.0], [8.0, 8.0], false);
        }));
        assert!(result.is_err());
        assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, initial);
    }

    #[test]
    fn window_draw_list_rejects_a_later_begin_of_the_same_window_before_ffi() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let draw_list = ui
            .window("reopened window draw-list provenance")
            .build(|| ui.get_window_draw_list())
            .expect("the source window should submit");
        let raw = draw_list.draw_list;

        ui.window("reopened window draw-list provenance").build(|| {
            let initial = unsafe { (*raw)._ClipRectStack.Size };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = draw_list.push_clip_rect([0.0, 0.0], [8.0, 8.0], false);
            }));
            assert!(result.is_err());
            assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, initial);
        });
    }

    #[test]
    fn window_draw_list_survives_internal_reentry_of_its_window() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
            io.set_config_flags(io.config_flags() | crate::ConfigFlags::DOCKING_ENABLE);
        }
        let _ = ctx.font_atlas().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let host_name = format!("WindowOverViewport_{:08X}", ui.main_viewport().id().raw());
        ui.window(host_name).build(|| {
            let draw_list = ui.get_window_draw_list();
            let raw = draw_list.draw_list;
            let initial = unsafe { (*raw)._ClipRectStack.Size };

            let _ =
                ui.dockspace_over_main_viewport_with_flags(0.into(), crate::DockNodeFlags::NONE);

            drop(draw_list.push_clip_rect([0.0, 0.0], [8.0, 8.0], false));
            assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, initial);
        });
    }

    #[test]
    fn frame_draw_lists_may_outlive_the_window_that_acquired_the_wrapper() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let ui_ref: &crate::Ui = ui;
        let draw_list = ui_ref
            .window("frame draw-list source window")
            .build(|| ui_ref.get_background_draw_list())
            .expect("the source window should submit");
        let raw = draw_list.draw_list;
        let initial = unsafe { (*raw)._ClipRectStack.Size };

        draw_list.with_clip_rect([0.0, 0.0], [8.0, 8.0], || {
            assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, initial + 1);
        });
        assert_eq!(unsafe { (*raw)._ClipRectStack.Size }, initial);
    }

    #[test]
    fn channels_split_merges_after_panic() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        unsafe { sys::ImDrawList_AddDrawCmd(raw_draw_list) };

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            ui: None,
            provenance: DrawListProvenance::Frame,
        };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.channels_split(2, |channels| {
                channels.set_current(1);
                panic!("forced panic while channels are split");
            });
        }));

        assert!(result.is_err());
        unsafe {
            assert_eq!((*raw_draw_list)._Splitter._Count, 1);
            assert_eq!((*raw_draw_list)._Splitter._Current, 0);
        }

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn channels_split_rejects_same_draw_list_nesting_before_ffi() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());
        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            ui: None,
            provenance: DrawListProvenance::Frame,
        };

        draw_list.channels_split(2, |channels| {
            let before_count = unsafe { (*raw_draw_list)._Splitter._Count };
            let before_current = unsafe { (*raw_draw_list)._Splitter._Current };
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                draw_list.channels_split(2, |_| {});
            }))
            .expect_err("nested split should panic");
            let message = panic
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("non-string panic payload");
            assert!(message.contains("does not support nested splits"));
            assert_eq!(unsafe { (*raw_draw_list)._Splitter._Count }, before_count);
            assert_eq!(
                unsafe { (*raw_draw_list)._Splitter._Current },
                before_current
            );
            channels.set_current(1);
        });

        assert_eq!(unsafe { (*raw_draw_list)._Splitter._Count }, 1);
        assert_eq!(unsafe { (*raw_draw_list)._Splitter._Current }, 0);
        drop(draw_list);
        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn channels_split_allows_independent_draw_lists_to_nest() {
        let shared_a = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        let shared_b = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        let raw_a = unsafe { sys::ImDrawList_ImDrawList(shared_a) };
        let raw_b = unsafe { sys::ImDrawList_ImDrawList(shared_b) };
        assert!(!raw_a.is_null() && !raw_b.is_null());
        let draw_list_a = DrawListMut {
            draw_list: raw_a,
            ui: None,
            provenance: DrawListProvenance::Frame,
        };
        let draw_list_b = DrawListMut {
            draw_list: raw_b,
            ui: None,
            provenance: DrawListProvenance::Frame,
        };

        draw_list_a.channels_split(2, |channels_a| {
            channels_a.set_current(1);
            draw_list_b.channels_split(3, |channels_b| channels_b.set_current(2));
            assert_eq!(unsafe { (*raw_b)._Splitter._Count }, 1);
            assert_eq!(unsafe { (*raw_b)._Splitter._Current }, 0);
            assert_eq!(unsafe { (*raw_a)._Splitter._Count }, 2);
            assert_eq!(unsafe { (*raw_a)._Splitter._Current }, 1);
        });

        drop(draw_list_a);
        drop(draw_list_b);
        unsafe {
            sys::ImDrawList_destroy(raw_a);
            sys::ImDrawList_destroy(raw_b);
            sys::ImDrawListSharedData_destroy(shared_a);
            sys::ImDrawListSharedData_destroy(shared_b);
        }
    }

    #[test]
    fn channels_split_rejects_zero_channels() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            ui: None,
            provenance: DrawListProvenance::Frame,
        };
        let initial_count = unsafe { (*raw_draw_list)._Splitter._Count };
        let initial_current = unsafe { (*raw_draw_list)._Splitter._Current };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.channels_split(0, |_| {});
        }));

        assert!(result.is_err());
        unsafe {
            assert_eq!((*raw_draw_list)._Splitter._Count, initial_count);
            assert_eq!((*raw_draw_list)._Splitter._Current, initial_current);
        }

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn channels_split_rejects_oversized_channel_counts() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            ui: None,
            provenance: DrawListProvenance::Frame,
        };
        let initial_count = unsafe { (*raw_draw_list)._Splitter._Count };
        let initial_current = unsafe { (*raw_draw_list)._Splitter._Current };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.channels_split(i32::MAX as usize + 1, |_| {});
        }));

        assert!(result.is_err());
        unsafe {
            assert_eq!((*raw_draw_list)._Splitter._Count, initial_count);
            assert_eq!((*raw_draw_list)._Splitter._Current, initial_current);
        }

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn draw_list_point_count_helpers_reject_overflow() {
        assert!(
            std::panic::catch_unwind(|| {
                let _ = len_i32("Polyline::build()", "points", (i32::MAX as usize) + 1);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = len_i32(
                    "DrawListMut::add_concave_poly_filled()",
                    "points",
                    (i32::MAX as usize) + 1,
                );
            })
            .is_err()
        );
    }
}
