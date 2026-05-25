# Bevy Backend Issue Audit - 2026-05-25

Status: Post-closeout audit

This note records a follow-up issue audit after
`654ac26 feat(bevy): finalize prelaunch backend refactor` and
`3ba5356 docs(bevy): update prelaunch handoff after commit`.

## Sources Checked

- `Latias94/dear-imgui-rs` GitHub issues via the GitHub API.
- `Latias94/dear-imgui-rs` issue search for `bevy`.
- External issue themes from the prelaunch design:
  - `vladbat00/bevy_egui`
  - `jbrd/bevy_mod_imgui`

Observed state:

- `Latias94/dear-imgui-rs` has no open issue in the fetched issue list.
- The fetched issue objects are all closed: `#1`, `#6`, `#8`, `#9`, `#11`, `#12`, `#14`, `#15`,
  `#16`, `#19`, `#20`, `#21`, `#22`, `#26`, `#27`, `#28`, and `#29`.
- `repo:Latias94/dear-imgui-rs is:issue bevy` returned `total_count=0`.
- No evidence was found for an unhandled Bevy-specific issue in this repository issue tracker.

## Repository Issue Sweep

The full local repository issue history maps to release hygiene, core API expectations, and extension
crate usability. None of the repository issues is Bevy-specific, but several are relevant to Bevy
users because they affect safe API calls, input, sys builds, or platform/runtime expectations.

