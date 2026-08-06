//! RAII tokens for scoped ImGui state
//!
//! Many Dear ImGui operations push/pop native state. Public tokens remain useful for advanced
//! control. Tokens backed by the same native resource stack must finish in LIFO order, while
//! independent stacks may be ended independently. Window- and table-local tokens must also finish
//! in the exact native scope that created them, and a table cannot end while a native scope created
//! inside it remains active. Invalid order or provenance is diagnosed before FFI and native
//! cleanup is deferred when the original scope can be restored safely. Closure helpers are the
//! canonical path because lexical nesting makes those contracts visible.
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let _group = ui.begin_group();
//! ui.text("Inside a group");
//! // Group ends automatically when `_group` is dropped
//! ```
//!
//! Quick example (manual end):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let token = ui.begin_group();
//! ui.text("Manual end");
//! token.end(); // explicit end instead of relying on Drop
//! ```
//!
/// This is a macro used internally by dear-imgui to create StackTokens
/// representing various global state in Dear ImGui.
///
/// These tokens may be dropped or ended explicitly. Tokens sharing a native resource stack must
/// finish in reverse creation order, and window- or table-local tokens must finish in their
/// originating scope. A table cannot end while a native scope created inside that table remains
/// active. The shared tracker rejects violations before entering Dear ImGui and defers cleanup
/// until the original scope is current when that recovery is valid.
macro_rules! create_token {
    (
        $(#[$struct_meta:meta])*
        $v:vis struct $token_name:ident<'ui>;

        pop $pop:expr;

        $(#[$end_meta:meta])*
        drop { $on_drop:expr }
    ) => {
        #[must_use]
        $(#[$struct_meta])*
        #[doc = "\n# Drop order\n\nTokens sharing this native resource stack must finish in reverse creation order. Window- or table-local tokens must also finish in their originating scope. Prefer the corresponding closure helper when available. Invalid use panics before FFI, with cleanup deferred when the original scope can be restored safely."]
        pub struct $token_name<'a> {
            scope: $crate::scope::NativeScopeToken<'a>,
            _phantom: std::marker::PhantomData<&'a $crate::Ui>,
        }

        impl<'a> $token_name<'a> {
            /// Creates a new token type.
            pub(crate) fn new(ui: &'a $crate::Ui) -> Self {
                Self {
                    scope: ui.begin_native_scope($pop, stringify!($token_name)),
                    _phantom: std::marker::PhantomData,
                }
            }

            $(#[$end_meta])*
            #[doc = "\n# Panics\n\nPanics before FFI if a later token on the same native resource stack is active or this token is no longer in its originating native scope."]
            #[inline]
            pub fn end(self) {
                // left empty for drop
            }
        }

        impl Drop for $token_name<'_> {
            fn drop(&mut self) {
                self.scope.finish_with(|| {
                    // Execute provided drop expression; callers wrap unsafe if needed.
                    $on_drop
                });
            }
        }
    }
}
