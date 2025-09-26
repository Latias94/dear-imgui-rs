use static_assertions::assert_not_impl_any;

// Compile-time checks for Send/Sync markers
#[test]
fn context_and_fonts_thread_markers() {
    // Context must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui::Context: Send, Sync);

    // Font and FontAtlas must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui::Font: Send, Sync);
    assert_not_impl_any!(dear_imgui::FontAtlas: Send, Sync);

    // OwnedDrawData must NOT be Send/Sync (retains shared textures list pointer)
    assert_not_impl_any!(dear_imgui::render::draw_data::OwnedDrawData: Send, Sync);

    // DrawData/DrawList views (render module) are frame-bound, not thread-safe
    assert_not_impl_any!(dear_imgui::render::draw_data::DrawData: Send, Sync);
    assert_not_impl_any!(dear_imgui::render::draw_data::DrawList: Send, Sync);

    // Immediate draw list handle is UI-thread bound
    assert_not_impl_any!(dear_imgui::DrawListMut<'static>: Send, Sync);
}
