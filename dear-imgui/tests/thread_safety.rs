use static_assertions::{assert_impl_all, assert_not_impl_any};

// Compile-time checks for Send/Sync markers
#[test]
fn context_and_fonts_thread_markers() {
    // Context must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui::Context: Send, Sync);

    // Font and FontAtlas must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui::Font: Send, Sync);
    assert_not_impl_any!(dear_imgui::FontAtlas: Send, Sync);

    // OwnedDrawData SHOULD be Send + Sync (deep-copied and owned)
    assert_impl_all!(dear_imgui::render::draw_data::OwnedDrawData: Send, Sync);
}
