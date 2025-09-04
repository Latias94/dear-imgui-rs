//! Token system for automatic resource management in Dear ImGui
//!
//! This module provides a token-based system for managing Dear ImGui's various
//! state stacks. Tokens ensure that push/pop operations are properly paired
//! and automatically cleaned up when they go out of scope.
//!
//! This implementation is based on the proven design from imgui-rs.

/// Creates a token type that automatically calls a cleanup function when dropped
///
/// This macro is used internally by dear-imgui to create StackTokens
/// representing various global state in Dear ImGui.
///
/// These tokens can either be allowed to drop or dropped manually
/// by calling `end` on them. Preventing this token from dropping,
/// or moving this token out of the block it was made in can have
/// unintended side effects, including failed asserts in the Dear ImGui C++.
///
/// In general, if you're looking at this, don't overthink these -- just slap
/// a `_token` as their binding name and allow them to drop.
#[macro_export]
macro_rules! create_token {
    (
        $(#[$struct_meta:meta])*
        pub struct $token_name:ident<'ui>;

        $(#[$end_meta:meta])*
        drop { $on_drop:expr }
    ) => {
        #[must_use]
        $(#[$struct_meta])*
        pub struct $token_name<'ui>(std::marker::PhantomData<&'ui $crate::ui::Ui<'ui>>);

        impl<'ui> $token_name<'ui> {
            /// Creates a new token type.
            pub(crate) fn new() -> Self {
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
                unsafe { $on_drop }
            }
        }
    };
}




