---
name: "upgrade-dear-imgui-stack"
description: "Use when a user asks to upgrade Dear ImGui, cimgui, ImPlot, ImPlot3D, ImNodes, ImGuizmo, Dear ImGui Test Engine, or related bindings in this repository. Refresh submodules, regenerate pregenerated native/WASM bindings, audit safe Rust API and backend shim changes, update examples/docs/changelog/versioning, and validate release readiness."
---

# Upgrade Dear ImGui Stack

Use this skill for repository-local Dear ImGui stack upgrades in `dear-imgui-rs`.

Read `references/workspace-upgrade-checklist.md` before making changes.

## Quick start

1. Confirm the requested upstream targets and whether this is just an integration bump or a release cut.
2. Confirm the repository's canonical binding generator, libclang major version, target profiles, and current clean verification hashes before moving any submodule. Run `python tools/upstream_contract.py --check` and preserve the accepted facts/decision manifests as the review baseline. A different libclang version may produce a valid but non-canonical binding diff.
3. Use primary sources for upstream changes: Dear ImGui release notes / changelog, `cimgui`, `cimplot`, `cimplot3d`, and `imgui_test_engine` commits or changelogs.
4. Prefer the repository scripts for submodule refresh, bindings generation, version bumps, pre-publish checks, and publishing.

## Workflow

1. Define scope.
   - Minimum scope for a Dear ImGui core bump is `dear-imgui-sys` + `dear-imgui-rs`.
   - Common coupled scope also includes `dear-implot-sys`, `dear-implot3d-sys`, and `dear-imgui-test-engine-sys`.
   - If upstream backends or platform callbacks changed, include backend crates, `backend_shim`, and mobile/SDL examples in the audit scope.
   - If the target release is breaking, scan the previous release notes for deprecations promised for removal in this release and remove them now.

2. Refresh submodules and pregenerated bindings.
   - Prefer `tools/update_submodule_and_bindings.py` over manual per-crate binding steps.
   - Regenerate both native pregenerated bindings and WASM pregenerated bindings when ABI or public header shape changes.
   - Keep `imgui_test_engine` in sync when the new Dear ImGui version changes internal hooks or test-engine integration points.
   - After regeneration, use `python tools/upstream_contract.py --write-review-template` to enumerate every live declaration, enum, field (including bitfield/array width), typedef (including declarations without generator locations), layout, and nested-source-pin delta. The inventory must give every source an API-contract provider; Test Engine is audited from its final checked-in Rust binding and unknown public syntax is a hard failure. Do not update the accepted snapshot until every item has a reviewed classification and safe items name compile/runtime evidence; then use `--update-snapshot` followed by `--check`.

3. Audit the sys layer and safe layer together.
   - Compare upstream release notes and generated binding diffs against current safe wrappers.
   - Look for added or removed functions, changed return types, enums, flags, struct fields, renamed APIs, callback ABI changes, backend/platform hooks, and new style/spec fields.
   - Never treat equal structure size and alignment as proof that a handwritten FFI mirror is compatible. Audit field order, offsets, types, names, and semantics. Prefer a `repr(transparent)` wrapper around the generated type; if a mirror is unavoidable, add offset checks and behavior sentinels in addition to size checks.
   - For changed public native aggregates, run the source-build-only `abi-probe` profile. It compares C++ `sizeof`, `alignof`, and selected field offsets with the Rust binding; it is a release/CI proof and must never become a prebuilt-consumer requirement.
   - Audit lifecycle and ownership fields even when they should remain hidden from the safe API. A new queue pointer, status transition, or native auto-completion rule can change Rust-side ownership without changing the public ABI size.
   - For Dear ImGui core bumps, re-audit the local stack layout compatibility patch: the `imgui.cpp` `ItemSize()` / `ItemAdd()` hook markers, `dear-imgui-sys/src/stack_layout_shim.cpp`, the `stack_layout_imgui_*.cpp.inc` snippets, native prebuilt manifest feature detection, and the `node_editor_showcase` blueprints layout.
   - Do not stop at raw compatibility. If upstream semantics changed, prefer a coherent safe API refactor over thin shims.
   - When Dear ImGui backends move, audit `dear-imgui-sys::backend_shim`, backend crates, and any examples that expose the changed integration path.

4. Update examples and docs.
   - Add or adjust examples that exercise newly exposed safe APIs, especially for ImPlot / ImPlot3D / test-engine / backend changes.
   - Update `CHANGELOG.md`, compatibility docs, publishing docs, and release notes.
   - For an actual release, convert the top `Unreleased` notes into a dated release entry and add a concise `Highlights` section.

5. Align versions and release metadata.
   - Keep the published workspace on a single release train minor unless there is a strong reason not to.
   - Update `dear-imgui-build-support` and every `*-sys` dependency edge when the workspace release train changes.
   - Refresh workspace and example lockfiles after version changes.
   - Re-check publish order and packaging assumptions whenever a new published helper crate is introduced or repriced.

6. Validate.
   - Serialize Cargo builds and nextest execution on shared development machines, reusing the workspace target directory.
   - Verify regenerated binding hashes before formatting and workspace checks.
   - Require a clean upstream-contract audit after the reviewed snapshot update; binding hashes alone cannot approve a changed API or ownership contract.
   - Run the native aggregate ABI probe whenever core declarations or handwritten public mirrors changed.
   - Run the repository pre-publish checks.
   - Run targeted tests and example checks for the upgraded surface.
   - Dry-run the publish flow before considering the work done.

## Bundled Resources

- `references/workspace-upgrade-checklist.md`
  Use this for the repository-specific upstream map, command recipes, validation matrix, and release/doc checklist.
