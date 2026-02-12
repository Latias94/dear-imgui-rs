use static_assertions::assert_impl_all;
use static_assertions::assert_not_impl_any;

// Compile-time checks for Send/Sync markers
#[test]
fn context_and_fonts_thread_markers() {
    // Context must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui_rs::Context: Send, Sync);

    // Font and FontAtlas must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui_rs::Font: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::FontAtlas: Send, Sync);

    // OwnedDrawData must NOT be Send/Sync (retains shared textures list pointer)
    assert_not_impl_any!(dear_imgui_rs::render::draw_data::OwnedDrawData: Send, Sync);

    // Threaded snapshot types MUST be Send/Sync
    assert_impl_all!(dear_imgui_rs::render::snapshot::FrameSnapshot: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::DrawDataSnapshot: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::DrawListSnapshot: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::DrawCmdSnapshot: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureRequest: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureFeedback: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureOp: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureUploadRect: Send, Sync);

    // DrawData/DrawList views (render module) are frame-bound, not thread-safe
    assert_not_impl_any!(dear_imgui_rs::render::draw_data::DrawData: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::render::draw_data::DrawList: Send, Sync);

    // Immediate draw list handle is UI-thread bound
    assert_not_impl_any!(dear_imgui_rs::DrawListMut<'static>: Send, Sync);
}
