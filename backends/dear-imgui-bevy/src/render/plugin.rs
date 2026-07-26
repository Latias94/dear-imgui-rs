//! Render-world plugin installation and Dear ImGui callback ownership.

use super::extract::{
    extract_imgui_bevy_textures, extract_imgui_render_frame, resolve_extracted_imgui_render_routes,
};
use super::pass::{ensure_presentable_window_outputs, render_imgui_overlay};
use super::pipeline::queue_imgui_pipelines;
use super::prepare::{
    commit_imgui_render_frame, initialize_imgui_gpu_resources, prepare_imgui_render_frame,
    prepare_imgui_texture_bind_groups, prepare_imgui_uniform_bind_groups,
    release_imgui_renderer_resources, upload_imgui_buffers,
};
use super::resources::{ImguiRenderDeviceState, ImguiRenderExtractionInstalled};
use super::*;

/// Marker proving the render feature is compiled in.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct RenderFeature;

/// Stable render-world ordering points for passes that compose with Dear ImGui.
///
/// The sets are installed in both the [`Core2d`] and [`Core3d`] schedules after Bevy scene
/// post-processing and before upscaling. Passes in [`Self::AfterOverlay`] must preserve the
/// current single-sample result: resolving an older MSAA attachment can overwrite the UI.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImguiRenderSystems {
    /// Runs after Bevy scene post-processing and before the Dear ImGui overlay.
    BeforeOverlay,
    /// Contains the Dear ImGui overlay pass.
    Overlay,
    /// Runs after Dear ImGui and before Bevy upscaling.
    AfterOverlay,
}

/// Controls the relative order of Dear ImGui and Bevy UI on the same camera.
///
/// This setting takes effect when the `bevy-ui` Cargo feature is enabled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImguiUiRenderOrder {
    /// Draw Bevy UI first and Dear ImGui above it.
    #[default]
    ImguiAboveBevyUi,
    /// Draw Dear ImGui first and Bevy UI above it.
    BevyUiAboveImgui,
}

pub(crate) fn render_integration_available(app: &App) -> bool {
    app.get_sub_app(RenderApp).is_some()
}

