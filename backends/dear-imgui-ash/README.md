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

- Supports Dear ImGui 1.92+ managed texture create/update/destroy requests through `RenderedFrame`.
- Sets `ImGuiBackendFlags_RendererHasTextures` and `ImGuiBackendFlags_RendererHasVtxOffset`.
- Upload path uses in-flight fences to avoid `vkQueueWaitIdle` stalls.
- Sub-rect texture updates (uses `UpdateRect` bounding box).

## Managed textures

`AshRenderer::cmd_draw` consumes a `RenderedFrame`, uploads its owned texture requests, reconciles
request-bound feedback, and only then reads the frame's immutable draw data.

- Font atlas textures are registered by ImGui itself.
- Register an `OwnedTextureData` with `Context::register_texture(texture)`. Registration transfers
  ownership to the Context and returns a `ManagedTextureId` for widgets and draw lists.
- Use `Context::with_texture_mut(id, |texture| ...)` for later pixel updates and
  `Context::remove_texture(id)` to begin renderer-aware retirement.

Safe image APIs do not accept borrowed `TextureData`; this prevents native draw commands from
retaining a pointer after the Rust borrow ends.

### GPU-safe retirement

Recording a Vulkan command buffer does not mean the GPU has finished using its textures. A destroy
request therefore moves the managed texture into a retirement queue instead of immediately freeing
it or acknowledging the request.

`AshRenderer::cmd_draw` returns the highest pending `TextureRetirementBatch`. The safe
`wait_for_texture_retirements(batch)` path waits for device idle and only then releases the Vulkan
resources. The next frame can then acknowledge Dear ImGui's repeated destroy request.
`pending_texture_retirement()` recovers the current token if command recording returns an error
after retirement began.

Advanced render loops may instead associate the batch with synchronization submitted after all
relevant Ash uploads, main-viewport commands, and secondary-viewport commands. Once every relevant
queue has completed, `unsafe complete_texture_retirements_with_fences(batch, fences)` validates
that each supplied fence is signaled before releasing anything. The call remains unsafe because
Vulkan cannot prove fence device lineage or that the supplied fences cover every queue which could
still reference the batch.

The retirement protocol is identical for classic render passes and dynamic rendering. In multi-
viewport mode, render/submit the main viewport, run the secondary viewport renderer callbacks, and
only then establish the completion point associated with the batch. Merely finishing command
recording is never sufficient.

Call `AshRenderer::shutdown(&mut imgui)` before dropping a single-viewport Context or renderer.
Shutdown waits for device idle, destroys active and retiring GPU textures, resets Context-owned
renderer bindings, and then releases the renderer consumer. In multi-viewport mode, call the
owning renderer runtime's `shutdown(&mut imgui)` before shutting down the platform runtime.

## External textures & custom sampler

To display an existing Vulkan image via the legacy `TextureId` path:

- `AshRenderer::register_external_texture_with_sampler(image_view, sampler) -> TextureId`
- `AshRenderer::update_external_texture_view(texture_id, image_view) -> RendererResult<bool>`
- `AshRenderer::update_external_texture_sampler(texture_id, sampler) -> RendererResult<bool>`
- `AshRenderer::unregister_texture(texture_id) -> RendererResult<()>` (frees the descriptor set only for textures
  registered via `register_external_texture_with_sampler()`)

These safe mutation and removal methods wait for device idle before changing a descriptor set. The
matching `unsafe *_unchecked` methods are available when the application can independently prove
that no submitted or recorded command still accesses that descriptor set.

## Native multi-viewport

Select exactly one surface adapter:

- `multi-viewport-winit`: Winit owns the platform windows; `ash-window` creates each surface.
- `multi-viewport-sdl3`: SDL3 owns the platform windows and supplies
  `ImGuiPlatformIO::Platform_CreateVkSurface`.

