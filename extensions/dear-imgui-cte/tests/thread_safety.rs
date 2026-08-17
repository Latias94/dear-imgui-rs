use dear_imgui_cte::{
    Language, NotificationType, Notifications, NotificationsRenderer, Palette, Position, Selection,
    TextDiff, TextDiffRenderer, TextEditor, TextEditorRenderer,
};
use static_assertions::{assert_impl_all, assert_not_impl_any};

#[test]
fn native_editor_owners_and_frame_builders_are_not_thread_safe() {
    assert_not_impl_any!(TextEditor: Send, Sync, Clone);
    assert_not_impl_any!(TextEditorRenderer<'static, 'static>: Send, Sync);
    assert_not_impl_any!(TextDiff: Send, Sync, Clone);
    assert_not_impl_any!(TextDiffRenderer<'static, 'static>: Send, Sync);
    assert_not_impl_any!(Notifications: Send, Sync, Clone);
    assert_not_impl_any!(NotificationsRenderer<'static, 'static>: Send, Sync);
}

#[test]
fn copied_value_types_are_plain_thread_safe_rust_values() {
    assert_impl_all!(Position: Send, Sync, Copy, Clone);
    assert_impl_all!(Selection: Send, Sync, Copy, Clone);
    assert_impl_all!(Language: Send, Sync, Copy, Clone);
    assert_impl_all!(Palette: Send, Sync, Clone);
    assert_impl_all!(NotificationType: Send, Sync, Copy, Clone);
}
