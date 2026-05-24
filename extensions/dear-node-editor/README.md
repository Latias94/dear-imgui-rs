# dear-node-editor

[![Crates.io](https://img.shields.io/crates/v/dear-node-editor.svg)](https://crates.io/crates/dear-node-editor)
[![Documentation](https://docs.rs/dear-node-editor/badge.svg)](https://docs.rs/dear-node-editor)

Safe Rust bindings for [imgui-node-editor](https://github.com/thedmd/imgui-node-editor) through the
[`cimnodes_editor`](https://github.com/cimgui/cimnodes_editor) wrapper and a repository-owned C ABI
shim.

This crate coexists with `dear-imnodes`:

- `dear-imnodes` wraps ImNodes and is the current wasm-capable node editor path.
- `dear-node-editor` wraps imgui-node-editor and is native-only in the first integration phase.

The native-only scope is a workspace integration boundary, not a claim that the
upstream C++ library can never run on WebAssembly. This repository's WASM path
uses an import-style provider module with explicit generated exports; node
editor support needs its own provider wiring before it can be exposed safely on
wasm.

The sys layer deliberately exposes `NodeId`, `PinId`, and `LinkId` as pointer-sized integer values
instead of upstream C++ helper pointers. The safe layer wraps those values as Rust newtypes and uses
RAII tokens for editor frames, nodes, pins, create/delete sessions, shortcuts, and style pushes.

## Upstream Relationship

This crate is a safe Rust layer over:

- [`imgui-node-editor`](https://github.com/thedmd/imgui-node-editor)
- [`cimnodes_editor`](https://github.com/cimgui/cimnodes_editor)

The `node_editor_showcase` example follows the upstream blueprints example
structure and uses the same style of stack layout calls (`BeginHorizontal`,
`BeginVertical`, and `Spring`) through `dear-imgui-rs` safe wrappers. Those
stack layout helpers are compatibility APIs implemented by `dear-imgui-sys`;
they are not official Dear ImGui APIs.

The vendored native `imgui-node-editor` sources are MIT-licensed. See
`extensions/dear-node-editor-sys/third-party/cimnodes_editor/imgui-node-editor/LICENSE`
in the repository and `dear-imgui-sys/THIRD_PARTY_NOTICES.md` for the stack
layout compatibility notice.

## Compatibility

| Item                 | Version |
|----------------------|---------|
| Crate                | 0.14.0  |
| dear-imgui-rs        | 0.14.0  |
| dear-node-editor-sys | 0.14.0  |

## Quick Start

```rust
use dear_node_editor::{EditorContext, NodeEditorUiExt, NodeId, PinId, LinkId, PinKind};

fn setup(imgui: &dear_imgui_rs::Context) -> EditorContext {
    EditorContext::create(imgui)
}

fn draw(ui: &dear_imgui_rs::Ui, editor_ctx: &EditorContext) {
    let editor = ui.node_editor(editor_ctx, "node-editor", [0.0, 400.0]);

    editor.node(NodeId::new(1), |node| {
        ui.text("Node A");
        node.pin(PinId::new(11), PinKind::Output, |_pin| {
            ui.text("out");
        });
    });

    editor.node(NodeId::new(2), |node| {
        node.pin(PinId::new(21), PinKind::Input, |_pin| {
            ui.text("in");
        });
    });

    editor.link(LinkId::new(100), PinId::new(11), PinId::new(21));
    editor.end();
}
```

If an application stores `dear_imgui_rs::Context` and `EditorContext` in the same owner struct,
declare `EditorContext` before the ImGui context so the editor is dropped first. Native
`imgui-node-editor` state is tied to the owning ImGui context.

```rust
struct UiState {
    node_editor: dear_node_editor::EditorContext,
    imgui: dear_imgui_rs::Context,
}
```

The creation configuration is queryable through `EditorContext::config()`:

```rust
fn inspect(editor: &dear_node_editor::EditorContext) {
    let config = editor.config();
    assert!(config.custom_zoom_levels.is_empty());
}
```

The lower-level RAII tokens are still available when manual scoping is clearer:

```rust
fn draw_manual(ui: &dear_imgui_rs::Ui, editor: &dear_node_editor::NodeEditorFrame<'_>) {
    let node = editor.begin_node(NodeId::new(1));
    ui.text("Node A");
    let pin = node.begin_pin(PinId::new(11), PinKind::Output);
    ui.text("out");
    pin.end();
    node.end();
}
```

Run the repository examples:

```bash
# Minimal interaction sample based on upstream basic-interaction-example.
cargo run -p dear-imgui-examples --bin node_editor_basic --features node-editor

# Blueprint-style showcase based on upstream blueprints-example.
cargo run -p dear-imgui-examples --bin node_editor_showcase --features node-editor
```
