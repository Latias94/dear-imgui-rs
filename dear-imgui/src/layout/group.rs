use crate::Ui;
use crate::sys;

create_token!(
    /// Tracks a layout group that can be ended with `end` or by dropping.
    pub struct GroupToken<'ui>;

    /// Drops the layout group manually. You can also just allow this token
    /// to drop on its own.
    drop { unsafe { sys::igEndGroup() } }
);

impl Ui {
    /// Creates a layout group and starts appending to it.
    ///
    /// Returns a `GroupToken` that must be ended by calling `.end()`.
    #[doc(alias = "BeginGroup")]
    pub fn begin_group(&self) -> GroupToken<'_> {
        unsafe { sys::igBeginGroup() };
        GroupToken::new(self)
    }

    /// Creates a layout group and runs a closure to construct the contents.
    ///
    /// May be useful to handle the same mouse event on a group of items, for example.
    #[doc(alias = "BeginGroup")]
    pub fn group<R, F: FnOnce() -> R>(&self, f: F) -> R {
        let group = self.begin_group();
        let result = f();
        group.end();
        result
    }
}
