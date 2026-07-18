# dear-imgui-test-engine

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-test-engine.svg)](https://crates.io/crates/dear-imgui-test-engine)
[![Documentation](https://docs.rs/dear-imgui-test-engine/badge.svg)](https://docs.rs/dear-imgui-test-engine)

Safe, idiomatic Rust integration for [Dear ImGui Test Engine](https://github.com/ocornut/imgui_test_engine) on top of `dear-imgui-rs`.

- Transactional ownership: one engine attachment per ImGui Context.
- Typed lifecycle and FFI errors with copied native diagnostics.
- Test queue helpers with explicit Ready/Queued/Running/Terminal state.
- Runtime controls: speed, verbosity, capture, abort.
- UI integration: show built-in test engine windows in an active ImGui frame.

For native build/link options, see `extensions/dear-imgui-test-engine-sys/README.md`.

## Links

- Upstream: https://github.com/ocornut/imgui_test_engine
- Low-level crate: `dear-imgui-test-engine-sys`
- Example: `examples/imgui_test_engine_basic.rs`

## Compatibility

| Item                        | Version |
|-----------------------------|---------|
| Crate                       | 0.16.0  |
| dear-imgui-rs               | 0.16.0  |
| dear-imgui-test-engine-sys  | 0.16.0  |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md).

## Quick Start

```toml
[dependencies]
dear-imgui-rs = "0.16.0"
dear-imgui-test-engine = "0.16.0"
```

```rust
use dear_imgui_rs as imgui;
use dear_imgui_test_engine as test_engine;

let mut imgui_ctx = imgui::Context::create();
assert!(imgui_ctx.font_atlas().build());

let mut engine = test_engine::TestEngine::create()?;
engine.start(&mut imgui_ctx)?;
engine.set_run_speed(test_engine::RunSpeed::Fast)?;
engine.register_default_tests()?;
engine.queue_tests(
    test_engine::TestGroup::Tests,
    None,
    test_engine::RunFlags::RUN_FROM_COMMAND_LINE,
)?;

// Repeat this block in your frame loop. A graphical application should submit
// the rendered frame to its renderer before calling post_swap().
imgui_ctx.prepare_frame(imgui::FramePrepareOptions::new(
    [128.0, 128.0],
    1.0 / 60.0,
));
let frame = imgui_ctx.begin_frame();
engine.show_windows(frame.ui(), None)?;
drop(frame.render());
engine.post_swap()?;

// Explicit shutdown reports cleanup failures and is idempotent.
engine.shutdown()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Notes

- `dear-imgui-test-engine-sys` automatically enables the required `IMGUI_ENABLE_TEST_ENGINE` define on `dear-imgui-sys`.
- Engine-first and Context-first teardown are both supported. Context-first teardown leaves the
  upstream shutdown/settings hooks installed until `ImGui::DestroyContext()`, then marks the Rust
  attachment as Context-destroyed before the wrapper destroys the detached engine.
- Call `shutdown()` when cleanup errors must be handled. `Drop` is non-panicking and retries
  one-shot native failures on a best-effort basis.
- A queued/running run rejects another queue request. Consume a terminal summary with
  `take_terminal_summary()` before queuing again.
- `show_windows()` accepts only a `Ui` from the attached Context during an active native frame.
- Upstream Dear ImGui Test Engine has its own license terms; review `extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine/imgui_test_engine/LICENSE.txt` before shipping commercial products.

## Features

- `capture` (default): enable screenshot/video capture helpers.
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
- `dear-imgui-sys` provides the hook symbols when `test-engine` is enabled. This avoids workspace feature-unification causing linker errors.
- If your tests don't interact with the UI, ensure you depend on `dear-imgui-test-engine` (or
  `dear-imgui-test-engine-sys`) and call `engine.start(&mut context)` before registration/queueing.
