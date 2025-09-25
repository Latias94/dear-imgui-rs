# dear-imnodes

Safe, idiomatic Rust bindings for [ImNodes](https://github.com/Nelarius/imnodes) via the [cimnodes](https://github.com/cimgui/cimnodes) C API, aligned with our dear-imgui workspace and BEST_PRACTICES.

- Ui extension: `ui.imnodes(&ctx)` returns a `NodesUi` for the current frame
- Contexts: `Context` (global) and `EditorContext` (per-editor) with `Drop`
- RAII tokens: `editor()`, `node(id)`, `input_attr(id)`, `output_attr(id)`, `static_attr(id)`
- Strongly-typed enums/bitflags for style and attributes
- Helpers: links, selection, node positions, minimap, IO setup

## Compatibility

| Item              | Version |
|-------------------|---------|
| Crate             | 0.1.x   |
| dear-imgui        | 0.2.x   |
| dear-imnodes-sys  | 0.1.x   |

See also: [docs/COMPATIBILITY.md](../../docs/COMPATIBILITY.md) for the full workspace matrix.

## Example

```rust
use dear_imgui::Ui;
use dear_imnodes as imnodes;

fn draw(ui: &Ui, nodes_ctx: &imnodes::Context, editor: &imnodes::EditorContext) {
    let nodes = ui.imnodes(nodes_ctx);
    let editor = nodes.editor(Some(editor));

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
}
```

See the crate docs for more.

