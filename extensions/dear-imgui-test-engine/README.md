# dear-imgui-test-engine

[![Crates.io](https://img.shields.io/crates/v/dear-imgui-test-engine.svg)](https://crates.io/crates/dear-imgui-test-engine)
[![Documentation](https://docs.rs/dear-imgui-test-engine/badge.svg)](https://docs.rs/dear-imgui-test-engine)

Safe, idiomatic Rust integration for [Dear ImGui Test Engine](https://github.com/ocornut/imgui_test_engine) on top of `dear-imgui-rs`.

- Engine lifetime helpers: create/start/stop/destroy via RAII.
- Test queue helpers: queue tests/perfs, inspect queue/running state.
- Runtime controls: speed, verbosity, capture, abort.
- UI integration: show built-in test engine windows in an active ImGui frame.

## Links

- Upstream: https://github.com/ocornut/imgui_test_engine
- Low-level crate: `dear-imgui-test-engine-sys`
- Example: `examples/imgui_test_engine_basic.rs`

## Compatibility

| Item                        | Version |
|-----------------------------|---------|
| Crate                       | 0.10.x  |
| dear-imgui-rs               | 0.10.x  |
| dear-imgui-test-engine-sys  | 0.10.x  |

See also: [docs/COMPATIBILITY.md](https://github.com/Latias94/dear-imgui-rs/blob/main/docs/COMPATIBILITY.md).

## Quick Start

```toml
[dependencies]
dear-imgui-rs = "0.10"
dear-imgui-test-engine = "0.10"
```

```rust
use dear_imgui_rs as imgui;
use dear_imgui_test_engine as test_engine;

let mut imgui_ctx = imgui::Context::create();
let mut engine = test_engine::TestEngine::create();

engine.start(&imgui_ctx);

// In your frame loop
let ui = imgui_ctx.frame();
engine.show_windows(&ui, None);

// Queue all tests from CLI context
let _ = engine.queue_tests(
    test_engine::TestGroup::Tests,
    None,
    test_engine::RunFlags::RUN_FROM_COMMAND_LINE,
);
```

## Notes

- `dear-imgui-test-engine-sys` automatically enables the required `IMGUI_ENABLE_TEST_ENGINE` define on `dear-imgui-sys`.
- Upstream Dear ImGui Test Engine has its own license terms; review `extensions/dear-imgui-test-engine-sys/third-party/imgui_test_engine/imgui_test_engine/LICENSE.txt` before shipping commercial products.