The features are mutually exclusive, native-only, and each enables
`dear-imgui-rs/multi-viewport`. Do not use workspace `--all-features` for this crate. The Winit
route must create `dear_imgui_winit::multi_viewport::WinitPlatformRuntime`; the SDL3 route must
initialize `Sdl3PlatformBackend::init_for_vulkan` before attaching the renderer runtime.

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
The runtime never destroys it. Before claiming callback slots, `attach` checks that required
handles are non-null, queue-family indices are in range and expose queues, the graphics family
supports graphics, and the present family can present color-attachment swapchains with at least
one format and present mode on this surface. Winit attachment also verifies the active platform
backend name. SDL3 attachment verifies both the SDL3 backend name and its
`Platform_CreateVkSurface` capability. An invalid configuration or adapter therefore fails before
the renderer is consumed or any renderer callback is published; `AshViewportAttachError` returns
the unchanged renderer to the caller.

### Winit integration

Initialize the owning Winit platform runtime first, then consume the renderer into
`WinitViewportRuntime` before Dear ImGui creates any secondary platform window:

```rust,no_run
use dear_imgui_ash::{AshRenderer, multi_viewport as ash_mvp};
use dear_imgui_rs::Context;
use dear_imgui_winit::multi_viewport as winit_mvp;
use std::sync::Arc;

# fn attach_viewports(
#     renderer: AshRenderer,
#     imgui: &mut Context,
#     main_window: Arc<winit::window::Window>,
#     config: ash_mvp::VulkanViewportConfig,
# ) -> Result<
#     (winit_mvp::WinitPlatformRuntime, ash_mvp::WinitViewportRuntime),
#     Box<dyn std::error::Error>,
# > {
imgui.enable_multi_viewport();
let platform = winit_mvp::WinitPlatformRuntime::new(imgui, main_window)?;

// SAFETY: all raw handles and queue-family indices in config belong to the
// renderer's logical-device lineage. The wrapper owns renderer address stability.
let renderer = unsafe { ash_mvp::WinitViewportRuntime::attach(imgui, renderer, config)? };
# Ok((platform, renderer))
# }
```

For SDL3, initialize `Sdl3PlatformBackend::init_for_vulkan` first and then call
`multi_viewport_sdl3::Sdl3ViewportRuntime::attach`. Both adapters use the same
`VulkanViewportConfig` and backend-local runtime control.

Each frame, render and present the main window first, then render secondary windows:

```rust,no_run
# use dear_imgui_rs::Context;
# fn render_secondary_windows(imgui: &mut Context) {
imgui.update_platform_windows();
imgui.render_platform_windows_default();
# }
```

### Ownership and shutdown

The Ash runtime owns the renderer in stable boxed storage and claims only the five `Renderer_*`
slots. The wrapper itself may be moved safely. Attachment refuses occupied renderer callbacks,
foreign `RendererUserData`, an already registered renderer, missing platform lifecycle callbacks,
and secondary platform windows created before renderer registration. It never overwrites platform
slots. Callback panics, reentry, Vulkan failures, and ownership drift are contained and reported by
the next Rust entry point such as `poll_fault` or `cmd_draw`.

Shut down the renderer runtime before the platform backend:

```rust,no_run
# use dear_imgui_ash::multi_viewport::WinitViewportRuntime;
# use dear_imgui_rs::Context;
# use dear_imgui_winit::multi_viewport::WinitPlatformRuntime;
# fn shutdown(
#     renderer: &mut WinitViewportRuntime,
#     platform: &mut WinitPlatformRuntime,
#     imgui: &mut Context,
# ) -> Result<(), Box<dyn std::error::Error>> {
renderer.shutdown(imgui)?;
platform.shutdown()?;
# Ok(())
# }
```

The renderer runtime destroys secondary renderer resources and clears only callback slots it still
owns. Context attachments enforce renderer-before-platform teardown even when the Context is
dropped first. Explicit shutdown remains the preferred path because it reports cleanup errors and
allows recoverable completion-wait failures to be retried before the Vulkan device, instance, or
main validation surface is dropped.

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
| Crate         | 0.16.0  |
| dear-imgui-rs | 0.16.0  |
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
# let command_buffer = vk::CommandBuffer::null();
let frame = imgui.render();
let retirement = renderer.cmd_draw(command_buffer, frame)?;

// Submit command_buffer. The safe path waits for all device work before releasing retired textures.
if let Some(batch) = retirement {
    renderer.wait_for_texture_retirements(batch)?;
}

renderer.shutdown(&mut imgui)?;
# Ok(()) }
```
