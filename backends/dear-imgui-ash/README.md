# dear-imgui-ash

Vulkan (Ash) renderer backend for `dear-imgui-rs`.

This crate is native-only (no `wasm32` support).

## Status

Experimental. API may change.

## Features

- Supports Dear ImGui 1.92+ texture management (`DrawData::textures()`), including create/update/destroy.
- Sets `ImGuiBackendFlags_RendererHasTextures` and `ImGuiBackendFlags_RendererHasVtxOffset`.
- Upload path uses in-flight fences to avoid `vkQueueWaitIdle` stalls.
- Sub-rect texture updates (uses `UpdateRect` bounding box).

## User-created textures (ImTextureData)

`DrawData::textures()` is derived from ImGui's internal `PlatformIO.Textures[]` list.

- Font atlas textures are registered by ImGui itself.
- If you create `TextureData` yourself (e.g. `OwnedTextureData::new()`), register it once via
  `Context::register_user_texture(&mut tex)` so the renderer can receive Create/Update/Destroy
  requests.
  - Prefer `Context::register_user_texture_token(&mut tex)` to automatically unregister on drop.

If you skip registration and still use `&mut TextureData` in widgets, `ImDrawCmd_GetTexID()` may
assert in debug builds when the draw command refers to a texture that was never uploaded (TexID=0).

## External textures & custom sampler

To display an existing Vulkan image via the legacy `TextureId` path:

- `AshRenderer::register_external_texture_with_sampler(image_view, sampler) -> TextureId`
- `AshRenderer::update_external_texture_view(texture_id, image_view) -> bool`
- `AshRenderer::update_external_texture_sampler(texture_id, sampler) -> bool`
- `AshRenderer::unregister_texture(texture_id)` (frees the descriptor set only for textures
  registered via `register_external_texture_with_sampler()`)

## Multi-viewport (winit)

This crate can render Dear ImGui secondary viewports (additional OS windows) when using the winit
platform backend.

Requirements:

- Enable the renderer feature: `dear-imgui-ash/multi-viewport-winit`
- Enable the core feature: `dear-imgui-rs/multi-viewport`
- Use `dear-imgui-winit` multi-viewport helpers to create/destroy OS windows and route events

Minimal integration outline:

```rust,no_run
use dear_imgui_ash::multi_viewport as ash_mvp;
use dear_imgui_winit::multi_viewport as winit_mvp;

# fn example(
#   renderer: &mut dear_imgui_ash::AshRenderer,
#   imgui: &mut dear_imgui_rs::Context,
#   window: &winit::window::Window,
#   entry: ash::Entry,
#   instance: ash::Instance,
#   physical_device: ash::vk::PhysicalDevice,
#   present_queue: ash::vk::Queue,
#   graphics_queue_family_index: u32,
#   present_queue_family_index: u32,
# ) {
imgui.enable_multi_viewport();
winit_mvp::init_multi_viewport_support(imgui, window);

// Install renderer callbacks (do this after the renderer is in its final memory location).
ash_mvp::enable(
    renderer,
    imgui,
    entry,
    instance,
    physical_device,
    present_queue,
    graphics_queue_family_index,
    present_queue_family_index,
);

// Each frame (after rendering/presenting the main window):
imgui.update_platform_windows();
imgui.render_platform_windows_default();
# }
```

Notes:

- Secondary viewports create their own Vulkan `SurfaceKHR` + swapchain via `ash-window`.
- Swapchain formats may differ per viewport. The renderer caches pipelines per `vk::Format`.

## sRGB / Gamma

This backend follows the same approach as the WGPU backend in this repo:

- ImGui colors/texels are treated as sRGB values stored in UNORM formats.
- When rendering into an sRGB framebuffer, the fragment shader applies `pow(color.rgb, 2.2)` to
  convert to linear before output (so the sRGB render target can encode correctly).

If your swapchain/render target uses an sRGB format (e.g. `VK_FORMAT_B8G8R8A8_SRGB`), set
`Options::framebuffer_srgb = true`.

Note: internally managed textures default to `vk::Format::R8G8B8A8_UNORM` (not `*_SRGB`) to keep
this behavior consistent. If you register external descriptor sets that sample from `*_SRGB`
textures, the shader gamma path will not match (you'll effectively decode twice).

## Reference

This backend is inspired by the excellent `imgui-rs-vulkan-renderer` project:
<https://github.com/adrien-ben/imgui-rs-vulkan-renderer>

## Quick start

```rust,no_run
use ash::vk;
use dear_imgui_ash::{AshRenderer, Options};
use dear_imgui_rs::Context;

# fn example() -> Result<(), dear_imgui_ash::RendererError> {
// Create your Vulkan instance/device/queue/command_pool/render_pass first...
# let (instance, physical_device, device, queue, command_pool, render_pass) = todo!();

let mut imgui = Context::create();
let mut renderer = AshRenderer::with_default_allocator(
    &instance,
    physical_device,
    device.clone(),
    queue,
    command_pool,
    render_pass,
    &mut imgui,
    Some(Options::default()),
)?;

// In your render loop (inside a render pass):
// let draw_data = imgui.render();
// renderer.cmd_draw(command_buffer, &draw_data)?;
# let _ = vk::CommandBuffer::null();
# Ok(()) }
```
