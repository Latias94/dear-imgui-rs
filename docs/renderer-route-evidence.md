# Renderer Route Evidence

This matrix records the evidence that exists for each first-party native
multi-viewport route. It intentionally separates platform callback safety from
renderer transaction evidence. A route is not promoted to an exact native
support claim merely because its example compiles or its shared runtime tests
pass.

## Evidence levels

| Level | Meaning |
| --- | --- |
| A | A route-level native smoke or validation executable observes the relevant window/renderer transaction and teardown. Shared unit tests may strengthen the result, but are not the only evidence. |
| B | The route has shared runtime tests, route wrappers, compile coverage, or an example with the intended order, but no dedicated route-level native smoke for the full row. Treat native behavior as unverified for the missing cells. |
| C | The route is intentionally unsupported for native multi-viewport, or the renderer has no applicable route. |

## Matrix

| Route | Platform callback containment | Secondary before main | Main surface / present / synchronization | Teardown | Level and remaining gap |
| --- | --- | --- | --- | --- | --- |
| SDL3 + OpenGL3 (official) | U2 callback-lease and ABI tests cover the owned trampoline boundary and foreign-table rejection. | Official backend path is covered by crate-level integration, but no dedicated repository native smoke currently records the complete sequence. | Upstream OpenGL3 owns the renderer transaction; this repository does not claim a route-level present trace. | Shared SDL ownership tests cover ordered shutdown; no route-specific native teardown report. | **B**. Add a native smoke if exact SDL3/OpenGL3 secondary/present/teardown claims are required. |
| SDL3 + SDLRenderer3 | Platform callbacks are covered by the shared SDL lease tests. | Not applicable: this integration is intentionally single-window. | Not applicable for native multi-viewport. | Single-window renderer shutdown only. | **C** for native multi-viewport. Documentation must keep this route out of the supported MV list. |
| SDL3 + SDLGPU3 | Shared SDL lease and renderer callback contract tests cover the Rust/C boundary. | Wrapper/example code exercises the route shape, but no native CI smoke records secondary-before-main ordering. | The route has SDL GPU submission hooks, yet native acquire/submit/present evidence is not stored in a route smoke artifact. | Shared runtime cleanup tests exist; route-level partial-failure and native teardown remain unverified. | **B**. Treat native MV behavior as unverified until a bounded SDL GPU smoke exists. |
| SDL3 + Glow | SDL callback containment and Glow callback/runtime tests cover first-fault and foreign-state rejection. | SDL3 Glow smoke records secondary IDs and merge before the main present bracket. | The smoke records main-present bracketing, GL-state restoration, sampler isolation, and callback state. | Shared Glow teardown tests cover context-first/error/panic paths; the smoke does not yet emit explicit renderer/platform teardown flags. | **A-**. Add explicit teardown flags to the smoke output or keep teardown sourced to shared unit tests. |
| SDL3 + WGPU | SDL adapter and shared WGPU attachment/fault tests cover callback/runtime identity. | The SDL3 example has the intended secondary -> main acquire -> render -> submit/present order. | The example handles surface loss/outdated/timeout/suboptimal paths, but no SDL3 native smoke records the complete transaction. | Shared WGPU cleanup tests exist; route-level SDL3 shutdown and partial-failure evidence is missing. | **B**. Add a native SDL3 WGPU smoke or mark those cells unverified in release reports. |
| SDL3 + Ash | SDL adapter and Ash shared runtime tests cover identity and first-fault propagation. | The adapter path is compiled and exercised by shared tests, but no native SDL3 smoke records secondary command submission before main acquisition. | Ash low-level tests cover swapchain recovery and fences; the SDL3 wrapper has no native present/teardown smoke. | Shared context-first/Drop/retry cleanup tests exist; SDL3 route shutdown evidence is missing. | **B**. Ash SDL3 is the highest-priority missing native route smoke. |
| Winit + WGPU | Winit adapter, WGPU attachment, and shared fault tests cover the owning callback boundary. | CI smoke observes secondary submission before main acquire and main present. | The smoke records render submission, present, and the main-surface transaction. Policy/error retry injection is mainly covered by shared tests rather than the wrapper. | Shared teardown plus the smoke's runtime completion cover context-first and renderer retirement. | **A**. Add route-level retry injection only if stronger surface-recovery evidence is needed. |
| Winit + Glow | No official first-party native multi-viewport route is published for this combination. | Not applicable. | Not applicable. | Not applicable. | **C** for the current supported route inventory. |
| Winit + Ash | Winit adapter and Ash shared runtime tests cover attachment and fault identity. | Validation smoke observes secondary create/resize/merge before main present. | The smoke records secondary render/present and GPU-idle shutdown; command-buffer lineage remains an unsafe caller contract. | Validation smoke records renderer/runtime/platform shutdown and GPU idle, with shared retry tests. | **A**. Add wrapper-level surface-retry injection only if it becomes a release requirement. |

## Shared evidence and limits

The WGPU, Glow, and Ash shared runtimes cover first-fault ordering, Context or
device identity, foreign attachment rejection, and repeated cleanup. Those tests
do not prove that every native platform route reaches the runtime in the same
way. The route cells above therefore remain separate even where they share an
implementation module.

The current smoke entry points are:

- `examples/ci/wgpu_multi_viewport_smoke.rs`
- `examples/ci/sdl3_glow_multi_viewport_smoke.rs`
- `examples/ci/ash_vulkan_validation_smoke.rs`

They are orchestrated by the existing runtime-gate tooling. No new evidence
database, source-shape parser, or renderer facade is introduced by this
matrix.

## Conformance result

The U6 review found no failing renderer ownership or ordering contract that
justifies a public WGPU, Glow, or Ash API change. U6 therefore closes the
renderer implementation portion as evidence hardening. The native gaps above
remain explicit follow-up work for a desktop-capable CI environment; they are
not silently promoted to exact support.
