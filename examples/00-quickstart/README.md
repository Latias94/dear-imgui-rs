# dear-app quickstarts

These three binaries use the same `dear-app` runtime state machine. Move the UI body to the next
level only when the application needs the additional capability.

| Level | Entry | Adds | Command |
| --- | --- | --- | --- |
| Minimal UI | `run_ui` | Persistent closure state and `&Ui` | `cargo run -p dear-imgui-examples --bin hello_world` |
| Fallible frame | `run_frame` | `FrameContext`, typed user errors, explicit exit, add-ons, and the active GPU generation | `cargo run -p dear-imgui-examples --bin fallible_frame` |
| Full lifecycle | `run` + `Application` | Initialization, events, pre-frame Context mutation, GPU recovery, and shutdown hooks | `cargo run -p dear-imgui-examples --bin application_lifecycle` |

`run_ui` is implemented as an infallible adapter over `run_frame`; `run_frame` is implemented as
an `Application::frame` adapter. All three therefore share surface admission, GPU recovery, Test
Engine presentation, error ordering, and exactly-once shutdown.

`FrameContext::request_exit` is normal control flow. A frame error remains primary if a callback
both requests exit and returns an error. Application-hook failures carry an `ApplicationStage` and
retain the original error through `std::error::Error::source`.