pub(crate) fn install_render_extraction(
    app: &mut App,
    ui_render_order: ImguiUiRenderOrder,
) -> bool {
    install_imgui_shader_asset(app);
    app.init_resource::<crate::context::ImguiFrameMailbox>();
    app.init_resource::<ImguiRendererReleases>();
    let snapshot_mailbox = app
        .world()
        .resource::<crate::context::ImguiFrameMailbox>()
        .clone();
    let diagnostics = app
        .world()
        .resource::<crate::route::ImguiDiagnostics>()
        .clone();

    if app.get_sub_app_mut(RenderApp).is_none() {
        return false;
    }

    let renderer_releases = app.world().resource::<ImguiRendererReleases>().clone();

    let render_app = app
        .get_sub_app_mut(RenderApp)
        .expect("RenderApp availability was checked before installing callbacks");

    if render_app
        .world()
        .contains_resource::<ImguiRenderExtractionInstalled>()
    {
        return true;
    }

    render_app
        .init_resource::<ImguiExtractedRenderFrame>()
        .init_resource::<ImguiExtractedBevyTextures>()
        .init_resource::<ImguiPreparedRenderFrame>()
        .init_resource::<ImguiGpuBuffers>()
        .init_resource::<ImguiRenderPipeline>()
        .init_resource::<SpecializedRenderPipelines<ImguiRenderPipeline>>()
        .init_resource::<ImguiTextureBindGroups>()
        .init_resource::<ImguiQueuedPipelines>()
        .init_resource::<ImguiRenderDeviceState>()
        .insert_resource(snapshot_mailbox)
        .insert_resource(renderer_releases)
        .insert_resource(diagnostics)
        .insert_resource(ImguiRenderExtractionInstalled)
        .add_systems(
            RenderStartup,
            initialize_imgui_gpu_resources.ambiguous_with_all(),
        )
        .add_systems(
            ExtractSchedule,
            (extract_imgui_bevy_textures, extract_imgui_render_frame).chain(),
        )
        .add_systems(
            Render,
            (
                release_imgui_renderer_resources,
                resolve_extracted_imgui_render_routes,
                prepare_imgui_render_frame,
                queue_imgui_pipelines,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        )
        .add_systems(
            Render,
            upload_imgui_buffers.in_set(RenderSystems::PrepareResources),
        )
        .add_systems(
            Render,
            (prepare_imgui_texture_bind_groups, commit_imgui_render_frame)
                .chain()
                .in_set(RenderSystems::PrepareBindGroups),
        )
        .add_systems(
            Render,
            prepare_imgui_uniform_bind_groups.in_set(RenderSystems::PrepareBindGroups),
        )
        .add_systems(
            RenderGraph,
            ensure_presentable_window_outputs.in_set(RenderGraphSystems::Finish),
        )
        .configure_sets(
            Core2d,
            (
                ImguiRenderSystems::BeforeOverlay,
                ImguiRenderSystems::Overlay,
                ImguiRenderSystems::AfterOverlay,
            )
                .chain()
                .after(Core2dSystems::PostProcess)
                .before(upscaling),
        )
        .configure_sets(
            Core3d,
            (
                ImguiRenderSystems::BeforeOverlay,
                ImguiRenderSystems::Overlay,
                ImguiRenderSystems::AfterOverlay,
            )
                .chain()
                .after(Core3dSystems::PostProcess)
                .before(upscaling),
        );

    #[cfg(feature = "bevy-ui")]
    match ui_render_order {
        ImguiUiRenderOrder::ImguiAboveBevyUi => {
            render_app
                .add_systems(
                    Core2d,
                    render_imgui_overlay
                        .in_set(ImguiRenderSystems::Overlay)
                        .after(bevy_ui_render::ui_pass),
                )
                .add_systems(
                    Core3d,
                    render_imgui_overlay
                        .in_set(ImguiRenderSystems::Overlay)
                        .after(bevy_ui_render::ui_pass),
                );
        }
        ImguiUiRenderOrder::BevyUiAboveImgui => {
            render_app
                .add_systems(
                    Core2d,
                    render_imgui_overlay
                        .in_set(ImguiRenderSystems::Overlay)
                        .before(bevy_ui_render::ui_pass),
                )
                .add_systems(
                    Core3d,
                    render_imgui_overlay
                        .in_set(ImguiRenderSystems::Overlay)
                        .before(bevy_ui_render::ui_pass),
                );
        }
    }

    #[cfg(not(feature = "bevy-ui"))]
    {
        let _ = ui_render_order;
        render_app
            .add_systems(
                Core2d,
                render_imgui_overlay.in_set(ImguiRenderSystems::Overlay),
            )
            .add_systems(
                Core3d,
                render_imgui_overlay.in_set(ImguiRenderSystems::Overlay),
            );
    }

    true
}

pub(crate) fn install_standard_draw_callbacks_for_context(
    context: &mut imgui::Context,
) -> Result<(), &'static str> {
    if let Some(slot) = standard_draw_callback_conflict(context) {
        return Err(slot);
    }

    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io.set_draw_callback_reset_render_state_raw(Some(imgui_bevy_draw_callback_reset));
        platform_io.set_draw_callback_set_sampler_linear_raw(Some(imgui_bevy_draw_callback_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(imgui_bevy_draw_callback_nearest));
    }
    Ok(())
}

pub(crate) fn standard_draw_callback_conflict(context: &imgui::Context) -> Option<&'static str> {
    let platform_io = context.platform_io();
    for (slot, actual, expected) in [
        (
            "DrawCallback_ResetRenderState",
            platform_io.draw_callback_reset_render_state_raw(),
            imgui_bevy_draw_callback_reset
                as unsafe extern "C" fn(
                    *const imgui::sys::ImDrawList,
                    *const imgui::sys::ImDrawCmd,
                ),
        ),
        (
            "DrawCallback_SetSamplerLinear",
            platform_io.draw_callback_set_sampler_linear_raw(),
            imgui_bevy_draw_callback_linear
                as unsafe extern "C" fn(
                    *const imgui::sys::ImDrawList,
                    *const imgui::sys::ImDrawCmd,
                ),
        ),
        (
            "DrawCallback_SetSamplerNearest",
            platform_io.draw_callback_set_sampler_nearest_raw(),
            imgui_bevy_draw_callback_nearest
                as unsafe extern "C" fn(
                    *const imgui::sys::ImDrawList,
                    *const imgui::sys::ImDrawCmd,
                ),
        ),
    ] {
        if actual.is_some_and(|actual| !std::ptr::fn_addr_eq(actual, expected)) {
            return Some(slot);
        }
    }
    None
}

