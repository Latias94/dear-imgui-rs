# Migrating from 0.23 to 0.24

## Rust toolchain

Upgrade the workspace toolchain to Rust 1.97.1 or later. The same minimum applies to the core,
Bevy backend, extensions, and build helpers.

## Context and stack layouts

| In 0.23 | In 0.24 |
| --- | --- |
| `internal::update_hovered_window_and_capture_flags(pos)` | Call `context.update_hovered_window_and_capture_flags(pos)` on the context that owns the window. Both coordinates must be finite. |
| `SuspendedContext`, `Context::suspend`, and `activate` | Store `Context` directly. Multiple contexts can live on the same OS thread. Each context's safe APIs bind to the correct native owner. |
| `SuspendedContext::try_with_active` | Use `Context::try_with_current` for a checked operation that must finish its frame. For raw calls without a mutable Context borrow, use `context.binding().with_bound_context(...)`. |
| `FrameToken::try_render` returns only an error | Handle `Err((frame, error))`. A rejected consumer leaves the frame open. If native rendering already finished, retry with `frame.try_render(...)` or drop the frame. Do not issue more UI commands or switch renderer paths. Snapshot methods report `SnapshotError::FrameAlreadyRendered` in this state. To propagate the error and discard the frame, use `.map_err(\|(_frame, error)\| error)?`. |
| Rebinding an inactive Context separately for each UI call | Top-level `Context::frame` and `Context::begin_frame` leave their Context current. To bind once across a scoped submission step, use `frame.with_ui(\|ui\| ...)`. `Context::frame_with_result` and window builders also bind their callbacks and restore the preceding Context. |
| `ChildWindow::new(name).build(ui, f)` or `ui.child_window(name).build(ui, f)` | Use `ui.child_window(name).build(f)`. The builder already owns the borrow of its `Ui`. |
| `ui.get_window_draw_list()` | Use `ui.with_window_draw_list(\|draw_list\| ...)` inside the window. Draw-list builders and tokens must finish inside that callback. Background and foreground draw lists remain frame-scoped. |
| `begin_horizontal_stack_layout`, `begin_vertical_stack_layout`, `horizontal_stack_layout`, `vertical_stack_layout`, and `stack_layout_spring` | Use `begin_horizontal`, `begin_vertical`, `horizontal`, `vertical`, and `spring`, respectively. |
| Reusing a resolved stack layout ID in the same window and frame | Supply a distinct ID for each layout in that window. IDs can be reused in another window or a later frame. |
| Calling `ui.spring(weight, spacing)` without a layout | Place the spring inside an active stack layout in the current window. A suspension pauses the outer layout until it resumes. |

## Core API

| In 0.23 | In 0.24 |
| --- | --- |
| Importing widget submodules from the crate root | Import them from `dear_imgui_rs::widget`. `dear_imgui_rs::prelude::*` brings in the everyday frame and window types. |
| Naming types returned by draw-list methods | Import builders, `ChannelsSplit`, `DrawListClipRectToken`, and `ImColor32` from `dear_imgui_rs::draw`. |
| Passing typed arguments to `Ui::push_id` | Use `stacks::Id<'_>` to name the input enum. The crate-root `Id` remains the resolved numeric ID. |
| Pushing a string ID with embedded NULs | `Ui::push_id` now hashes the whole string, like `Ui::get_id`, without replacing embedded NULs. This does not change NUL handling in widget labels passed as C strings. |
| `Drag::new(label).build(ui, value)` | Use `ui.drag_config(label).build(value)` or the typed `drag_float_config` and `drag_int_config` helpers. Range and array builders also capture the UI. |
| `ListBox::new(label)`, `ListBoxToken`, or `.build(ui, f)` | Use `ui.list_box_config(label).build(f)`. `build_simple` and `build_simple_i32` no longer take a UI argument. |
| `TabBar::new(id).build(ui, f)` or `TabItem::new(label).build(ui, f)` | Use `ui.tab_bar_config(id).build(f)` or `ui.tab_item_config(label).build(f)`. The direct `Ui` tab methods still return tokens when needed. |
| `ui.slider(label, min, max, value)` | Use `ui.slider(label, value, min, max)`, matching `slider_f32` and `slider_i32`. |
| `AngleSlider::new(label)` or `VerticalSlider::new(label, size, min, max)` | Use `ui.slider_angle_config(label)` or `ui.v_slider_config(label, size, min, max)`, then call `.build(value)`. |
| Infallibly converting a `MouseButton` for an invisible button | Use `InvisibleButtonMouseButtons::try_from(button)` and handle `Extra1` and `Extra2`. Pass the result to `.mouse_buttons(...)` or directly to `ui.invisible_button_options`. |
| `unsafe { ui.style() }` | Use `ui.style()` for a live read-only view and `ui.clone_style()` for a snapshot. |
| Creating a raw storage view to update `OwnedStateStorage` | Use the owner's safe `get_int`, `get_bool`, `get_float`, corresponding setters, and `clear`. |
| Borrowing `ffi::RawBox` through `AsRef` | Use `as_ptr()` to publish the pointer. Only call `unsafe { owner.as_ref_unchecked() }` when native writes and overlapping exclusive borrows cannot occur during the borrow. |
| Deserializing numeric flags with unknown bits | The numeric encoding is unchanged. Deserialize now rejects bits outside the public flag set. |

