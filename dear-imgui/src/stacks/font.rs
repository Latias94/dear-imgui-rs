use crate::fonts::FontId;
use crate::{Ui, sys};

/// # Parameter stacks (shared)
impl Ui {
    /// Switches to the given font at its configured reference size.
    ///
    /// Dear ImGui 1.92 can rasterize a font at multiple sizes. This convenience
    /// method preserves the pre-1.92 behavior by using the reference size
    /// supplied when the font was added. Use [`Ui::push_font_with_size`] to
    /// preserve the current size or select another runtime size explicitly.
    /// A font without a reference size also preserves the current size.
    ///
    /// Returns a `FontStackToken` that must be popped by calling `.pop()`
    ///
    /// # Panics
    ///
    /// Panics before calling Dear ImGui if the `FontId` came from a different atlas,
    /// was invalidated by font atlas mutation, or is no longer present in the
    /// current context's atlas.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let font_data_sources = [];
    /// // At initialization time
    /// let my_custom_font = ctx.font_atlas().add_font(&font_data_sources);
    /// # let ui = ctx.frame();
    /// // During UI construction
    /// let font = ui.push_font(my_custom_font);
    /// ui.text("I use the custom font!");
    /// font.pop();
    /// ```
    #[doc(alias = "PushFont")]
    pub fn push_font(&self, id: FontId) -> FontStackToken<'_> {
        self.run_with_bound_context(|| unsafe {
            let font_ptr =
                crate::fonts::validate_font_id_for_current_context(id, "Ui::push_font()");
            sys::igPushFont(font_ptr, (*font_ptr).LegacySize);
        });
        FontStackToken::new(self)
    }
}

create_token!(
    /// Tracks a font pushed to the font stack that can be popped by calling `.end()`
    /// or by dropping.
    #[doc(alias = "PopFont")]
    pub struct FontStackToken<'ui>;

    /// Pops a change from the font stack.
    drop { unsafe { sys::igPopFont() } }
);

impl FontStackToken<'_> {
    /// Pops a change from the font stack.
    pub fn pop(self) {
        self.end()
    }
}

#[cfg(test)]
mod tests {
    const ROBOTO_MEDIUM: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
    ));

    #[test]
    fn push_font_uses_the_size_supplied_when_the_font_was_added() {
        let mut ctx = crate::Context::create();
        let small = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(13.0)]);
        let large = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(29.0)]);
        assert_eq!(small.reference_size(), Some(13.0));
        assert_eq!(large.reference_size(), Some(29.0));
        let _ = ctx.font_atlas().build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);

        let ui = ctx.frame();
        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);

        {
            let _font = ui.push_font(large);
            assert_eq!(ui.current_font(), large);
            assert_eq!(ui.current_font_size(), 29.0);
        }

        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);
    }

    #[test]
    fn push_font_preserves_current_size_without_a_reference_size() {
        let mut ctx = crate::Context::create();
        let _consumer = ctx
            .create_renderer_consumer()
            .expect("the managed renderer consumer should attach");
        let small = ctx
            .font_atlas()
            .add_font(&[crate::FontSource::default_font_with_size(13.0)]);
        // SAFETY: the vendored bytes contain the complete, unmodified Roboto Medium TTF.
        let dynamic = ctx
            .font_atlas()
            .add_font(&[unsafe { crate::FontSource::ttf_data(ROBOTO_MEDIUM) }]);
        assert_eq!(dynamic.reference_size(), None);
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.io_mut()
            .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);

        let ui = ctx.frame();
        assert_eq!(ui.current_font(), small);
        assert_eq!(ui.current_font_size(), 13.0);

        let _font = ui.push_font(dynamic);
        assert_eq!(ui.current_font(), dynamic);
        assert_eq!(ui.current_font_size(), 13.0);
    }
}
