---
name: "upgrade-dear-imgui-stack"
description: "Use when a user asks to upgrade Dear ImGui, cimgui, ImPlot, ImPlot3D, ImNodes, ImGuizmo, Dear ImGui Test Engine, or related bindings in this repository. Refresh submodules, regenerate pregenerated native/WASM bindings, audit safe Rust API and backend shim changes, update examples/docs/changelog/versioning, and validate release readiness."
---

# Upgrade Dear ImGui Stack

Use this skill for repository-local Dear ImGui stack upgrades in `dear-imgui-rs`.

Read `references/workspace-upgrade-checklist.md` before making changes.

## Principles

1. Treat upstream source, release notes, and regenerated bindings as the canonical change record.
   Do not replace semantic review with generated API snapshots or source-text inference.
2. Use the canonical libclang version and binding profiles before moving source pins so the binding
   diff remains attributable to the upstream change.
3. Review sys and safe layers together. A generated symbol is not a safe Rust API until ownership,
   lifetime, callback, and ABI behavior are modeled and tested.
4. Prefer a coherent breaking refactor over a compatibility shim when the old contract is unsound.

## Workflow

1. Define the coupled scope.
   - A core bump always includes `dear-imgui-sys` and `dear-imgui-rs`.
   - Include Test Engine when Dear ImGui internals or hooks moved.
   - Include extension sys crates whose generators embed or depend on the updated ImGui revision.
   - Include platform and renderer backends when callback, viewport, texture, or draw-data contracts
     changed.

2. Establish the baseline.
   - Record the current submodule SHAs and canonical binding verification result.
   - Read primary upstream release notes and commit ranges.
   - Note public declarations, internal lifecycle changes, callback ABI changes, and backend changes
     that require focused review.

3. Refresh sources and bindings.
   - Prefer `tools/update_submodule_and_bindings.py` for source updates.
   - Regenerate native and WASM bindings for every affected profile.
   - Run `cargo run -p xtask -- verify-bindings --allow-dirty` with canonical libclang.
   - Build the real Emscripten provider with
     `python tools/ci/verify_wasm_provider.py --check-rust-route` when its source or profile changes.

4. Audit the generated diff and native implementation.
   - Check added and removed functions, enum representation, fields, defaults, callback signatures,
     aggregate parameters and returns, and source revision markers.
   - Inspect hidden queue, ownership, status, and teardown semantics even when layout is unchanged.
   - Never accept size/alignment alone as proof for a handwritten FFI mirror. Prefer generated or
     transparent types; otherwise add field-offset and behavior probes.
   - Re-audit local C++ shims and source patches against their upstream markers.

5. Design the safe Rust response.
   - Search rustdoc aliases, Rust names, and sys call sites to find existing semantic wrappers.
   - For every user-relevant addition, explicitly add/extend a safe wrapper, retain a documented
     unsafe sys-only path, or defer it for a separate lifetime design.
   - Add compile-fail tests for lifetime/state restrictions and native/runtime tests for ABI or
     backend behavior. Symbol-name matching is discovery, not proof.

6. Finish the workspace change.
   - Update examples that exercise the changed user path.
   - Update `CHANGELOG.md`, compatibility docs, and release metadata.
   - Remove deprecations promised for the target breaking release.
   - Run targeted tests first, then the repository release checks. Serialize Cargo on shared
     machines and reuse the workspace target directory.

## Bundled resource

- `references/workspace-upgrade-checklist.md` contains the upstream map, PowerShell command recipes,
  repository-specific audit areas, and validation matrix.