Window and image builders accept sizes that convert to `[f32; 2]`. Color arguments in image
builders and `Ui::text_colored` accept `Color`, RGBA arrays, and compatible vector types.

## WGPU renderer

| In 0.23 | In 0.24 |
| --- | --- |
| `wgpu-27`, `wgpu-28`, `wgpu-29`, or `wgpu-30` | Remove the feature and upgrade the application to WGPU 30. |
| `webgl-wgpu*` and `webgpu-wgpu*` | Use `webgl` and `webgpu`. Both enable the core WASM import path. |
| Selecting Winit or SDL3 at Cargo feature resolution | Enable either or both platform features. Instantiate the integration for each Context from the matching Winit or SDL3 module. |
| sRGB transfer applied to the combined fragment color | The shader decodes the interpolated vertex tint before modulation. Use an sRGB texture view for sRGB-encoded application textures. |
| Keeping the renderer alive until explicit `shutdown` or Context teardown | Drop the renderer to release its WGPU handles. The Context resets managed textures before the next renderer uses them. Use `shutdown` for synchronous error reporting. |

## Ash renderer

| In 0.23 | In 0.24 |
| --- | --- |
| Choosing an allocator through Cargo features | Use `with_default_allocator`, `with_gpu_allocator`, or `with_vk_mem_allocator`. The built-in allocator is always available and both optional allocators can be enabled together. |
| Choosing Winit or SDL3 at Cargo feature resolution | Enable either or both features, then attach the route for the Context's platform owner. |
| sRGB transfer applied to the combined fragment color | The shader decodes the interpolated vertex tint before modulation. Use an sRGB image view for sRGB-encoded application textures. |
| Preparing textures after acquiring a swapchain image | Prepare before acquisition. If the upload ring returns `VK_NOT_READY` as the only fault, skip the frame and retry on the next redraw. |
| Requiring explicit synchronous `shutdown` to release a single-viewport renderer | Drop the renderer while the Vulkan device, queue, command pool, and loader are live. Use `shutdown` to receive cleanup errors and reset the Context immediately. |

## Prebuilt archives

