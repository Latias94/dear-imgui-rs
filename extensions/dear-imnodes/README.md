# dear-imnodes

Safe, idiomatic Rust bindings for [ImNodes](https://github.com/Nelarius/imnodes) via the [cimnodes](https://github.com/cimgui/cimnodes) C API, aligned with our dear-imgui workspace and BEST_PRACTICES.

- Ui extension: `ui.imnodes(&ctx)` returns a `NodesUi` for the current frame
- Contexts: `Context` (global) and `EditorContext` (per-editor) with `Drop`
- RAII tokens: `editor()`, `node(id)`, `input_attr(id)`, `output_attr(id)`, `static_attr(id)`
- Strongly-typed enums/bitflags for style and attributes
- Helpers: links, selection, node positions, minimap, IO setup

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui/main/screenshots/imnodes-basic.png" alt="ImNodes" width="75%"/>
  <br/>
</p>

## Compatibility

| Item              | Version |
|-------------------|---------|
| Crate             | 0.1.x   |
| dear-imgui        | 0.2.x   |
| dear-imnodes-sys  | 0.1.x   |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Quick Start

Basic setup and per-frame usage:

```rust
use dear_imgui::Ui;
use dear_imnodes as imnodes;

// One-time setup (alongside your ImGui context)
fn init(imgui_ctx: &dear_imgui::Context) -> (imnodes::Context, imnodes::EditorContext) {
    let nodes_ctx = imnodes::Context::create(imgui_ctx);
    let editor_ctx = imnodes::EditorContext::create();
    (nodes_ctx, editor_ctx)
}

// Per-frame draw
fn draw(ui: &Ui, nodes_ctx: &imnodes::Context, editor_ctx: &imnodes::EditorContext) {
    let nodes = ui.imnodes(nodes_ctx);
    let editor = nodes.editor(Some(editor_ctx));

    // A simple node with input/output pins
    let _n = editor.node(1).title_bar(|| ui.text("My Node"));
    let _in = editor.input_attr(10, imnodes::PinShape::CircleFilled);
    ui.text("In");
    _in.end();
    let _out = editor.output_attr(11, imnodes::PinShape::QuadFilled);
    ui.text("Out");
    _out.end();

    // Draw a link
    editor.link(100, 10, 11);

    // Handle new link creation
    if let Some(link) = editor.is_link_created() {
        // link.start_attr, link.end_attr, link.from_snap
    }

    // Optional: Mini-map
    editor.minimap(0.25, imnodes::MiniMapLocation::TopRight);
}
```

### IO and Interaction

Bind common shortcuts to ImNodes IO (call while the editor is active):

```rust
// Ctrl to detach links; multi-select via Shift; emulate 3-button mouse with Alt
editor.enable_link_detach_with_ctrl();
editor.enable_multiple_select_with_shift();
editor.emulate_three_button_mouse_with_alt();

// Misc IO tweaks
editor.set_alt_mouse_button(1);            // e.g. MouseRight
editor.set_auto_panning_speed(200.0);
```

### Styling

Use presets or fine-tune values. You can push scoped styles/colors (RAII) or set persistent style:

```rust
// Presets
editor.style_colors_dark();

// Push a color for this scope
let _color = editor.push_color(imnodes::ColorElement::Link, [0.9, 0.3, 0.3, 1.0]);

// Push a style var for this scope
let _sv = editor.push_style_var(
    imnodes::StyleVar::LinkThickness,
    imnodes::style::StyleVarValue::Float(3.0),
);

// Persistent style
editor.set_grid_spacing(32.0);
editor.set_node_corner_rounding(6.0);

// Read/Write a style color (persistent)
let link_rgba = editor.get_color(imnodes::ColorElement::Link);
editor.set_color(imnodes::ColorElement::GridLinePrimary, [0.6, 0.6, 0.8, 1.0]);
```

### Node Positions and Queries

```rust
// Position nodes (grid/editor/screen space helpers available)
editor.set_node_pos_grid(1, [100.0, 120.0]);
let size = editor.get_node_dimensions(1); // [w, h]

// Hover/active queries
if editor.is_editor_hovered() { /* ... */ }
if let Some(node_id) = editor.hovered_node() { /* ... */ }
if editor.is_attribute_active() { /* ... */ }
```

### Selection and Link Lifecycle

```rust
// Selection helpers
let selected_nodes = editor.selected_nodes();
let selected_links = editor.selected_links();

// Link lifecycle
if let Some(created) = editor.is_link_created_with_nodes() {
    // created.start_node, created.start_attr, created.end_node, created.end_attr, created.from_snap
}
if let Some(link_id) = editor.is_link_destroyed() {
    // handle removal
}
```

### Saving/Loading Editor State

Use `EditorContext` to persist per-editor state across sessions, or use post-editor helpers for the current editor:

```rust
// Per-editor state (no active frame required)
let s = editor_ctx.save_state_to_ini_string();
editor_ctx.load_state_from_ini_string(&s);
editor_ctx.save_state_to_ini_file("nodes.ini");
editor_ctx.load_state_from_ini_file("nodes.ini");

// Or directly after ending a frame
let post = editor.end();
let s2 = post.save_state_to_ini_string();
post.load_state_from_ini_string(&s2);
post.save_state_to_ini_file("nodes.ini");
```

See crate docs for the full API surface and patterns.

