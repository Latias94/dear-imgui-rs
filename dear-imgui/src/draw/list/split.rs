use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::sys;

use super::super::channels::ChannelsSplit;
use super::super::util::count_to_i32;
use super::DrawListMut;

struct ChannelsSplitMergeGuard<'ui>(NativeScopeToken<'ui>);

impl Drop for ChannelsSplitMergeGuard<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// Drawing functions
impl<'ui> DrawListMut<'ui> {
    pub(in crate::draw) fn assert_splitter_available(&self) {
        if let Some(ui) = self.ui {
            ui.assert_draw_list_splitter_available(self.draw_list);
        }
    }

    /// Split draw into multiple channels and merge automatically at the end of the closure.
    ///
    /// # Panics
    ///
    /// Panics if the count is outside `1..=i32::MAX`, this draw list is already split,
    /// or a native extension is using its splitter.
    #[doc(alias = "ChannelsSplit")]
    pub fn channels_split(&self, channels_count: usize, f: impl FnOnce(&ChannelsSplit<'_>)) {
        assert!(channels_count > 0, "channels_count must be greater than 0");
        let channels_count_i32 = count_to_i32(
            "DrawListMut::channels_split()",
            "channels_count",
            channels_count,
        );

        let ui = self.ui();
        ui.with_bound_context(|| {
            self.assert_bound_scope("DrawListMut::channels_split()");
            self.assert_splitter_available();
            let draw_list = self.draw_list;
            let splitter = unsafe { &(*draw_list)._Splitter };
            assert!(
                splitter._Current == 0 && splitter._Count <= 1,
                "DrawListMut::channels_split() does not support nested splits for the same draw list"
            );

            unsafe { sys::ImDrawList_ChannelsSplit(draw_list, channels_count_i32) };
            let _merge_guard = ChannelsSplitMergeGuard(ui.begin_native_scope(
                NativeScopePop::DrawListMergeChannels {
                    draw_list,
                    window_scoped: self.is_window_scoped(),
                },
                "DrawListMut::channels_split()",
            ));
            f(&ChannelsSplit::new(self, channels_count));
        });
    }
}