| In 0.23 | In 0.24 |
| --- | --- |
| A remote `*_SYS_PREBUILT_URL` pointing to an archive or library | Use an HTTPS URL to a packaged `.tar.gz` or `.tgz` and publish a `SHA256SUMS` file alongside it. You can also set `DEAR_IMGUI_SHA256SUMS` to a local manifest. |
| An implicit cache beside the crate's source tree | Cargo's `OUT_DIR` supplies the default cache. Set `DEAR_IMGUI_CACHE_DIR` for a shared cache root or `*_SYS_CACHE_DIR` for a crate-specific override. |
| A custom core archive built for the 0.23 native ABI | Rebuild the archive and its manifest against this checkout. The core frame fast path reads the native current-context slot exported by the archive. Cargo does not build that shim when linking a matching archive. |
| Prebuilt caches from an earlier checkout | Populate the new cache online before building offline, or select a local archive explicitly. Downloaded archives are still verified against their full SHA-256 digest. Extraction uses a short content-addressed path and accepts only regular files and directories in archives. `Auto` can fall back to source after a rejected discovered archive. |
| `cargo run --features package-bin --bin package` in a sys crate | Run `cargo run -p xtask -- package <sys-crate>` from the workspace. Choose the artifact profile with `--features`, pass `--target` for a specific Rust target, and select a destination with `--out-dir`. `--all` builds the core and matching extensions. For Windows MSVC, set Cargo's `crt-static` target feature for `/MT`; `--crt mt` or `--crt md` verifies the resulting profile. The output includes `SHA256SUMS`. |

## Fonts

Use `atlas.try_add_font(&[FontSource::ttf_bytes(bytes).font_index(index)])` for checked loading. The atlas copies the input and reports unsupported or malformed data as a `FontLoadError`. A native atlas using FreeType accepts supported outline TTF, OTF, and TTC faces. An atlas using stb keeps the existing bounded-read checks. OpenType SVG and embedded bitmap formats are not accepted through this checked entry point.

## Text input

| In 0.23 | In 0.24 |
| --- | --- |
| `ImString` | Use `String`. Use `String::with_capacity` or a widget's `capacity_hint` for large fields. |
| `ui.input_text_imstr(label, &mut value)` | Use `ui.input_text(label, &mut value)` with a `String`. |
| `ui.input_text_multiline_imstr(label, &mut value, size)` | Use `ui.input_text_multiline(label, &mut value, size)` with a `String`. |
| `TextCallbackData::str()` returns `&str` | Handle the `Result<&str, Utf8Error>`. Use `str_lossy()` for display that can replace invalid sequences. |
| `ui.radio_button_bool(label, active)` | Use `ui.radio_button(label, active)`. |

## Native extensions

| In 0.23 | In 0.24 |
| --- | --- |
| `dear_imnodes::Context` | Use `dear_imnodes::ImNodesContext`. |
| `plot.plot_draw_list()` | Use `plot.with_plot_draw_list(\|draw_list\| ...)` in ImPlot and ImPlot3D. |
| Node Editor draw-list getters | Use `with_node_background_draw_list(node, f)` on the editor frame and `with_background_draw_list(f)` or `with_foreground_draw_list(f)` on a group hint. |
| ImPlot `set_next_axis_links` and unsafe axis link pointers | In `with_plot_setup`, call `setup.setup_axis_links` with optional `&Cell<f64>` values declared outside the setup closure. The wrapper copies the limits in and writes back the final range when the plot ends. Use the copied limits APIs when writeback is unnecessary. |
| `GuizmoContext::new().begin_frame(ui)` | Import `dear_imguizmo::GuizmoExt` and call `ui.guizmo()`. |
| `GizmoUi::draw_axes` and `draw_cubes` accept a slice of any `Mat4Like` | Pass a contiguous `&[[f32; 16]]` of column-major transforms. This allows the renderer to borrow the matrices without allocating or repacking them. |

## File browser

| In 0.23 | In 0.24 |
| --- | --- |
| Native file dialogs are enabled by default | Enable the `native-rfd` feature explicitly. The default uses the ImGui UI and tracing. |
| `ThumbnailBackend` takes a provider that decodes on the UI thread | Create a `ThumbnailDecoder::spawn` pool once and pass a reference to it from each frame. The renderer still uploads and retires textures on the UI thread. |
| `set_filters` does not report invalid regex literals | Handle `IgfdFilterParseError` from `set_filters`, `add_filter`, and `extend_filters`. Invalid patterns leave the preceding filter configuration unchanged. |

System places refresh synchronously. In-progress thumbnail requests can be retried when
the decoder is replaced. On Unix without exclusive rename support, the default filesystem
can move regular files using a no-replace hard link and subsequent unlink. Directories and
links still require native exclusive rename support.