pub(crate) fn standard_draw_callback_occupied(context: &imgui::Context) -> Option<&'static str> {
    let platform_io = context.platform_io();
    [
        (
            "DrawCallback_ResetRenderState",
            platform_io.draw_callback_reset_render_state_raw(),
        ),
        (
            "DrawCallback_SetSamplerLinear",
            platform_io.draw_callback_set_sampler_linear_raw(),
        ),
        (
            "DrawCallback_SetSamplerNearest",
            platform_io.draw_callback_set_sampler_nearest_raw(),
        ),
    ]
    .into_iter()
    .find_map(|(slot, callback)| callback.is_some().then_some(slot))
}

pub(crate) fn standard_draw_callback_contract(context: &imgui::Context) -> [usize; 3] {
    let platform_io = context.platform_io();
    [
        platform_io
            .draw_callback_reset_render_state_raw()
            .map_or(0, |callback| callback as usize),
        platform_io
            .draw_callback_set_sampler_linear_raw()
            .map_or(0, |callback| callback as usize),
        platform_io
            .draw_callback_set_sampler_nearest_raw()
            .map_or(0, |callback| callback as usize),
    ]
}

pub(crate) fn clear_standard_draw_callbacks_if_owned(context: &mut imgui::Context) {
    let platform_io = context.platform_io_mut();
    unsafe {
        if platform_io
            .draw_callback_reset_render_state_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    imgui_bevy_draw_callback_reset
                        as unsafe extern "C" fn(
                            *const imgui::sys::ImDrawList,
                            *const imgui::sys::ImDrawCmd,
                        ),
                )
            })
        {
            platform_io.set_draw_callback_reset_render_state_raw(None);
        }
        if platform_io
            .draw_callback_set_sampler_linear_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    imgui_bevy_draw_callback_linear
                        as unsafe extern "C" fn(
                            *const imgui::sys::ImDrawList,
                            *const imgui::sys::ImDrawCmd,
                        ),
                )
            })
        {
            platform_io.set_draw_callback_set_sampler_linear_raw(None);
        }
        if platform_io
            .draw_callback_set_sampler_nearest_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    imgui_bevy_draw_callback_nearest
                        as unsafe extern "C" fn(
                            *const imgui::sys::ImDrawList,
                            *const imgui::sys::ImDrawCmd,
                        ),
                )
            })
        {
            platform_io.set_draw_callback_set_sampler_nearest_raw(None);
        }
    }
}

#[used]
static IMGUI_BEVY_DRAW_CALLBACK_RESET_TAG: u8 = 0x31;
#[used]
static IMGUI_BEVY_DRAW_CALLBACK_LINEAR_TAG: u8 = 0x52;
#[used]
static IMGUI_BEVY_DRAW_CALLBACK_NEAREST_TAG: u8 = 0x73;

#[inline(never)]
pub(super) unsafe extern "C" fn imgui_bevy_draw_callback_reset(
    _parent_list: *const imgui::sys::ImDrawList,
    _cmd: *const imgui::sys::ImDrawCmd,
) {
    // The callback address is protocol data. A distinct volatile tag keeps LTO/ICF from folding
    // this marker together with the sampler markers.
    unsafe { std::ptr::read_volatile(&IMGUI_BEVY_DRAW_CALLBACK_RESET_TAG) };
}

#[inline(never)]
pub(super) unsafe extern "C" fn imgui_bevy_draw_callback_linear(
    _parent_list: *const imgui::sys::ImDrawList,
    _cmd: *const imgui::sys::ImDrawCmd,
) {
    unsafe { std::ptr::read_volatile(&IMGUI_BEVY_DRAW_CALLBACK_LINEAR_TAG) };
}

#[inline(never)]
pub(super) unsafe extern "C" fn imgui_bevy_draw_callback_nearest(
    _parent_list: *const imgui::sys::ImDrawList,
    _cmd: *const imgui::sys::ImDrawCmd,
) {
    unsafe { std::ptr::read_volatile(&IMGUI_BEVY_DRAW_CALLBACK_NEAREST_TAG) };
}

fn install_imgui_shader_asset(app: &mut App) {
    app.init_resource::<Assets<Shader>>();
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            IMGUI_SHADER_HANDLE.id(),
            Shader::from_wgsl(IMGUI_SHADER_SOURCE, "dear_imgui_bevy/imgui.wgsl"),
        )
        .expect("UUID shader handles are always valid asset ids");
}
