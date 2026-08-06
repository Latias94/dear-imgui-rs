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

- Supports Dear ImGui 1.92+ managed texture create/update/destroy requests through `PendingFrame`.
- Sets `ImGuiBackendFlags_RendererHasTextures` and `ImGuiBackendFlags_RendererHasVtxOffset`.
- Upload path uses in-flight fences to avoid `vkQueueWaitIdle` stalls.
- Sub-rect texture updates (uses `UpdateRect` bounding box).

### Shader artifacts

Managed textures are tightly packed RGBA bytes and are always stored as
`vk::Format::R8G8B8A8_UNORM`. Use external sampled-image registration for application images with
another compatible format.

Regenerate the checked-in SPIR-V after changing a shader source:

```console
python tools/generate_ash_shaders.py --compiler /path/to/glslangValidator
```

The generator runs each source from the shader directory with a stable relative source name, so
debug metadata does not embed a checkout path. It compiles every output into an isolated temporary
directory and publishes the SPIR-V files and manifest only after the complete set passes validation;
if any replacement fails, the generator restores every previously published artifact.
The compiler-free check verifies manifest hashes, exact embedded `OpSource`, and stable debug
`OpString` filenames:

```console
python tools/generate_ash_shaders.py --check
```

Authoritative CI additionally builds the pinned glslang 15.1.0 source revision and requires a clean
byte-identical regeneration. Run the same contract locally with a matching compiler:

```console
python tools/generate_ash_shaders.py --check --recompile --compiler /path/to/glslangValidator
```

## Managed textures

`AshRenderer::cmd_draw` consumes a `PendingFrame`, uploads its owned texture requests, reconciles
request-bound feedback into a `ReconciledFrame`, and only then reads immutable draw data.

Each `AshRenderer` owns the sole `SynchronousRendererConsumer` generation created for the Context
passed to its constructor. Create one renderer per Context and pass `renderer.renderer_consumer()?`
to `Context::render`. `cmd_draw` rejects a frame from another Context or consumer generation before
recording GPU work, and the consumed frame lease prevents native `DrawData` from escaping its
Context borrow.

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
`pending_texture_retirement() -> RendererResult<Option<TextureRetirementBatch>>` recovers the
current token if command recording returns an error after retirement began. Resource and texture
entries consistently return `RendererDestroyed` after shutdown, before touching Vulkan.

Device idle covers submitted work, not command buffers that were recorded but never submitted.
Retirement, legacy texture updates, and shutdown invalidate any such command buffer that references
released renderer resources. Submitting it afterwards violates `AshRenderer::cmd_draw`'s safety
contract. The application must also wait the fence for an in-flight frame before Ash reuses that
frame's internal mesh slot.

Advanced render loops may instead associate the batch with synchronization submitted after all
relevant Ash uploads, main-viewport commands, and secondary-viewport commands. Once every relevant
queue has completed, `unsafe complete_texture_retirements_with_fences(batch, fences)` validates
that each supplied fence is signaled before releasing anything. The call remains unsafe because
Vulkan cannot prove fence device lineage or that the supplied fences cover every queue which could
still reference the batch.

The retirement protocol is identical for classic render passes and dynamic rendering. In
multi-viewport mode, call the owning runtime's `prepare_context(&mut context)` or consume an existing
pending frame with `prepare_frame(frame)` before any renderer callback. That no-surface phase returns
the `ReconciledFrame` used for secondary and main viewport work, so secondary viewports never observe
the previous texture revision merely because platform WSI requires them to submit before the main
viewport. Establish the completion point only after every relevant secondary and main submission.
Merely finishing command recording is never sufficient.

Call `AshRenderer::shutdown(&mut imgui)` before dropping a single-viewport Context or renderer.
Shutdown waits for device idle, destroys active and retiring GPU textures, resets Context-owned
renderer bindings, and then releases the renderer consumer. In multi-viewport mode, call the
owning renderer runtime's `shutdown(&mut imgui)` before shutting down the platform runtime.
Dropping a renderer while its Context is still alive deliberately does not release Vulkan
resources: `Drop` cannot validate and commit the Context texture-reset transaction. Explicit
shutdown is therefore required for deterministic cleanup; after native Context teardown, `Drop`
may release any remaining Vulkan resources best-effort.

