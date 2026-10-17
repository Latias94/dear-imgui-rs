# Dear ImGui 1.93 migration

The `0.21.0` release train uses the updated cimgui docking pin, embedding Dear ImGui **1.93.1 WIP** after the 1.93.0 release. Upgrade all workspace crate dependencies together to `"0.21"` and rebuild custom native archives and WASM providers with the updated headers and bindings. Applications migrating from published 0.20.0 should also read the [editor backend migration guide](EDITOR_BACKEND_MIGRATION.md).

## Custom drawing

`DrawFlags` replaces `DrawCornerFlags`, `PolylineFlags`, and `DrawListFlags`.

| Previous API | Replacement |
| --- | --- |
| `DrawCornerFlags::DEFAULT` | `DrawFlags::NONE` |
| `DrawCornerFlags::TOP_LEFT`, `ALL`, `NO_ROUNDING` | `DrawFlags::ROUND_TOP_LEFT`, `ROUND_ALL`, `ROUND_NONE` |
| `PolylineFlags::CLOSED` | `DrawFlags::CLOSED` |
| `DrawListFlags::ANTI_ALIASED_LINES`, `ANTI_ALIASED_FILL` | `DrawFlags::AA_LINES`, `AA_FILL` |
| `DrawListFlags::ALLOW_VTX_OFFSET` | `DrawFlags::USE_VTX_OFFSET` |
| `path_stroke(color, flags, thickness)` | `path_stroke(color, thickness, flags)` |

Direct drawing helpers with a new final `flags` parameter accept `DrawFlags::NONE` for defaults. Builders expose `.flags(...)`. Closed shapes default to inside strokes; lines and paths default to centered strokes. Choose `STROKE_INSIDE`, `STROKE_CENTER`, `STROKE_CENTER_BIASED`, or `STROKE_OUTSIDE` explicitly when needed.

Filled rectangles now require ordered minimum/maximum coordinates on both axes. Normalize transformed data endpoints before drawing; reversed bounds produce no geometry. This matters for plot axes whose screen-space Y direction is opposite to data-space Y.

`add_line` no longer offsets coordinates by half a pixel. Circle and ngon radii no longer shrink by half a pixel. Prefer `add_line_h` / `add_line_v` for pixel-aligned axis lines, or temporarily use `STROKE_LEGACY` to match older custom rendering:

```rust,ignore
draw_list.with_draw_flags(DrawFlags::STROKE_LEGACY, true, || {
    draw_list.add_line(start, end, color).build();
});
```

Flag scopes restore defaults during unwinding and support up to three nested scopes per draw list. Stroke positions other than legacy, joins, and caps belong on primitives. The current upstream ellipse implementation requires an explicit `STROKE_LEGACY` primitive flag even inside a legacy scope. Rectangle, circle, ellipse, and ngon anti-aliasing is configured through the scope. `Rect::flags` accepts corner rounding and stroke placement; configure AA with `with_draw_flags` rather than per-rectangle flags, which upstream does not preserve on every drawing path.

## Style, atlas, and text filters

- Replace `curve_tessellation_tol` / its setter with `curve_tessellation_max_error` / its setter. The new unit is pixels; use the square root of the old tolerance.
- Remove `anti_aliased_lines_use_tex` configuration: anti-aliased line rendering now always uses textures. `anti_aliased_line_ends` controls the new end anti-aliasing option.
- Set `FontAtlasFlags` before the first build. `NO_BAKED_LINES` disables line and line-end anti-aliasing (do not re-enable it through draw flags); `NO_BAKED_ROUND_CORNERS` uses tessellated corners.
- Replace `filter.draw_with_size(ui, width)` with `ui.set_next_item_width(width); filter.draw(ui)`. `draw_with_hint` customizes the empty-field hint. Space-separated terms now require all words, quoted terms match phrases, and exclusions always take precedence.
- Use `ui.with_item_flag(ItemFlags::MIXED_VALUE, true, || ...)` for multi-object property editors.

## Renderers and extensions

- Framebuffer scales must be finite and strictly positive on both axes, including values returned by platform callbacks. Use zero display or physical framebuffer dimensions to suspend rendering while minimized; keep pixel density positive.
- Handle the new `DrawCmd::SetSamplerFromTexture` and `DrawCmdSnapshot::SetSamplerFromTexture` variants. Glow restores each texture's filtering settings; WGPU and Ash reject this unsupported command before drawing. Bevy abandons the affected Context snapshot before upload and reports `ImguiDiagnosticKind::TextureSamplerUnsupported` with origin `RenderPreparation`. Ordinary linear and nearest sampling remain available.
- Glow defaults to a lower-left clip origin. Applications using an upper-left origin must configure `GlowClipOrigin::UpperLeft`, or explicitly select `Auto` for driver querying. The SDL3 OpenGL3 owner exposes `set_clip_origin(imgui, Sdl3OpenGl3ClipOrigin::UpperLeft)`. The native OpenGL3 shim exposes `dear_imgui_backend_opengl3_set_clip_origin` as well.
- CTE upstream removed notifications; use an application-owned notification UI. `add_squiggle` now takes a final `SquiggleStyle::Wave` or `Background`. New palette entries cover current-line highlighting.
- `node_background_draw_list` returns `Option<DrawListMut>`. Call it after dropping the node token and handle nodes not submitted in this frame.
- Native custom font loaders must adopt the glyph-index callback signatures and `FontSrcGetGlyphIndexFromCodepoint`. Raw glyph remapping remains sys-only.
