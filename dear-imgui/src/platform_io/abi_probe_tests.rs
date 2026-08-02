use crate::sys;

fn public_aggregate_layout_mismatches(
    probe: &sys::DearImguiRsPublicAggregateLayoutProbe,
) -> Vec<&'static str> {
    let mut mismatches = Vec::new();
    macro_rules! require_match {
        ($name:literal, $actual:expr, $expected:expr) => {
            if $actual != $expected {
                mismatches.push($name);
            }
        };
    }

    require_match!(
        "ImDrawData.size",
        probe.ImDrawDataSize,
        std::mem::size_of::<sys::ImDrawData>()
    );
    require_match!(
        "ImDrawData.align",
        probe.ImDrawDataAlign,
        std::mem::align_of::<sys::ImDrawData>()
    );
    require_match!(
        "ImDrawData.FrameCount",
        probe.ImDrawDataFrameCountOffset,
        std::mem::offset_of!(sys::ImDrawData, FrameCount)
    );
    require_match!(
        "ImDrawData.CmdLists",
        probe.ImDrawDataCmdListsOffset,
        std::mem::offset_of!(sys::ImDrawData, CmdLists)
    );
    require_match!(
        "ImDrawData.Textures",
        probe.ImDrawDataTexturesOffset,
        std::mem::offset_of!(sys::ImDrawData, Textures)
    );
    require_match!(
        "ImTextureData.size",
        probe.ImTextureDataSize,
        std::mem::size_of::<sys::ImTextureData>()
    );
    require_match!(
        "ImTextureData.align",
        probe.ImTextureDataAlign,
        std::mem::align_of::<sys::ImTextureData>()
    );
    require_match!(
        "ImTextureData.UniqueID",
        probe.ImTextureDataUniqueIDOffset,
        std::mem::offset_of!(sys::ImTextureData, UniqueID)
    );
    require_match!(
        "ImTextureData.QueueUserData",
        probe.ImTextureDataQueueUserDataOffset,
        std::mem::offset_of!(sys::ImTextureData, QueueUserData)
    );
    require_match!(
        "ImTextureData.TexID",
        probe.ImTextureDataTexIDOffset,
        std::mem::offset_of!(sys::ImTextureData, TexID)
    );
    require_match!(
        "ImTextureData.Updates",
        probe.ImTextureDataUpdatesOffset,
        std::mem::offset_of!(sys::ImTextureData, Updates)
    );
    require_match!(
        "ImTextureData.WantDestroyNextFrame",
        probe.ImTextureDataWantDestroyNextFrameOffset,
        std::mem::offset_of!(sys::ImTextureData, WantDestroyNextFrame)
    );
    require_match!(
        "ImGuiPlatformIO.size",
        probe.ImGuiPlatformIOSize,
        std::mem::size_of::<sys::ImGuiPlatformIO>()
    );
    require_match!(
        "ImGuiPlatformIO.align",
        probe.ImGuiPlatformIOAlign,
        std::mem::align_of::<sys::ImGuiPlatformIO>()
    );
    require_match!(
        "ImGuiPlatformIO.Platform_SessionDate",
        probe.ImGuiPlatformIOSessionDateOffset,
        std::mem::offset_of!(sys::ImGuiPlatformIO, Platform_SessionDate)
    );
    require_match!(
        "ImGuiPlatformIO.Renderer_RenderState",
        probe.ImGuiPlatformIORenderStateOffset,
        std::mem::offset_of!(sys::ImGuiPlatformIO, Renderer_RenderState)
    );
    require_match!(
        "ImGuiPlatformIO.Platform_CreateWindow",
        probe.ImGuiPlatformIOPlatformCreateWindowOffset,
        std::mem::offset_of!(sys::ImGuiPlatformIO, Platform_CreateWindow)
    );
    require_match!(
        "ImGuiPlatformIO.Renderer_CreateWindow",
        probe.ImGuiPlatformIORendererCreateWindowOffset,
        std::mem::offset_of!(sys::ImGuiPlatformIO, Renderer_CreateWindow)
    );
    require_match!(
        "ImGuiPlatformIO.Monitors",
        probe.ImGuiPlatformIOMonitorsOffset,
        std::mem::offset_of!(sys::ImGuiPlatformIO, Monitors)
    );
    require_match!(
        "ImGuiPlatformIO.Textures",
        probe.ImGuiPlatformIOTexturesOffset,
        std::mem::offset_of!(sys::ImGuiPlatformIO, Textures)
    );
    require_match!(
        "ImGuiPlatformIO.Viewports",
        probe.ImGuiPlatformIOViewportsOffset,
        std::mem::offset_of!(sys::ImGuiPlatformIO, Viewports)
    );
    mismatches
}

#[test]
fn cpp_public_aggregate_layout_probe_matches_rust() {
    let mut probe = sys::DearImguiRsPublicAggregateLayoutProbe::default();
    assert!(sys::ImGui_ProbePublicAggregateLayouts(&mut probe));
    assert_eq!(
        public_aggregate_layout_mismatches(&probe),
        Vec::<&str>::new()
    );
}

#[test]
fn public_aggregate_layout_probe_rejects_an_abi_mismatch() {
    let mut probe = sys::DearImguiRsPublicAggregateLayoutProbe::default();
    assert!(sys::ImGui_ProbePublicAggregateLayouts(&mut probe));
    probe.ImTextureDataQueueUserDataOffset += 1;
    assert_eq!(
        public_aggregate_layout_mismatches(&probe),
        vec!["ImTextureData.QueueUserData"]
    );
}
