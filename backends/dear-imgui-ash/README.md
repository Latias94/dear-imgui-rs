# dear-imgui-ash

Vulkan (Ash) renderer backend for `dear-imgui-rs`.

Vulkan rendering is native-only. The wasm32 stub returns `RendererError::UnsupportedTarget`.

## Status

Experimental. API may change.

## Loader (linked vs loaded)

This backend is compatible with both `ash` loader modes:

- **Loaded (recommended for CI)**: use `ash::Entry::load()` (runtime loader, no `vulkan-1.lib` link).
- **Linked**: use `ash::Entry::linked()` (requires Vulkan loader import library at link time, e.g. Vulkan SDK).

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

## Native multi-viewport

Select exactly one surface adapter:

- `multi-viewport-winit`: Winit owns the platform windows; `ash-window` creates each surface.
- `multi-viewport-sdl3`: SDL3 owns the platform windows and supplies
  `ImGuiPlatformIO::Platform_CreateVkSurface`.

The features are mutually exclusive, native-only, and each enables
`dear-imgui-rs/multi-viewport`. Do not use workspace `--all-features` for this crate. The Winit
route must call `dear_imgui_winit::multi_viewport::init_multi_viewport_support`; the SDL3 route
must call `dear_imgui_sdl3::init_for_vulkan` before installing the renderer.

### VulkanViewportConfig and preflight

`VulkanViewportConfig` carries application-owned Vulkan handles for secondary swapchains:

```rust,no_run
# use ash::vk;
# use dear_imgui_ash::multi_viewport::VulkanViewportConfig;
# fn config(
#     entry: ash::Entry,
#     instance: ash::Instance,
#     physical_device: vk::PhysicalDevice,
#     main_surface: vk::SurfaceKHR,
#     present_queue: vk::Queue,
#     graphics_family: u32,
#     present_family: u32,
# ) -> VulkanViewportConfig {
VulkanViewportConfig {
    entry,
    instance,
    physical_device,
    validation_surface: main_surface,
    present_queue,
    graphics_queue_family_index: graphics_family,
    present_queue_family_index: present_family,
}
# }
```

All handles must have one device lineage: the instance owns the physical device and
`validation_surface`; `AshRenderer`'s device was created from that physical device with
`VK_KHR_swapchain`; both queues belong to that device and to the declared families. The unsafe
entry point cannot prove those raw-handle relationships.

`validation_surface` is an existing, live application surface, normally the main window surface.
The runtime never destroys it. Before claiming callback slots, `enable` checks that required
handles are non-null, queue-family indices are in range and expose queues, the graphics family
supports graphics, and the present family can present color-attachment swapchains with at least
one format and present mode on this surface. This makes an invalid configuration fail before any
secondary window callback can run.

### Winit integration

Place the renderer, or an owner containing it, at a stable address. Initialize the platform first,
then install the renderer before Dear ImGui creates any secondary platform window:

```rust,no_run
use dear_imgui_ash::{AshRenderer, multi_viewport as ash_mvp};
use dear_imgui_rs::Context;
use dear_imgui_winit::multi_viewport as winit_mvp;

# fn enable_viewports(
#     renderer: &mut Box<AshRenderer>,
#     imgui: &mut Context,
#     main_window: &winit::window::Window,
#     config: ash_mvp::VulkanViewportConfig,
# ) -> Result<(), Box<dyn std::error::Error>> {
imgui.enable_multi_viewport();
winit_mvp::init_multi_viewport_support(imgui, main_window);

// SAFETY: Box keeps the renderer address stable. The context, renderer,
// platform windows, and Vulkan objects remain live and serialized until shutdown.
unsafe { ash_mvp::enable(renderer.as_mut(), imgui, config)? };
# Ok(())
# }
```

For SDL3, replace the two Winit calls with `dear_imgui_sdl3::init_for_vulkan` and
`dear_imgui_ash::multi_viewport_sdl3::enable`. Both adapters use the same
`VulkanViewportConfig` and runtime.

Each frame, render and present the main window first, then render secondary windows:

```rust,no_run
# use dear_imgui_rs::Context;
# fn render_secondary_windows(imgui: &mut Context) {
imgui.update_platform_windows();
imgui.render_platform_windows_default();
# }
```

### Ownership and shutdown

The Ash runtime claims only the five `Renderer_*` slots. It refuses occupied renderer callbacks,
foreign `RendererUserData`, an already registered renderer, missing platform lifecycle callbacks,
and secondary platform windows created before renderer registration. It never overwrites platform
slots. While active, do not move, reinitialize, shut down, concurrently access, or drop the
renderer, and do not replace viewport `RendererUserData`.

Shut down the renderer runtime before the platform backend:

```rust,no_run
# use dear_imgui_ash::multi_viewport as ash_mvp;
# use dear_imgui_rs::Context;
# use dear_imgui_winit::multi_viewport as winit_mvp;
# fn shutdown(imgui: &mut Context) -> Result<(), ash_mvp::CallbackOwnershipError> {
ash_mvp::shutdown_multi_viewport_support(imgui)?;
winit_mvp::shutdown_multi_viewport_support(imgui);
# Ok(())
# }
```

The renderer helper destroys secondary windows before releasing surfaces, swapchains, and callback
state, and clears only slots it still owns. Explicit shutdown must complete before the renderer,
context, platform backend/windows, Vulkan device, instance, or main validation surface is dropped.

Secondary viewports negotiate their own surface format and extent. Pipelines are cached by
`vk::Format`; resize, minimized extents, out-of-date/suboptimal swapchains, and per-image
synchronization are owned by the shared runtime.

### `NoRendererClear`

Without `ViewportFlags::NO_RENDERER_CLEAR`, a secondary viewport clears to the renderer's viewport
clear color. With the flag, the attachment uses `DONT_CARE`, not `LOAD`, matching the Dear ImGui
renderer contract for a newly acquired swapchain image. Prior contents are undefined; a host that
needs preservation must own a different composition path rather than relying on swapchain history.

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

## Compatibility

| Item          | Version |
|---------------|---------|
| Crate         | 0.15.1  |
| dear-imgui-rs | 0.15.1  |
| ash           | 0.38    |
| ash-window    | 0.13 (`multi-viewport-winit`) |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

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
