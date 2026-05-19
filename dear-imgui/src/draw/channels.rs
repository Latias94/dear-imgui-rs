use crate::sys;

use super::list::DrawListMut;
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
    use std::marker::PhantomData;

    #[test]
    fn with_clip_rect_pops_after_panic() {
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
        let _ = ctx.font_atlas_mut().build();
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
    fn channels_split_merges_after_panic() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        unsafe { sys::ImDrawList_AddDrawCmd(raw_draw_list) };

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
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
    fn channels_split_rejects_zero_channels() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
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
            _phantom: PhantomData,
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
