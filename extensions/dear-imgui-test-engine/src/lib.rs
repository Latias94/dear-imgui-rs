//! Dear ImGui Test Engine bindings for `dear-imgui-rs`.
//!
//! This crate wraps `dear-imgui-test-engine-sys` with a small safe API for
//! engine lifetime management and per-frame UI integration.

mod attachment;
mod config;
mod counts;
mod engine;
mod error;
mod frame_driver;
mod results;
mod runner;
mod script;
mod state;

#[cfg(test)]
mod tests;

#[cfg(feature = "capture")]
pub use config::CaptureFlags;
pub use config::{CaptureOutput, InputMode, RunFlags, RunSpeed, TestGroup, VerboseLevel};
pub use counts::{ScriptCount, ScriptLimit};
pub use engine::TestEngine;
pub use error::{TestEngineError, TestEngineResult, TestEngineStatus};
pub use frame_driver::{
    CaptureProviderError, FrameDriverError, FrameDriverPhase, RunMode, TestFrameDriver,
};
#[cfg(feature = "capture")]
pub use frame_driver::{CaptureRequest, CapturingTestFrameDriver, Rgba8};
pub use results::{ResultSummary, RunOutcome, RunReport};
#[cfg(feature = "capture")]
pub use runner::{CapturingDriverRunResult, CapturingRunnerError};
pub use runner::{
    HeadlessRenderError, HeadlessRunnerError, RunnerControl, RunnerError, TestRunner,
};
pub use script::ScriptTest;
pub use state::{AttachmentState, RunState};

pub use dear_imgui_test_engine_sys as raw;

pub(crate) use script::Script;
