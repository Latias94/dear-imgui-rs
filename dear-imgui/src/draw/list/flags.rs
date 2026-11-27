use crate::sys;

use super::super::counts::DrawFlags;
use super::super::util::{CORNER_FLAGS, assert_draw_flags};
use super::DrawListMut;

struct DrawFlagsGuard<'draw_list> {
    draw_list: *mut sys::ImDrawList,
    scope: Option<crate::scope::NativeScopeToken<'draw_list>>,
}

impl Drop for DrawFlagsGuard<'_> {
    fn drop(&mut self) {
        if let Some(scope) = self.scope.as_mut() {
            scope.finish();
        } else {
            // Standalone draw lists used by internal native tests have no Ui scope.
            unsafe { sys::ImDrawList_PopDrawFlag(self.draw_list) };
        }
    }
}

impl DrawListMut<'_> {
    /// Return the currently enabled public drawing flags.
    pub fn flags(&self) -> DrawFlags {
        self.with_raw_bound("DrawListMut::flags()", |draw_list| {
            DrawFlags::from_bits_truncate(unsafe { (*draw_list).Flags } as u32)
        })
    }

    /// Enable or disable drawing options while `f` runs, then restore prior defaults.
    ///
    /// The scope may configure anti-aliasing, legacy stroke positioning, text pixel
    /// snapping, rounded corners, and texture/vertex-offset options. Other stroke
    /// positions, closure, joins, and caps must be configured on individual primitives.
    ///
    /// Scopes nest and restore their state during unwinding. Dear ImGui currently
    /// supports three nested flag scopes per draw list; exceeding its capacity panics
    /// before touching the native stack. Unknown flags and unsupported scope options
    /// also panic before mutation. Enabling rounding flags that conflict with the
    /// current scope panics as well; disable the prior rounding mode first when
    /// switching modes. A window draw list must remain in its originating window
    /// scope until the closure returns. Vertex offsets require renderer support.
    ///
    /// ```no_run
    /// # use dear_imgui_rs::{DrawFlags, DrawListMut};
    /// # fn draw(draw_list: &DrawListMut<'_>) {
    /// draw_list.with_draw_flags(DrawFlags::AA_LINE_ENDS, true, || {
    ///     draw_list.add_line([10.0, 10.0], [30.0, 20.0], [1.0; 4])
    ///         .thickness(4.0)
    ///         .flags(DrawFlags::CAP_SQUARE)
    ///         .build();
    /// });
    /// # }
    /// ```
    #[doc(alias = "PushDrawFlag", alias = "PopDrawFlag")]
    pub fn with_draw_flags<R>(&self, flags: DrawFlags, enabled: bool, f: impl FnOnce() -> R) -> R {
        const CALLER: &str = "DrawListMut::with_draw_flags()";
        assert_draw_flags(
            CALLER,
            flags,
            DrawFlags::from_bits_retain(sys::ImDrawFlags_AllowInPushScope_ as u32),
        );
        let stroke = flags.bits() & sys::ImDrawFlags_StrokeMask_ as u32;
        assert!(
            stroke == 0 || stroke == DrawFlags::STROKE_LEGACY.bits(),
            "DrawListMut::with_draw_flags() only supports STROKE_LEGACY as a scoped stroke position"
        );
        let guard = self.with_raw_bound(CALLER, |draw_list| unsafe {
            let current = DrawFlags::from_bits_truncate((*draw_list).Flags as u32);
            let effective = if enabled {
                current | flags
            } else {
                current.difference(flags)
            };
            assert_draw_flags(CALLER, effective & CORNER_FLAGS, CORNER_FLAGS);
            let stack = &(*draw_list)._DrawFlagsStack;
            assert!(
                usize::from(stack.Size) < stack.LocalData.len(),
                "DrawListMut::with_draw_flags() exceeded the native draw-flag scope capacity"
            );
            sys::ImDrawList_PushDrawFlag(draw_list, flags.bits() as sys::ImDrawFlags, enabled);
            DrawFlagsGuard {
                draw_list,
                scope: self.ui.map(|ui| {
                    ui.begin_native_scope(
                        crate::scope::NativeScopePop::DrawListPopDrawFlag {
                            draw_list,
                            window_scoped: self.is_window_scoped(),
                        },
                        "DrawListDrawFlagScope",
                    )
                }),
            }
        });
        let result = f();
        drop(guard);
        result
    }
}
