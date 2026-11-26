# Migrating from 0.23 to 0.24

## Context and stack layouts

| In 0.23 | In 0.24 |
| --- | --- |
| `internal::update_hovered_window_and_capture_flags(pos)` | Call `context.update_hovered_window_and_capture_flags(pos)` on the context that owns the window. Both coordinates must be finite. |
| `SuspendedContext`, `Context::suspend`, and `activate` | Store `Context` directly. Multiple contexts can live on the same OS thread. Each context's safe APIs bind to the correct native owner. |
| `SuspendedContext::try_with_active` | Use `Context::try_with_current` for a checked operation that must finish its frame. For raw calls without a mutable Context borrow, use `context.binding().with_bound_context(...)`. |
| `FrameToken::try_render` returns only an error | Handle `Err((frame, error))`. A rejected consumer leaves the frame open. If native rendering already finished, retry with `frame.try_render(...)` or drop the frame. Do not issue more UI commands or switch renderer paths. Snapshot methods report `SnapshotError::FrameAlreadyRendered` in this state. To propagate the error and discard the frame, use `.map_err(\|(_frame, error)\| error)?`. |
| Reusing a resolved stack layout ID in the same window and frame | Supply a distinct ID for each layout in that window. IDs can be reused in another window or a later frame. |
| Calling `ui.spring(weight, spacing)` without a layout | Place the spring inside an active stack layout in the current window. A suspension pauses the outer layout until it resumes. |

## Prebuilt archives

| In 0.23 | In 0.24 |
| --- | --- |
| A remote `*_SYS_PREBUILT_URL` pointing to an archive or library | Use an HTTPS URL to a packaged `.tar.gz` or `.tgz` and publish a `SHA256SUMS` file alongside it. You can also set `DEAR_IMGUI_SHA256SUMS` to a local manifest. |
| An implicit cache beside the crate's source tree | Cargo's `OUT_DIR` supplies the default cache. Set `DEAR_IMGUI_CACHE_DIR` for a shared cache root or `*_SYS_CACHE_DIR` for a crate-specific override. |
| A custom core archive built for the 0.23 native ABI | Rebuild the archive and its manifest against this checkout. The core frame fast path reads the native current-context slot exported by the archive. Cargo does not build that shim when linking a matching archive. |

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
| ImPlot `set_next_axis_links` and unsafe axis link pointers | In `with_plot_setup`, call `setup.setup_axis_links` with optional `&Cell<f64>` values declared outside the setup closure. The wrapper copies the limits in and writes back the final range when the plot ends. Use the copied limits APIs when writeback is unnecessary. |
| `GuizmoContext::new().begin_frame(ui)` | Import `dear_imguizmo::GuizmoExt` and call `ui.guizmo()`. |
| `GizmoUi::draw_axes` and `draw_cubes` accept a slice of any `Mat4Like` | Pass a contiguous `&[[f32; 16]]` of column-major transforms. This allows the renderer to borrow the matrices without allocating or repacking them. |

## File browser

| In 0.23 | In 0.24 |
| --- | --- |
| Native file dialogs are enabled by default | Enable the `native-rfd` feature explicitly. The default uses the ImGui UI and tracing. |
| `ThumbnailBackend` takes a provider that decodes on the UI thread | Create a `ThumbnailDecoder::spawn` pool once and pass a reference to it from each frame. The renderer still uploads and retires textures on the UI thread. |
| `set_filters` does not report invalid regex literals | Handle `IgfdFilterParseError` from `set_filters`, `add_filter`, and `extend_filters`. Invalid patterns leave the preceding filter configuration unchanged. |