| Issue | User concern | Bevy release relevance | Audit result |
| --- | --- | --- | --- |
| [`#1`](https://github.com/Latias94/dear-imgui-rs/issues/1) FFI type mismatch on Linux | Generated binding type correctness | Indirect: Bevy depends on the same `dear-imgui-sys` layer | Closed since v0.2.0; no Bevy-specific gap found. Current Bevy gates already exercise the sys crate through normal native and wasm checks. |
| [`#6`](https://github.com/Latias94/dear-imgui-rs/issues/6) stale wasm pregenerated bindings | Wasm release hygiene after an ImGui update | Indirect and important for Bevy wasm compile confidence | Covered by current checked wasm bindings and Bevy wasm compile gates. Browser runtime IME/clipboard remains a documented platform boundary, not this issue's binding skew. |
| [`#8`](https://github.com/Latias94/dear-imgui-rs/issues/8) missing `dear-app` crate release | Crate publishing completeness | General release hygiene | No Bevy code gap. The takeaway is to ensure `dear-imgui-bevy` is included in release/package checks before publication. |
| [`#9`](https://github.com/Latias94/dear-imgui-rs/issues/9) window close API | ImGui window title-bar close should update user state | Indirect: Bevy users write the same safe UI code | Covered by `Window::opened(&mut bool)` in core. Bevy multi-viewport close routing is separately covered by viewport tests for secondary OS-window close requests. |
| [`#11`](https://github.com/Latias94/dear-imgui-rs/issues/11) key modifier mappings | Backends need to submit Ctrl/Shift/Alt/Super | Directly relevant to Bevy input | Covered: core exposes modifier keys, and `dear-imgui-bevy` synchronizes `Key::ModCtrl`, `ModShift`, `ModAlt`, and `ModSuper` from Bevy input state. |
| [`#12`](https://github.com/Latias94/dear-imgui-rs/issues/12) optional build/download dependencies | Avoid surprising HTTP/async build dependencies | General release hygiene | Covered for the concern that matters to backend consumers: HTTP download support is feature-gated through `prebuilt` / `build-support/download`; normal source builds do not require it. |
| [`#14`](https://github.com/Latias94/dear-imgui-rs/issues/14) custom backend usage | Users need platform + renderer responsibilities to be explicit | Indirect and conceptually relevant | The Bevy plugin now supplies an integrated platform/input/render backend, advertises backend status, and documents unsupported policy areas. No new Bevy blocker. |
| [`#15`](https://github.com/Latias94/dear-imgui-rs/issues/15) file browser reopen visibility | Extension state-machine usability | Not Bevy-specific | No Bevy backend gap. It reflects the same user expectation that immediate-mode helper state should be explicit and reusable. |
| [`#16`](https://github.com/Latias94/dear-imgui-rs/issues/16) file browser filters | Extension default-state semantics | Not Bevy-specific | No Bevy backend gap. No action needed for Bevy release maturity. |
| [`#19`](https://github.com/Latias94/dear-imgui-rs/issues/19) imnodes demo crash | Extension UB / safe wrapper correctness | Not Bevy-specific | Reported fixed before this lane; no evidence that the Bevy backend adds an imnodes-specific risk. |
| [`#20`](https://github.com/Latias94/dear-imgui-rs/issues/20) MinGW link failure | Windows native link completeness | Indirect: affects consumers on Windows | Covered by the `dear-imgui-sys` Windows link fix for `ShellExecuteW` / `shell32`; not a Bevy-specific blocker. |
| [`#21`](https://github.com/Latias94/dear-imgui-rs/issues/21) "Can't run an example" | Users expect ImGui core alone to open/draw a window | Indirect and relevant to docs/examples | Bevy integration addresses this by owning the Bevy schedule/input/render path. README examples should continue to show plugin setup instead of naked `Context::frame()` usage. |
| [`#22`](https://github.com/Latias94/dear-imgui-rs/issues/22) external GL context path | Renderer backend must respect externally managed context/lifetime | Indirect renderer-lifecycle signal | Not directly applicable to Bevy's WGPU render-app path. The equivalent risk is stale renderer/platform callback cleanup, which is covered by Bevy plugin tests. |
| [`#26`](https://github.com/Latias94/dear-imgui-rs/issues/26) ImPlot item styling | Extension API completeness | Not Bevy-specific | No Bevy backend gap. Bevy users can still use extensions through the same core UI API once extension APIs are present. |
| [`#27`](https://github.com/Latias94/dear-imgui-rs/issues/27) `Condition::Never` invalid value | Safe API exposed an invalid ImGui condition | Indirect but high relevance: all backend users call the same API | Covered in current tree: `Condition::Never` is absent, and `dear-imgui/tests/condition_values.rs` checks valid condition values. This was the main core API issue that could have leaked into Bevy examples/user code. |
| [`#28`](https://github.com/Latias94/dear-imgui-rs/issues/28) Windows/MSVC build depended on libclang | Normal source build should not require bindgen/libclang | Indirect and important for Bevy adoption | Covered by current `dear-imgui-sys` build defaults: normal builds prefer checked-in pregenerated bindings, and bindgen is an explicit maintainer/regeneration path. |
| [`#29`](https://github.com/Latias94/dear-imgui-rs/issues/29) `ChildFlags` public access | Safe API export gap | Indirect: Bevy users use the same widgets | Covered in current tree: `ChildFlags` is publicly re-exported from `dear-imgui/src/window/mod.rs`, with regression coverage in `dear-imgui/tests/window_child_flags.rs`. |

Complete sweep conclusion:

- No repository issue identifies a missing `dear-imgui-bevy` feature or a broken Bevy workflow.
- The Bevy-relevant repository issues are already covered by the current core API, sys build, input,
  and plugin-lifecycle implementation.
- The remaining maturity questions come from external Bevy UI integration history: GPU/runtime
  validation, wasm/browser policy, picking policy, texture interop examples, and accessibility
  boundary documentation.

## External Issue Theme Audit

### Covered By The Prelaunch Refactor

- Explicit camera / viewport targeting:
  - `vladbat00/bevy_egui#315` asked how to specify the camera / viewport for UI rendering.
  - Current `dear-imgui-bevy` exposes `render::ImguiOverlayCamera`, preserves a fallback for simple apps, extracts `Camera.viewport`, applies render-pass viewport state, and clips scissors to the Bevy camera viewport.

- Bevy image sampler semantics:
  - `jbrd/bevy_mod_imgui#72` reported ignored texture samplers.
  - Current renderer bind groups include samplers. Registered Bevy images use `GpuImage::sampler`, while managed Dear ImGui textures keep standard linear/nearest sampler callbacks.

- Native docking multi-viewport baseline:
  - `vladbat00/bevy_egui#454`, `jbrd/bevy_mod_imgui#2`, and `jbrd/bevy_mod_imgui#8` show that native extra windows are a recurring demand.
  - This repository already has the native `render,multi-viewport` path and closeout coverage. Remaining window-manager polish stays follow-up scope, not a reopened prelaunch blocker.

- Frame scheduling and render-app readiness:
  - `vladbat00/bevy_egui#287` shows that UI integrations can suffer from schedule-order ambiguity.
  - `vladbat00/bevy_egui#389` shows that render pass output can be observed before it has been prepared.
  - Current `dear-imgui-bevy` uses explicit `ImguiBeginFrame`, `ImguiPrimaryContextPass`, and
    `ImguiEndFrame` schedules. Render support is advertised through `ImguiBackendStatus` only after
    the Bevy `RenderApp` integration is installed, and renderer tests cover stale callback
    replacement plus render-resource installation.
  - User UI system ordering inside `ImguiPrimaryContextPass` remains application-owned; callers
    should order their own UI systems when one panel depends on another.

- DPI / scale-factor feedback:
  - Historical Bevy ImGui issue themes include scale-factor and multi-window DPI changes.
  - Current input, lifecycle, and viewport tests cover display framebuffer scale, invalid scale
    fallback, secondary-window framebuffer scale, and DPI feedback callbacks.

- Shutdown / stale callback cleanup:
  - `jbrd/bevy_mod_imgui#20` reported shutdown panic behavior in another Bevy ImGui integration.
  - Current plugin and viewport tests cover clearing backend user data, replacing stale platform and
    renderer callbacks, and dropping context state before Dear ImGui destroys platform windows.

- Minimized / occlusion feedback:
  - The second audit pass found stale README wording that claimed minimized feedback always fell
    back to `false`.
  - Current code maps Bevy `WindowOccluded` events into Dear ImGui minimized feedback for secondary
    viewport windows and preserves the last observed value when `Window` has no persistent field.
  - README was corrected in this audit pass.

### Still Worth Splitting Into Follow-Ups

1. Pixel-level GPU/color harness

   External signals:

   - `vladbat00/bevy_egui#291` tracks color-test mismatch.
   - `jbrd/bevy_mod_imgui#65` reported corrupted startup rendering in a Bevy renderer transition.

   Current state:

   - Gamma/compositing selection is covered by unit tests.
   - Split-screen viewport behavior is covered by extraction/preparation/scissor tests.
   - There is no pixel-level screenshot/color harness for split-screen, startup, or wasm color output.

   Recommendation:

   - Open a narrow follow-up for a GPU screenshot/color harness. Start with native render target
     capture for primary, split viewport, and render-to-image paths; only add wasm color capture if
     the browser harness cost is acceptable.

2. Wasm/browser IME, clipboard, keyboard layout, and virtual keyboard positioning

   External signals:

   - `vladbat00/bevy_egui#246` tracks broad wasm input issues.
   - `vladbat00/bevy_egui#447` tracks wasm IME text editing.
   - `vladbat00/bevy_egui#446` tracks native Linux IME/backspace behavior.
   - `vladbat00/bevy_egui#185` tracks browser clipboard canvas routing.
   - `vladbat00/bevy_egui#247` tracks browser emoji picker positioning.
   - `vladbat00/bevy_egui#169` tracks non-QWERTY keyboard layout behavior on wasm.

   Current state:

   - `dear-imgui-bevy` compiles for wasm with and without `render`.
   - README documents that clipboard is application-provided: the plugin preserves an existing
     `Context::set_clipboard_backend` but does not install a native or browser clipboard backend.
   - README documents wasm runtime IME and clipboard as host-dependent.
   - Bevy `0.19.0-rc.2` has `bevy_clipboard` / `system_clipboard`, but this backend does not
     currently adapt that resource into Dear ImGui's `ClipboardBackend` trait.
   - The backend queues `KeyboardInput.text`, maps physical keys including Backspace, queues
     committed IME text, and tracks IME enable/disable; preedit text is not injected.
   - There is no browser runtime smoke test for IME, clipboard, non-QWERTY input, or virtual keyboard placement.

   Recommendation:

   - Keep current release wording honest.
   - Split a platform workstream only if browser runtime support becomes a release goal. It should
     include a real browser harness, not just `wasm32-unknown-unknown` compile gates.

3. Bevy picking integration policy

   External signals:

   - `vladbat00/bevy_egui#348` questions whether UI integrations should block Bevy picking by default.
   - `vladbat00/bevy_egui#436` reports picking blocked even when UI is visually under Bevy UI.

   Current state:

   - `dear-imgui-bevy` exposes `ImguiInputCapture` and `imgui_wants_*` run conditions.
   - The backend intentionally does not consume or rewrite Bevy input/picking messages.
   - README documents picking integration as outside the backend.

   Recommendation:

   - Do not change the default backend behavior.
   - Consider an optional picking adapter/example that shows how to wire Bevy picking policy from
     `ImguiInputCapture` without making the core plugin opinionated.

4. Embedded Bevy UI node / safe-area recipes

   External signals:

   - `vladbat00/bevy_egui#421` asks about rendering UI into a Bevy UI node / viewport node setup.
   - `vladbat00/bevy_egui#444` asks how to change the useful UI space.

   Current state:

   - Camera viewport and render-to-image texture interop provide lower-level pieces.
   - There is no first-class Bevy UI node adapter or concise recipe for responsive sidebars,
     safe areas, or UI-node render targets.

   Recommendation:

   - Treat this as product/documentation work, not backend correctness.
   - A focused example could combine `Camera.viewport`, `ImguiOverlayDisabled`, and
     `ImguiBevyTextures` to show a scene texture inside an ImGui/editor layout.

5. Texture hot-resize / re-register ergonomics

   External signals:

   - `jbrd/bevy_mod_imgui#70` asks for re-registering textures after resize.
   - `jbrd/bevy_mod_imgui#75` asks for a custom texture example demonstrating sampler behavior.

   Current state:

   - `ImguiBevyTextures::register` is idempotent for the same `Handle<Image>`.
   - `ImguiBevyTextures::unregister` removes the mapping.
   - Render-world bind groups are rebuilt from the current `RenderAssets<GpuImage>` entry each
     prepare pass, so a resized Bevy image under the same handle should naturally refresh its bind
     group.
   - Render-world bind groups remove stale Bevy image bindings when a registration disappears.
   - There is no small public example dedicated to Bevy image sampler/hot-resize behavior.

   Recommendation:

   - Add a focused texture-interoperability example or test for unregister/register churn if this
     becomes a user-facing concern.
   - The existing implementation looks structurally safe for same-handle resize, but a regression
     test for immediate unregister/register and a minimal sampler example would be cheap.

6. Accessibility / AccessKit boundary

   External searches did not show a strong `bevy_egui` issue signal for AccessKit, but Bevy users
   may still expect accessibility behavior from engine UI integrations.

   Current state:

   - `dear-imgui-bevy` does not expose an accessibility tree or AccessKit integration.
   - Dear ImGui's immediate-mode widgets do not automatically map to Bevy UI accessibility nodes.
   - Bevy `0.19.0-rc.2` includes `bevy_a11y` and AccessKit support, but the backend does not
     generate `AccessibilityNode` components for Dear ImGui widgets.

   Recommendation:

   - If this backend is positioned as an editor/runtime UI layer, document accessibility as not
     provided unless a separate design exists.

## Recommended Next Work

The prelaunch lane can remain closed. The issue audit suggests two concrete follow-on lanes, in
priority order:

1. `bevy-backend-gpu-runtime-validation-v1`
   - Pixel/color screenshot harness.
   - Startup render safety.
   - Split-screen camera viewport pixel proof.
   - Optional browser color capture if feasible.

2. `bevy-backend-platform-policy-v1`
   - Browser wasm IME/clipboard/non-QWERTY runtime smoke.
   - Optional Bevy picking adapter/example.
   - Texture interop example for sampler and hot-resize churn.
   - Accessibility support-boundary documentation.

None of these findings invalidate the closed BPR work. They are follow-up product hardening items
exposed by issue history rather than missing fixes in the completed prelaunch refactor.