Shutdown prepares the Context texture reset while the complete Vulkan texture map is still intact.
If preparation or a retryable device wait fails, no binding is reset and the renderer keeps its
consumer for a later retry. `ERROR_DEVICE_LOST` is terminal: Ash releases the no-longer-reachable
map, commits the prepared reset, and then returns the original device-loss error.

That order is significant: `Context::prepare_renderer_texture_reset` validates an idle renderer
before resource release, and its returned permit may be committed only after the corresponding
Vulkan resources are actually gone. Finishing CPU command recording is not enough.

## External textures and sampler selection

Ash models an external texture as an application-owned sampled image. The renderer allocates and
owns its set-0 sampled-image descriptor, while the application keeps ownership of the image and
view:

```rust,no_run
# use ash::vk;
# use dear_imgui_ash::{AshRenderer, RendererResult};
# use dear_imgui_rs::TextureId;
# fn register(
#     renderer: &mut AshRenderer,
#     image_view: vk::ImageView,
# ) -> RendererResult<TextureId> {
// SAFETY: the view belongs to renderer's device, remains live until unregister plus GPU
// completion, and its subresources remain in the declared layout while referenced.
let texture = unsafe {
    renderer.register_external_texture(
        image_view,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    )?
};
# Ok(texture)
# }
```

Registration is intentionally `unsafe`: Rust cannot prove the raw Vulkan device lineage, image
usage, layout transitions, or GPU lifetime. `update_external_texture` and `unregister_texture` are
also unsafe and wait for device idle before changing or freeing the descriptor. That wait covers
submitted work, but the caller must still guarantee that no previously recorded command buffer
will be submitted later. Their `*_unchecked` counterparts additionally skip the device wait.

The default sampler is linear. Select nearest sampling for one image by bracketing it with the
standard callbacks published on the owning Context, then restore linear sampling for later draws:

```rust,no_run
# use dear_imgui_rs::{Context, TextureId};
# fn draw_external(context: &mut Context, texture: TextureId) {
let nearest = context
    .platform_io()
    .draw_callback_set_sampler_nearest_raw()
    .expect("the Ash renderer must be attached");
let linear = context
    .platform_io()
    .draw_callback_set_sampler_linear_raw()
    .expect("the Ash renderer must be attached");
let ui = context.frame();

unsafe {
    ui.get_window_draw_list()
        .add_callback(nearest, std::ptr::null_mut(), 0);
}
ui.image(texture, [256.0, 256.0]);
unsafe {
    ui.get_window_draw_list()
        .add_callback(linear, std::ptr::null_mut(), 0);
}
# }
```

