---
name: "upgrade-dear-imgui-stack"
description: "Upgrade Dear ImGui/cimgui, ImPlot/ImPlot3D, ImNodes/node-editor, ImGuizmo/quat, CTE, or Test Engine in this repository. Read upstream version and implementation differences, adapt Rust and backend behavior, regenerate native/WASM bindings, and validate the integration."
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
   - Find all workspace consumers of each changed contract, including Bevy/custom renderers,
     detached snapshots, application integrations, and examples; do not scope by crate names alone.

2. Establish the baseline.
   - Record committed wrapper and nested source SHAs, existing worktree differences, and canonical
     binding verification results. Resolve the target revisions before comparing versions.
   - Read the full release-note sections spanning that range, including breaking changes and
     post-release/WIP changes when the selected branch is ahead of a release tag.
   - Read the complete C/C++ header and implementation diff for each upgraded component. Follow
     changed contracts into callers and version-gated branches, including unchanged extensions
     that consume changed ImGui internals. Use the checklist's semantic review guidance.
   - Account for each material change as an adaptation, compatible native behavior, intentional
     sys-only API, or explicit deferral, with source evidence and an appropriate verification.
     Complete this accounting before treating interface compilation as upgrade success.

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
   - Trace values through their producers and consumers: units, coordinate spaces, defaults,
     texture-table contents, assertions, frame validity, and callback ordering can change while
     function names and layouts stay compatible. Inspect handwritten mirrors independently of
     generated bindings.
   - Never accept size/alignment alone as proof for a handwritten FFI mirror. Prefer generated or
     transparent types; otherwise add field-offset and behavior probes.
   - Re-audit local C++ shims and source patches against the actual compiled input. Remove a
     workaround only after confirming upstream fixes its underlying behavior. Keep required
     adaptations consistent across source, prebuilt/package, and WASM provider routes.

5. Design the safe Rust response.
   - Search rustdoc aliases, Rust names, and sys call sites to find existing semantic wrappers.
   - For every user-relevant addition, explicitly add/extend a safe wrapper, retain a documented
     unsafe sys-only path, or defer it for a separate lifetime design.
   - Add compile-fail tests for lifetime/state restrictions and native/runtime tests for ABI or
     backend behavior. Symbol-name matching is discovery, not proof.
   - For a semantic regression, exercise the production native path and observe its result
     (geometry, pixels, state, callbacks, or ownership). Choose inputs that distinguish the old
     and new contracts; include relevant DPI, bounds, or lifecycle cases. A copied implementation
     in a test is not independent evidence.

6. Finish the workspace change.
   - Update examples that exercise the changed user path.
   - Update `CHANGELOG.md`, compatibility docs, and release metadata.
   - Remove deprecations promised for the target breaking release.
   - Run targeted tests first, then the repository release checks. Serialize Cargo on shared
     machines and reuse the workspace target directory.
   - Report reviewed source ranges, resolved behavioral changes, and verification limits. Mark
     static review, compilation, and executed runtime evidence separately; retain unresolved
     platform/runtime gaps rather than claiming coverage from regeneration or a green build.

## Bundled resource

- `references/workspace-upgrade-checklist.md` contains the upstream map, PowerShell command recipes,
  repository-specific audit areas, and validation matrix.
