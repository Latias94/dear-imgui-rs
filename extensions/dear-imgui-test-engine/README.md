# dear-imgui-test-engine

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-test-engine.svg)](https://crates.io/crates/dear-imgui-test-engine)
[![Documentation](https://docs.rs/dear-imgui-test-engine/badge.svg)](https://docs.rs/dear-imgui-test-engine)

Safe, idiomatic Rust integration for [Dear ImGui Test Engine](https://github.com/ocornut/imgui_test_engine) on top of `dear-imgui-rs`.

- Transactional ownership: one live engine attachment per process, including suspended Contexts.
- Typed lifecycle and FFI errors with copied native diagnostics.
- Test queue helpers with explicit Ready/Queued/Running/Terminal state.
- A bounded `TestRunner` that reports five product outcomes separately from infrastructure errors.
- Move-only reports tied to a stable engine ID, run ID, and exact selected-test manifest.
- Runtime controls: speed, verbosity, capture output, abort.
- UI integration: show built-in test engine windows in an active ImGui frame.

For native build/link options, see `extensions/dear-imgui-test-engine-sys/README.md`.

## Links

- Upstream: https://github.com/ocornut/imgui_test_engine
- Low-level crate: `dear-imgui-test-engine-sys`
- Example: `examples/04-integration/test_engine_integration.rs`

## Compatibility

| Item                        | Version |
|-----------------------------|---------|
| Crate                       | 0.16.0-alpha.3  |
| dear-imgui-rs               | 0.16.0-alpha.3  |
| dear-imgui-test-engine-sys  | 0.16.0-alpha.3  |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md).

## Quick Start

Until `0.16.0-alpha.3` is published, use matching Git dependencies from `main`:

```toml
[dependencies]
dear-imgui-rs = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
dear-imgui-test-engine = { git = "https://github.com/Latias94/dear-imgui-rs", branch = "main" }
```

After publication, use the exact prerelease requirements:

```toml
[dependencies]
dear-imgui-rs = "=0.16.0-alpha.3"
dear-imgui-test-engine = "=0.16.0-alpha.3"
```

```rust
use std::convert::Infallible;
use std::num::NonZeroU64;

use dear_imgui_rs as imgui;
use dear_imgui_test_engine as test_engine;

let mut imgui_ctx = imgui::Context::create();
imgui_ctx
    .font_atlas()
    .try_claim_legacy_renderer()
    .expect("headless tests require the legacy font-atlas capability")
    .build();

let mut engine = test_engine::TestEngine::create()?;
engine.start(&mut imgui_ctx)?;
engine.set_run_speed(test_engine::RunSpeed::Fast)?;
engine.register_default_tests()?;
imgui_ctx.prepare_frame(imgui::FramePrepareOptions::new(
    [128.0, 128.0],
    1.0 / 60.0,
));

let report = test_engine::TestRunner::new(&mut engine)
    .frame_budget(NonZeroU64::new(600).unwrap())
    .run_headless(&mut imgui_ctx, |ui, _frame_index| {
        // Draw the application UI that script tests drive.
        ui.text("Application under test");
        Ok::<_, Infallible>(test_engine::RunnerControl::Continue)
    })?;

println!(
    "outcome: {:?}, frames: {}, mode: {:?}",
    report.outcome(),
    report.frames(),
    report.mode(),
);

for test in report.tests() {
    println!("{}/{}: {:?}", test.category(), test.name(), test.status());
}

// Explicit shutdown reports cleanup failures and is idempotent.
engine.shutdown()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`RunReport::outcome()` is one of:

| Outcome | Meaning |
| --- | --- |
| `Passed` | At least one test ran and every test passed. |
| `Failed` | At least one test ran and one or more tests failed. |
| `NoMatch` | The filter reached terminal state without executing a test. |
| `TimedOut` | The primary frame budget expired and the runner drained the queue. |
| `Aborted` | The application requested abort and the runner drained the queue. |

All five are product outcomes returned in `Ok(RunReport)`. `HeadlessRunnerError<E>` and
`RunnerError<ApplicationError, PrepareError, RenderError, PresentError>` are reserved for
infrastructure failures such as FFI/status errors, a wrong or dead Context, an already-open frame,
application/backend failures, a managed renderer routed through headless mode, or a cleanup queue
that cannot settle. A CI gate should therefore require `RunOutcome::Passed`; it must not interpret
every `Ok` report as success.

Use `run_graphical` for graphical or texture-using tests. Implement `TestFrameDriver` on the object
that owns backend preparation, main-target rendering, and surface presentation:

- `prepare` consumes `FrameToken` and returns the driver's GAT-backed `PreparedFrame`. A
  single-window driver can use `ReconciledFrame`; a multi-viewport route keeps its own prepared
  transaction so secondary completion, deferred faults, and renderer-specific retirement proof are
  not erased. Managed renderers use their own `SynchronousRendererConsumer`; legacy renderers use
  `FrameToken::render_legacy`.
- `prepared_context_id` reports the Context carried by that prepared transaction. The Test Engine
  verifies it before allowing the main target to render.
- `render_main` consumes `PreparedFrame` and returns `MainRenderOutcome::ReadyToPresent` or
  `MainRenderOutcome::Skipped`. OpenGL-style drivers may draw the main viewport and then complete
  auxiliary contexts here, as long as all draw work finishes before the method returns.
- `present` performs exactly one main-surface presentation after the native pre-swap hook succeeds.

The ready path is strictly ordered:

```text
application UI -> prepare -> render main -> Test Engine pre-swap -> present -> Test Engine post-swap
```

A skipped main render returns `FrameDriveOutcome::Skipped` without calling pre-swap, `present`, or
post-swap. This lets timeout, occlusion, surface-loss, and outdated-swapchain recovery remain normal
backend flow instead of pretending that a presentation occurred. A failed present never calls
post-swap. `RunMode::Graphical` records the selected integration path; it is not independent proof
that the operating system displayed the swapchain image.
`run_headless` uses `RunMode::Headless` and is intentionally stricter: it drives frames through the
legacy renderer path and rejects Contexts that advertise a managed-texture renderer. Headless mode
does not create a renderer consumer or silently ignore texture work.

With the optional `capture` feature, implement `CapturingTestFrameDriver` and call
`run_graphical_with_capture`. The framebuffer provider is installed only for that run, and callback
errors or panics are returned without crossing the C ABI. Enabling the feature alone does not make a
headless run capable of screenshots.

## Notes

- `dear-imgui-test-engine-sys` automatically enables the required `IMGUI_ENABLE_TEST_ENGINE` define on `dear-imgui-sys`.
- Engine-first and Context-first teardown are both supported. Context-first teardown leaves the
  upstream shutdown/settings hooks installed until `ImGui::DestroyContext()`, then marks the Rust
  attachment as Context-destroyed before the wrapper destroys the detached engine.
- Upstream Test Engine uses process-global state. Starting a second engine, even on a suspended
  Context, returns `TestEngineStatus::BindingOccupied`. `stop()` retains that ownership because the
  upstream hooks remain installed; `shutdown()` or Context destruction releases it after unbind.
- Call `shutdown()` when cleanup errors must be handled. `Drop` is non-panicking and retries
  one-shot native failures on a best-effort basis.
- A queued/running run rejects another queue request. Consume a terminal summary with
  `take_terminal_summary()` before queuing again. The summary is derived only from that run's exact
  queue manifest; results from earlier filters cannot affect it.
- `TestRunner` owns queueing, application UI, frame rendering, renderer invocation, presentation
  boundaries, timeout/abort draining, and terminal-summary validation as one bounded operation.
  Integrations that already own the queue can use `TestEngine::drive_frame`; the raw swap hooks are
  intentionally not public.
- `register_builtin_test_suite()` returns an engine-generation-bound token. Validate runner output
  by passing the exact `RunReport` to `validate_registered_test_suite()`. Manually pumped suite
  integrations should use `take_terminal_test_suite_result()`, which atomically consumes and
  validates the active run instead of accepting caller-created aggregate counters.
- `show_windows()` accepts only a `Ui` from the attached Context during an active native frame.
- Upstream Dear ImGui Test Engine has its own license terms; review `extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine/imgui_test_engine/LICENSE.txt` before shipping commercial products.

## Features

- `capture`: opt into screenshot/video helpers and the run-scoped framebuffer-provider API. It is
  disabled by default and requires `run_graphical_with_capture` plus a capturing frame driver.
- `freetype`: passthrough to `dear-imgui-rs/freetype` and `dear-imgui-test-engine-sys/freetype`.

## Demo Tests

This crate bundles a small set of built-in demo tests (for validating integration):

```rust
let mut imgui_ctx = imgui::Context::create();
let mut engine = test_engine::TestEngine::create()?;
engine.start(&mut imgui_ctx)?;
engine.register_default_tests()?;
```

To write tests from Rust without dealing with C++ callbacks, use script tests:

```rust
engine.add_script_test("my_app", "open_settings", |t| {
    t.set_ref("Main Window")?;
    t.wait_for_item("Settings", test_engine::ScriptCount::new(60)?)?;
    t.wait_for_item_visible("Settings", test_engine::ScriptCount::new(60)?)?;
    t.item_click("Settings")?;
    t.input_text_replace("Search", "foo", true)?;
    t.menu_click("File/Save")?;
    t.scroll_to_item_y("Advanced Options")?;
    t.scroll_to_top("Advanced Options")?;
    t.yield_frames(test_engine::ScriptCount::new(2)?)
})?;
```

Script tests do not provide a `GuiFunc` (they don't draw any UI). They are meant to drive UI that your
application already renders every frame.

## Build notes

- This crate enables `dear-imgui-rs/test-engine` (and therefore `dear-imgui-sys/test-engine`) because the upstream Test Engine relies on ImGui hook symbols.
- Test Engine is native source-only. It has no `prebuilt` or `wasm` feature, its core hook feature
  forces `build-from-source`, and WASM/prebuilt-package combinations are rejected before linking.
- `dear-imgui-sys` provides the hook symbols when `test-engine` is enabled. This avoids workspace feature-unification causing linker errors.
- If your tests don't interact with the UI, ensure you depend on `dear-imgui-test-engine` (or
  `dear-imgui-test-engine-sys`) and call `engine.start(&mut context)` before registration/queueing.
