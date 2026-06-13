use crate::{Ui, sys};

/// # ID stack
impl Ui {
    /// Pushes an identifier to the ID stack.
    ///
    /// Returns an `IdStackToken` that can be popped by calling `.end()`
    /// or by dropping manually.
    ///
    /// # Examples
    /// Dear ImGui uses labels to uniquely identify widgets. For a good explanation, see this part of the [Dear ImGui FAQ][faq]
    ///
    /// [faq]: https://github.com/ocornut/imgui/blob/v1.84.2/docs/FAQ.md#q-why-is-my-widget-not-reacting-when-i-click-on-it
    ///
    /// In `dear-imgui-rs` the same applies, we can manually specify labels with the `##` syntax:
    ///
    /// ```no_run
    /// # let mut imgui = dear_imgui_rs::Context::create();
    /// # let ui = imgui.frame();
    ///
    /// ui.button("Click##button1");
    /// ui.button("Click##button2");
    /// ```
    ///
    /// But sometimes we want to create widgets in a loop, or we want to avoid
    /// having to manually give each widget a unique label. In these cases, we can
    /// push an ID to the ID stack:
    ///
    /// ```no_run
    /// # let mut imgui = dear_imgui_rs::Context::create();
    /// # let ui = imgui.frame();
    ///
    /// for i in 0..10 {
    ///     let _id = ui.push_id(i);
    ///     ui.button("Click");
    /// }
    /// ```
    #[doc(alias = "PushID")]
    pub fn push_id<'a, T: Into<Id<'a>>>(&self, id: T) -> IdStackToken<'_> {
        let id = id.into();
        self.run_with_bound_context(|| unsafe {
            match id {
                Id::Int(i) => sys::igPushID_Int(i),
                Id::Str(s) => sys::igPushID_Str(self.scratch_txt(s)),
                Id::Ptr(p) => sys::igPushID_Ptr(p),
            }
        });
        IdStackToken::new(self)
    }
}

create_token!(
    /// Tracks an ID pushed to the ID stack that can be popped by calling `.pop()`
    /// or by dropping. See [`crate::Ui::push_id`] for more details.
    pub struct IdStackToken<'ui>;

    /// Pops a change from the ID stack
    drop { unsafe { sys::igPopID() } }
);

impl IdStackToken<'_> {
    /// Pops a change from the ID stack.
    pub fn pop(self) {
        self.end()
    }
}

// ============================================================================
// Focus scope stack
// ============================================================================

create_token!(
    /// Tracks a pushed focus scope, popped on drop.
    pub struct FocusScopeToken<'ui>;

    /// Pops a focus scope.
    #[doc(alias = "PopFocusScope")]
    drop { unsafe { sys::igPopFocusScope() } }
);

impl Ui {
    /// Push a focus scope (affects e.g. navigation focus allocation).
    ///
    /// Returns a `FocusScopeToken` which will pop the focus scope when dropped.
    #[doc(alias = "PushFocusScope")]
    pub fn push_focus_scope(&self, id: crate::Id) -> FocusScopeToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igPushFocusScope(id.raw()) });
        FocusScopeToken::new(self)
    }
}

/// Represents an identifier that can be pushed to the ID stack
#[derive(Copy, Clone, Debug)]
pub enum Id<'a> {
    /// Integer identifier
    Int(i32),
    /// String identifier
    Str(&'a str),
    /// Pointer identifier
    Ptr(*const std::ffi::c_void),
}

impl From<i32> for Id<'_> {
    fn from(i: i32) -> Self {
        Id::Int(i)
    }
}

impl From<usize> for Id<'_> {
    fn from(i: usize) -> Self {
        Id::Int(i as i32)
    }
}

impl<'a> From<&'a str> for Id<'a> {
    fn from(s: &'a str) -> Self {
        Id::Str(s)
    }
}

impl<'a> From<&'a String> for Id<'a> {
    fn from(s: &'a String) -> Self {
        Id::Str(s.as_str())
    }
}

impl<T> From<*const T> for Id<'_> {
    fn from(p: *const T) -> Self {
        Id::Ptr(p as *const std::ffi::c_void)
    }
}

impl<T> From<*mut T> for Id<'_> {
    fn from(p: *mut T) -> Self {
        Id::Ptr(p as *const std::ffi::c_void)
    }
}
