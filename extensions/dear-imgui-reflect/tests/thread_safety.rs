use static_assertions::assert_not_impl_any;

#[test]
fn reflection_pass_state_is_ui_thread_bound() {
    assert_not_impl_any!(dear_imgui_reflect::ReflectSession: Send, Sync);
    assert_not_impl_any!(dear_imgui_reflect::Inspector<'static, 'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_reflect::InspectorPathGuard: Send, Sync);
}