Raw draw callbacks may inspect the active device, command buffer, pipeline, and two-set pipeline
layout through `unsafe AshRenderState::with_current`. The borrow is scoped to the callback and
cannot escape. Commands recorded there must preserve the active render-pass, synchronization, and
resource-lifetime contracts; use the standard reset callback before later ImGui draws if custom
commands replace renderer state.

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
# use dear_imgui_ash::multi_viewport::{ViewportSwapchainPolicy, VulkanViewportConfig};
# fn config(
#     entry: ash::Entry,
#     instance: ash::Instance,
#     physical_device: vk::PhysicalDevice,
#     main_surface: vk::SurfaceKHR,
#     main_surface_format: vk::SurfaceFormatKHR,
#     main_present_mode: vk::PresentModeKHR,
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
    swapchain_policy: ViewportSwapchainPolicy::from_main_surface(
        main_surface_format,
        main_present_mode,
    ),
    swapchain_image_usage: vk::ImageUsageFlags::empty(),
}
# }
```

`swapchain_policy` is resolved for every secondary surface during creation and recreation.
`from_main_surface` requires the main swapchain's complete format/color-space pair and preserves
its presentation intent: `FIFO`/`FIFO_RELAXED` select automatic VSync, while other modes select
automatic no-VSync with portable fallback. Use `SurfaceFormatPolicy::AutoSrgb` when secondary
surfaces may not expose the main pair, or `PresentModePolicy::Exact` when fallback is not allowed.
Unsupported exact choices return `SurfaceSupportError` instead of silently selecting a different
swapchain configuration.

`swapchain_image_usage` requests additional usage beyond the color-attachment bit required by the
renderer. Attachment and every swapchain rebuild verify that the surface supports the complete
set; unsupported transfer, storage, or other usage fails with `ImageUsageUnsupported` before a
secondary swapchain is created.

All handles must have one device lineage: the instance owns the physical device and
`validation_surface`; `AshRenderer`'s device was created from that physical device with
`VK_KHR_swapchain`; both queues belong to that device and to the declared families. The unsafe
entry point cannot prove those raw-handle relationships.

Vulkan also requires external host synchronization for queue access. For the lifetime of the
viewport runtime, serialize its secondary-window `queue_submit`, `queue_present`, and
`device_wait_idle` work with every application host call that touches the same graphics queue,
present queue, or logical device. This applies when both configured queue handles are identical as
well as when presentation uses a separate family.

Use one acquire semaphore, command buffer, and submit fence per frame in flight, but one present
semaphore per swapchain image. A submit fence does not prove that the presentation engine has
finished waiting on a semaphore. The Ash examples also treat acquire through successful submit as
one transaction: any intervening wait, reset, record, or submit failure idles the device, replaces
the abandoned frame sync, and rebuilds the swapchain before rendering can continue.

`validation_surface` is an existing, live application surface, normally the main window surface.
The runtime never destroys it. Before claiming callback slots, `attach` checks that required
handles are non-null, queue-family indices are in range and expose queues, the graphics family
supports graphics, and the present family can present color-attachment swapchains with at least
one format and present mode on this surface. Winit attachment requires the exact live
`WinitPlatformRuntime` owner and validates its Context and callback ownership. SDL3 attachment
requires the exact live `Sdl3PlatformBackend` owner initialized by
`init_for_vulkan` and leases its `Platform_CreateVkSurface` capability. An invalid configuration or
adapter therefore fails before the renderer is consumed or any renderer callback is published;
`AshViewportAttachError` returns the unchanged renderer to the caller.

### Winit integration

Initialize the owning Winit platform runtime first, then consume the renderer into
`WinitViewportRuntime` before Dear ImGui creates any secondary platform window:

```rust,no_run
use dear_imgui_ash::{AshRenderer, multi_viewport as ash_mvp};
use dear_imgui_rs::Context;
use dear_imgui_winit::{HiDpiMode, WinitPlatform, multi_viewport as winit_mvp};
use std::sync::Arc;

# fn attach_viewports(
#     renderer: AshRenderer,
#     imgui: &mut Context,
#     main_window: Arc<winit::window::Window>,
#     config: ash_mvp::VulkanViewportConfig,
# ) -> Result<
#     (WinitPlatform, winit_mvp::WinitPlatformRuntime, ash_mvp::WinitViewportRuntime),
#     Box<dyn std::error::Error>,
# > {
imgui.enable_multi_viewport();
let mut platform = WinitPlatform::new(imgui)?;
platform.attach_window(Arc::clone(&main_window), HiDpiMode::Default, imgui)?;
let runtime = winit_mvp::WinitPlatformRuntime::new(imgui, &platform)?;

// SAFETY: all raw handles and queue-family indices in config belong to the
// renderer's logical-device lineage. The wrapper owns renderer address stability.
let renderer = unsafe { ash_mvp::WinitViewportRuntime::attach(imgui, &runtime, renderer, config)? };
# Ok((platform, runtime, renderer))
# }
```

Custom Winit-compatible platform implementations may use `unsafe attach_unchecked` only when every
viewport `PlatformHandle` points to a live `winit::Window` that outlives the renderer runtime.

For SDL3, initialize `Sdl3PlatformBackend::init_for_vulkan` first and pass that owner to
`multi_viewport_sdl3::Sdl3ViewportRuntime::attach(imgui, &platform, renderer, config)`. The
renderer retains an exclusive, generation-bound surface-provider lease. SDL shutdown is rejected
while that lease is live, and every surface creation revalidates the SDL callback owner and the
specific viewport sidecar immediately before calling native code.

Each frame, reconcile managed textures before either main or secondary viewport work. WSI-sensitive
integrations may then submit secondary swapchains before acquiring the main surface, while simpler
integrations may choose another order. The texture retirement completion point must cover both:

```rust,ignore
let (mut frame, prepared_retirement) = renderer_runtime.prepare_context(&mut imgui)?;

frame.update_and_render_platform_windows_default();
let recorded_retirement = unsafe {
    renderer_runtime.cmd_draw_reconciled(main_command_buffer, frame)?
};
submit_main_viewport(main_command_buffer)?;

