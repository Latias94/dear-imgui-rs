use static_assertions::assert_impl_all;
use static_assertions::assert_not_impl_any;

// Compile-time checks for Send/Sync markers
#[test]
fn thread_safety_context_and_render_markers() {
    // Context must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui_rs::Context: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ContextBinding: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ContextAttachmentHandle: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ContextAttachmentLease: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ContextPlatformAttachmentRelease<'static>: Send, Sync);
    assert_impl_all!(dear_imgui_rs::ManagedTextureId: Send, Sync);

    // Font handles and FontAtlas must NOT be Send/Sync
    assert_not_impl_any!(dear_imgui_rs::FontAtlas: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::SharedFontAtlas: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::FontId: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::BakedFont<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::CustomRectId: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::CustomRectSnapshot<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::FontAtlasTexture<'static>: Send, Sync);

    assert_not_impl_any!(dear_imgui_rs::render::PendingFrame<'static>: Send, Sync, Clone, Copy);
    assert_not_impl_any!(dear_imgui_rs::render::ReconciledFrame<'static>: Send, Sync, Clone, Copy);
    assert_not_impl_any!(dear_imgui_rs::render::SynchronousRendererConsumer: Send, Sync, Clone);
    assert_not_impl_any!(dear_imgui_rs::render::DetachedRendererConsumer: Send, Sync, Clone);
    assert_impl_all!(dear_imgui_rs::render::DrawRequirements: Send, Sync, Clone, Copy);

    // Detached snapshots move across threads but cannot be cloned.
    assert_impl_all!(dear_imgui_rs::render::snapshot::FrameSnapshot: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::render::snapshot::FrameSnapshot: Clone);
    assert_impl_all!(dear_imgui_rs::render::snapshot::DrawDataSnapshot: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::DrawListSnapshot: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::DrawCmdSnapshot: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureRequest: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureFeedback: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureOp: Send, Sync);
    assert_impl_all!(dear_imgui_rs::render::snapshot::TextureUploadRect: Send, Sync);
    assert_impl_all!(dear_imgui_rs::MultiSelectRequest: Send, Sync, Clone);
    assert_impl_all!(dear_imgui_rs::MultiSelectResult: Send, Sync, Clone);

    // DrawData/DrawList views (render module) are frame-bound, not thread-safe
    assert_not_impl_any!(dear_imgui_rs::render::draw_data::DrawData: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::render::draw_data::DrawList: Send, Sync);

    // Immediate draw list handle is UI-thread bound
    assert_not_impl_any!(dear_imgui_rs::DrawListMut<'static>: Send, Sync);

    // Scoped state storage views remain UI-thread bound.
    assert_not_impl_any!(dear_imgui_rs::StateStorage<'static>: Send, Sync);
}

#[test]
fn thread_safety_core_scope_tokens_are_ui_bound() {
    assert_not_impl_any!(dear_imgui_rs::WindowToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ChildWindowToken<'static>: Send, Sync);

    assert_not_impl_any!(dear_imgui_rs::FontStackToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ColorStackToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::StyleStackToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ItemWidthStackToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TextWrapPosStackToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::IdStackToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::FocusScopeToken<'static>: Send, Sync);

    assert_not_impl_any!(dear_imgui_rs::GroupToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ClipRectToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ListClipperToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::UnknownCountListClipperToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::DragDropSourceTooltip<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::DragDropTarget<'static>: Send, Sync);

    assert_not_impl_any!(dear_imgui_rs::ComboBoxToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ListBoxToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::DisabledToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ButtonRepeatToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::MainMenuBarToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::MenuBarToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::MenuToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::PopupToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::ModalPopupToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TabBarToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TabItemToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TableToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TableBackgroundChannelToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TableColumnChannelToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TooltipToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::TreeNodeToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::MultiSelectScope<'static>: Send, Sync);
}

#[cfg(feature = "stack-layout")]
#[test]
fn thread_safety_stack_layout_tokens_are_ui_bound() {
    assert_not_impl_any!(dear_imgui_rs::HorizontalStackLayoutToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::VerticalStackLayoutToken<'static>: Send, Sync);
    assert_not_impl_any!(dear_imgui_rs::StackLayoutSuspensionToken<'static>: Send, Sync);
}
