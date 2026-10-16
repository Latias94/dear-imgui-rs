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
| SDL3 + WGPU | SDL adapter and shared WGPU attachment/fault tests cover callback/runtime identity. | The bounded SDL3 example submits real scene producers before secondary UI consumers, before main acquisition. | Native smoke checks distinct texture/native-target associations, then a shared registration at revision two with `PresentationRequested` in the same UI epoch. Shared tests cover partial faults. | Native smoke merges one panel, closes another, verifies exact-instance retirement, and releases renderer before platform. SDL lease tests also cover Context-first native lifetime. | **A** on Windows. Surface-retry injection and other desktop targets remain separate evidence. |
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
- `examples/02-docking/sdl3_wgpu_multi_viewport.rs --smoke-frames 120`
- `examples/04-integration/wgpu_texture_inspector.rs --smoke-frames 16` with `texture-inspector-multi-viewport`

The CI smoke entry points use the existing runtime-gate tooling; the two public example scenarios
are additional bounded native runs. No new evidence database, source-shape parser, or renderer
facade is introduced by this matrix.

## Editor contract evidence (2026-10-16)

The editor integration adds explicit WGPU recording, submission and presentation capabilities.
The native Windows runs below use WGPU 30 on an NVIDIA RTX 4090; they establish API-stage and
resource-lifetime evidence, not compositor acceptance or physical pixel visibility.

| Scenario | Observed result |
| --- | --- |
| SDL3 native editor smoke | Two exact secondary registrations first consume distinct red/blue scene bindings, then the same replaced binding at revision two. Both request presentation in the same epoch. Merge/close retires both old registrations, followed by ordered shutdown. |
| Winit/Dear App texture inspector | A real HDR source is converted to RGBA8, resized under stable registrations, then consumed by main and secondary native allocations at revision two. Both reach `PresentationRequested` during the bounded run. |
| Pending native work | Independent Vulkan and DX12 tests submit real acquired frames, retire Context/platform ownership, and then present, discard, or drop pending work. Leases retain the exact native allocation until frame and Surface release. |
| Detached WGPU executor | A real worker thread records UI sampling, then submits an engine producer before that consumer. Red external and green managed-texture pixels are read back; stale targets, out-of-order epochs, partial upload feedback and dropped preparation are exercised separately. |
| Format conversion | Mandatory GPU readback checks real Depth32Float, R32Uint and Rgba16Float source conversion across layer/mip changes, exposure/range mapping and resize. Missing adapters fail this check. |

`dear-app` also has an explicitly run native undock/merge/close/device-loss regression. Recovery
keeps Context and main allocation identity while recreating secondary allocations and configuring
new Surfaces for the replacement Device. See [the migration guide](EDITOR_BACKEND_MIGRATION.md)
for the ownership and reporting boundaries.

The pending-frame tests above passed with explicit Vulkan-only and DX12-only Instances. With
WGPU 30.0.1 and NVIDIA driver 617.14 on this Windows system, a multi-backend `Backends::PRIMARY`
Instance can instead terminate with exit code 2173 during submitted-frame discard teardown. A
pure WGPU diagnostic reproduces this without a Dear ImGui Context or this crate's native leases;
the underlying cause remains unresolved. This is a recorded failure, not a passing multi-backend
claim. See [the WGPU validation notes](../backends/dear-imgui-wgpu/README.md#native-discard-teardown-validation)
for separate-process commands and the preserved reproducer.

The corresponding local regression matrix completed serially in the existing target directory:

| Check | Result and scope |
| --- | --- |
| Core `multi-viewport` | 420 library tests passed. |
| Winit `multi-viewport` | 151 regular tests, two explicitly run native regressions, and ten doctests passed. |
| SDL3 `multi-viewport` | 84 library tests and twelve doctests passed, including owned drop handoff and native lease lifetime. |
| Dear App `multi-viewport,test-engine` | 81 library tests passed; three native regressions were also run explicitly. Application doctests and example all-target checks passed. |
| WGPU 27/28/29/30 | Separate Winit and SDL3 route compile checks passed for each major. Seven detached-executor regressions passed on each major; WGPU 30 full route suites and 38 doctests also passed. |
| Other native consumers | Ash/Winit and Bevy multi-viewport compile checks passed. The ignored Glow external-context regression was run explicitly and passed. |
| WASM | WGPU 30 single-window `cargo check --target wasm32-unknown-unknown --features webgpu` passed. This is compile evidence, not a browser runtime result. |
| Formatting | `cargo fmt --check` passed for the changed core, platform, WGPU, application, and example packages. |

The Linux Test Engine gate remains Linux/display-tool specific; compiling its executable on
Windows does not establish its X11, Lavapipe, or interactive dragging assertions. Cocoa, X11,
Wayland and the other renderer rows retain the limits stated above.