let retirement = merge_retirement_batches(prepared_retirement, recorded_retirement)?;
complete_after_all_relevant_submissions(retirement)?;
```

### Ownership and shutdown

The Ash runtime owns the renderer in stable boxed storage and claims only the five `Renderer_*`
slots. The wrapper itself may be moved safely. Attachment refuses occupied renderer callbacks,
foreign `RendererUserData`, an already registered renderer, missing platform lifecycle callbacks,
an existing `RENDERER_HAS_VIEWPORTS` capability, and secondary platform windows created before
renderer registration. It never overwrites platform slots. Callback panics, reentry, Vulkan
failures, and ownership drift are contained and reported by the next Rust entry point such as
`poll_fault` or `cmd_draw`. Replacing any claimed renderer callback immediately clears
`RENDERER_HAS_VIEWPORTS`, so a partial foreign callback table is never advertised as a usable
renderer backend. Every Rust and C callback entry also requires Ash's renderer capability, the
platform capability, and both platform create/destroy callbacks to remain present. Losing any
dependency records a typed fault and stops callback work before another Vulkan command is issued.

A failed `Renderer_CreateWindow` remains registered until its matching destroy callback. The
runtime reasserts `PlatformRequestClose` after ImGui clears the same-frame request at the end of
`UpdatePlatformWindows()`. Once a swapchain image has been acquired, every fallible wait, command
buffer, draw, fence, and submit step either completes or performs an idle-and-rebuild recovery;
the acquired image and binary semaphore are never silently carried into the next frame.

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
platform.shutdown(imgui)?;
# Ok(())
# }
```

The renderer runtime destroys secondary renderer resources and clears only callback slots it still
owns. Context attachments enforce renderer-before-platform teardown even when the Context is
dropped first. Explicit shutdown remains the preferred path because it reports cleanup errors and
allows recoverable completion-wait failures to be retried before the Vulkan device, instance, or
main validation surface is dropped.

Secondary viewports negotiate their own surface format and extent. Pipelines are cached by
`vk::Format`; logical resize, DPI-only framebuffer-scale changes, minimized extents,
out-of-date/suboptimal swapchains, and per-image synchronization are owned by the shared runtime.

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
this behavior consistent. If you register an external sampled image backed by an `*_SRGB` format,
the shader gamma path will not match (you'll effectively decode twice).

## Compatibility

| Item          | Version |
|---------------|---------|
| Crate         | 0.16.0-alpha.2  |
| dear-imgui-rs | 0.16.0-alpha.2  |
| ash           | 0.38    |
| ash-window    | 0.13 (`multi-viewport-winit`) |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Reference

This backend is inspired by the excellent `imgui-rs-vulkan-renderer` project:
<https://github.com/adrien-ben/imgui-rs-vulkan-renderer>

## Quick start

```rust,no_run
use ash::vk;
use dear_imgui_ash::{AshRenderer, AshRendererConfig, Options};
use dear_imgui_rs::Context;

# fn example() -> Result<(), dear_imgui_ash::RendererError> {
// Create your Vulkan instance/device/queue/command_pool/render_pass first...
# let (instance, physical_device, device, queue, command_pool, render_pass) = todo!();

let mut imgui = Context::create();
let renderer_config = AshRendererConfig::with_render_pass(
    device.clone(),
    queue,
    command_pool,
    render_pass,
)
.with_options(Options::default());
// SAFETY: all handles share one live device lineage; the queue, command pool, and render pass
// satisfy AshRenderer's documented graphics, transfer, and target-compatibility requirements.
let mut renderer = unsafe {
    AshRenderer::with_default_allocator(
        &instance,
        physical_device,
        renderer_config,
        &mut imgui,
    )?
};

// In your render loop (inside a render pass):
# let command_buffer = vk::CommandBuffer::null();
let frame = imgui.render(renderer.renderer_consumer()?);
// SAFETY: command_buffer is recording inside the compatible render pass and will be submitted
// before renderer resources referenced by it are changed or destroyed.
let retirement = unsafe { renderer.cmd_draw(command_buffer, frame)? };

// Submit command_buffer. The safe path waits for all device work before releasing retired textures.
if let Some(batch) = retirement {
    renderer.wait_for_texture_retirements(batch)?;
}

renderer.shutdown(&mut imgui)?;
# Ok(()) }
```
