use crate::sys;

use super::super::channels::ChannelsSplit;
use super::super::util::count_to_i32;
use super::{ChannelsSplitMergeGuard, DrawListMut};

/// Drawing functions
impl<'ui> DrawListMut<'ui> {
    /// Split draw into multiple channels and merge automatically at the end of the closure.
    #[doc(alias = "ChannelsSplit")]
    pub fn channels_split<F: FnOnce(&ChannelsSplit<'ui>)>(&'ui self, channels_count: usize, f: F) {
        assert!(channels_count > 0, "channels_count must be greater than 0");
        let channels_count_i32 = count_to_i32(
            "DrawListMut::channels_split()",
            "channels_count",
            channels_count,
        );

        unsafe { sys::ImDrawList_ChannelsSplit(self.draw_list, channels_count_i32) };
        let _merge_guard = ChannelsSplitMergeGuard { draw_list: self };
        f(&ChannelsSplit::new(self, channels_count));
    }
}
