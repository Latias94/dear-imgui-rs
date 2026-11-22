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
they are not official Dear ImGui APIs. Enable this crate's `blueprints` feature
to compile and link the patched native core artifact. Ordinary node-editor use
does not enable that artifact profile.

The vendored native `imgui-node-editor` sources are MIT-licensed. See
`extensions/dear-node-editor-sys/third-party/cimnodes_editor/imgui-node-editor/LICENSE`
in the repository and `dear-imgui-sys/THIRD_PARTY_NOTICES.md` for the stack
layout compatibility notice.

## Compatibility

The `0.22` release train requires Rust 1.95 or newer. Upgrade all workspace crates together. See the [0.22 migration notes](../../docs/COMPATIBILITY.md#022-release-train).

| Item                 | Version |
|----------------------|---------|
| Crate                | 0.22.0  |
| dear-imgui-rs        | 0.22.0  |
| dear-node-editor-sys | 0.22.0  |

## Quick Start

```rust
use dear_node_editor::{EditorContext, NodeEditorUiExt, NodeId, PinId, LinkId, PinKind};

fn setup(imgui: &dear_imgui_rs::Context) -> EditorContext {
    EditorContext::create(imgui)
}

fn draw(ui: &dear_imgui_rs::Ui, editor_ctx: &EditorContext) {
    ui.with_node_editor(editor_ctx, "node-editor", [0.0, 400.0], |editor| {
        editor.node(NodeId::new(1), |node| {
            ui.text("Node A");
            node.pin(PinId::new(11), PinKind::Output, |_pin| ui.text("out"));
        });

        editor.node(NodeId::new(2), |node| {
            node.pin(PinId::new(21), PinKind::Input, |_pin| ui.text("in"));
        });
        editor.link(LinkId::new(100), PinId::new(11), PinId::new(21));
    });
}
```

To migrate from `ui.node_editor(...)`, move the statements up to `editor.end()`
inside `ui.with_node_editor(..., |editor| { ... })`. Return any result from the
closure. It owns the editor frame through normal return and unwinding. The
manual `unsafe` entry point requires the caller to balance the native canvas in
the same window and drawing channel. Use `editor.with_suspended(|| ...)` for
screen-space popups. Suspensions can nest inside a node or pin.

Call `show_node_context_menu` or `show_link_context_menu` on the suspended canvas
before opening `ui.popup`. A successful query moves the host window's cursor.
Store the returned ID until the user chooses a menu item. Inside the popup you
can read or change the editor's selection, inspect link endpoints, and queue a
`delete_node` or `delete_link` request. `break_node_links` and `break_pin_links`
queue the live links attached to a node or pin. On a later frame, call
`begin_delete` on the canvas and update your graph for each accepted ID.

If an application stores `dear_imgui_rs::Context` and `EditorContext` in the same owner struct,
declare `EditorContext` before the ImGui context so the editor is dropped first. Native
`imgui-node-editor` state is tied to the owning ImGui context. Its native owner
holds the allocator thread until the editor has been destroyed.

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

The corresponding scoped and manual entry points are:

| Operation | Scoped entry point | Unsafe manual entry point |
| --- | --- | --- |
| Editor | `ui.with_node_editor` | `ui.node_editor` |
| Node and pin | `editor.node`, `node.pin` | `editor.begin_node`, `node.begin_pin` |
| Group hint | `editor.group_hint` | `editor.begin_group_hint` |
| Creation preview | `editor.with_create` | `editor.begin_create` |
| Screen-space drawing | `editor.with_suspended` | `editor.suspend` |

`with_create` supplies a `CreateCandidate` and a response handle. Match on the
candidate and use the response to accept or reject it. The initial color and
thickness style the preview, and the styled response methods can override them.

Run the repository examples:

```bash
# Minimal copy-ready interaction sample.
cargo run -p dear-imgui-examples --bin node_editor_minimal --features node-editor

# Blueprint-style showcase based on upstream blueprints-example.
cargo run -p dear-imgui-examples --bin node_editor_showcase --features node-editor-blueprints
```
