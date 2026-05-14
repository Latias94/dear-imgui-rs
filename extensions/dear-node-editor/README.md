# dear-node-editor

[![Crates.io](https://img.shields.io/crates/v/dear-node-editor.svg)](https://crates.io/crates/dear-node-editor)
[![Documentation](https://docs.rs/dear-node-editor/badge.svg)](https://docs.rs/dear-node-editor)

Safe Rust bindings for [imgui-node-editor](https://github.com/thedmd/imgui-node-editor) through the
[`cimnodes_editor`](https://github.com/cimgui/cimnodes_editor) wrapper and a repository-owned C ABI
shim.

This crate coexists with `dear-imnodes`:

- `dear-imnodes` wraps ImNodes and is the current wasm-capable node editor path.
- `dear-node-editor` wraps imgui-node-editor and is native-only in the first integration phase.

The sys layer deliberately exposes `NodeId`, `PinId`, and `LinkId` as pointer-sized integer values
instead of upstream C++ helper pointers. The safe layer wraps those values as Rust newtypes and uses
RAII tokens for editor frames, nodes, pins, create/delete sessions, shortcuts, and style pushes.

## Compatibility

| Item                 | Version |
|----------------------|---------|
| Crate                | 0.12.0  |
| dear-imgui-rs        | 0.12.0  |
| dear-node-editor-sys | 0.12.0  |

## Quick Start

```rust
use dear_node_editor::{EditorContext, NodeEditorUiExt, NodeId, PinId, LinkId, PinKind};

fn setup(imgui: &dear_imgui_rs::Context) -> EditorContext {
    EditorContext::create(imgui)
}

fn draw(ui: &dear_imgui_rs::Ui, editor_ctx: &EditorContext) {
    let editor = ui.node_editor(editor_ctx, "node-editor", [0.0, 400.0]);

    let node = editor.begin_node(NodeId::new(1));
    ui.text("Node A");
    {
        let pin = node.begin_pin(PinId::new(11), PinKind::Output);
        ui.text("out");
        pin.end();
    }
    node.end();

    editor.link(LinkId::new(100), PinId::new(11), PinId::new(21));
    editor.end();
}
```

Run the repository example:

```bash
cargo run -p dear-imgui-examples --bin node_editor_basic --features node-editor
```
