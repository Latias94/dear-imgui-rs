//! RAII tokens for scoped ImGui state
//!
//! Many Dear ImGui operations push/pop state (style, fonts, groups, clip rects,
//! etc.). In this crate, these are modeled as small RAII tokens that pop the
//! state when dropped, helping you write exception-safe, early-return friendly
//! code.
//!
//! Example:
//! ```no_run
//! # use dear_imgui::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let _group = ui.begin_group();
//! ui.text("Inside a group");
//! // Group ends automatically when `_group` is dropped
//! ```
//!
//! Quick example (manual end):
//! ```no_run
//! # use dear_imgui::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let token = ui.begin_group();
//! ui.text("Manual end");
//! token.end(); // explicit end instead of relying on Drop
//! ```
//!
#[macro_export]
/// This is a macro used internally by dear-imgui to create StackTokens
/// representing various global state in Dear ImGui.
///
/// These tokens can either be allowed to drop or dropped manually
/// by calling `end` on them. Preventing this token from dropping,
/// or moving this token out of the block it was made in can have
/// unintended side effects, including failed asserts in the Dear ImGui C++.
///
/// In general, if you're looking at this, don't overthink these -- just slap
/// a `_token` as their binding name and allow them to drop.
macro_rules! create_token {
    (
        $(#[$struct_meta:meta])*
        $v:vis struct $token_name:ident<'ui>;

        $(#[$end_meta:meta])*
        drop { $on_drop:expr }
    ) => {
        #[must_use]
        $(#[$struct_meta])*
        pub struct $token_name<'a>(std::marker::PhantomData<&'a $crate::Ui>);

        impl<'a> $token_name<'a> {
            /// Creates a new token type.
            pub(crate) fn new(_: &'a $crate::Ui) -> Self {
                Self(std::marker::PhantomData)
            }

            $(#[$end_meta])*
            #[inline]
            pub fn end(self) {
                // left empty for drop
            }
        }

        impl Drop for $token_name<'_> {
            fn drop(&mut self) {
                // Execute provided drop expression; callers wrap unsafe if needed
                $on_drop
            }
        }
    }
}
