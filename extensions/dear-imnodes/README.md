# dear-imnodes

[![Crates.io](https://img.shields.io/crates/v/dear-imnodes.svg)](https://crates.io/crates/dear-imnodes)
[![Documentation](https://docs.rs/dear-imnodes/badge.svg)](https://docs.rs/dear-imnodes)

Safe, idiomatic Rust bindings for [ImNodes](https://github.com/Nelarius/imnodes) via the [cimnodes](https://github.com/cimgui/cimnodes) C API, aligned with our dear-imgui workspace and BEST_PRACTICES.

- Ui extension: `ui.imnodes(&ctx)` returns a `NodesUi` for the current frame
- Contexts: `Context` (global) and `EditorContext` (per-editor) with `Drop`
- Typestate editor phases: configure through `NodeEditorSetup`, then consume it with
  `begin_nodes()` before submitting nodes
- RAII tokens: `node(id)`, then node-owned `input_attr(id)`, `output_attr(id)`, and `static_attr(id)` scopes
- Strongly-typed enums/bitflags for style and attributes
- Helpers: links, selection, node positions, minimap, IO setup

<p align="center">
  <img src="https://raw.githubusercontent.com/Latias94/dear-imgui-rs/main/screenshots/imnodes-basic.png" alt="ImNodes" width="75%"/>
  <br/>
</p>

## Links

- Upstream: https://github.com/Nelarius/imnodes
- C API: https://github.com/cimgui/cimnodes

## Compatibility

| Item              | Version |
|-------------------|---------|
| Crate             | 0.16.0-alpha.3  |
| dear-imgui-rs     | 0.16.0-alpha.3  |
| dear-imnodes-sys  | 0.16.0-alpha.3  |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md) for the full workspace matrix.

## Quick Start

Basic setup and per-frame usage:

```rust
use dear_imgui_rs::Ui;
use dear_imnodes as imnodes;

 // One-time setup (alongside your ImGui context)
 fn init(imgui_ctx: &dear_imgui_rs::Context) -> (imnodes::Context, imnodes::EditorContext) {
     let nodes_ctx = imnodes::Context::create(imgui_ctx);
     let editor_ctx = nodes_ctx.create_editor_context();
     (nodes_ctx, editor_ctx)
 }

// Per-frame draw
 fn draw(ui: &Ui, nodes_ctx: &imnodes::Context, editor_ctx: &imnodes::EditorContext) {
    let nodes = ui.imnodes(nodes_ctx);
    let setup = nodes.editor(Some(editor_ctx));

    // Native node-record mutations must happen before node submission starts.
    setup.set_node_pos_grid(imnodes::NodeId::new(1), [100.0, 120.0]);
    let editor = setup.begin_nodes();

    // A simple node with input/output pins
    let node = imnodes::NodeId::new(1);
    let input = imnodes::PinId::new(10);
    let output = imnodes::PinId::new(11);
    let link_id = imnodes::LinkId::new(100);

    let node_token = editor.node(node);
    node_token.title_bar(|| ui.text("My Node"));
    let _in = node_token.input_attr(input, imnodes::PinShape::CircleFilled);
    ui.text("In");
    _in.end();
    let _out = node_token.output_attr(output, imnodes::PinShape::QuadFilled);
    ui.text("Out");
    _out.end();
    node_token.end();

     // Draw a link
     editor.link(link_id, input, output);

     // Optional: Mini-map
     editor.minimap(0.25, imnodes::MiniMapLocation::TopRight);

     // End the editor and handle post-editor interactions
     let post = editor.end();
     if let Some(link) = post.is_link_created() {
         // link.start_attr, link.end_attr, link.from_snap
     }
 }
 ```

### IO and Interaction

Bind common shortcuts to ImNodes IO during the setup phase:

```rust
// Ctrl to detach links; multi-select via Shift; emulate 3-button mouse with Alt
setup.enable_link_detach_with_ctrl();
setup.enable_multiple_select_with_shift();
setup.emulate_three_button_mouse_with_alt();

// Misc IO tweaks
setup.set_alt_mouse_button(MouseButton::Right);
setup.set_auto_panning_speed(200.0);
```

### Styling

Use presets or fine-tune values. You can push scoped styles/colors (RAII) or set persistent style:

```rust
// Persistent configuration belongs to the setup phase.
setup.style_colors_dark();
setup.set_grid_spacing(32.0);
setup.set_node_corner_rounding(6.0);
let link_rgba = setup.get_color(imnodes::ColorElement::Link);
setup.set_color(imnodes::ColorElement::GridLinePrimary, [0.6, 0.6, 0.8, 1.0]);

let editor = setup.begin_nodes();

// Scoped styles belong to the node-submission phase.
let _color = editor.push_color(imnodes::ColorElement::Link, [0.9, 0.3, 0.3, 1.0]);

// Push a style var for this scope
let _sv = editor.push_style_var(
    imnodes::StyleVar::LinkThickness,
    imnodes::style::StyleVarValue::Float(3.0),
);

```

### Node Positions and Queries

```rust
// Position nodes before submitting them (grid/editor/screen space helpers available)
let node = imnodes::NodeId::new(1);
setup.set_node_pos_grid(node, [100.0, 120.0]);
let editor = setup.begin_nodes();

// The queued position is applied only when this matching ID is submitted.
let node_token = editor.node(node);
// ... submit its title and pins ...
node_token.end();

// End the editor before running post-editor queries
let post = editor.end();
if post.is_editor_hovered() { /* ... */ }
if let Some(node_id) = post.hovered_node() { /* ... */ }
if post.is_attribute_active() { /* ... */ }

// Panning changes apply to the next frame and only accept IDs submitted by this frame.
let centered = post.center_on_submitted_node(node);
```

### Selection and Link Lifecycle

 ```rust
 let post = editor.end();

 // Selection helpers
 let selected_nodes = post.selected_nodes();
 let selected_links = post.selected_links();

 // ID-specific operations return false without calling native code when that ID was not
 // submitted by the frame that produced this PostEditor snapshot.
 let selected = post.select_node(node_id);

 // Link lifecycle
 if let Some(created) = post.is_link_created_with_nodes() {
     // created.start_node, created.start_attr, created.end_node, created.end_attr, created.from_snap
 }
 if let Some(link_id) = post.is_link_destroyed() {
     // handle removal
 }
 ```

`NodeEditorSetup`, `NodeEditor`, `PostEditor`, and `BoundEditor` retain an internal lease on an explicit `EditorContext`. Dropping the public editor handle after creating one of these scopes is safe; the native editor is released after its final Rust scope ends.

### Saving/Loading Editor State

 Use `EditorContext` to persist per-editor state across sessions, or use post-editor helpers for the current editor:

 ```rust
 // Per-editor state (no active frame required)
 let bound = nodes_ctx.bind_editor(editor_ctx);
 let s = bound.save_state_to_ini_string();
 bound.load_state_from_ini_string(&s);
 bound.save_state_to_ini_file("nodes.ini");
 bound.load_state_from_ini_file("nodes.ini");

 // Or directly after ending a frame
 let post = editor.end();
 let s2 = post.save_state_to_ini_string();
 post.load_state_from_ini_string(&s2);
 post.save_state_to_ini_file("nodes.ini");
 ```

The string-based save/load methods are available on every supported target. Direct file methods are native-only; on `wasm32`, persist the returned INI string through the browser or application storage layer instead.

See crate docs for the full API surface and patterns.
