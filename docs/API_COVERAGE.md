# High-level API coverage

`dear-imgui-rs` provides safe, ergonomic Rust APIs over the raw `dear-imgui-sys` bindings.
Coverage is a semantic review problem: finding the same native symbol in Rust source does not prove
that the wrapper has correct ownership, lifetime, callback, or ABI behavior.

## Coverage policy

Add a safe wrapper when the native operation can be expressed with enforceable Rust invariants.
Prefer typed flags, owned copies, validated IDs, scoped tokens, and borrows tied to the owning
`Context`, frame, atlas, or renderer epoch.

Keep an operation sys-only when a safe contract would be misleading. Current examples include:

- C variadic formatting entry points;
- obsolete Columns APIs, which are superseded by tables;
- process-global allocator and raw hook configuration;
- raw `MemAlloc` and `MemFree` ownership;
- internal or evolving font-loader callbacks; and
- APIs whose native parser accepts an unbounded external buffer.

The sys crates remain the explicit unsafe escape hatch. A wrapper may be added later when its
ownership and lifetime design is clear.

## Reviewing an upstream update

The vendored source and regenerated bindings are the canonical diff. Do not infer Rust safety from
symbol names or maintain a second generated snapshot of the same declarations.

1. Read the upstream release notes and inspect every updated submodule commit range.
2. Regenerate all maintained bindings with the canonical libclang version:

   ```console
   cargo run -p xtask -- verify-bindings --allow-dirty
   ```

3. Review generated binding changes, especially callback signatures, aggregate parameters and
   returns, field layout, enum representation, ownership comments, and newly exposed functions.
4. Search the high-level crate for an existing semantic equivalent:

   ```console
   rg -n 'alias = "FooBar"|foo_bar\(|sys::igFooBar' dear-imgui/src
   ```

5. For each user-relevant addition, choose one explicit outcome: add or extend a safe wrapper, keep
   it sys-only with a documented reason, or defer it because a separate lifetime design is needed.
6. Prove the chosen contract with the narrowest appropriate mechanism: a Rust unit test,
   compile-fail doctest, native C++/Rust ABI probe, or backend runtime test.

`#[doc(alias = "...")]` improves discovery and prevents duplicate wrappers. It is not itself proof
of safe coverage.

## Lifetime-sensitive designs

The high-level layer already models the major ownership-sensitive surfaces:

- `BakedFont<'ui>` resolves native baked storage through a live frame borrow, while glyph queries
  return owned metric copies.
- `FontAtlasTexture<'_>` is a read-only atlas lease; rebuild, clear, and pixel mutation reject a
  live lease.
- `CustomRectId` is atlas-validated, and custom-rectangle queries return owned snapshots.
- external font sources that rely on native parsers remain explicitly unsafe constructors.
- curated demo and diagnostics windows are safe; the destructive font-atlas panel is isolated.
- clipper tokens enforce their exact frame, window, table, and native stack order.
- rendered frames and snapshots are move-only leases tied to one renderer consumer and epoch.

## Context and callback rules

Only the core Context binding implementation may switch native `GImGui`. Backends and extensions
must use the scoped Context APIs instead of calling `igSetCurrentContext` in production code.

Dear ImGui platform callbacks that pass `ImVec2` or `ImVec4` by value are compiler-ABI-sensitive.
Repository-owned C++ shims translate those slots to pointer or out-parameter callbacks. Changes to
these callbacks require the existing native ABI probes on MSVC and the affected backend runtime
tests; source-text scanning is not an ABI test.

## Adding a wrapper

- Put the API in the module that owns its state and lifetime.
- Add the upstream C++ name as a rustdoc alias.
- Avoid exposing raw pointers, process-global state, or unvalidated native IDs from safe methods.
- Add a headless test when possible and a runtime or ABI test when native behavior matters.
- Update `CHANGELOG.md` for user-visible additions or migration requirements.

Use `repo-ref/` for local comparison checkouts and scratch notes. User-facing decisions belong in
the API documentation, tests, and changelog.
